use std::sync::MutexGuard;
use std::collections::HashMap;

use futures::{Async, Future, Canceled};
use futures::sync::oneshot::{channel as oneshot, Sender, Receiver};
use futures::sync::mpsc::{UnboundedSender};
use serde::Serialize;
use serde_cbor::ser::Serializer as Cbor;
use tk_http::websocket::{Packet};
use mopa;

use proto::Response;
use proto::{RequestTrait, NotificationTrait, REQUEST, NOTIFICATION};
use proto::dir_commands::AppendDir;


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        UnexpectedTermination {
            from(Canceled)
        }
    }
}



pub trait Request: Serialize + 'static {
    type Response: 'static;
    fn type_name(&self) -> &'static str;
}

pub trait Notification: Serialize + 'static {
    fn type_name(&self) -> &'static str;
}

pub struct RequestFuture<R> {
    chan: Receiver<R>,
}

pub trait WrapTrait: mopa::Any {
    fn is_request(&self) -> bool;
    fn serialize_req(&self, request_id: u64) -> Packet;
    fn serialize(&self) -> Packet;
}

mopafy!(WrapTrait);

pub struct RequestWrap<R: RequestTrait> {
    request: R,
    chan: Option<Sender<R::Response>>,
}

pub struct NotificationWrap<N: NotificationTrait> {
    data: N,
}

pub trait RequestClient {
    fn request_channel(&self) -> &UnboundedSender<Box<WrapTrait>>;
    fn request<R>(&self, request: R) -> RequestFuture<R::Response>
        where R: RequestTrait + 'static
    {
        let (tx, rx) = oneshot();
        self.request_channel().send(Box::new(RequestWrap {
            request: request,
            chan: Some(tx),
        })).map_err(|e| {
            // We expect `rx` to get cancellation notice in case of error, so
            // process does not hang, after logging the message
            error!("Error sending request: {}", e)
        }).ok();
        return RequestFuture { chan: rx };
    }
}

pub trait RequestDispatcher {
    fn request_registry(&self) -> MutexGuard<HashMap<u64, Box<WrapTrait>>>;
    fn respond(&self, request_id: u64, response: Response) {
        match response {
            Response::AppendDir(ad) => {
                let mut requests = self.request_registry();
                match requests.remove(&request_id) {
                    Some(mut r) => {
                        r.downcast_mut::<RequestWrap<AppendDir>>()
                        .map(|r| {
                            r.chan.take().unwrap()
                            .send(ad)
                            .unwrap_or_else(|_| info!("Useless reply"))
                        })
                        .unwrap_or_else(|| {
                            error!("Wrong reply type for {}", request_id);
                        });
                    }
                    None => {
                        error!("Unsolicited reply {}", request_id);
                    }
                }
            }
        }
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

impl<R> Future for RequestFuture<R> {
    type Item = R;
    type Error = Error;
    fn poll(&mut self) -> Result<Async<R>, Error> {
        self.chan.poll().map_err(Into::into)
    }
}


impl<N: NotificationTrait> NotificationWrap<N> {
    pub fn new(n: N) -> NotificationWrap<N> {
        NotificationWrap {
            data: n,
        }
    }
}

