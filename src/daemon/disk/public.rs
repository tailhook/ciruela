use std::sync::Arc;

use futures::sync::mpsc::{unbounded, UnboundedSender};
use futures_cpupool::CpuPool;
use tk_easyloop;

use config::Config;
use disk::dispatcher::Dispatcher;
use disk::message::Message;
use disk::{Init, Error};
use metadata::Meta;


#[derive(Clone)]
pub struct Disk {
    tx: UnboundedSender<Message>,
}


impl Disk {
    pub fn new(num_threads: usize, config: &Arc<Config>)
        -> Result<(Disk, Init), Error>
    {
        let (tx, rx) = unbounded();
        Ok((Disk {
            tx: tx,
        }, Init {
            pool: CpuPool::new(num_threads),
            config: config.clone(),
            rx: rx,
        }))
    }
}

pub fn start(init: Init, metadata: &Meta) -> Result<(), Error> {
   tk_easyloop::spawn(Dispatcher::new(init, metadata));
   Ok(())
}
