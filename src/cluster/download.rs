use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf, Component};
use std::sync::{Arc, Mutex, MutexGuard};
use std::cell::{RefCell, RefMut};

use abstract_ns::Name;
use dir_signature::v1::{Parser, Hashes, Header, Entry, ParseError};

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
    pub(crate) location: Location,
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

/// Structure allows to lookup index easily and modify it
#[derive(Debug, Clone)]
pub struct MutableIndex {
    header: Header,
    root: BTreeMap<OsString, Item>,
    location: Location,
}

pub trait SealedIndex {
    fn get_location(&self) -> Location;
    fn get_file(&self, path: &Path) -> Option<(bool, u64, Hashes)>;
}

/// This is an index that can be queried by path
pub trait MaterializedIndex: SealedIndex {
}

/// Error parsing raw index into a materialized one
#[derive(Fail, Debug)]
#[fail(display="{}", _0)]
pub struct IndexParseError(IndexParseEnum);

#[derive(Fail, Debug)]
enum IndexParseEnum {
    /// Io error when parsing index
    #[fail(display="IO error: {}", _0)]
    Io(io::Error),
    /// Syntax of the index error or one of the hashsums
    #[fail(display="ParseError: {}", _0)]
    Parse(ParseError),
    /// Invalid path in index file
    #[fail(display="Invalid path in index: {:?}", _0)]
    InvalidPath(PathBuf),
    /// Path conflict in index file
    #[fail(display="The following path conflicts with others: {:?}", _0)]
    PathConflict(PathBuf),
    #[doc(hidden)]
    #[fail(display="unreachable")]
    __Nonexhaustive,
}

fn fill_dirs<R>(root: &RefCell<BTreeMap<OsString, Item>>,
    mut parser: Parser<R>)
    -> Result<(), IndexParseEnum>
    where R: io::BufRead
{
    let mut dir = root.borrow_mut();
    for entry in parser.iter() {
        let entry = entry.map_err(IndexParseEnum::Parse)?;
        match entry {
            Entry::Dir(path) => {
                drop(dir);
                dir = root.borrow_mut();
                for component in path.components() {
                    match component {
                        Component::RootDir => {}
                        Component::Normal(chunk) => {
                            let next = RefMut::map(dir, |dir| {
                                dir.entry(chunk.to_owned())
                                .or_insert_with(|| {
                                    Item::Dir(BTreeMap::new())
                                })
                            });
                            let is_okay = matches!(*next, Item::Dir(..));
                            if is_okay {
                                dir = RefMut::map(next, |item| {
                                    match *item {
                                        Item::Dir(ref mut dir) => dir,
                                        _ => unreachable!(),
                                    }
                                });
                            } else {
                                return Err(IndexParseEnum::PathConflict(
                                    path.to_path_buf()));
                            }
                        }
                        _ => return Err(IndexParseEnum::InvalidPath(
                            path.to_path_buf())),
                    }
                }
            }
            Entry::File { path, exe, size, hashes } => {
                dir.insert(path.file_name()
                    .ok_or_else(|| {
                        IndexParseEnum::InvalidPath(path.to_path_buf())
                    })?
                    .to_owned(),
                    Item::File { exe, size, hashes });
            }
            Entry::Link(path, dest) => {
                dir.insert(path.file_name()
                    .ok_or_else(|| {
                        IndexParseEnum::InvalidPath(path.to_path_buf())
                    })?
                    .to_owned(),
                    Item::Link(dest));
            }
        }
    }
    Ok(())
}

impl RawIndex {
    /// Convert index into a mutable index
    pub fn into_mut(self) -> Result<MutableIndex, IndexParseError> {
        self._into_mut()
        .map_err(IndexParseError)
    }
    fn _into_mut(self) -> Result<MutableIndex, IndexParseEnum> {
        let RawIndex {data, location} = self;
        let parser = Parser::new(Cursor::new(&data))
            .map_err(IndexParseEnum::Parse)?;
        let header = parser.get_header();
        let root = RefCell::new(BTreeMap::new());
        fill_dirs(&root, parser)?;
        let root = root.into_inner();
        return Ok(MutableIndex { header, root, location });
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


impl SealedIndex for MutableIndex {
    fn get_location(&self) -> Location {
        self.location.clone()
    }
    fn get_file(&self, path: &Path) -> Option<(bool, u64, Hashes)> {
        let mut cur = &self.root;
        for component in path.parent()?.components() {
            cur = match component {
                Component::RootDir => cur,
                Component::Normal(item) => match *cur.get(item)? {
                    Item::Dir(ref next) => next,
                    _ => return None,
                },
                _ => return None,
            }
        }
        match cur.get(path.file_name()?) {
            Some(&Item::File { ref hashes, exe, size, .. }) => {
                Some((exe, size, hashes.clone()))
            }
            _ => None,
        }
    }
}

impl MaterializedIndex for MutableIndex {
}
