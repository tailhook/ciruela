use std::net::SocketAddr;

use time;
use futures::future::{Future, FutureResult, ok};
use futures::stream::Stream;
use minihttp::Status;
use minihttp::server::{Proto, Encoder, EncoderDone, Error, Config};
use minihttp::server::buffered::{Request, BufferedDispatcher};
use tokio_core::io::Io;
use tokio_core::net::TcpListener;
use tokio_core::reactor::Handle;


const BODY: &'static str = "Not found";

fn service<S:Io>(_: Request, mut e: Encoder<S>)
    -> FutureResult<EncoderDone<S>, Error>
{
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


pub fn start(addr: SocketAddr, handle: &Handle) {
    let listener = TcpListener::bind(&addr, handle).unwrap();
    let cfg = Config::new().done();

    handle.spawn(listener.incoming()
        .map_err(|e| { println!("Accept error: {}", e); })
        .map(move |(socket, addr)| {
            Proto::new(socket, &cfg, BufferedDispatcher::new(addr, || service))
            .map_err(|e| { println!("Connection error: {}", e); })
        })
        .buffer_unordered(1000)
          .for_each(|()| Ok(())));
}
