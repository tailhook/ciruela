use std::borrow;
use std::collections::{VecDeque, HashSet};
use std::hash;
use std::path::{Path, PathBuf};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicUsize};

use ciruela::Hash;
use ciruela::{ImageId, VPath};
use config::Directory;
use futures::sync::oneshot::Sender;
use tracking::BlockData;
use tracking::fetch_index::Index;


#[derive(Debug, Clone)]
pub struct Block {
    pub hash: Hash,
    pub path: Arc<PathBuf>,
    pub offset: u64,
}

#[derive(Debug)]
pub struct Slice {
    pub blocks: VecDeque<Block>,
}

#[derive(Debug)]
pub struct Slices {
    pub start: u8,
    pub slices: Mutex<Option<[Slice; 16]>>,
}

#[derive(Debug)]
pub struct Downloading {
    pub virtual_path: VPath,
    pub replacing: bool,
    pub image_id: ImageId,
    pub config: Arc<Directory>,
    pub slices: Slices,
    pub index_fetched: AtomicBool,
    pub bytes_total: AtomicUsize,
    pub bytes_fetched: AtomicUsize,
    pub blocks_total: AtomicUsize,
    pub blocks_fetched: AtomicUsize,
}

impl Slice {
    fn new() -> Slice {
        Slice {
            blocks: VecDeque::new(),
        }
    }
    fn add_blocks<I: Iterator<Item=Block>>(&mut self, blocks: I) {
        // TODO(tailhook) optimize the clone
        self.blocks.extend(blocks);
    }
    fn add_block(&mut self, b: Block) {
        self.blocks.push_back(b);
    }
}


impl Slices {
    pub fn new() -> Slices {
        Slices {
            start: 0,
            slices: Mutex::new(None),
        }
    }
    pub fn start(&self, index: &Index) {
        let blocks = index.entries.iter()
            .flat_map(|entry| {
                use dir_signature::v1::Entry::*;
                match *entry {
                    Dir(..) => Vec::new(),
                    Link(..) => Vec::new(),
                    File { ref hashes, ref path, .. } => {
                        let arc = Arc::new(path.clone());
                        let mut result = Vec::new();
                        for (i, hash) in hashes.iter().enumerate() {
                            let hash = Hash::new(hash);
                            result.push(Block {
                                hash: hash,
                                path: arc.clone(),
                                offset: (i as u64)*index.block_size,
                            });
                        }
                        result
                    }
                }
            }).collect::<Vec<_>>();
        let mut slices = [
            Slice::new(), Slice::new(), Slice::new(), Slice::new(),
            Slice::new(), Slice::new(), Slice::new(), Slice::new(),
            Slice::new(), Slice::new(), Slice::new(), Slice::new(),
            Slice::new(), Slice::new(), Slice::new(), Slice::new(),
        ];
        let num = slices.len();
        for (idx, chunk) in blocks.windows(100).enumerate() {
            slices[idx % num].add_blocks(chunk.iter().cloned());
        }
        *self.slices.lock().expect("slices not poisoned") = Some(slices);
    }
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
    pub fn report_block(&self, block: &BlockData) {
        self.bytes_fetched.fetch_add(block.len(), Relaxed);
        self.blocks_fetched.fetch_add(1, Relaxed);
    }
}

#[cfg(test)]
mod test {
    use std::mem::size_of;
    use super::*;

    #[test]
    #[cfg(target_arch="x86_64")]
    fn size() {
        assert_eq!(size_of::<Downloading>(), 2056);
    }
}
