use std::collections::HashSet;
use std::fmt;
use std::hash::{Hasher};
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::sync::{Arc};

use futures::{Future, Stream};
use futures::stream::MapErr;
use futures::future::{FutureResult, ok};
use futures::sync::mpsc::{UnboundedReceiver};
use serde_cbor::de::from_slice;
use tk_http::websocket::{self, Frame, Loop, ServerCodec};
use tk_easyloop::{spawn, handle};
use tk_bufstream::{WriteFramed, ReadFramed};
use tokio_io::{AsyncRead, AsyncWrite};

use named_mutex::{Mutex, MutexGuard};
use proto::message::{Message};
use proto::{RequestClient, RequestDispatcher, Sender};
use proto::{Registry, StreamExt, PacketStream};
use proto::{Response, WrapTrait, Notification};
use index::{ImageId};
use remote::Remote;
use tracking::Tracking;

lazy_static! {
    static ref CONNECTION_ID: AtomicUsize = AtomicUsize::new(0);
}

#[derive(Clone)]
pub struct Connection(Arc<ConnectionState>);

struct ConnectionState {
    id: usize,
    addr: SocketAddr,
    sender: Sender,
    registry: Registry,
    connected: AtomicBool,
    // TODO(tailhook) is this images thing needed?
    images: Mutex<HashSet<ImageId>>,
}


pub struct Dispatcher {
    connection: Connection,
    tracking: Tracking,
    requests: Registry,
}

pub struct Responder<R: Response> {
    request_id: u64,
    chan: Sender,
    item: PhantomData<R>,
}

impl Dispatcher {
    pub fn new(cli: Connection, registry: &Registry, tracking: &Tracking)
        -> Dispatcher
    {
        cli.0.connected.store(true, Ordering::SeqCst);
        let disp = Dispatcher {
            connection: cli.clone(),
            tracking: tracking.clone(),
            requests: registry.clone(),
        };
        return disp;
    }
}

impl Connection {
    pub fn incoming<S>(addr: SocketAddr,
               out: WriteFramed<S, ServerCodec>,
               inp: ReadFramed<S, ServerCodec>,
               remote: &Remote, tracking: &Tracking)
        -> (Connection, Loop<S,
            PacketStream<
                MapErr<UnboundedReceiver<Box<WrapTrait>>,
                        fn(()) -> &'static str>>,
            Dispatcher>)
        where S: AsyncRead + AsyncWrite
    {
        // TODO(tailhook) not sure how large backpressure should be
        let (tx, rx) = Sender::channel();
        let rx = rx.map_err(closed as fn(()) -> &'static str);
        let id = CONNECTION_ID.fetch_add(1, Ordering::SeqCst);
        let registry = Registry::new();
        let cli = Connection(Arc::new(ConnectionState {
            id: id,
            addr: addr,
            sender: tx,
            connected: AtomicBool::new(true),
            registry: registry.clone(),
            images: Mutex::new(HashSet::new(), "connection_images"),
        }));
        let disp = Dispatcher::new(cli.clone(), &registry, tracking);
        let rx = rx.packetize(&registry);
        let fut = Loop::server(out, inp, rx, disp,
            remote.websock_config(), &handle());
        return (cli, fut);
    }

    pub fn hanging_requests(&self) -> usize {
        self.0.registry.get_size()
    }

    pub fn is_connected(&self) -> bool {
        self.0.connected.load(Ordering::SeqCst)
    }

    pub fn outgoing(addr: SocketAddr, registry: &Registry)
        -> (Connection, UnboundedReceiver<Box<WrapTrait>>)
    {
        let (tx, rx) = Sender::channel();
        let id = CONNECTION_ID.fetch_add(1, Ordering::SeqCst);
        let cli = Connection(Arc::new(ConnectionState {
            id: id,
            addr: addr,
            sender: tx,
            connected: AtomicBool::new(false),
            registry: registry.clone(),
            images: Mutex::new(HashSet::new(), "connection_images"),
        }));
        return (cli, rx);
    }

    pub fn addr(&self) -> SocketAddr {
        self.0.addr
    }

    pub fn images(&self) -> MutexGuard<HashSet<ImageId>> {
        self.0.images.lock()
    }

    pub fn has_image(&self, id: &ImageId) -> bool {
        self.images().contains(id)
    }
    pub fn notification<N: Notification>(&self, n: N) {
        self.0.sender.notification(n)
    }
}

impl RequestClient for Connection {
    fn request_channel(&self) -> &Sender {
        &self.0.sender
    }
}

fn closed(():()) -> &'static str {
    "channel closed"
}

impl websocket::Dispatcher for Dispatcher {
    // TODO(tailhook) implement backpressure
    type Future = FutureResult<(), websocket::Error>;
    fn frame(&mut self, frame: &Frame) -> Self::Future {
        use proto::message::Notification as N;

        match *frame {
            Frame::Binary(data) => match from_slice(data) {
                Ok(Message::Request(rid, req)) => {
                    use proto::message::Request::*;
                    match req {
                        AppendDir(ad) => {
                            self.tracking.append_dir(ad,
                                Responder::new(rid, self));
                        }
                        ReplaceDir(ad) => {
                            self.tracking.replace_dir(ad,
                                Responder::new(rid, self));
                        }
                        GetIndex(gi) => {
                            self.tracking.get_index(gi,
                                Responder::new(rid, self));
                        }
                        GetBlock(gb) => {
                            self.tracking.get_block(gb,
                                Responder::new(rid, self));
                        }
                        GetBaseDir(gb) => {
                            self.tracking.get_base_dir(gb,
                                Responder::new(rid, self));
                        }
                    }
                }
                Ok(Message::Response(request_id, resp)) => {
                    self.respond(request_id, resp);
                }
                Ok(Message::Notification(N::PublishImage(idx))) => {
                    self.connection.images().insert(idx.id.clone());
                    self.tracking.remote()
                        .inner().declared_images
                        .entry(idx.id.clone())
                        .or_insert_with(HashSet::new)
                        .insert(self.connection.clone());
                    // TODO(tailhook) wakeup remote subsystem, so it can
                    // fetch image from this peer if image is currently in
                    // hanging state
                }
                Ok(Message::Notification(N::ReceivedImage(_))) => {
                    // ignoring for now
                    // TODO(tailhook) forward the notification
                }
                Ok(Message::Notification(N::AbortedImage(_))) => {
                    // ignoring for now
                    // TODO(tailhook) forward the notification
                }
                Err(e) => {
                    match *frame {
                        Frame::Binary(x) => {
                            error!("Failed to deserialize frame, \
                                error: {}, frame: {}", e,
                                String::from_utf8_lossy(x));
                        }
                        _ => {
                            error!("Failed to deserialize frame, \
                                error: {}, frame: {:?}", e, frame);
                        }
                    }
                }
            },
            _ => {
                error!("Bad frame received: {:?}", frame);
            }
        }
        ok(())
    }
}

impl RequestDispatcher for Dispatcher {
    fn request_registry(&self) -> &Registry {
        &self.requests
    }
}

impl ::std::hash::Hash for Connection {
    fn hash<H>(&self, state: &mut H)
        where H: Hasher
    {
        self.0.id.hash(state)
    }
}

impl PartialEq for Connection {
    fn eq(&self, other: &Connection) -> bool {
        self.0.id == other.0.id
    }
}

impl Eq for Connection {}

impl<R: Response> Responder<R> {
    fn new(request_id: u64, disp: &Dispatcher) -> Responder<R> {
        Responder {
            request_id: request_id,
            chan: disp.connection.0.sender.clone(),
            item: PhantomData,
        }
    }
}

impl<R: Response> Responder<R> {
    pub fn respond_with_future<F>(self, fut: F)
        where F: Future<Item=R> + 'static,
              F::Error: ::std::error::Error + Send + 'static,
    {
        spawn(fut.then(move |res| {
            match res {
                Ok(value) => {
                    self.chan.response(self.request_id, value);
                }
                Err(e) => {
                    error!("{} error: {}", R::static_type_name(), e);
                    self.chan.error_response(self.request_id, e);
                }
            };
            Ok(())
        }));
    }
    pub fn respond_now(self, value: R) {
        self.chan.response(self.request_id, value);
    }
    pub fn error_now<E: fmt::Display + Send + 'static>(self, e: E) {
        self.chan.error_response(self.request_id, e);
    }
}
