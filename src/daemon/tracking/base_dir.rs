use std::borrow;
use std::hash;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize};


use ciruela::VPath;


#[derive(Debug)]
pub struct BaseDir {
    pub virtual_path: VPath,
    pub num_subdirs: AtomicUsize,
    pub num_downloading: AtomicUsize,
}

impl borrow::Borrow<Path> for BaseDir {
    fn borrow(&self) -> &Path {
        self.virtual_path.borrow()
    }
}

impl borrow::Borrow<VPath> for BaseDir {
    fn borrow(&self) -> &VPath {
        &self.virtual_path
    }
}

impl hash::Hash for BaseDir {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.virtual_path.hash(state)
    }
}

impl PartialEq for BaseDir {
    fn eq(&self, other: &BaseDir) -> bool {
        self.virtual_path.eq(&other.virtual_path)
    }
}

impl Eq for BaseDir {}
