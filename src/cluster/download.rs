use std::path::PathBuf;
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::sync::{Arc, Mutex};

use abstract_ns::Name;
use dir_signature::v1::Hashes;

use {VPath};
use failure_tracker::{SlowHostFailures};


#[derive(Debug, Clone)]
pub struct Location(Arc<Mutex<Pointer>>);


#[derive(Debug)]
pub(crate) struct Pointer {
    vpath: VPath,
    candidate_hosts: HashSet<Name>,
    failures: SlowHostFailures,
}

/// Raw index returned by cluster protocol
#[derive(Debug, Clone)]
pub struct RawIndex {
    data: Vec<u8>,
    loc: Location,
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
