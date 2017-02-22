use futures::future::{FutureResult, ok};
use futures::sync::mpsc::{channel, Sender, Receiver};
use serde_cbor::de::from_slice;
use minihttp::websocket::{Frame, Packet, Dispatcher, Error};

use ciruela::proto::{Message, Request};


pub struct Connection {
    channel: Sender<Packet>,
}

impl Connection {
    pub fn new() -> (Connection, Receiver<Packet>) {
        // TODO(tailhook) not sure how large backpressure should be
        let (tx, rx) = channel(1);
        return (
            Connection {
                channel: tx,
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
                    unimplemented!();
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
