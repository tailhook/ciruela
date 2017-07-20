use std::borrow;
use std::hash;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicUsize};

use index::Index;
use ciruela::{ImageId, VPath};
use config::Directory;
use tracking::Block;

pub struct Downloading {
    pub virtual_path: VPath,
    pub image_id: ImageId,
    pub config: Arc<Directory>,
    pub index_fetched: AtomicBool,
    pub bytes_total: AtomicUsize,
    pub bytes_fetched: AtomicUsize,
    pub blocks_total: AtomicUsize,
    pub blocks_fetched: AtomicUsize,
}

impl borrow::Borrow<Path> for Downloading {
    fn borrow(&self) -> &Path {
        self.virtual_path.borrow()
    }
}
impl borrow::Borrow<VPath> for Downloading {
    fn borrow(&self) -> &VPath {
        &self.virtual_path
    }
}

impl hash::Hash for Downloading {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.virtual_path.hash(state)
    }
}

impl PartialEq for Downloading {
    fn eq(&self, other: &Downloading) -> bool {
        self.virtual_path.eq(&other.virtual_path)
    }
}

impl Eq for Downloading {}

impl Downloading {
    pub fn index_fetched(&self, index: &Index) {
        self.bytes_total.store(index.bytes_total as usize, Relaxed);
        self.blocks_total.store(index.blocks_total as usize, Relaxed);
        self.index_fetched.store(true, Relaxed);
    }
    pub fn report_block(&self, block: &Block) {
        self.bytes_fetched.fetch_add(block.len(), Relaxed);
        self.blocks_fetched.fetch_add(1, Relaxed);
    }
}
