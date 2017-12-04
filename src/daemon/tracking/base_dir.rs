use std::collections::{HashMap, BTreeSet};
use std::collections::hash_map::Entry;
use std::sync::{Arc};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, Duration};

use futures::Future;

use atomic::Atomic;
use config::Directory;
use cleanup::{sort_out};
use disk::{self, Disk};
use metadata::{self, Meta};
use named_mutex::{Mutex, MutexGuard};
use peers::config::get_hash;
use proto::{Hash, BaseDirState};
use tracking::Subsystem;
use {VPath};


/// Time hashes are kept in cache so we can skip checking if some peer sends
/// us the same hash again
const RETAIN_TIME: u64 = 300_000;


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Meta(err: metadata::Error) {
            description(err.description())
            display("{}", err)
            from(err)
        }
        Disk(err: disk::Error) {
            description(err.description())
            display("{}", err)
            from(err)
        }
    }
}


#[derive(Debug)]
pub struct BaseDir {
    pub path: VPath,
    pub config: Arc<Directory>,
    hash: Atomic<Hash>,
    last_scan: Atomic<Instant>,
    num_subdirs: AtomicUsize,
    num_downloading: AtomicUsize,
    recon_table: Mutex<HashMap<Hash, Instant>>,
}

impl BaseDir {
    pub fn hash(&self) -> Hash {
        self.hash.load(Ordering::SeqCst)
    }
    pub fn subdirs(&self) -> usize {
        self.num_subdirs.load(Ordering::SeqCst)
    }
    pub fn downloading(&self) -> usize {
        self.num_downloading.load(Ordering::SeqCst)
    }
    pub fn is_superset_of(&self, hash: Hash) -> bool {
        if self.hash() == hash {
            return true;
        }
        let cut_off = Instant::now() -
            Duration::from_millis(RETAIN_TIME);
        match self.recon_table().get(&hash) {
            Some(&time) if time > cut_off => true,
            _ => false,
        }
    }
    pub fn last_scan(&self) -> Instant {
        self.last_scan.load(Ordering::SeqCst)
    }
    fn recon_table(&self) -> MutexGuard<HashMap<Hash, Instant>> {
        self.recon_table.lock()
    }
    pub fn commit_scan(dir_data: BaseDirState, config: &Arc<Directory>,
        scan_time: Instant, sys: &Subsystem)
    {
        let state = &mut *sys.state();
        let ref mut lst = state.base_dir_list;
        let hash = Hash::for_object(&dir_data.dirs);
        let down = state.in_progress.iter()
            .filter(|x| x.virtual_path.parent() == dir_data.path)
            .count();
        match state.base_dirs.entry(dir_data.path.clone()) {
            Entry::Vacant(e) => {
                debug!("New base dir {:?}: {}", dir_data.path, hash);
                let new = Arc::new(BaseDir {
                    config: config.clone(),
                    path: dir_data.path,
                    hash: Atomic::new(hash),
                    last_scan: Atomic::new(scan_time),
                    num_subdirs: AtomicUsize::new(dir_data.dirs.len()),
                    num_downloading: AtomicUsize::new(down.into()),
                    recon_table: Mutex::new(HashMap::new(), "base_dir"),
                });
                lst.push(new.clone());
                e.insert(new);
                // TODO(tailhook) send to gossip
            }
            Entry::Occupied(e) => {
                let val = e.get();
                val.last_scan.store(scan_time, Ordering::SeqCst);
                let old_hash = val.hash();
                if old_hash != hash {
                    debug!("Updated base dir {:?}: {}",
                        dir_data.path, hash);
                    val.hash.store(hash, Ordering::SeqCst);
                    val.num_subdirs.store(dir_data.dirs.len(),
                        Ordering::SeqCst);
                    let mut table = val.recon_table();
                    let cut_off = Instant::now() -
                        Duration::from_millis(RETAIN_TIME);
                    table.retain(|_, x| *x >= cut_off);
                    table.insert(old_hash, Instant::now());
                    table.shrink_to_fit();
                }
            }
        }
    }
}

pub fn scan(path: &VPath, config: &Arc<Directory>, meta: &Meta, disk: &Disk)
    -> Box<Future<Item=BaseDirState, Error=Error>>
{
    let path = path.clone();
    let config = config.clone();
    Box::new(
        meta.scan_dir(&path).map_err(Error::Meta)
        .join(disk.read_keep_list(&config).map_err(Error::Disk))
        .map(move |(dirs, keep_list)| {
            let images = dirs.into_iter().map(|(name, state)| {
                (path.suffix().join(name), state)
            }).collect();
            let sorted = sort_out(&config, images, &keep_list);
            let dirs = sorted.used.into_iter()
                .map(|(x, state)| {
                    (x.file_name().expect("dir name is left intact")
                      .to_str().expect("valid dir name")
                      .to_string(),
                     state)
                })
                .collect();

            let kl: BTreeSet<String> = keep_list.into_iter()
                .filter_map(|p| {
                    p.strip_prefix(path.suffix()).ok()
                    .and_then(|name| name.to_str())
                    .map(|name| {
                        assert!(name.find('/').is_none());
                        name.to_string()
                    })
                }).collect();
            BaseDirState {
                path: path,
                config_hash: get_hash(&config),
                keep_list_hash: Hash::for_object(&kl),
                dirs: dirs,
            }
        }))
}
