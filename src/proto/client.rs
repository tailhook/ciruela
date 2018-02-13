use std::net::SocketAddr;
use std::time::Duration;

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
use index::{GetIndex};
use index::ImageId;
use proto::{StreamExt};
use proto::message::{Message, Request, Notification};
use proto::index_commands::{PublishImage, GetIndexResponse};
use proto::block_commands::{GetBlockResponse};
use proto::request::{Sender, Error, RequestDispatcher, RequestClient};
use proto::request::{Registry};


#[derive(Clone)]
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

struct MyDispatcher<L: Listener, B, I> {
    requests: Registry,
    channel: Sender,
    blocks: B,
    indexes: I,
    listener: L,
}

impl<L: Listener, B, I> Drop for MyDispatcher<L, B, I> {
    fn drop(&mut self) {
        self.listener.closed();
    }
}

impl<L, B, I> Dispatcher for MyDispatcher<L, B, I>
    where L: Listener, B: GetBlock, I: GetIndex,
{
    type Future = FutureResult<(), WsError>;
    fn frame(&mut self, frame: &Frame) -> FutureResult<(), WsError> {
        match *frame {
            Frame::Binary(data) => match from_slice(data) {
                Ok(Message::Request(req_id, Request::GetIndex(idx))) => {
                    let chan = self.channel.clone();
                    spawn(self.indexes.read_index(&idx.id)
                        .then(move |res| {
                            match res {
                                Ok(data) => {
                                    chan.response(req_id,
                                        GetIndexResponse {
                                            // TODO(tailhook) optimize clone
                                            data: data.as_ref().to_vec(),
                                        })
                                }
                                Err(e) => {
                                    error!("Error getting index: {}", e);
                                    chan.error_response(req_id,
                                        Error::IndexNotFound)
                                }
                            }
                            Ok(())
                        }));
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
                                            // TODO(tailhook) don't copy?
                                            data: block.as_ref().to_vec(),
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

impl<L: Listener, B, I> RequestDispatcher for MyDispatcher<L, B, I> {
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
    pub fn spawn<L, B, I>(addr: SocketAddr, host: String,
        blocks: B, indexes: I, listener: L)
        -> ClientFuture
        where L: Listener + 'static,
              B: GetBlock + Send + 'static,
              I: GetIndex + Send + 'static,
    {
        let (tx, rx) = oneshot();
        let (ctx, crx) = Sender::channel();
        let wcfg = Config::new()
            .message_timeout(Duration::new(120, 0))
            .byte_timeout(Duration::new(5, 0))
            .ping_interval(Duration::new(1, 0))
            .done();
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
                    indexes: indexes,
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
    pub fn register_index(&self, id: &ImageId) {
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
