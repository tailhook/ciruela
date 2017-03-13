use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashMap;

use futures::{Async, Future, Canceled};
use futures::future::{FutureResult, ok};
use futures::stream::Stream;
use futures::sync::mpsc::{unbounded, UnboundedSender};
use futures::sync::oneshot::{channel as oneshot, Sender, Receiver};
use futures_cpupool::CpuPool;
use mopa;
use tk_http::websocket::client::{HandshakeProto, SimpleAuthorizer};
use tk_http::websocket::{Loop, Frame, Error as WsError, Dispatcher, Config};
use tk_http::websocket::{Packet};
use serde_cbor::ser::Serializer as Cbor;
use serde_cbor::de::from_slice;
use serde::Serialize;
use tk_easyloop;
use tokio_core::net::TcpStream;

use proto::{RequestTrait, NotificationTrait, REQUEST, NOTIFICATION};
use proto::{Message, Response};
use proto::dir_commands::AppendDir;
use proto::index_commands::PublishIndex;


pub struct ImageInfo {
    pub image_id: Vec<u8>,
    pub index_data: Vec<u8>,
    pub location: PathBuf,
}


pub struct Client {
    channel: UnboundedSender<Box<WrapTrait>>,
    pool: CpuPool,
    local_images: HashMap<Vec<u8>, Arc<ImageInfo>>,
}

pub struct ClientFuture {
    chan: Receiver<Client>,
}

pub struct RequestFuture<R> {
    chan: Receiver<R>,
}

trait WrapTrait: mopa::Any {
    fn is_request(&self) -> bool;
    fn serialize_req(&self, request_id: u64) -> Packet;
    fn serialize(&self) -> Packet;
}

mopafy!(WrapTrait);

struct RequestWrap<R: RequestTrait> {
    request: R,
    chan: Option<Sender<R::Response>>,
}

struct NotificationWrap<N: NotificationTrait> {
    data: N,
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        UnexpectedTermination {
            from(Canceled)
        }
    }
}

struct MyDispatcher {
    requests: Arc<Mutex<HashMap<u64, Box<WrapTrait>>>>,
}

impl Dispatcher for MyDispatcher {
    type Future = FutureResult<(), WsError>;
    fn frame(&mut self, frame: &Frame) -> FutureResult<(), WsError> {
        match *frame {
            Frame::Binary(data) => match from_slice(data) {
                Ok(Message::Request(..)) => {
                    unimplemented!();
                }
                Ok(Message::Response(request_id, Response::AppendDir(ad))) => {
                    let mut requests = self.requests.lock()
                        .expect("requests are not poisoned");
                    match requests.remove(&request_id) {
                        Some(mut r) => {
                            r.downcast_mut::<RequestWrap<AppendDir>>()
                            .map(|r| r.chan.take().unwrap().complete(ad))
                            .unwrap_or_else(|| {
                                error!("Wrong reply type for {}", request_id);
                            })
                        }
                        None => {
                            error!("Unsolicited reply {}", request_id);
                        }
                    }
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

impl Client {
    pub fn spawn(addr: SocketAddr, host: &Arc<String>, pool: &CpuPool)
        -> ClientFuture
    {
        let host = host.to_string();
        let (tx, rx) = oneshot();
        let (ctx, crx) = unbounded();
        let wcfg = Config::new().done();
        let requests = Arc::new(Mutex::new(HashMap::new()));
        let request_id = Arc::new(AtomicUsize::new(0));
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
                tx.complete(Client {
                    channel: ctx,
                    pool: pool,
                    local_images: HashMap::new(),
                });
                let disp = MyDispatcher {
                    requests: requests.clone(),
                };
                let stream = crx.map(move |req| {
                    if req.is_request() {
                        let r_id = request_id.fetch_add(1, Ordering::SeqCst);
                        let packet = req.serialize_req(r_id as u64);
                        requests.lock().unwrap()
                            .insert(r_id as u64, req);
                        packet
                    } else {
                        req.serialize()
                    }
                }).map_err(|_| Error::UnexpectedTermination);
                Loop::client(out, inp, stream, disp, &wcfg)
                .map_err(|e| println!("websocket closed: {}", e))
            })
        );
        return ClientFuture {
            chan: rx,
        }
    }
    pub fn request<R>(&self, request: R) -> RequestFuture<R::Response>
        where R: RequestTrait + 'static
    {
        let (tx, rx) = oneshot();
        self.channel.send(Box::new(RequestWrap {
            request: request,
            chan: Some(tx),
        })).map_err(|e| {
            // We expect `rx` to get cancellation notice in case of error, so
            // process does not hang, after logging the message
            error!("Error sending request: {}", e)
        }).ok();
        return RequestFuture { chan: rx };
    }
    pub fn register_index(&mut self, info: &Arc<ImageInfo>) {
        self.local_images.insert(info.image_id.to_vec(), info.clone());
        self.channel.send(Box::new(NotificationWrap {
            data: PublishIndex {
                image_id: info.image_id.to_vec(),
            },
        })).map_err(|e| {
            // We expect `rx` to get cancellation notice in case of error, so
            // process does not hang, after logging the message
            error!("Error sending request: {}", e)
        }).ok();
    }
}

impl Future for ClientFuture {
    type Item = Client;
    type Error = ();
    fn poll(&mut self) -> Result<Async<Client>, ()> {
        self.chan.poll().map_err(|_| ())
    }
}

impl<R> Future for RequestFuture<R> {
    type Item = R;
    type Error = Error;
    fn poll(&mut self) -> Result<Async<R>, Error> {
        self.chan.poll().map_err(Into::into)
    }
}

impl<R: RequestTrait> WrapTrait for RequestWrap<R> {
    fn is_request(&self) -> bool {
        true
    }
    fn serialize_req(&self, request_id: u64) -> Packet {
        let mut buf = Vec::new();
        (REQUEST, self.request.type_name(), request_id, &self.request)
            .serialize(&mut Cbor::new(&mut buf))
            .expect("Can always serialize request data");
        return Packet::Binary(buf);
    }
    fn serialize(&self) -> Packet { unreachable!(); }
}

impl<N: NotificationTrait> WrapTrait for NotificationWrap<N> {
    fn is_request(&self) -> bool {
        false
    }
    fn serialize_req(&self, _: u64) -> Packet { unreachable!(); }
    fn serialize(&self) -> Packet {
        let mut buf = Vec::new();
        (NOTIFICATION, self.data.type_name(), &self.data)
            .serialize(&mut Cbor::new(&mut buf))
            .expect("Can always serialize request data");
        return Packet::Binary(buf);
    }
}
