use std::io;
use std::fs::File;
use std::path::{Path, PathBuf};

use openat::Dir;

use metadata::error::Error;


pub trait DirExt: Sized {
    fn create_meta_dir<P: AsRef<Path>>(&self, p: P) -> Result<(), Error>;
    fn meta_sub_dir<P: AsRef<Path>>(&self, p: P) -> Result<Self, Error>;
    fn create_meta_file<P: AsRef<Path>>(&self, p: P) -> Result<File, Error>;
    fn read_meta_file<P: AsRef<Path>>(&self, p: P) -> Result<File, Error>;
    fn rename_meta<A, B>(&self, a: A, b: B) -> Result<(), Error>
        where A: AsRef<Path>, B: AsRef<Path>;
    fn open_meta_dir<P: AsRef<Path>>(&self, path: P) -> Result<Dir, Error>;
}

impl DirExt for Dir {
    fn create_meta_dir<P: AsRef<Path>>(&self, p: P) -> Result<(), Error> {
        // TODO(tailhook) Fix race condition when directory is being created
        //                by another thread
        let path = p.as_ref();
        Dir::create_dir(self, path, 0o755)
        .map_err(|e| Error::WriteMeta(path.to_path_buf(), e))
    }
    fn meta_sub_dir<P: AsRef<Path>>(&self, p: P) -> Result<Dir, Error> {
        let path = p.as_ref();
        Dir::sub_dir(self, path)
        .map_err(|e| Error::OpenMeta(path.to_path_buf(), e))
    }
    fn create_meta_file<P: AsRef<Path>>(&self, p: P) -> Result<File, Error> {
        let path = p.as_ref();
        Dir::create_file(self, path, 0o644)
        .map_err(|e| Error::WriteMeta(path.to_path_buf(), e))
    }
    fn read_meta_file<P: AsRef<Path>>(&self, p: P) -> Result<File, Error> {
        let path = p.as_ref();
        Dir::open_file(self, path)
        .map_err(|e| Error::ReadMeta(path.to_path_buf(), e))
    }
    fn rename_meta<A, B>(&self, a: A, b: B) -> Result<(), Error>
        where A: AsRef<Path>, B: AsRef<Path>
    {
        let dest = b.as_ref();
        Dir::local_rename(self, a.as_ref(), dest)
        .map_err(|e| Error::WriteMeta(dest.to_path_buf(), e))
    }
    fn open_meta_dir<P: AsRef<Path>>(&self, path: P) -> Result<Dir, Error> {
        match self.sub_dir(path.as_ref()) {
            Ok(dir) => Ok(dir),
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    self.create_meta_dir(&path)?;
                    self.meta_sub_dir(&path)
                } else {
                    Err(Error::OpenMeta(recover(self, path), e))
                }
            }
        }
    }
}

pub fn recover<P: AsRef<Path>>(dir: &Dir, path: P) -> PathBuf {
    let mut result = dir.recover_path()
        .unwrap_or_else(|e| {
            warn!("Error recovering path {:?}: {}", dir, e);
            PathBuf::from("<unknown>")
        });
    result.push(path.as_ref());
    result
}
