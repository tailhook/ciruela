use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::{Future, Stream};
use futures::stream::MapErr;
use futures::future::{FutureResult, ok};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use serde_cbor::de::from_slice;
use tk_http::websocket::{self, Frame, Packet, Error, Config, Loop};
use tk_http::websocket::ServerCodec;
use tk_easyloop::spawn;
use tk_bufstream::{WriteFramed, ReadFramed};
use tokio_core::net::TcpStream;

use ciruela::proto::{Message, Request, Notification, serialize_response};
use ciruela::ImageId;
use metadata::Meta;

lazy_static! {
    static ref connection_id: AtomicUsize = AtomicUsize::new(0);
}

#[derive(Clone)]
pub struct Connection(Arc<ConnectionState>);

struct ConnectionState {
    id: usize,
    sender: UnboundedSender<Packet>,
    images: Mutex<HashSet<ImageId>>,
}


pub struct Dispatcher {
    connection: Connection,
    metadata: Meta,
}

impl Connection {
    pub fn new(out: WriteFramed<TcpStream, ServerCodec>,
               inp: ReadFramed<TcpStream, ServerCodec>,
               meta: &Meta, cfg: &Arc<Config>)
        -> (Connection, Loop<TcpStream,
            MapErr<UnboundedReceiver<Packet>, fn(()) -> &'static str>,
            Dispatcher>)
    {
        // TODO(tailhook) not sure how large backpressure should be
        let (tx, rx) = unbounded();
        let rx = rx.map_err(closed as fn(()) -> &'static str);
        let id = connection_id.fetch_add(1, Ordering::SeqCst);
        let cli = Connection(Arc::new(ConnectionState {
            id: id,
            sender: tx,
            images: Mutex::new(HashSet::new()),
        }));
        let disp = Dispatcher {
            connection: cli.clone(),
            metadata: meta.clone(),
        };
        let fut = Loop::server(out, inp, rx, disp, cfg);
        return (cli, fut);
    }

    fn images(&self) -> MutexGuard<HashSet<ImageId>> {
        self.0.images.lock()
            .expect("images are not poisoned")
    }
}

fn closed(():()) -> &'static str {
    "channel closed"
}

impl websocket::Dispatcher for Dispatcher {
    // TODO(tailhook) implement backpressure
    type Future = FutureResult<(), Error>;
    fn frame(&mut self, frame: &Frame) -> Self::Future {
        match *frame {
            Frame::Binary(data) => match from_slice(data) {
                Ok(Message::Request(request_id, Request::AppendDir(ad))) => {
                    let chan = self.connection.0.sender.clone();
                    spawn(self.metadata.append_dir(ad).then(move |res| {
                        let send_res = match res {
                            Ok(value) => {
                                chan.send(serialize_response(
                                    request_id, "AppendDir", value))
                            }
                            Err(e) => {
                                error!("AppendDir error: {}", e);
                                chan.send(serialize_response(
                                    request_id, "Error", e.to_string()))
                            }
                        };
                        send_res.map_err(|e| {
                            error!("Failed to send response: {}", e)
                        }).ok();
                        Ok(())
                    }));
                }
                Ok(Message::Response(..)) => {
                    unimplemented!();
                }
                Ok(Message::Notification(Notification::PublishIndex(idx))) => {
                    self.connection.images().insert(idx.image_id);
                    // TODO(tailhook) wakeup remote subsystem, so it can
                    // fetch image from this peer if image is currently in
                    // hanging state
                }
                Err(e) => {
                    error!("Failed to deserialize frame, \
                        error: {}, frame: {:?}", e, frame);
                }
            },
            _ => {
                error!("Bad frame received: {:?}", frame);
            }
        }
        ok(())
    }
}

impl Hash for Connection {
    fn hash<H>(&self, state: &mut H)
        where H: Hasher
    {
        self.0.id.hash(state)
    }
}

impl PartialEq for Connection {
    fn eq(&self, other: &Connection) -> bool {
        self.0.id == other.0.id
    }
}

impl Eq for Connection {}
