use std::fmt;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf, Component};
use std::sync::Arc;

use virtual_path::VPath;
use dir_util::recover_path;
use metadata::Error;
use openat::{self, Entry, SimpleType, Metadata};


#[derive(Clone)]
pub struct Dir(Arc<openat::Dir>);


fn check_component<'x>(cmp: Component<'x>) -> Result<&'x OsStr, Error> {
    match cmp {
        Component::Normal(x) => Ok(x),
        _ => Err(Error::InvalidPath),
    }
}

impl Dir {
    pub fn open_root(path: &Path) -> Result<Dir, Error> {
        match openat::Dir::open(path) {
            Ok(x) => Ok(Dir(Arc::new(x))),
            Err(e) => {
                Err(Error::OpenRoot(path.to_path_buf(), e))
            }
        }
    }
    pub fn path<N: AsRef<Path>>(&self, name: N) -> PathBuf {
        recover_path(&self.0, name)
    }
    fn epath(&self, e: &Entry) -> PathBuf {
        recover_path(&self.0, e.file_name())
    }
    fn create_component(&self, name: &OsStr) -> Result<Dir, Error> {
        match self.0.sub_dir(name) {
            Ok(d) => Ok(Dir(Arc::new(d))),
            Err(ref e) if e.kind() == io::ErrorKind::NotFound
            => match self.0.create_dir(name, 0o755) {
                Ok(()) => match self.0.sub_dir(name) {
                    Ok(dir) => Ok(Dir(Arc::new(dir))),
                    Err(e) => Err(Error::CreateDirRace(self.path(name), e)),
                },
                Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists
                => {
                    self.0.sub_dir(name)
                    .map_err(|e| Error::Open(self.path(name), e))
                    .map(|x| Dir(Arc::new(x)))
                }
                Err(e) => Err(Error::CreateDir(self.path(name), e)),
            },
            Err(e) => Err(Error::Open(self.path(name), e)),
        }
    }
    pub fn ensure_dir<P: AsRef<Path>>(&self, path: P) -> Result<Dir, Error> {
        self._ensure_dir(path.as_ref())
    }
    fn _ensure_dir(&self, path: &Path) -> Result<Dir, Error> {
        let mut dir = self.clone();
        for cmp in path.components() {
            dir = dir.create_component(check_component(cmp)?)?;
        }
        Ok(dir)
    }
    pub fn open_vpath(&self, path: &VPath) -> Result<Dir, Error> {
        let mut dir = self.0.sub_dir(path.key())
            .map_err(|e| Error::Open(self.path(path.key()), e))?;
        for cmp in path.names() {
            dir = dir.sub_dir(cmp)
                .map_err(|e| Error::Open(recover_path(&dir, cmp), e))?;
        }
        Ok(Dir(Arc::new(dir)))
    }
    pub fn open_path(&self, path: &Path) -> Result<Dir, Error> {
        let mut piter = path.components();
        let mut dir = if let Some(c) = piter.next() {
            let val = check_component(c)?;
            self.0.sub_dir(val)
                .map_err(|e| Error::Open(self.path(val), e))?
        } else {
            return Ok(self.clone())
        };
        for cmp in piter {
            let val = check_component(cmp)?;
            dir = dir.sub_dir(val)
                .map_err(|e| Error::Open(recover_path(&dir, val), e))?;
        }
        Ok(Dir(Arc::new(dir)))
    }
    pub fn dir_if_exists(&self, name: &str)
        -> Result<Option<Dir>, Error>
    {
        assert!(name != "." && name != ".." && name.find("/").is_none());
        match self.0.sub_dir(name) {
            Ok(dir) => Ok(Some(Dir(Arc::new(dir)))),
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(e) => Err(Error::Open(self.path(name), e)),
        }
    }
    pub fn file_meta(&self, name: &str) -> Result<Option<Metadata>, Error> {
        match self.0.metadata(name) {
            Ok(m) => Ok(Some(m)),
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Read(self.path(name), e)),
        }
    }
    pub fn remove_file(&self, name: &str) -> Result<(), Error> {
        match self.0.remove_file(name) {
            Ok(()) => Ok(()),
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::Remove(self.path(name), e)),
        }
    }
    pub fn replace_file<E, F>(&self, name: &str, f: F) -> Result<(), Error>
        where F: FnOnce(File) -> Result<(), E>,
              E: ::std::error::Error + Send + 'static,
    {
        let tmpname = format!(".tmp.{}", name);
        let file = match self.0.write_file(&tmpname, 0o644) {
            Ok(file) => file,
            Err(e) => {
                return Err(Error::WriteMeta(self.path(name), e));
            }
        };
        f(file)
            .map_err(|e| Error::Encode(
                self.path(name),
                Box::new(e) as Box<::std::error::Error + Send>))?;
        // Note: we rely on in-process metadata locking for files so don't
        // check when replacing
        self.0.local_rename(&tmpname, name)
            .map_err(|e| Error::WriteMeta(self.path(name), e))?;
        Ok(())
    }
    pub fn rename(&self, from: &str, to: &str) -> Result<(), Error> {
        self.0.local_rename(from, to)
            .map_err(|e| Error::Rename(self.path(from), self.path(to), e))?;
        Ok(())
    }
    pub fn rename_broken_file<E: fmt::Display>(&self, name: &str, reason: E) {
        let bu_name = format!(".{}.backup", name);
        error!("{}. Renaming file to {:?}", reason, bu_name);
        self.0.local_rename(name, &bu_name)
            .map_err(|e| {
                error!("Can't rename broken file {:?} -> {:?}: {}",
                    self.path(name), self.path(bu_name), e);
            }).ok();
    }
    pub fn open_file(&self, name: &str) -> Result<Option<File>, Error> {
        match self.0.open_file(name) {
            Ok(f) => Ok(Some(f)),
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(e) => Err(Error::Read(self.path(name), e)),
        }
    }
    pub fn read_file<R, E, F>(&self, name: &str, f: F)
        -> Result<Option<R>, Error>
        where F: FnOnce(File) -> Result<R, E>,
              E: ::std::error::Error + Send + 'static,
    {
        let file = match self.0.open_file(name) {
            Ok(f) => f,
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(e) => return Err(Error::Read(self.path(name), e)),
        };
        f(file)
            .map_err(|e| Error::Decode(
                self.path(name),
                Box::new(e) as Box<::std::error::Error + Send>))
            .map(Some)
    }
    pub fn list_dirs(&self) -> Result<Vec<String>, Error> {
        let err = &|e| {
            Error::ListDir(self.path(""), e)
        };
        let mut result = Vec::new();
        for entry in self.0.list_dir(".").map_err(err)? {
            let entry = entry.map_err(err)?;
            let filename = entry.file_name();
            let filename = match filename.to_str() {
                Some(v) => v.to_string(),
                None => {
                    warn!("invalid filename {:?}", filename);
                    continue;
                }
            };
            match entry.simple_type() {
                Some(SimpleType::Dir) => result.push(filename),
                Some(_) => {
                    warn!("Path {:?} is not a dir", self.epath(&entry));
                }
                None => {
                    let meta = self.0.metadata(&entry).map_err(err)?;
                    if meta.simple_type() == SimpleType::Dir {
                        result.push(filename);
                    } else {
                        warn!("Path {:?} is not a dir", self.epath(&entry));
                    }
                }
            }
        }
        Ok(result)
    }
    pub fn list_files(&self, suffix: &str) -> Result<Vec<String>, Error> {
        let err = &|e| {
            Error::ListDir(self.path(""), e)
        };
        let mut result = Vec::new();
        for entry in self.0.list_dir(".").map_err(err)? {
            let entry = entry.map_err(err)?;
            let filename = entry.file_name();
            let filename = match filename.to_str() {
                Some(v) => v.to_string(),
                None => {
                    warn!("invalid filename {:?}", filename);
                    continue;
                }
            };
            if filename.starts_with(".") {
                continue;
            }
            if !filename.ends_with(suffix) {
                continue;
            }
            match entry.simple_type() {
                Some(SimpleType::File) => result.push(filename),
                Some(_) => {
                    warn!("Path {:?} is not a file", self.epath(&entry));
                }
                None => {
                    let meta = self.0.metadata(&entry).map_err(err)?;
                    if meta.simple_type() == SimpleType::File {
                        result.push(filename);
                    } else {
                        warn!("Path {:?} is not a file", self.epath(&entry));
                    }
                }
            }
        }
        Ok(result)
    }
}
