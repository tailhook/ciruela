use std::collections::BTreeMap;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use futures::future::{Future, FutureResult, ok};
use futures::stream::Stream;
use self_meter_http::{Meter, ProcessReport, ThreadReport};
use serde::Serialize;
use serde_json;
use time;
use tk_easyloop::{spawn, handle};
use tk_easyloop;
use tk_http::Status;
use tk_http::server::buffered::{Request, BufferedDispatcher};
use tk_http::server::{Proto, Encoder, EncoderDone, Error, Config};
use tk_listen::ListenExt;
use tokio_core::net::TcpListener;


use mask::Mask;
use remote::websocket::Connection;
use remote::Remote;
use tracking::Tracking;


const BODY: &'static str = "Not found";

fn serve_json<S, V: Serialize>(mut e: Encoder<S>, data: &V)
    -> EncoderDone<S>
{
    e.status(Status::Ok);
    e.add_chunked().unwrap();
    e.format_header("Date", time::now_utc().rfc822()).unwrap();
    e.add_header("Server",
        concat!("ciruela/", env!("CARGO_PKG_VERSION"))
    ).unwrap();
    if e.done_headers().unwrap() {
        serde_json::to_writer(io::BufWriter::new(&mut e), data)
            .expect("can always serialize");
    }
    e.done()
}

fn service<S>(req: Request, mut e: Encoder<S>,
    meter: &Meter, tracking: &Tracking)
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
        if req.path().starts_with("/status/") {
            #[derive(Serialize)]
            struct Report<'a> {
                process: ProcessReport<'a>,
                threads: ThreadReport<'a>,
                version: &'static str,
            }
            ok(serve_json(e, &Report {
                process: meter.process_report(),
                threads: meter.thread_report(),
                version: env!("CARGO_PKG_VERSION"),
            }))
        } else if req.path().starts_with("/downloading/") {
            #[derive(Serialize)]
            pub struct Progress {
                pub image_id: String,
                pub mask: Mask,
                pub stalled: bool,
            }
            ok(serve_json(e, &tracking.get_in_progress()
                .iter().map(|(path, p)| (path, Progress {
                    image_id: format!("{}", p.image_id),
                    mask: p.mask,
                    stalled: p.stalled,
                })).collect::<BTreeMap<_, _>>()))
        } else {
            e.status(Status::NotFound);
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
}


pub fn start(addr: SocketAddr, remote: &Remote, tracking: &Tracking,
    meter: &Meter)
    -> Result<(), io::Error>
{
    let listener = TcpListener::bind(&addr, &handle())?;
    let cfg = Config::new().done();
    let remote = remote.clone();
    let tracking = tracking.clone();
    let meter = meter.clone();

    spawn(listener.incoming()
        .sleep_on_error(Duration::from_millis(100), &tk_easyloop::handle())
        .map(move |(socket, addr)| {
            let remote = remote.clone();
            let t1 = tracking.clone();
            let t2 = tracking.clone();
            let meter = meter.clone();
            Proto::new(socket, &cfg,
                BufferedDispatcher::new_with_websockets(addr, &handle(),
                    move |r, e| service(r, e, &meter, &t1),
                    move |out, inp| {
                        let (cli, fut) = Connection::incoming(
                            addr, out, inp, &remote, &t2);
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
