mod append_dir;
mod config;
mod error;
mod dir_ext;

use std::io;
use std::sync::Arc;

use openat::Dir;
use futures_cpupool::{CpuPool, CpuFuture};

use ciruela::proto::{AppendDir, AppendDirAck};
use config::Config;

pub use self::error::Error;
pub use self::config::find_config_dir;


#[derive(Clone)]
pub struct Meta {
    cpu_pool: CpuPool,
    config: Arc<Config>,
    base_dir: Arc<Dir>,
}

impl Meta {
    pub fn new(num_threads: usize, config: &Arc<Config>)
        -> Result<Meta, io::Error>
    {
        let dir = Dir::open(&config.db_dir)?;
        // TODO(tailhook) start rescanning the database for consistency
        Ok(Meta {
            cpu_pool: CpuPool::new(num_threads),
            config: config.clone(),
            base_dir: Arc::new(dir),
        })
    }
    pub fn append_dir(&self, params: AppendDir)
        -> CpuFuture<AppendDirAck, Error>
    {
        let meta = self.clone();
        self.cpu_pool.spawn_fn(move || {
            append_dir::start(params, &meta)
        })
    }
}
