use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::collections::HashMap;

use futures::{Async, Future, Canceled, Stream};
use futures::sync::oneshot::{channel as oneshot, Sender as OneShot, Receiver};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use serde::Serialize;
use serde_cbor::ser::Serializer as Cbor;
use tk_http::websocket::{Packet};
use mopa;

use {ImageId};
use proto::{REQUEST, RESPONSE, NOTIFICATION};
use proto::message;
use proto::dir_commands::{AppendDir, ReplaceDir};
use proto::index_commands::GetIndex;
use proto::block_commands::GetBlock;
use proto::p2p_commands::GetBaseDir;


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        UnexpectedTermination {
            from(Canceled)
        }
        IndexNotFound {
        }
        BlockNotFound {
        }
        IndexParseError(id: ImageId, err: ::dir_signature::v1::ParseError) {
            context(id: &'a ImageId, err: ::dir_signature::v1::ParseError)
            -> (id.clone(), err)
        }
        CantReadBlock {
        }
    }
}



pub trait Request: Serialize + Send + 'static {
    type Response: Send + 'static;
    fn type_name(&self) -> &'static str;
}

pub trait Response: Serialize + Send + 'static {
    fn type_name(&self) -> &'static str;
    fn static_type_name() -> &'static str;
}

pub trait Notification: Serialize + Send + 'static {
    fn type_name(&self) -> &'static str;
}

pub struct RequestFuture<R> {
    chan: Receiver<R>,
}

pub trait WrapTrait: mopa::Any + Send {
    fn is_request(&self) -> bool;
    fn send_error(&mut self, err: String);
    fn serialize_req(&self, request_id: u64) -> Packet;
    fn serialize(&self) -> Packet;
}

mopafy!(WrapTrait);

pub struct RequestWrap<R: Request> {
    request: R,
    chan: Option<OneShot<R::Response>>,
}

pub struct NotificationWrap<N: Notification> {
    data: N,
}

pub struct ResponseWrap<R: Response> {
    request_id: u64,
    data: R,
}

pub struct ErrorWrap<E: fmt::Display + Send> {
    request_id: u64,
    error: E,
}

pub struct PacketStream<S> {
    stream: S,
    registry: Registry,
}

#[derive(Clone)]
pub struct Registry(Arc<Mutex<RegistryInner>>);

#[derive(Clone)]
pub struct Sender(UnboundedSender<Box<WrapTrait>>);

struct RegistryInner {
    requests: HashMap<u64, Box<WrapTrait>>,
    last_request_id: u64,
}

pub trait RequestClient {
    fn request_channel(&self) -> &Sender;
    fn request<R>(&self, request: R) -> RequestFuture<R::Response>
        where R: Request + 'static
    {
        let (tx, rx) = oneshot();
        self.request_channel().0.send(Box::new(RequestWrap {
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

fn respond<T: Request, D: RequestDispatcher + ?Sized>(request_id: u64,
    value: T::Response, dispatcher: &D)
{
    let requests = dispatcher.request_registry();
    match requests.remove(request_id) {
        Some(mut r) => {
            r.downcast_mut::<RequestWrap<T>>()
            .map(|r| {
                r.chan.take().unwrap()
                .send(value)
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

fn respond_error<D: RequestDispatcher + ?Sized>(request_id: u64,
    err: String, dispatcher: &D)
{
    let requests = dispatcher.request_registry();
    match requests.remove(request_id) {
        Some(mut r) => {
            error!("Error in request {}: {}", request_id, err);
            r.send_error(err);
        }
        None => {
            error!("Unsolicited error {}", request_id);
        }
    }
}

pub trait RequestDispatcher {
    fn request_registry(&self) -> &Registry;
    fn respond(&self, request_id: u64, response: message::Response) {
        use proto::message::Response as R;
        match response {
            R::AppendDir(x) => respond::<AppendDir, _>(request_id, x, self),
            R::ReplaceDir(x) => respond::<ReplaceDir, _>(request_id, x, self),
            R::GetIndex(x) => respond::<GetIndex, _>(request_id, x, self),
            R::GetBlock(x) => respond::<GetBlock, _>(request_id, x, self),
            R::GetBaseDir(x) => respond::<GetBaseDir, _>(request_id, x, self),
            R::Error(x) => respond_error(request_id, x, self),
        }
    }
}

impl<R: Request> WrapTrait for RequestWrap<R> {
    fn is_request(&self) -> bool {
        true
    }
    fn send_error(&mut self, err: String) {
        // TODO(tailhook) propagate the error
        // currently we just close channel to wake up a listener
        self.chan.take().unwrap();
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

impl<N: Notification> WrapTrait for NotificationWrap<N> {
    fn is_request(&self) -> bool {
        false
    }
    fn send_error(&mut self, _err: String) { unreachable!(); }
    fn serialize_req(&self, _: u64) -> Packet { unreachable!(); }
    fn serialize(&self) -> Packet {
        let mut buf = Vec::new();
        (NOTIFICATION, self.data.type_name(), &self.data)
            .serialize(&mut Cbor::new(&mut buf))
            .expect("Can always serialize request data");
        return Packet::Binary(buf);
    }
}

impl<N: Response> WrapTrait for ResponseWrap<N> {
    fn is_request(&self) -> bool {
        false
    }
    fn serialize_req(&self, _: u64) -> Packet { unreachable!(); }
    fn send_error(&mut self, _err: String) { unreachable!(); }
    fn serialize(&self) -> Packet {
        let mut buf = Vec::new();
        (RESPONSE, self.data.type_name(), self.request_id, &self.data)
            .serialize(&mut Cbor::new(&mut buf))
            .expect("Can always serialize request data");
        return Packet::Binary(buf);
    }
}

impl<E: fmt::Display + Send + 'static> WrapTrait for ErrorWrap<E> {
    fn is_request(&self) -> bool {
        false
    }
    fn serialize_req(&self, _: u64) -> Packet { unreachable!(); }
    fn send_error(&mut self, _err: String) { unreachable!(); }
    fn serialize(&self) -> Packet {
        let mut buf = Vec::new();
        (RESPONSE, "Error", self.request_id, self.error.to_string())
            .serialize(&mut Cbor::new(&mut buf))
            .expect("Can always serialize request data");
        return Packet::Binary(buf);
    }
}

impl<R: fmt::Debug> Future for RequestFuture<R> {
    type Item = R;
    type Error = Error;
    fn poll(&mut self) -> Result<Async<R>, Error> {
        match self.chan.poll()? {
            Async::Ready(x) => {
                debug!("Received response {:?}", x);
                Ok(Async::Ready(x))
            }
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}


impl<N: Notification> NotificationWrap<N> {
    pub fn new(n: N) -> NotificationWrap<N> {
        NotificationWrap {
            data: n,
        }
    }
}

impl<T> PacketStream<T> {
    pub fn new(inner: T, registry: &Registry)
        -> PacketStream<T>
    {
        PacketStream {
            stream: inner,
            registry: registry.clone(),
        }
    }
}

impl<T: Stream<Item=Box<WrapTrait>>> Stream for PacketStream<T> {
    type Item = Packet;
    type Error = T::Error;
    fn poll(&mut self) -> Result<Async<Option<Packet>>, T::Error> {
        match self.stream.poll() {
            Ok(Async::Ready(Some(val))) => {
                if val.is_request() {
                    let mut reg = self.registry.lock();
                    reg.last_request_id += 1;
                    while reg.requests.contains_key(&reg.last_request_id) {
                        // in case we're on 32bit platform and counter
                        // is wrapped
                        reg.last_request_id += 1;
                    }
                    let r_id = reg.last_request_id;
                    let packet = val.serialize_req(r_id);
                    reg.requests.insert(r_id, val);
                    Ok(Async::Ready(Some(packet)))
                } else {
                    Ok(Async::Ready(Some(val.serialize())))
                }
            }
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}

impl Registry {
    pub fn new() -> Registry {
        Registry(Arc::new(Mutex::new(
            RegistryInner {
                requests: HashMap::new(),
                last_request_id: 0,
            }
        )))
    }
    fn lock(&self) -> MutexGuard<RegistryInner> {
        self.0.lock().expect("request registry is not poisoned")
    }
    pub fn remove(&self, request_id: u64) -> Option<Box<WrapTrait>> {
        self.lock().requests.remove(&request_id)
    }
}

impl Sender {
    pub fn channel()
        -> (Sender, UnboundedReceiver<Box<WrapTrait>>)
    {
        let (tx, rx) = unbounded();
        (Sender(tx), rx)
    }
    pub fn notification<N: Notification>(&self, n: N) {
        self.0.send(Box::new(NotificationWrap::new(n)))
        .map_err(|e| {
            // We expect `rx` to get cancellation notice in case of error, so
            // process does not hang, after logging the message
            error!("Error sending notification: {}", e)
        }).ok();
    }
    pub fn response<R: Response>(&self, request_id: u64, data: R) {
        self.0.send(Box::new(ResponseWrap {
            request_id: request_id,
            data: data,
        }))
        .map_err(|e| {
            // We expect `rx` to get cancellation notice in case of error, so
            // process does not hang, after logging the message
            error!("Error sending response: {}", e)
        }).ok();
    }
    pub fn error_response<E>(&self, request_id: u64, e: E)
        where E: fmt::Display + Send + 'static
    {
        self.0.send(Box::new(ErrorWrap {
            request_id: request_id,
            error: e,
        }))
        .map_err(|e| {
            // We expect `rx` to get cancellation notice in case of error, so
            // process does not hang, after logging the message
            error!("Error sending response: {}", e)
        }).ok();
    }
}

