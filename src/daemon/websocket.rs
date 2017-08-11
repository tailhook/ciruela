use std::collections::HashSet;
use std::hash::{Hasher};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::atomic::{AtomicUsize, Ordering};

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
use ciruela::proto::message::{Message, Request};
use ciruela::proto::{GetIndex, GetIndexResponse};
use ciruela::proto::{GetBlock, GetBlockResponse};
use ciruela::proto::{GetBaseDirResponse};
use ciruela::proto::{RequestClient, RequestDispatcher, Sender};
use ciruela::proto::{RequestFuture, Registry, StreamExt, PacketStream};
use ciruela::proto::{WrapTrait, Notification};
use ciruela::{ImageId, Hash};
use remote::Remote;

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
    remote: Remote,
    requests: Registry,
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
    pub fn new(cli: Connection, remote: &Remote)
        -> (Dispatcher, Registry)
    {
        let registry = Registry::new();
        let disp = Dispatcher {
            connection: cli.clone(),
            remote: remote.clone(),
            requests: registry.clone(),
        };
        return (disp, registry);
    }
}

impl Connection {
    pub fn incoming(addr: SocketAddr,
               out: WriteFramed<TcpStream, ServerCodec>,
               inp: ReadFramed<TcpStream, ServerCodec>,
               remote: &Remote)
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
        let (disp, registry) = Dispatcher::new(cli.clone(), &remote);
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

    pub fn fetch_index(&self, id: &ImageId) -> RequestFuture<GetIndexResponse>
    {
        debug!("Fetching index {}", id);
        self.request(GetIndex {
            id: id.clone()
        })
    }
    pub fn fetch_block(&self, hash: &Hash) -> RequestFuture<GetBlockResponse>
    {
        debug!("Fetching block {}", hash);
        self.request(GetBlock {
            hash: hash.clone()
        })
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
                Ok(Message::Request(request_id, Request::AppendDir(ad))) => {
                    let chan = self.connection.0.sender.clone();
                    spawn(self.remote.meta().append_dir(ad).then(move |res| {
                        match res {
                            Ok(value) => {
                                chan.response(request_id, value);
                            }
                            Err(e) => {
                                error!("AppendDir error: {}", e);
                                chan.error_response(request_id, e);
                            }
                        };
                        Ok(())
                    }));
                }
                Ok(Message::Request(request_id, Request::ReplaceDir(ad))) => {
                    let chan = self.connection.0.sender.clone();
                    spawn(self.remote.meta().replace_dir(ad).then(move |res| {
                        match res {
                            Ok(value) => {
                                chan.response(request_id, value);
                            }
                            Err(e) => {
                                error!("ReplaceDir error: {}", e);
                                chan.error_response(request_id, e);
                            }
                        };
                        Ok(())
                    }));
                }
                Ok(Message::Request(request_id, Request::GetIndex(gi))) => {
                    //let chan = self.connection.0.sender.clone();
                    unimplemented!();
                }
                Ok(Message::Request(request_id, Request::GetBlock(gb))) => {
                    //let chan = self.connection.0.sender.clone();
                    unimplemented!();
                }
                Ok(Message::Request(request_id, Request::GetBaseDir(binfo)))
                => {
                    let chan = self.connection.0.sender.clone();
                    match self.remote.config().dirs.get(binfo.path.key()) {
                        Some(cfg) => {
                            spawn(
                                base_dir::scan(&binfo.path, cfg,
                                    self.remote.meta(), self.remote.disk())
                                .then(move |res| {
                                    match res {
                                        Ok(value) => {
                                            chan.response(request_id,
                                                GetBaseDirResponse {
                                                    config_hash:
                                                        value.config_hash,
                                                    keep_list_hash:
                                                        value.keep_list_hash,
                                                    dirs: value.dirs,
                                                });
                                        }
                                        Err(e) => {
                                            error!("GetBaseDir error: {}", e);
                                            chan.error_response(request_id, e);
                                        }
                                    };
                                    Ok(())
                                }));
                        }
                        None => {
                            chan.error_response(request_id,
                                Error::ConfigNotFound);
                            error!("GetBaseDir error: config not found");
                        }
                    };

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
