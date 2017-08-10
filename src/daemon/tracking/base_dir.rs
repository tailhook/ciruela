use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};


use atomic::Atomic;
use ciruela::{VPath, Hash};
use config::Directory;

#[derive(Debug)]
pub struct BaseDir {
    pub path: VPath,
    pub config: Arc<Directory>,
    hash: Atomic<Hash>,
    num_subdirs: AtomicUsize,
    num_downloading: AtomicUsize,
}

impl BaseDir {
    pub fn new(path: VPath, config: &Arc<Directory>) -> BaseDir {
        BaseDir {
            path: path,
            config: config.clone(),
            hash: Atomic::new(Hash::new(&[0u8; 32])),
            num_subdirs: AtomicUsize::new(0),
            num_downloading: AtomicUsize::new(0),
        }
    }
    pub fn hash(&self) -> Hash {
        self.hash.load(Ordering::SeqCst)
    }
    pub fn restore(path: VPath, config: &Arc<Directory>, hash: Hash,
                   num_subdirs: usize, num_downloading: usize)
        -> BaseDir
    {
        BaseDir {
            path: path,
            config: config.clone(),
            // TODO(tailhook)
            hash: Atomic::new(hash),
            num_subdirs: AtomicUsize::new(num_subdirs),
            num_downloading: AtomicUsize::new(num_downloading),
        }
    }
    // Assumes that new dir is downloading immediately
    pub fn new_subdir(&self) {
        self.num_subdirs.fetch_add(1, Ordering::Relaxed);
        self.num_downloading.fetch_add(1, Ordering::Relaxed);
    }
}
