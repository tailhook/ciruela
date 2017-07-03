use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use futures::{Async, Future};
use futures::future::{FutureResult, ok};
use futures::stream::Stream;
use futures::sync::oneshot::{channel as oneshot, Receiver};
use futures_cpupool::CpuPool;
use tk_http::websocket::client::{HandshakeProto, SimpleAuthorizer};
use tk_http::websocket::{Loop, Frame, Error as WsError, Dispatcher, Config};
use serde_cbor::de::from_slice;
use tk_easyloop;
use tokio_core::net::TcpStream;

use {ImageId, Hash};
use proto::{StreamExt};
use proto::message::{Message, Request};
use proto::index_commands::{PublishImage, GetIndexResponse};
use proto::request::{Sender, Error, RequestDispatcher, RequestClient};
use proto::request::{Registry};


pub struct ImageInfo {
    pub image_id: ImageId,
    pub block_size: u64,
    pub index_data: Vec<u8>,
    pub location: PathBuf,
    pub blocks: HashMap<Hash, BlockPointer>,
}

pub struct BlockPointer {
    pub file: Arc<PathBuf>,
    pub offset: u64,
}


pub struct Client {
    channel: Sender,
    pool: CpuPool,
    local_images: Arc<Mutex<HashMap<ImageId, Arc<ImageInfo>>>>,
}

pub struct ClientFuture {
    chan: Receiver<Client>,
}


struct MyDispatcher {
    requests: Registry,
    channel: Sender,
    local_images: Arc<Mutex<HashMap<ImageId, Arc<ImageInfo>>>>,
}

impl Dispatcher for MyDispatcher {
    type Future = FutureResult<(), WsError>;
    fn frame(&mut self, frame: &Frame) -> FutureResult<(), WsError> {
        match *frame {
            Frame::Binary(data) => match from_slice(data) {
                Ok(Message::Request(req_id, Request::GetIndex(idx))) => {
                    self.local_images.lock()
                        .expect("local_images is not poisoned")
                        .get(&idx.id)
                        .map(|img| {
                            self.channel.response(req_id,
                                GetIndexResponse {
                                    // TODO(tailhook) optimize clone
                                    data: img.index_data.clone(),
                                })
                        })
                        .unwrap_or_else(|| {
                            self.channel.error_response(req_id,
                                Error::IndexNotFound)
                        });
                }
                Ok(Message::Request(_req_id, _msg)) => {
                    unimplemented!();
                }
                Ok(Message::Response(request_id, resp)) => {
                    self.respond(request_id, resp)
                }
                Ok(Message::Notification(..)) => {
                    unimplemented!();
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

impl RequestDispatcher for MyDispatcher {
    fn request_registry(&self) -> &Registry {
        &self.requests
    }
}

impl RequestClient for Client {
    fn request_channel(&self) -> &Sender {
        &self.channel
    }
}

impl Client {
    pub fn spawn(addr: SocketAddr, host: &Arc<String>, pool: &CpuPool)
        -> ClientFuture
    {
        let host = host.to_string();
        let (tx, rx) = oneshot();
        let (ctx, crx) = Sender::channel();
        let wcfg = Config::new().done();
        let requests = Registry::new();
        let pool = pool.clone();
        tk_easyloop::spawn(
            TcpStream::connect(&addr, &tk_easyloop::handle())
            .map_err(move |e| {
                error!("Error connecting to {}: {}", addr, e);
            })
            .and_then(move |sock| {
                HandshakeProto::new(sock, SimpleAuthorizer::new(&*host, "/"))
                .map_err(move |e| {
                    error!("Error connecting to {}: {}", addr, e);
                })
            })
            .and_then(move |(out, inp, ())| {
                info!("Connected to {}", addr);
                let local_images = Arc::new(Mutex::new(HashMap::new()));
                tx.send(Client {
                    channel: ctx.clone(),
                    pool: pool,
                    local_images: local_images.clone(),
                }).unwrap_or_else(|_| {
                    info!("Client future discarded before connected");
                });
                let disp = MyDispatcher {
                    requests: requests.clone(),
                    channel: ctx,
                    local_images: local_images,
                };
                let stream = crx.packetize(&requests)
                    .map_err(|_| Error::UnexpectedTermination);
                Loop::client(out, inp, stream, disp, &wcfg,
                    &tk_easyloop::handle())
                .map_err(|e| println!("websocket closed: {}", e))
            })
        );
        return ClientFuture {
            chan: rx,
        }
    }
    pub fn register_index(&mut self, info: &Arc<ImageInfo>) {
        self.local_images.lock()
            .expect("local_images is not poisoned")
            .insert(info.image_id.clone(), info.clone());
        self.channel.notification(
            PublishImage {
                image_id: info.image_id.clone(),
            }
        );
    }
}

impl Future for ClientFuture {
    type Item = Client;
    type Error = ();
    fn poll(&mut self) -> Result<Async<Client>, ()> {
        self.chan.poll().map_err(|_| ())
    }
}
