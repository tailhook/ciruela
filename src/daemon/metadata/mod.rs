mod error;
mod dir;

mod append_dir;
mod first_scan;
mod read_index;
mod scan;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures_cpupool::{CpuPool, CpuFuture};

use ciruela::database::signatures::State;
use ciruela::proto::{AppendDir, AppendDirAck};
use ciruela::{ImageId, VPath};
use config::Config;
use index::Index;
use tracking::{Tracking, BaseDir};

use self::dir::Dir;
pub use self::error::Error;


#[derive(Clone)]
pub struct Meta {
    cpu_pool: CpuPool,
    config: Arc<Config>,
    writing: Arc<Mutex<HashMap<VPath, Arc<State>>>>,
    base_dir: Dir,
    tracking: Tracking,
}

impl Meta {
    pub fn new(num_threads: usize, config: &Arc<Config>,
        tracking: &Tracking)
        -> Result<Meta, Error>
    {
        let dir = Dir::open_root(&config.db_dir)?;
        // TODO(tailhook) start rescanning the database for consistency
        Ok(Meta {
            cpu_pool: CpuPool::new(num_threads),
            config: config.clone(),
            base_dir: dir,
            tracking: tracking.clone(),
            writing: Arc::new(Mutex::new(HashMap::new())),
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
    pub fn scan_base_dirs(&self) -> CpuFuture<Vec<(VPath, usize)>, Error> {
        let meta = self.clone();
        self.cpu_pool.spawn_fn(move || {
            first_scan::scan(&meta)
        })
    }
    pub fn scan_dir(&self, dir: &Arc<BaseDir>)
        -> CpuFuture<Vec<(String, State)>, Error>
    {
        let dir = dir.clone();
        let meta = self.clone();
        self.cpu_pool.spawn_fn(move || {
            scan::all_states(&dir.virtual_path, &meta)
        })
    }

    fn signatures(&self) -> Result<Dir, Error> {
        self.base_dir.ensure_dir("signatures")
    }
    fn indexes(&self) -> Result<Dir, Error> {
        self.base_dir.ensure_dir("indexes")
    }
}
