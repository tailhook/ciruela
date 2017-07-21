use std::borrow;
use std::hash;
use std::path::{Path};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};


use ciruela::VPath;
use config::Directory;


#[derive(Debug)]
pub struct BaseDir {
    pub path: VPath,
    pub config: Arc<Directory>,
    num_subdirs: AtomicUsize,
    num_downloading: AtomicUsize,
}

impl BaseDir {
    pub fn new(path: VPath, config: &Arc<Directory>) -> BaseDir {
        BaseDir {
            path: path,
            config: config.clone(),
            num_subdirs: AtomicUsize::new(0),
            num_downloading: AtomicUsize::new(0),
        }
    }
    pub fn restore(path: VPath, config: &Arc<Directory>,
                   num_subdirs: usize, num_downloading: usize)
        -> BaseDir
    {
        BaseDir {
            path: path,
            config: config.clone(),
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
