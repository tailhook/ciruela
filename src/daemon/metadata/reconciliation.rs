use std::time::{Instant, SystemTime};
use std::path::PathBuf;
use std::collections::{BTreeMap, HashMap};

use ciruela::database::signatures::State;


pub type Hash = [u8; 64];
pub type Name = String;

/// Holds current hash, and cache of hashes that were before this one
///
/// There are two cases when hash is added:
///
/// 1. When we update the hash (i.e. add or remove directory)
/// 2. When hash is received from peer we have checked the list images
///    and it turned out that is is outdated (i.e. it was a subset of
///    images we have here, or all new images match cleanup rule)
///
/// Also we flush the cache and recheck it again if `keep-list-file` changes,
/// because that would mean some images might be useful again.
///
pub struct Recon {
    hash: Hash,
    cached_parents: HashMap<Hash, Instant>,
}

/// This structure is sent by peer if we ask for reconciliation of the dir
pub struct BaseDir {
    id: String,
    sub_path: PathBuf,
    config_hash: Hash,
    keep_list_hash: Hash,
    images: BTreeMap<Name, State>,
}

pub struct ReconData {
    pub id: String,
    pub sub_path: PathBuf,
    pub config_hash: Hash,
    pub keep_list: Vec<String>,
    pub images: BTreeMap<Name, State>,
}
