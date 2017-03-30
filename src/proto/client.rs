use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
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

use proto::{StreamExt};
use proto::message::Message;
use proto::index_commands::PublishIndex;
use proto::request::{Sender, Error, RequestDispatcher, RequestClient};
use proto::request::{Registry};


pub struct ImageInfo {
    pub image_id: Vec<u8>,
    pub index_data: Vec<u8>,
    pub location: PathBuf,
}


pub struct Client {
    channel: Sender,
    pool: CpuPool,
    local_images: HashMap<Vec<u8>, Arc<ImageInfo>>,
}

pub struct ClientFuture {
    chan: Receiver<Client>,
}


struct MyDispatcher {
    requests: Registry,
}

impl Dispatcher for MyDispatcher {
    type Future = FutureResult<(), WsError>;
    fn frame(&mut self, frame: &Frame) -> FutureResult<(), WsError> {
        match *frame {
            Frame::Binary(data) => match from_slice(data) {
                Ok(Message::Request(..)) => {
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
                tx.send(Client {
                    channel: ctx,
                    pool: pool,
                    local_images: HashMap::new(),
                }).unwrap_or_else(|_| {
                    info!("Client future discarded before connected");
                });
                let disp = MyDispatcher {
                    requests: requests.clone(),
                };
                let stream = crx.packetize(&requests)
                    .map_err(|_| Error::UnexpectedTermination);
                Loop::client(out, inp, stream, disp, &wcfg)
                .map_err(|e| println!("websocket closed: {}", e))
            })
        );
        return ClientFuture {
            chan: rx,
        }
    }
    pub fn register_index(&mut self, info: &Arc<ImageInfo>) {
        self.local_images.insert(info.image_id.to_vec(), info.clone());
        self.channel.notification(
            PublishIndex {
                image_id: info.image_id.to_vec(),
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
