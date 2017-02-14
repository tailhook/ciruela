use std::net::SocketAddr;
use std::sync::Arc;

use futures::{Async, Future};
use futures::sync::oneshot::{channel as oneshot, Receiver};
use tk_easyloop;
use tokio_core::net::TcpStream;
use minihttp::websocket::client::{HandshakeProto, SimpleAuthorizer};


pub struct Client {
}

pub struct ClientFuture {
    chan: Receiver<Client>,
}

impl Client {
    pub fn spawn(addr: SocketAddr, host: &Arc<String>) -> ClientFuture {
        let host = host.to_string();
        let (tx, rx) = oneshot();
        tk_easyloop::spawn(
            TcpStream::connect(&addr, &tk_easyloop::handle())
            .from_err()
            .and_then(move |sock| {
                HandshakeProto::new(sock, SimpleAuthorizer::new(&*host, "/"))
            })
            .map(move |conn| {
                info!("Connected to {}", addr);
                tx.complete(Client {
                    // conn:  // TODO
                });
            })
            .map_err(move |e| {
                error!("Error connecting to {}: {}", addr, e);
            })
        );
        return ClientFuture {
            chan: rx,
        }
    }
}

impl Future for ClientFuture {
    type Item = Client;
    type Error = ();
    fn poll(&mut self) -> Result<Async<Client>, ()> {
        self.chan.poll().map_err(|_| ())
    }
}
