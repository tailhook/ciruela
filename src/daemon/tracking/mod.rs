use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};

use metadata::Meta;
use remote::Remote;
use disk::Disk;

pub struct Tracking {
    tx: UnboundedSender<Command>,
}

pub struct TrackingInit {
    rx: UnboundedReceiver<Command>,
}

pub enum Command {

}

impl Tracking {
    pub fn new() -> (Tracking, TrackingInit) {
        let (tx, rx) = unbounded();
        (Tracking {
            tx: tx,
         },
         TrackingInit {
            rx: rx,
         })
    }
}

pub fn start(init: TrackingInit, meta: &Meta, remote: &Remote, disk: &Disk)
    -> Result<(), String> // actually void
{
    //
    Ok(())
}
