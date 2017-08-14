mod dir;
mod error;
mod keys;
mod upload;
mod first_scan;
mod read_index;
mod scan;
mod store_index;

use std::io;
use std::collections::{HashMap, BTreeMap};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use futures_cpupool::{CpuPool, CpuFuture};

use openat::Metadata;
use ciruela::database::signatures::State;
use ciruela::proto::{AppendDir, AppendDirAck};
use ciruela::proto::{ReplaceDir, ReplaceDirAck};
use ciruela::{ImageId, VPath};
use config::Config;
use index::Index;
use tracking::{Tracking};

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

fn is_fresher(meta: &Metadata, time: SystemTime) -> bool {
    let dur = time.duration_since(UNIX_EPOCH).expect("time is okay");
    return meta.stat().st_mtime as u64 > dur.as_secs() - 1;
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
            upload::start_append(params, &meta)
        })
    }
    pub fn replace_dir(&self, params: ReplaceDir)
        -> CpuFuture<ReplaceDirAck, Error>
    {
        let meta = self.clone();
        self.cpu_pool.spawn_fn(move || {
            upload::start_replace(params, &meta)
        })
    }
    pub fn dir_committed(&self, path: &VPath) {
        let meta = self.clone();
        let path: VPath = path.clone();
        self.cpu_pool.spawn_fn(move || -> Result<(), ()> {
            // need to offload to disk thread because we hold ``writing`` lock
            // in disk thread too
            if meta.writing().remove(&path).is_none() {
                error!("Spurious ack of writing {:?}", path);
            }
            Ok(())
        }).forget();
    }
    fn writing(&self) -> MutexGuard<HashMap<VPath, Arc<State>>> {
        self.writing.lock().expect("writing is not poisoned")
    }
    pub fn remove_state_file(&self, path: VPath, at: SystemTime)
        -> CpuFuture<(), Error>
    {
        // TODO(tailhook) maybe check that meta hasn't changed
        let meta = self.clone();
        self.cpu_pool.spawn_fn(move || {
            // need to lock "writing" to avoid race conditions
            let writing = meta.writing();
            if writing.contains_key(&path) {
                return Err(Error::CleanupCanceled(path));
            }
            let parent = meta.signatures()?.open_path(path.parent_rel())?;
            let state = format!("{}.state", path.final_name());
            if let Some(meta) = parent.file_meta(&state)? {
                if is_fresher(&meta, at) {
                    return Err(Error::CleanupCanceled(path));
                }
            } else {
                return Err(Error::FileWasVanished(parent.path(state)));
            }
            parent.remove_file(&state)?;
            Ok(())
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
    pub fn store_index(&self, id: &ImageId, data: Vec<u8>)
    {
        let meta = self.clone();
        let id = id.clone();
        self.cpu_pool.spawn_fn(move || -> Result<(), ()> {
            store_index::write(&id, data, &meta)
                .map_err(|e| {
                    error!("Can't store index {:?}: {}", id, e);
                })?;
            Ok(())
        }).forget();
    }
    pub fn scan_base_dirs(&self, tracking: &Tracking)
        -> CpuFuture<(), Error>
    {
        let meta = self.clone();
        let tracking = tracking.clone();
        self.cpu_pool.spawn_fn(move || {
            first_scan::scan(&meta, &tracking)
        })
    }
    pub fn scan_dir(&self, dir: &VPath)
        -> CpuFuture<BTreeMap<String, State>, Error>
    {
        let dir = dir.clone();
        let meta = self.clone();
        self.cpu_pool.spawn_fn(move || {
            match meta.signatures()?.open_vpath(&dir) {
                Ok(dir) => scan::all_states(&dir),
                Err(Error::Open(_, ref e))
                if e.kind() == io::ErrorKind::NotFound
                => Ok(BTreeMap::new()),
                Err(e) => Err(e.into()),
            }
        })
    }
    fn signatures(&self) -> Result<Dir, Error> {
        self.base_dir.ensure_dir("signatures")
    }
    fn indexes(&self) -> Result<Dir, Error> {
        self.base_dir.ensure_dir("indexes")
    }
}
