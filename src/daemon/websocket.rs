use std::collections::HashSet;
use std::hash::{Hasher};
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::{Future, Stream};
use futures::stream::MapErr;
use futures::future::{FutureResult, ok};
use futures::sync::mpsc::{UnboundedReceiver};
use serde_cbor::de::from_slice;
use tk_http::websocket::{self, Frame, Error, Config, Loop};
use tk_http::websocket::ServerCodec;
use tk_easyloop::{spawn, handle};
use tk_bufstream::{WriteFramed, ReadFramed};
use tokio_core::net::TcpStream;

use ciruela::proto::message::{Message, Request, Notification};
use ciruela::proto::{GetIndex, GetIndexResponse};
use ciruela::proto::{GetBlock, GetBlockResponse};
use ciruela::proto::{RequestClient, RequestDispatcher, Sender};
use ciruela::proto::{RequestFuture, Registry, StreamExt, PacketStream};
use ciruela::proto::{WrapTrait};
use ciruela::{ImageId, Hash};
use metadata::Meta;

lazy_static! {
    static ref CONNECTION_ID: AtomicUsize = AtomicUsize::new(0);
}

#[derive(Clone)]
pub struct Connection(Arc<ConnectionState>);

struct ConnectionState {
    id: usize,
    sender: Sender,
    images: Mutex<HashSet<ImageId>>,
}


pub struct Dispatcher {
    connection: Connection,
    metadata: Meta,
    requests: Registry,
}

impl Connection {
    pub fn new(out: WriteFramed<TcpStream, ServerCodec>,
               inp: ReadFramed<TcpStream, ServerCodec>,
               meta: &Meta, cfg: &Arc<Config>)
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
            sender: tx,
            images: Mutex::new(HashSet::new()),
        }));
        let registry = Registry::new();
        let disp = Dispatcher {
            connection: cli.clone(),
            metadata: meta.clone(),
            requests: registry.clone(),
        };
        let rx = rx.packetize(&registry);
        let fut = Loop::server(out, inp, rx, disp, cfg, &handle());
        return (cli, fut);
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
        info!("Fetching index {}", id);
        self.request(GetIndex {
            id: id.clone()
        })
    }
    pub fn fetch_block(&self, hash: &Hash) -> RequestFuture<GetBlockResponse>
    {
        info!("Fetching block {}", hash);
        self.request(GetBlock {
            hash: hash.clone()
        })
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
    type Future = FutureResult<(), Error>;
    fn frame(&mut self, frame: &Frame) -> Self::Future {
        match *frame {
            Frame::Binary(data) => match from_slice(data) {
                Ok(Message::Request(request_id, Request::AppendDir(ad))) => {
                    let chan = self.connection.0.sender.clone();
                    spawn(self.metadata.append_dir(ad).then(move |res| {
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
                Ok(Message::Request(request_id, Request::GetIndex(gi))) => {
                    //let chan = self.connection.0.sender.clone();
                    unimplemented!();
                }
                Ok(Message::Request(request_id, Request::GetBlock(gb))) => {
                    //let chan = self.connection.0.sender.clone();
                    unimplemented!();
                }
                Ok(Message::Response(request_id, resp)) => {
                    self.respond(request_id, resp);
                }
                Ok(Message::Notification(Notification::PublishImage(idx))) => {
                    self.connection.images().insert(idx.image_id);
                    // TODO(tailhook) wakeup remote subsystem, so it can
                    // fetch image from this peer if image is currently in
                    // hanging state
                }
                Err(e) => {
                    error!("Failed to deserialize frame, \
                        error: {}, frame: {:?}", e, frame);
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
