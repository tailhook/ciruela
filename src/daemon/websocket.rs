use std::collections::HashSet;
use std::fmt;
use std::hash::{Hasher};
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use futures::{Future, Stream};
use futures::stream::MapErr;
use futures::future::{FutureResult, ok};
use futures::sync::mpsc::{UnboundedReceiver};
use serde_cbor::de::from_slice;
use tk_http::websocket::{self, Frame, Loop};
use tk_http::websocket::ServerCodec;
use tk_easyloop::{spawn, handle};
use tk_bufstream::{WriteFramed, ReadFramed};
use tokio_core::net::TcpStream;

use base_dir;
use ciruela::proto::message::{Message};
use ciruela::proto::{GetIndex, GetIndexResponse};
use ciruela::proto::{GetBlock, GetBlockResponse};
use ciruela::proto::{GetBaseDirResponse};
use ciruela::proto::{RequestClient, RequestDispatcher, Sender};
use ciruela::proto::{RequestFuture, Registry, StreamExt, PacketStream};
use ciruela::proto::{Response, WrapTrait, Notification};
use ciruela::{ImageId, Hash};
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

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        ConfigNotFound {
            description("config not found")
        }
    }
}

impl Dispatcher {
    pub fn new(cli: Connection, tracking: &Tracking)
        -> (Dispatcher, Registry)
    {
        let registry = Registry::new();
        let disp = Dispatcher {
            connection: cli.clone(),
            tracking: tracking.clone(),
            requests: registry.clone(),
        };
        return (disp, registry);
    }
}

impl Connection {
    pub fn incoming(addr: SocketAddr,
               out: WriteFramed<TcpStream, ServerCodec>,
               inp: ReadFramed<TcpStream, ServerCodec>,
               remote: &Remote, tracking: &Tracking)
        -> (Connection, Loop<TcpStream,
            PacketStream<
                MapErr<UnboundedReceiver<Box<WrapTrait>>,
                        fn(()) -> &'static str>>,
            Dispatcher>)
    {
        // TODO(tailhook) not sure how large backpressure should be
        let (tx, rx) = Sender::channel();
        let rx = rx.map_err(closed as fn(()) -> &'static str);
        let id = CONNECTION_ID.fetch_add(1, Ordering::SeqCst);
        let cli = Connection(Arc::new(ConnectionState {
            id: id,
            addr: addr,
            sender: tx,
            images: Mutex::new(HashSet::new()),
        }));
        let (disp, registry) = Dispatcher::new(cli.clone(), tracking);
        let rx = rx.packetize(&registry);
        let fut = Loop::server(out, inp, rx, disp,
            remote.websock_config(), &handle());
        return (cli, fut);
    }

    pub fn outgoing(addr: SocketAddr)
        -> (Connection, UnboundedReceiver<Box<WrapTrait>>)
    {
        let (tx, rx) = Sender::channel();
        let id = CONNECTION_ID.fetch_add(1, Ordering::SeqCst);
        let cli = Connection(Arc::new(ConnectionState {
            id: id,
            addr: addr,
            sender: tx,
            images: Mutex::new(HashSet::new()),
        }));
        return (cli, rx);
    }

    pub fn addr(&self) -> SocketAddr {
        self.0.addr
    }

    fn images(&self) -> MutexGuard<HashSet<ImageId>> {
        self.0.images.lock()
            .expect("images are not poisoned")
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
        use ciruela::proto::message::Notification as N;

        match *frame {
            Frame::Binary(data) => match from_slice(data) {
                Ok(Message::Request(rid, req)) => {
                    use ciruela::proto::message::Request::*;
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
                    self.connection.images().insert(idx.id);
                    // TODO(tailhook) wakeup remote subsystem, so it can
                    // fetch image from this peer if image is currently in
                    // hanging state
                }
                Ok(Message::Notification(N::ReceivedImage(_))) => {
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
}
