use std::net::SocketAddr;

use time;
use futures::future::{Future, FutureResult, ok};
use futures::stream::Stream;
use minihttp::Status;
use minihttp::server::{Proto, Encoder, EncoderDone, Error, Config};
use minihttp::server::buffered::{Request, BufferedDispatcher};
use minihttp::websocket::{Loop, Config as WsConfig};
use tokio_core::io::Io;
use tokio_core::net::TcpListener;
use tokio_core::reactor::Handle;


use websocket::Connection;


const BODY: &'static str = "Not found";

fn service<S:Io>(req: Request, mut e: Encoder<S>)
    -> FutureResult<EncoderDone<S>, Error>
{
    if let Some(ws) = req.websocket_handshake() {
        e.status(Status::SwitchingProtocol);
        e.format_header("Date", time::now_utc().rfc822()).unwrap();
        e.add_header("Server",
            concat!("ciruela/", env!("CARGO_PKG_VERSION"))
        ).unwrap();
        e.add_header("Connection", "upgrade").unwrap();
        e.add_header("Upgrade", "websocket").unwrap();
        e.format_header("Sec-Websocket-Accept", &ws.accept).unwrap();
        e.done_headers().unwrap();
        ok(e.done())
    } else {
        e.status(Status::Ok);
        e.add_length(BODY.as_bytes().len() as u64).unwrap();
        e.format_header("Date", time::now_utc().rfc822()).unwrap();
        e.add_header("Server",
            concat!("ciruela/", env!("CARGO_PKG_VERSION"))
        ).unwrap();
        if e.done_headers().unwrap() {
            e.write_body(BODY.as_bytes());
        }
        ok(e.done())
    }
}


pub fn start(addr: SocketAddr, handle: &Handle) {
    let listener = TcpListener::bind(&addr, handle).unwrap();
    let cfg = Config::new().done();
    let wcfg = WsConfig::new().done();
    let h1 = handle.clone();

    handle.spawn(listener.incoming()
        .map_err(|e| { println!("Accept error: {}", e); })
        .map(move |(socket, addr)| {
            let wcfg = wcfg.clone();
            Proto::new(socket, &cfg,
                BufferedDispatcher::new_with_websockets(addr, &h1,
                    service,
                    move |out, inp| {
                        let (conn, rx) = Connection::new();
                        let rx = rx.map_err(|()| "channel closed");
                        Loop::new(out, inp, rx, conn, &wcfg)
                        .map_err(|e| debug!("websocket closed: {}", e))
                    }))
            .map_err(|e| { debug!("Connection error: {}", e); })
        })
        .buffer_unordered(1000)
          .for_each(|()| Ok(())));
}
