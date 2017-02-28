use futures::Future;
use futures::future::{FutureResult, ok};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use serde_cbor::de::from_slice;
use minihttp::websocket::{Frame, Packet, Dispatcher, Error};
use tk_easyloop::spawn;

use ciruela::proto::{Message, Request, serialize_response};
use metadata::Meta;


pub struct Connection {
    channel: UnboundedSender<Packet>,
    metadata: Meta,
}

impl Connection {
    pub fn new(metadata: &Meta) -> (Connection, UnboundedReceiver<Packet>) {
        // TODO(tailhook) not sure how large backpressure should be
        let (tx, rx) = unbounded();
        return (
            Connection {
                channel: tx,
                metadata: metadata.clone(),
            },
            rx);
    }
}

impl Dispatcher for Connection {
    // TODO(tailhook) implement backpressure
    type Future = FutureResult<(), Error>;
    fn frame(&mut self, frame: &Frame) -> Self::Future {
        match *frame {
            Frame::Binary(data) => match from_slice(data) {
                Ok(Message::Request(request_id, Request::AppendDir(ad))) => {
                    let chan = self.channel.clone();
                    spawn(self.metadata.append_dir(ad)
                    .map_err(|_| unimplemented!())
                    .map(move |res| {
                        chan.send(serialize_response(
                            request_id, "AppendDir", res))
                        .map_err(|e| error!("Failed to send response: {}", e))
                        .ok();
                    }));
                }
                Ok(Message::Response(..)) => {
                    unimplemented!();
                }
                Ok(Message::Notification(..)) => {
                    unimplemented!();
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
