use std::net::SocketAddr;

use futures::{Async, Future};
use futures::future::{FutureResult, ok};
use futures::stream::Stream;
use futures::sync::oneshot::{channel as oneshot, Receiver};
use tk_http::websocket::client::{HandshakeProto, SimpleAuthorizer};
use tk_http::websocket::{Loop, Frame, Error as WsError, Dispatcher, Config};
use serde_cbor::de::from_slice;
use tk_easyloop::{spawn, handle};
use tokio_core::net::TcpStream;

use blocks::{GetBlock, BlockHint};
use ImageId;
use proto::{StreamExt};
use proto::message::{Message, Request, Notification};
use proto::index_commands::{PublishImage, GetIndexResponse};
use proto::block_commands::{GetBlockResponse};
use proto::request::{Sender, Error, RequestDispatcher, RequestClient};
use proto::request::{Registry};


pub struct Client {
    channel: Sender,
}

pub struct ClientFuture {
    chan: Receiver<Client>,
}

pub trait Listener {
    fn notification(&self, n: Notification);
    fn closed(&self);
}

struct MyDispatcher<L: Listener, B> {
    requests: Registry,
    channel: Sender,
    blocks: B,
    listener: L,
}

impl<L: Listener, B> Drop for MyDispatcher<L, B> {
    fn drop(&mut self) {
        self.listener.closed();
    }
}

impl<L: Listener, B: GetBlock> Dispatcher for MyDispatcher<L, B> {
    type Future = FutureResult<(), WsError>;
    fn frame(&mut self, frame: &Frame) -> FutureResult<(), WsError> {
        match *frame {
            Frame::Binary(data) => match from_slice(data) {
                Ok(Message::Request(req_id, Request::GetIndex(idx))) => {
                    unimplemented!();
                    /*
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
                        }); */
                }
                Ok(Message::Request(req_id, Request::GetBlock(req))) => {
                    let chan = self.channel.clone();
                    let hash = req.hash.clone();
                    spawn(self.blocks.read_block(req.hash, BlockHint::empty())
                        .then(move |res| {
                            match res {
                                Ok(block) => {
                                    chan.response(req_id,
                                        GetBlockResponse {
                                            data: block,
                                        });
                                }
                                Err(e) => {
                                    error!("Can't read block {:?}: \
                                        {}", hash, e);
                                    chan.error_response(req_id,
                                        Error::CantReadBlock);
                                }
                            }
                            Ok(())
                        }));
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

impl<L: Listener, B> RequestDispatcher for MyDispatcher<L, B> {
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
    pub fn spawn<L, B>(addr: SocketAddr, host: String,
        blocks: B, listener: L)
        -> ClientFuture
        where L: Listener + 'static,
              B: GetBlock + Send + 'static,
    {
        let (tx, rx) = oneshot();
        let (ctx, crx) = Sender::channel();
        let wcfg = Config::new().done();
        let requests = Registry::new();
        spawn(
            TcpStream::connect(&addr, &handle())
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
                tx.send(Client {
                    channel: ctx.clone(),
                }).unwrap_or_else(|_| {
                    info!("Client future discarded before connected");
                });
                let disp = MyDispatcher {
                    requests: requests.clone(),
                    channel: ctx,
                    blocks: blocks,
                    listener: listener,
                };
                let stream = crx.packetize(&requests)
                    .map_err(|_| Error::UnexpectedTermination);
                Loop::client(out, inp, stream, disp, &wcfg, &handle())
                .map_err(|e| info!("websocket closed: {}", e))
            })
        );
        return ClientFuture {
            chan: rx,
        }
    }
    pub fn register_index(&mut self, id: &ImageId) {
        self.channel.notification(
            PublishImage {
                id: id.clone(),
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
