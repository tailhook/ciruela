use std::collections::HashMap;
use std::fs::{File};
use std::io::{self, Read, Seek, SeekFrom};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
use proto::message::{Message, Request, Notification};
use proto::index_commands::{PublishImage, GetIndexResponse};
use proto::block_commands::{GetBlockResponse};
use proto::request::{Sender, Error, RequestDispatcher, RequestClient};
use proto::request::{Registry};


pub struct ImageInfo {
    pub image_id: ImageId,
    pub block_size: u64,
    pub index_data: Vec<u8>,
    pub location: PathBuf,
    pub blocks: HashMap<Hash, BlockPointer>,
}

#[derive(Debug, Clone)]
pub struct BlockPointer {
    pub file: Arc<PathBuf>,
    pub offset: u64,
}


pub struct Client {
    channel: Sender,
    local_images: Arc<Mutex<HashMap<ImageId, Arc<ImageInfo>>>>,
}

pub struct ClientFuture {
    chan: Receiver<Client>,
}

pub trait Listener {
    fn notification(&self, n: Notification);
    fn closed(&self);
}

struct MyDispatcher<L: Listener> {
    requests: Registry,
    channel: Sender,
    local_images: Arc<Mutex<HashMap<ImageId, Arc<ImageInfo>>>>,
    pool: CpuPool,
    listener: L,
}

impl<L: Listener> Drop for MyDispatcher<L> {
    fn drop(&mut self) {
        self.listener.closed();
    }
}

impl<L: Listener> Dispatcher for MyDispatcher<L> {
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
                Ok(Message::Request(req_id, Request::GetBlock(req))) => {
                    let images = self.local_images.lock()
                        .expect("local_images is not poisoned");
                    for img in images.values() {
                        if let Some(block) = img.blocks.get(&req.hash) {
                            let chan = self.channel.clone();
                            let img = img.clone();
                            let block = block.clone();
                            self.pool.spawn_fn(move || {
                                match img.read_block(&block) {
                                    Ok(data) => {
                                        chan.response(req_id,
                                            GetBlockResponse {
                                                data: data,
                                            });
                                    }
                                    Err(e) => {
                                        error!("Can't read block {:?}: {}",
                                            block, e);
                                        chan.error_response(req_id,
                                            Error::CantReadBlock);
                                    }
                                }
                                Ok::<(), ()>(())
                            }).forget();
                            return ok(());
                        }
                    }
                    self.channel.error_response(req_id,
                        Error::BlockNotFound)
                }
                Ok(Message::Request(_req_id, _msg)) => {
                    unimplemented!();
                }
                Ok(Message::Response(request_id, resp)) => {
                    self.respond(request_id, resp)
                }
                Ok(Message::Notification(n)) => {
                    self.listener.notification(n);
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

impl<L: Listener> RequestDispatcher for MyDispatcher<L> {
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
    pub fn spawn<L>(addr: SocketAddr, host: String,
        pool: &CpuPool, listener: L)
        -> ClientFuture
        where L: Listener + 'static
    {
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
                HandshakeProto::new(sock, SimpleAuthorizer::new(host, "/"))
                .map_err(move |e| {
                    error!("Error connecting to {}: {}", addr, e);
                })
            })
            .and_then(move |(out, inp, ())| {
                info!("Connected to {}", addr);
                let local_images = Arc::new(Mutex::new(HashMap::new()));
                tx.send(Client {
                    channel: ctx.clone(),
                    local_images: local_images.clone(),
                }).unwrap_or_else(|_| {
                    info!("Client future discarded before connected");
                });
                let disp = MyDispatcher {
                    requests: requests.clone(),
                    channel: ctx,
                    local_images: local_images,
                    pool: pool,
                    listener: listener,
                };
                let stream = crx.packetize(&requests)
                    .map_err(|_| Error::UnexpectedTermination);
                Loop::client(out, inp, stream, disp, &wcfg,
                    &tk_easyloop::handle())
                .map_err(|e| info!("websocket closed: {}", e))
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
                id: info.image_id.clone(),
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

impl ImageInfo {
    fn read_block(&self, pointer: &BlockPointer) -> Result<Vec<u8>, io::Error>
    {
        let mut result = vec![0u8; self.block_size as usize];
        let mut file = File::open(self.location
            .join(pointer.file.strip_prefix("/")
                  .expect("block path is absolute")))?;
        file.seek(SeekFrom::Start(pointer.offset))?;
        let bytes = file.read(&mut result[..])?;
        result.truncate(bytes);
        Ok(result)
    }
}
