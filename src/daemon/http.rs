use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use futures::future::{Future, FutureResult, ok};
use futures::stream::Stream;
use time;
use tk_easyloop::{spawn, handle};
use tk_easyloop;
use tk_http::Status;
use tk_http::server::buffered::{Request, BufferedDispatcher};
use tk_http::server::{Proto, Encoder, EncoderDone, Error, Config};
use tk_listen::ListenExt;
use tokio_core::net::TcpListener;


use remote::websocket::Connection;
use remote::Remote;
use tracking::Tracking;
use ciruela::proto::Registry;


const BODY: &'static str = "Not found";

fn service<S>(req: Request, mut e: Encoder<S>)
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


pub fn start(addr: SocketAddr, remote: &Remote, tracking: &Tracking)
    -> Result<(), io::Error>
{
    let listener = TcpListener::bind(&addr, &handle())?;
    let cfg = Config::new().done();
    let remote = remote.clone();
    let tracking = tracking.clone();

    spawn(listener.incoming()
        .sleep_on_error(Duration::from_millis(100), &tk_easyloop::handle())
        .map(move |(socket, addr)| {
            let remote = remote.clone();
            let tracking = tracking.clone();
            Proto::new(socket, &cfg,
                BufferedDispatcher::new_with_websockets(addr, &handle(),
                    service,
                    move |out, inp| {
                        let (cli, fut) = Connection::incoming(
                            addr, out, inp, &remote, &tracking);
                        let token = remote.register_connection(&cli);
                        fut
                            .map_err(|e| debug!("websocket closed: {}", e))
                            .then(move |r| {
                                drop(token);
                                r
                            })
                    }),
                &tk_easyloop::handle())
            .map_err(|e| { debug!("Connection error: {}", e); })
        })
        .listen(1000));
    Ok(())
}
