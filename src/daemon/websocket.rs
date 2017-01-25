use futures::future::{FutureResult};
use futures::sync::mpsc::{channel, Sender, Receiver};

use minihttp::websocket::{Frame, Packet, Dispatcher, Error};


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
        unimplemented!();
    }
}
