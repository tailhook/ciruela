use std::net::SocketAddr;
use std::time::Duration;

use futures::{Future, Stream};
use futures::future::Either;
use futures::sync::mpsc::UnboundedReceiver;
use tokio_core::net::TcpStream;
use tk_easyloop::{spawn, handle, timeout};
use tk_http::websocket::client::{HandshakeProto, SimpleAuthorizer};
use tk_http::websocket::{Loop};

use proto::{WrapTrait, Registry};
use proto::{StreamExt};
use remote::websocket::{Dispatcher, Connection};
use remote::{Remote, Token};
use tracking::Tracking;


/// Connection timeout for server to server connections
///
/// It's usually cheap enough to reconnect to other host,
/// or retry connection again. More specifically
/// 1. If connection is timed out because of network loss, reconnect will just
///    work
/// 2. If connection is timed out because target machine is slow, we're probably
///    should avoid connecting to the node if possible
const CONNECTION_TIMEO_MS: u64 = 500;


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
        .map_err(|e| {
            error!("Tcp connection error: {}", e);
        })
        .and_then(move |sock| {
            HandshakeProto::new(sock, SimpleAuthorizer::new(
                "ciruela_internal", "/"))
            .map_err(|e| error!("Handshake error: {}", e))
        })
        .select2(timeout(Duration::from_millis(CONNECTION_TIMEO_MS)))
        .then(move |v| match v {
            Ok(Either::A((v, _))) => Ok(v),
            Ok(Either::B(((), _))) => {
                error!("Connection to {} timed out after {} ms",
                    addr, CONNECTION_TIMEO_MS);
                Err(())
            }
            Err(Either::A((e, _))) => {
                error!("Connection to {} failed: {:?}", addr, e);
                Err(())
            }
            Err(Either::B(_)) => unreachable!(),
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
