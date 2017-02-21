use std::net::SocketAddr;
use std::sync::Arc;

use futures::{Async, Future, Canceled};
use futures::future::{FutureResult, ok};
use futures::stream::Stream;
use futures::sync::oneshot::{channel as oneshot, Sender, Receiver};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use tk_easyloop;
use tokio_core::net::TcpStream;
use minihttp::websocket::client::{HandshakeProto, SimpleAuthorizer};
use minihttp::websocket::{Loop, Frame, Error as WsError, Dispatcher, Config};

use proto::Request;


pub struct Client {
    channel: UnboundedSender<Box<RequestTrait>>,
}

pub struct ClientFuture {
    chan: Receiver<Client>,
}

pub struct RequestFuture<R> {
    chan: Receiver<R>,
}

trait RequestTrait {
}

struct RequestWrap<R: Request> {
    request: R,
    chan: Sender<R::Response>,
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        UnexpectedTermination {
            from(Canceled)
        }
    }
}

struct MyDispatcher;

impl Dispatcher for MyDispatcher {
    type Future = FutureResult<(), WsError>;
    fn frame(&mut self, frame: &Frame) -> FutureResult<(), WsError> {
        println!("Frame arrived: {:?}", frame);
        ok(())
    }
}

impl Client {
    pub fn spawn(addr: SocketAddr, host: &Arc<String>) -> ClientFuture {
        let host = host.to_string();
        let (tx, rx) = oneshot();
        let (ctx, crx) = unbounded();
        let wcfg = Config::new().done();
        tk_easyloop::spawn(
            TcpStream::connect(&addr, &tk_easyloop::handle())
            .from_err()
            .and_then(move |sock| {
                HandshakeProto::new(sock, SimpleAuthorizer::new(&*host, "/"))
            })
            .map_err(move |e| {
                error!("Error connecting to {}: {}", addr, e);
            })
            .and_then(move |(out, inp, ())| {
                info!("Connected to {}", addr);
                tx.complete(Client {
                    channel: ctx,
                });
                let stream = crx.map(|req| {
                    unimplemented!();
                }).map_err(|_| Error::UnexpectedTermination);
                Loop::client(out, inp, stream, MyDispatcher, &wcfg)
                .map_err(|e| println!("websocket closed: {}", e))
            })
        );
        return ClientFuture {
            chan: rx,
        }
    }
    pub fn request<R>(&self, request: R) -> RequestFuture<R::Response>
        where R: Request + 'static
    {
        let (tx, rx) = oneshot();
        self.channel.send(Box::new(RequestWrap { request: request, chan: tx }));
        return RequestFuture { chan: rx };
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

impl<R: Request> RequestTrait for RequestWrap<R> {
}
