use std::path::PathBuf;
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::sync::{Arc, Mutex, MutexGuard};

use abstract_ns::Name;
use dir_signature::v1::Hashes;

use {VPath};
use failure_tracker::{SlowHostFailures};


#[derive(Debug, Clone)]
pub struct Location(Arc<Mutex<Pointer>>);


#[derive(Debug)]
pub(crate) struct Pointer {
    pub(crate) vpath: VPath,
    pub(crate) candidate_hosts: HashSet<Name>,
    pub(crate) failures: SlowHostFailures,
}


/// Raw index returned by cluster protocol
#[derive(Debug, Clone)]
pub struct RawIndex {
    pub(crate) data: Vec<u8>,
    pub(crate) loc: Location,
}

#[derive(Debug, Clone)]
enum Item {
    Dir(BTreeMap<OsString, Item>),
    File {
        exe: bool,
        size: u64,
        hashes: Hashes,
    },
    Link(PathBuf),
}

#[derive(Debug, Clone)]
pub struct MutableIndex {
    root: BTreeMap<OsString, Item>,
    loc: Location,
}


impl From<RawIndex> for MutableIndex {
    fn from(value: RawIndex) -> MutableIndex {
        unimplemented!();
    }
}

impl From<Pointer> for Location {
    fn from(ptr: Pointer) -> Location {
        Location(Arc::new(Mutex::new(ptr)))
    }
}

impl Location {
    pub(crate) fn lock(&self) -> MutexGuard<Pointer> {
        self.0.lock().expect("pointer is not poisoned")
    }
}
