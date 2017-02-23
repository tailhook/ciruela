use futures_cpupool::{CpuPool, CpuFuture};

use ciruela::proto::{AppendDir, AppendDirAck};


#[derive(Clone)]
pub struct Meta {
    cpu_pool: CpuPool,
}

impl Meta {
    pub fn new(num_threads: usize) -> Meta {
        Meta {
            cpu_pool: CpuPool::new(num_threads),
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
