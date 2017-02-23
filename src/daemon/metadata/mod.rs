use futures_cpupool::{CpuPool, CpuFuture};

use ciruela::proto::{AppendDir, AppendDirAck};
use config::Config;


#[derive(Clone)]
pub struct Meta {
    cpu_pool: CpuPool,
    config: Config,
}

impl Meta {
    pub fn new(num_threads: usize, config: &Config) -> Meta {
        Meta {
            cpu_pool: CpuPool::new(num_threads),
            config: config.clone(),
        }
    }
    pub fn append_dir(&self, append_dir: AppendDir)
        -> CpuFuture<AppendDirAck, ()>
    {
        self.cpu_pool.spawn_fn(|| {
            Ok((AppendDirAck {
                accepted: false,
            }))
        })
    }
}
