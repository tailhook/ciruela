use std::collections::{BTreeMap, HashSet};
use std::ffi::{OsString, OsStr};
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf, Component, Components};
use std::sync::{Arc, Mutex, MutexGuard};
use std::cell::{RefCell, RefMut};

use abstract_ns::Name;
use dir_signature::v1::{Parser, Hashes, Header, Entry, ParseError, Emitter};
use dir_signature::{get_hash, HashType};

use {VPath};
use id::ImageId;
use failure_tracker::{SlowHostFailures};


#[derive(Debug, Clone)]
pub struct Location(Arc<Mutex<Pointer>>);

/// Error when trying to modify index
#[derive(Fail, Debug)]
pub enum IndexUpdateError {
    /// Bad file path (i.e. one that contains `/../` or root)
    #[fail(display="bad file path")]
    BadPath,
    /// Tried to insert a file into another file or a symlink
    #[fail(display="intermediate component is not a directory")]
    NotADirectory,
    /// Error when reading files for hashing
    #[fail(display="error reading file: {}", _0)]
    Read(io::Error),
}

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
    RemoteFile {
        exe: bool,
        size: u64,
        hashes: Hashes,
    },
    Link(PathBuf),
    LocalFile {
        exe: bool,
        size: u64,
        hashes: Hashes,
    },
}

/// Structure allows to lookup index easily and modify it
#[derive(Debug, Clone)]
pub struct MutableIndex {
    header: Header,
    root: BTreeMap<OsString, Item>,
    location: Location,
    block_size: u64,
    hash_type: HashType,
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
                    Item::RemoteFile { exe, size, hashes });
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
        return Ok(MutableIndex {
            hash_type: header.get_hash_type(),
            block_size: header.get_block_size(),
            header, root, location,
        });
    }
    /// Parse image hash from the raw data
    pub fn get_hash(&self) -> Result<ImageId, io::Error> {
        get_hash(&mut Cursor::new(&self.data)).map(ImageId::from)
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
            Some(&Item::RemoteFile { ref hashes, exe, size, .. }) => {
                Some((exe, size, hashes.clone()))
            }
            _ => None,
        }
    }
}

impl MaterializedIndex for MutableIndex {
}

impl MutableIndex {
    /// Insert or replace a file creating intermediate directories
    ///
    /// Note: we don't follow symlinks and return ``NotADirectory`` error if
    /// it occurs in the middle of the path. Last component may refer either to
    /// a file, symlink or a directory, any of them will be replaced
    /// by a specified file.
    pub fn insert_file<R, P>(&mut self, path: P, file: R, executable: bool)
        -> Result<(), IndexUpdateError>
        where R: Read, P: AsRef<Path>
    {
        use self::IndexUpdateError as E;

        let path = path.as_ref();
        if !path.is_absolute() {
            return Err(E::BadPath);
        }
        let (fname, parent) = match (path.file_name(), path.parent()) {
            (Some(f), Some(p)) => (f, p),
            _ => return Err(E::BadPath),
        };
        let (size, hashes) = Hashes::hash_file(
            self.hash_type, self.block_size, file,
        ).map_err(|e| E::Read(e))?;
        _insert_file(&mut self.root, fname, parent.components(),
            Item::LocalFile {
                exe: executable, size, hashes,
            })
    }
    /// Convert index back into raw data, so that it can be used for upload
    pub fn to_raw_data(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1024);
        {
            let mut emitter = Emitter::new(
                self.hash_type, self.block_size, &mut buf,
            ).unwrap();
            _emit_dir(&mut emitter, &Path::new("/"), &self.root).unwrap();
            emitter.finish().unwrap();
        }
        return buf;
    }
    /// Get hash type of the underlying index
    pub fn hash_type(&self) -> HashType {
        self.hash_type
    }
    /// Get block size of the underlying index
    pub fn block_size(&self) -> u64 {
        self.block_size
    }
}

fn _emit_dir(emitter: &mut Emitter, path: &Path,
    dir: &BTreeMap<OsString, Item>)
    -> io::Result<()>
{
    use self::Item::*;
    if dir.is_empty() {
        return Ok(());
    }
    emitter.start_dir(path)?;
    for (key, item) in dir {
        match *item {
            Dir(..) => {},
            | RemoteFile { exe, size, ref hashes }
            | LocalFile { exe, size, ref hashes }
            => {
                emitter.add_file(key, exe, size, hashes)?;
            }
            Link(ref dest) => {
                emitter.add_symlink(key, dest)?;
            }
        }
    }
    for (key, item) in dir {
        match *item {
            Dir(ref subdir) => {
                let path = path.join(key);
                _emit_dir(emitter, &path, subdir)?;
            },
            _ => {}
        }
    }
    Ok(())
}

fn _insert_file(dir: &mut BTreeMap<OsString, Item>,
    fname: &OsStr, mut components: Components, item: Item)
    -> Result<(), IndexUpdateError>
{
    use self::IndexUpdateError as E;

    if let Some(component) = components.next()  {
        match component {
            Component::RootDir => _insert_file(dir, fname, components, item),
            Component::Normal(name) => {
                let e = dir.entry(name.to_owned())
                    .or_insert_with(|| Item::Dir(BTreeMap::new()));
                match *e {
                    Item::Dir(ref mut next) => {
                        _insert_file(next, fname, components, item)
                    }
                    _ => return Err(E::NotADirectory),
                }
            },
            _ => return Err(E::BadPath),
        }
    } else {
        dir.insert(fname.to_owned(), item);
        Ok(())
    }
}


#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};
    use std::collections::HashSet;
    use failure_tracker::SlowHostFailures;
    use VPath;
    use super::{Location, Pointer, RawIndex};

    const EXAMPLE: &str = "\
DIRSIGNATURE.v1 sha512/256 block_size=32768
/
  hello.txt f 6 a79eef66019bfb9a41f798f2cff2d2d36ed294cc3f96bf53bbfc5192ebe60192
  test.txt f 0
/subdir
  .hidden f 7 6d7f5f9804ee4dbc1ff7e12c7665387e0119e8ea629996c52d38b75c12ad0acf
  file.txt f 10 0119865c765e02554f6fc5a06fa76aa92c590c09225775c092144079f9964899
552ca5730ee95727e890a2155c88609d244624034ff70de264cf88220d11d6df
";

    #[test]
    fn roundtrip() {
        let test = RawIndex {
            data: EXAMPLE.as_bytes().to_owned(),
            location: Location(Arc::new(Mutex::new(Pointer {
                vpath: VPath::from("/somewhere/path"),
                candidate_hosts: HashSet::new(),
                failures: SlowHostFailures::new_slow(),
            }))),
        };
        let idx = test.into_mut().unwrap();
        let data = idx.to_raw_data();
        assert_eq!(String::from_utf8(data).unwrap(), EXAMPLE);
    }

}
