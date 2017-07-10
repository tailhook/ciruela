mod error;
mod public;

pub use self::public::{Disk, Image, start};
pub use self::error::Error;


use std::sync::Arc;

use futures::sync::mpsc::{UnboundedReceiver};
use futures_cpupool::CpuPool;


pub struct Init {
}
