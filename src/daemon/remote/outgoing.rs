use std::net::SocketAddr;

use futures::{Future, Stream};
use futures::sync::mpsc::UnboundedReceiver;
use tokio_core::net::TcpStream;
use tk_easyloop::{spawn, handle};
use tk_http::websocket::client::{HandshakeProto, SimpleAuthorizer};
use tk_http::websocket::{Loop};

use ciruela::proto::{WrapTrait, Registry};
use ciruela::proto::{StreamExt};
use remote::websocket::{Dispatcher, Connection};
use remote::{Remote, Token};
use tracking::Tracking;


fn closed(():()) -> &'static str {
    "channel closed"
}


pub fn connect(sys: &Remote, tracking: &Tracking, reg: &Registry,
    cli: Connection, tok: Token,
    addr: SocketAddr, rx: UnboundedReceiver<Box<WrapTrait>>)
{
    let sys = sys.clone();
    let reg = reg.clone();
    let tracking = tracking.clone();
    spawn(TcpStream::connect(&addr, &handle())
        .map_err(|_| unimplemented!())
        .and_then(move |sock| {
            HandshakeProto::new(sock, SimpleAuthorizer::new(
                "ciruela_internal", "/"))
            .map_err(|e| error!("Handshake error: {}", e))
        })
        .and_then(move |(out, inp, ())| {
            debug!("Established outgoing connection to {}", addr);
            // consider connection as non-failed right after handshake
            sys.inner().failures.reset(addr);
            let disp = Dispatcher::new(cli, &reg, &tracking);
            let rx = rx.map_err(closed as fn(()) -> &'static str);
            let rx = rx.packetize(&reg);
            Loop::client(out, inp, rx, disp, sys.websock_config(), &handle())
            .then(move |_| {
                drop(tok);
                Ok(())
            })
        }));
}
