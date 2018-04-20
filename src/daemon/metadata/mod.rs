mod dir;
mod error;
mod first_scan;
mod hardlink_sources;
mod index_gc;
mod keys;
mod read_index;
mod scan;
mod store_index;
mod upload;

use std::io;
use std::collections::{HashMap, HashSet, BTreeMap};
use std::sync::{Arc};
use std::time::{SystemTime, UNIX_EPOCH};

use openat::Metadata;
use futures_cpupool::{self, CpuPool, CpuFuture};
use self_meter_http::Meter;

use database::signatures::{State, SignatureEntry};
use proto::{AppendDir};
use proto::{ReplaceDir};
use {VPath};
use config::Config;
use tracking::Index;
use index::{ImageId};
use index_cache::{IndexData};
use metrics::{List, Metric};
use named_mutex::{Mutex, MutexGuard};
use void::Void;

use self::dir::Dir;
pub use self::upload::{Upload, Accept};
pub use self::error::Error;
pub use self::hardlink_sources::Hardlink;


#[derive(Clone)]
pub struct Meta(Arc<Inner>);

struct Writing {
    pub image: ImageId,
    pub signatures: Vec<SignatureEntry>,
    pub replacing: bool,
}

struct Inner {
    cpu_pool: CpuPool,
    config: Arc<Config>,
    writing: Mutex<HashMap<VPath, Writing>>,
    collecting: Mutex<Option<HashSet<ImageId>>>,
    base_dir: Dir,
}

fn is_fresher(meta: &Metadata, time: SystemTime) -> bool {
    let dur = time.duration_since(UNIX_EPOCH).expect("time is okay");
    return meta.stat().st_mtime as u64 > dur.as_secs() - 1;
}

impl Meta {
    pub fn new(num_threads: usize, config: &Arc<Config>, meter: &Meter)
        -> Result<Meta, Error>
    {
        let dir = Dir::open_root(&config.db_dir)?;
        // TODO(tailhook) start rescanning the database for consistency
        let m1 = meter.clone();
        let m2 = meter.clone();
        Ok(Meta(Arc::new(Inner {
            cpu_pool: futures_cpupool::Builder::new()
                .pool_size(num_threads)
                .name_prefix("disk-")
                .after_start(move || m1.track_current_thread_by_name())
                .before_stop(move || m2.untrack_current_thread())
                .create(),
            config: config.clone(),
            base_dir: dir,
            writing: Mutex::new(HashMap::new(), "metadata_writing"),
            collecting: Mutex::new(None, "metadata_collecting"),
        })))
    }
    pub fn get_image_id(&self, vpath: &VPath)
        -> CpuFuture<ImageId, Error>
    {
        let meta = self.clone();
        let vpath = vpath.clone();
        self.0.cpu_pool.spawn_fn(move || {
            let dir = meta.signatures()?.ensure_dir(vpath.parent_rel())?;
            if let Some(state) = dir.read_file(
                &format!("{}.state", vpath.final_name()),
                upload::read_state)?
            {
                Ok(state.image)
            } else {
                Err(Error::PathNotFound(vpath))
            }
        })
    }
    pub fn append_dir(&self, params: AppendDir)
        -> CpuFuture<Upload, Error>
    {
        let meta = self.clone();
        self.0.cpu_pool.spawn_fn(move || {
            upload::start_append(params, &meta)
        })
    }
    pub fn replace_dir(&self, params: ReplaceDir)
        -> CpuFuture<Upload, Error>
    {
        let meta = self.clone();
        self.0.cpu_pool.spawn_fn(move || {
            upload::start_replace(params, &meta)
        })
    }
    pub fn resume_dir(&self, path: &VPath)
        -> CpuFuture<ImageId, Error>
    {
        let meta = self.clone();
        let path = path.clone();
        self.0.cpu_pool.spawn_fn(move || {
            upload::resume_upload(&path, &meta)
        })
    }
    pub fn dir_aborted(&self, path: &VPath) -> CpuFuture<(), Void> {
        let meta = self.clone();
        let path: VPath = path.clone();
        self.0.cpu_pool.spawn_fn(move || {
            // need to offload to disk thread because we hold ``writing`` lock
            // in disk thread too
            let mut lock = meta.writing();
            if let Some(wr) = lock.remove(&path) {
                match upload::abort_dir(&path, wr, &meta) {
                    Ok(()) => {}
                    Err(e) => {
                        error!("Error commiting abort {:?}: {}", path, e);
                        // TODO(tailhook) die?
                    }
                }
            } else {
                error!("Spurious abort of writing {:?}", path);
            }
            Ok(())
        })
    }
    pub fn dir_committed(&self, path: &VPath) -> CpuFuture<(), Void> {
        let meta = self.clone();
        let path: VPath = path.clone();
        self.0.cpu_pool.spawn_fn(move || {
            let mut lock = meta.writing();
            if let Some(wr) = lock.remove(&path) {
                match upload::commit_dir(&path, wr, &meta) {
                    Ok(()) => {}
                    Err(e) => {
                        error!("Error commiting state {:?}: {}", path, e);
                        // TODO(tailhook) die?
                    }
                }
            } else {
                error!("Spurious ack of writing {:?}", path);
                // TODO(tailhook) die?
            }
            Ok(())
        })
    }
    fn writing(&self) -> MutexGuard<HashMap<VPath, Writing>> {
        self.0.writing.lock()
    }
    pub fn remove_state_file(&self, path: VPath, at: SystemTime)
        -> CpuFuture<(), Error>
    {
        // TODO(tailhook) maybe check that meta hasn't changed
        let meta = self.clone();
        self.0.cpu_pool.spawn_fn(move || {
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
    pub fn read_index_bytes(&self, index: &ImageId)
        -> CpuFuture<Vec<u8>, Error>
    {
        let meta = self.clone();
        let index = index.clone();
        self.0.cpu_pool.spawn_fn(move || {
            read_index::read_bytes(&index, &meta)
            .map_err(|e| {
                if !matches!(e, Error::IndexNotFound) {
                    info!("Error reading index {:?} from file: {}",
                          index, e);
                }
                e
            })
        })
    }
    pub fn read_index(&self, index: &ImageId)
        -> CpuFuture<IndexData, Error>
    {
        let meta = self.clone();
        let index = index.clone();
        self.0.cpu_pool.spawn_fn(move || {
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
        self.0.cpu_pool.spawn_fn(move || -> Result<(), ()> {
            meta.mark_used(&id);
            store_index::write(&id, data, &meta)
                .map_err(|e| {
                    error!("Can't store index {:?}: {}", id, e);
                })?;
            Ok(())
        }).forget();
    }
    pub fn scan_base_dirs<F>(&self, add_dir: F)
        -> CpuFuture<(), Error>
        where F: FnMut(VPath) + Send + 'static
    {
        let meta = self.clone();
        self.0.cpu_pool.spawn_fn(move || {
            first_scan::scan(&meta, add_dir)
        })
    }
    pub fn scan_dir(&self, path: &VPath)
        -> CpuFuture<BTreeMap<String, State>, Error>
    {
        // TODO(tailhook) check against config
        //                because it's called in web UI
        let path = path.clone();
        let meta = self.clone();
        self.0.cpu_pool.spawn_fn(move || {
            match meta.signatures()?.open_vpath(&path) {
                Ok(dir) => scan::all_states(&meta, &path, &dir),
                Err(Error::Open(_, ref e))
                if e.kind() == io::ErrorKind::NotFound
                => Ok(BTreeMap::new()),
                Err(e) => Err(e.into()),
            }
        })
    }
    pub fn files_to_hardlink(&self, dir: &VPath, index: &Index)
        -> CpuFuture<Vec<Hardlink>, Error>
    {
        let dir = dir.parent();
        let meta = self.clone();
        let index = index.clone();
        self.0.cpu_pool.spawn_fn(move || {
            hardlink_sources::files_to_link(index, dir, meta)
        })
    }
    pub fn is_writing(&self, dir: &VPath)
        -> CpuFuture<bool, Error>
    {
        let dir = dir.clone();
        let meta = self.clone();
        self.0.cpu_pool.spawn_fn(move || {
            let wr = meta.writing();
            Ok(wr.contains_key(&dir))
        })
    }
    fn signatures(&self) -> Result<Dir, Error> {
        self.0.base_dir.ensure_dir("signatures")
    }
    fn indexes(&self) -> Result<Dir, Error> {
        self.0.base_dir.ensure_dir("indexes")
    }
    fn mark_used(&self, image_id: &ImageId) {
        if let Some(ref mut set) = *self.0.collecting.lock() {
            set.insert(image_id.clone());
        }
    }
    pub fn index_gc(&self)
        -> CpuFuture<(), Error>
    {
        let meta = self.clone();
        self.0.cpu_pool.spawn_fn(move || {
            let writing = meta.writing().values()
                .map(|w| w.image.clone())
                .collect();
            *meta.0.collecting.lock() = Some(writing);
            index_gc::full_collection(&meta)
        })
    }
}

pub fn metrics() -> List {
    let gc = "metadata.index_gc";
    vec![
        (Metric(gc, "started"), &*index_gc::STARTED),
        (Metric(gc, "completed"), &*index_gc::COMPLETED),
        (Metric(gc, "deleted_images"), &*index_gc::DELETED_IMAGES),
    ]
}
