use std::borrow;
use std::collections::{VecDeque, HashSet};
use std::hash;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc};

use rand::{thread_rng, Rng};

use {ImageId, VPath};
use blocks::BlockHash;
use config::Directory;
use failure_tracker::HostFailures;
use mask::{AtomicMask, Mask};
use named_mutex::{Mutex, MutexGuard};
use tracking::fetch_index::Index;
use tracking::{BlockData};


pub const MAX_SLICES: usize = 15;


#[derive(Debug, Clone)]
pub struct Block {
    pub hash: BlockHash,
    pub path: Arc<PathBuf>,
    pub offset: u64,
}

#[derive(Debug)]
pub struct Slice {
    pub index: u8,
    pub in_progress: usize,
    pub blocks: VecDeque<Block>,
    pub failures: HostFailures,
}

#[derive(Debug)]
pub struct Slices {
    pub slices: Mutex<VecDeque<Slice>>,
}

#[derive(Debug)]
pub struct Downloading {
    pub virtual_path: VPath,
    pub replacing: bool,
    pub image_id: ImageId,
    pub config: Arc<Directory>,
    pub slices: Slices,
    pub mask: AtomicMask,
    pub index_fetched: AtomicBool,
    pub bytes_total: AtomicUsize,
    pub bytes_fetched: AtomicUsize,
    pub blocks_total: AtomicUsize,
    pub blocks_fetched: AtomicUsize,
    pub stalled: AtomicBool,
}

impl Slice {
    fn new(index: u8) -> Slice {
        Slice {
            index: index,
            blocks: VecDeque::new(),
            failures: HostFailures::new_default(),
            in_progress: 0,
        }
    }
    fn add_blocks<I: Iterator<Item=Block>>(&mut self, blocks: I) {
        // TODO(tailhook) optimize the clone
        self.blocks.extend(blocks);
    }
}


impl Slices {
    pub fn new() -> Slices {
        Slices {
            slices: Mutex::new(VecDeque::new(), "downloading_slices"),
        }
    }
    fn slices(&self) -> MutexGuard<VecDeque<Slice>> {
        self.slices.lock()
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
    pub fn is_stalled(&self) -> bool {
        self.stalled.load(Relaxed)
    }
    pub fn notify_stalled(&self) {
        self.stalled.store(true, Relaxed);
    }
    pub fn notify_unstalled(&self) {
        self.stalled.store(false, Relaxed);
    }
    pub fn index_fetched(&self, index: &Index) {
        self.notify_unstalled();
        self.bytes_total.store(index.bytes_total as usize, Relaxed);
        self.blocks_total.store(index.blocks_total as usize, Relaxed);
        self.index_fetched.store(true, Relaxed);
    }
    pub fn fill_blocks(&self, index: &Index, hardlinks: HashSet<PathBuf>) {
        let blocks = index.entries.iter()
            .flat_map(|entry| {
                use dir_signature::v1::Entry::*;
                match *entry {
                    Dir(..) => Vec::new(),
                    Link(..) => Vec::new(),
                    File { ref hashes, ref path, .. } => {
                        if hardlinks.contains(path) {
                            return Vec::new();
                        }
                        let arc = Arc::new(path.clone());
                        let mut result = Vec::new();
                        for (i, hash) in hashes.iter().enumerate() {
                            let hash = BlockHash::from_bytes(hash)
                                .expect("valid hash type");
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

        let mut slices = Vec::new();
        let mut mask = Mask::full();
        for (idx, chunk) in blocks.chunks(100).enumerate() {
            let s = idx % MAX_SLICES;
            while slices.len() <= s {
                let cur = slices.len();
                slices.push(Slice::new(cur as u8));
            }
            mask.slice_unfetched(s);
            slices[s].add_blocks(chunk.iter().cloned());
        }
        thread_rng().shuffle(&mut slices);
        self.slices().extend(slices);
        self.mask.set(mask);
    }
    pub fn report_block(&self, block: &BlockData) {
        self.notify_unstalled();
        self.bytes_fetched.fetch_add(block.len(), Relaxed);
        self.blocks_fetched.fetch_add(1, Relaxed);
    }
    pub fn report_slice(&self, slice: usize) {
        self.mask.set_fetched(slice)
    }
    pub fn slices(&self) -> MutexGuard<VecDeque<Slice>> {
        self.slices.slices()
    }
}
