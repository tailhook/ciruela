use std::io;
use std::sync::Arc;
use std::path::{Path, PathBuf, Component};
use std::ffi::OsStr;

use openat;
use metadata::Error;
use dir_util::recover_path;


#[derive(Clone)]
pub struct Dir(Arc<openat::Dir>);

impl Dir {
    pub fn open_root(path: &Path) -> Result<Dir, Error> {
        match openat::Dir::open(path) {
            Ok(x) => Ok(Dir(Arc::new(x))),
            Err(e) => {
                Err(Error::OpenRoot(path.to_path_buf(), e))
            }
        }
    }
    fn path(&self, name: &OsStr) -> PathBuf {
        recover_path(&self.0, name)
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
                    .map_err(|e| Error::OpenMeta(self.path(name), e))
                    .map(|x| Dir(Arc::new(x)))
                }
                Err(e) => Err(Error::CreateDir(self.path(name), e)),
            },
            Err(e) => Err(Error::OpenMeta(self.path(name), e)),
        }
    }
    pub fn ensure_dir(&self, path: &Path) -> Result<Dir, Error> {
        let mut dir = self.clone();
        for cmp in path.components() {
            match cmp {
                Component::Normal(x) => {
                    self.create_component(x)
                }
                _ => {
                    return Err(Error::InvalidPath);
                }
            };
        }
        Ok(dir)
    }
}
