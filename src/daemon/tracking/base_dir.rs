use std::collections::{BTreeMap, BTreeSet};
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use ciruela::database::signatures::State;
use ciruela::proto::BaseDirState;

use futures::Future;
use atomic::Atomic;
use ciruela::{VPath, Hash};
use config::Directory;
use peers::config::get_hash;
use tracking::Subsystem;


#[derive(Debug)]
pub struct BaseDir {
    pub path: VPath,
    pub config: Arc<Directory>,
    hash: Atomic<Hash>,
    last_scan: Atomic<Instant>,
    num_subdirs: AtomicUsize,
    num_downloading: AtomicUsize,
}

impl BaseDir {
    pub fn hash(&self) -> Hash {
        self.hash.load(Ordering::SeqCst)
    }
    pub fn last_scan(&self) -> Instant {
        self.last_scan.load(Ordering::SeqCst)
    }
    pub fn commit_scan(path: VPath, dirs: BTreeMap<String, State>,
        keep_list: BTreeSet<String>, scan_time: Instant, sys: &Subsystem)
    {
        let config = sys.config.dirs.get(path.key())
                        .expect("only scans configured basedirs");
        let mut state = &mut *sys.state();
        let ref mut lst = state.base_dir_list;
        let dir_data = BaseDirState {
            path: path,
            config_hash: get_hash(config),
            keep_list_hash: Hash::for_object(&keep_list),
            dirs: dirs,
        };
        let hash = Hash::for_object(&dir_data);
        let down = state.in_progress.iter()
            .filter(|x| x.virtual_path.parent() == dir_data.path)
            .count();
        match state.base_dirs.entry(dir_data.path.clone()) {
            Entry::Vacant(e) => {
                let new = Arc::new(BaseDir {
                    config: config.clone(),
                    path: dir_data.path,
                    hash: Atomic::new(hash),
                    last_scan: Atomic::new(scan_time),
                    num_subdirs: AtomicUsize::new(dir_data.dirs.len()),
                    num_downloading: AtomicUsize::new(down.into()),
                });
                lst.push(new.clone());
                e.insert(new);
                // TODO(tailhook) send to gossip
            }
            Entry::Occupied(e) => {
                let val = e.get();
                val.last_scan.store(scan_time, Ordering::SeqCst);
                if val.hash() != hash {
                    val.hash.store(hash, Ordering::SeqCst);
                    val.num_subdirs.store(dir_data.dirs.len(),
                        Ordering::SeqCst);
                }
            }
        }
    }
}
