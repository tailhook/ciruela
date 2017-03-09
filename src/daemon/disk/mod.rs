mod error;
mod public;
mod message;
mod dispatcher;

pub use self::public::{Disk, start};
pub use self::error::Error;


use std::sync::Arc;

use futures::sync::mpsc::{UnboundedReceiver};
use futures_cpupool::CpuPool;

use config::Config;


pub struct Init {
    pool: CpuPool,
    config: Arc<Config>,
    rx: UnboundedReceiver<message::Message>,
}
