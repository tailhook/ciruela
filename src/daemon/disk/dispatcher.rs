use std::sync::Arc;

use futures::{Future, Async, Stream};
use futures::sync::mpsc::{UnboundedReceiver};
use futures_cpupool::CpuPool;

use config::Config;
use disk::message::Message;
use disk::{Init};
use metadata::Meta;

pub struct Dispatcher {
    rx: UnboundedReceiver<Message>,
    meta: Meta,
    config: Arc<Config>,
    pool: CpuPool,
}

impl Dispatcher {
    pub fn new(init: Init, metadata: &Meta) -> Dispatcher {
        Dispatcher {
            rx: init.rx,
            meta: metadata.clone(),
            config: init.config,
            pool: init.pool,
        }
    }
}

impl Future for Dispatcher {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        while let Ok(Async::Ready(Some(msg))) = self.rx.poll() {
            match msg {
            }
        }
        Ok(Async::NotReady)
    }
}
