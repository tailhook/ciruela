mod config;
mod error;
mod dir_ext;

mod append_dir;
mod first_scan;
mod read_index;
mod scan;

use std::io;
use std::sync::Arc;
use std::path::PathBuf;

use openat::Dir;
use futures_cpupool::{CpuPool, CpuFuture};

use ciruela::ImageId;
use ciruela::proto::{AppendDir, AppendDirAck};
use cleanup;
use config::Config;
use disk::Disk;
use index::Index;
use tracking::{Tracking, BaseDir};

pub use self::error::Error;
pub use self::config::find_config_dir;


#[derive(Clone)]
pub struct Meta {
    cpu_pool: CpuPool,
    config: Arc<Config>,
    base_dir: Arc<Dir>,
    tracking: Tracking,
}

impl Meta {
    pub fn new(num_threads: usize, config: &Arc<Config>,
        tracking: &Tracking)
        -> Result<Meta, io::Error>
    {
        let dir = Dir::open(&config.db_dir)?;
        // TODO(tailhook) start rescanning the database for consistency
        Ok(Meta {
            cpu_pool: CpuPool::new(num_threads),
            config: config.clone(),
            base_dir: Arc::new(dir),
            tracking: tracking.clone(),
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
    pub fn read_index(&self, index: &ImageId)
        -> CpuFuture<Index, Error>
    {
        let meta = self.clone();
        let index = index.clone();
        self.cpu_pool.spawn_fn(move || {
            let res = read_index::read(&index, &meta);
            match res {
                Ok(ref index) => {
                    info!("Read index {:?} from file", &index.id);
                }
                Err(ref e) => {
                    info!("Error reading index {:?} from file: {}",
                          index, e);
                }
            }
            res
        })
    }
    pub fn scan_base_dirs(&self) -> CpuFuture<Vec<(PathBuf, usize)>, Error> {
        let meta = self.clone();
        self.cpu_pool.spawn_fn(move || {
            first_scan::scan(&meta)
        })
    }
    pub fn scan_dir(&self, dir: &Arc<BaseDir>)
        -> CpuFuture<Vec<cleanup::Image>, Error>
    {
        let dir = dir.clone();
        self.cpu_pool.spawn_fn(move || {
            scan::all_states(&dir.virtual_path)
            .map(move |ok| {
                ok.into_iter()
                .map(|(p, s)| cleanup::Image {
                    path: p,
                    target_state: s,
                }).collect()
            })
        })
    }
}
