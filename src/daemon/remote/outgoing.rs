use std::net::SocketAddr;

use futures::{Future, Stream};
use futures::sync::mpsc::UnboundedReceiver;
use tokio_core::net::TcpStream;
use tk_easyloop::{spawn, handle};
use tk_http::websocket::client::{HandshakeProto, SimpleAuthorizer};
use tk_http::websocket::{Loop};

use ciruela::proto::{WrapTrait};
use ciruela::proto::{StreamExt};
use websocket::{Dispatcher, Connection};
use remote::{Remote, Token};


fn closed(():()) -> &'static str {
    "channel closed"
}


pub fn connect(sys: &Remote, cli: Connection, tok: Token,
    addr: SocketAddr, rx: UnboundedReceiver<Box<WrapTrait>>)
{
    let sys = sys.clone();
    spawn(TcpStream::connect(&addr, &handle())
        .map_err(|_| unimplemented!())
        .and_then(move |sock| {
            HandshakeProto::new(sock, SimpleAuthorizer::new(
                "ciruela_internal", "/"))
            .map_err(|_| unimplemented!())
        })
        .and_then(move |(out, inp, ())| {
            debug!("Established outgoing connection to {}", addr);
            let (disp, registry) = Dispatcher::new(cli, &sys);
            let rx = rx.map_err(closed as fn(()) -> &'static str);
            let rx = rx.packetize(&registry);
            Loop::client(out, inp, rx, disp, sys.websock_config(), &handle())
            .then(move |_| {
                drop(tok);
                Ok(())
            })
        }));
}
