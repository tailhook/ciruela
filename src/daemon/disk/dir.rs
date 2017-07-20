use std::io;
use std::ops;
use std::path::{Path, PathBuf};

use openat::{Dir, SimpleType, AsPath, Entry};

use ciruela::VPath;
use disk::error::Error;

pub enum DirBorrow<'a> {
    Owned(Dir),
    Borrow(&'a Dir),
}

pub fn ensure_subdir<P: AsRef<Path>>(dir: &Dir, name: P)
    -> Result<Dir, Error>
{
    match dir.sub_dir(name.as_ref()) {
        Ok(dir) => Ok(dir),
        Err(ref e) if e.kind() == io::ErrorKind::NotFound
        => match dir.create_dir(name.as_ref(), 0o755) {
            Ok(()) => match dir.sub_dir(name.as_ref()) {
                Ok(dir) => Ok(dir),
                Err(e) => Err(Error::CreateDirRace(recover_path(dir, name), e)),
            },
            Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists
            => {
                dir.sub_dir(name.as_ref())
                .map_err(|e| Error::OpenDir(recover_path(dir, name), e))
            }
            Err(e) => Err(Error::CreateDir(recover_path(dir, name), e)),
        },
        Err(e) => Err(Error::OpenDir(recover_path(dir, name), e)),
    }
}

impl<'a> ops::Deref for DirBorrow<'a> {
    type Target = Dir;
    fn deref(&self) -> &Dir {
        match *self {
            DirBorrow::Owned(ref d) => d,
            DirBorrow::Borrow(d) => d,
        }
    }
}

pub fn recover_path<P: AsRef<Path>>(dir: &Dir, path: P) -> PathBuf {
    let mut result = dir.recover_path()
        .unwrap_or_else(|e| {
            warn!("Error recovering path {:?}: {}", dir, e);
            PathBuf::from("<unknown>")
        });
    result.push(path.as_ref());
    result
}

pub fn ensure_path<P: AsRef<Path>>(dir: &Dir, path: P)
    -> Result<DirBorrow, Error>
{
    let path = path.as_ref();
    let path = if path.is_absolute() {
        path.strip_prefix("/").unwrap()
    } else {
        path
    };
    let mut dir = DirBorrow::Borrow(dir);
    for component in path.iter() {
        // TODO(tailhook) check component validity
        dir = DirBorrow::Owned(ensure_subdir(&*dir, component)?);
    }
    Ok(dir)
}

pub fn open_path<P: AsRef<Path>>(dir: &Dir, path: P)
    -> Result<DirBorrow, Error>
{
    let path = path.as_ref();
    let path = if path.is_absolute() {
        path.strip_prefix("/").unwrap()
    } else {
        path
    };
    let mut dir = DirBorrow::Borrow(dir);
    for component in path.iter() {
        // TODO(tailhook) check component validity
        dir = DirBorrow::Owned(dir.sub_dir(component)
            .map_err(|e| Error::OpenDir(recover_path(&*dir, component), e))?);
    }
    Ok(dir)
}

pub fn ensure_virtual_parent<'x>(dir: &'x Dir, path: &VPath)
    -> Result<DirBorrow<'x>, Error>
{
    let mut dir = DirBorrow::Borrow(dir);
    for component in path.names().take(path.level()-1) {
        dir = DirBorrow::Owned(ensure_subdir(&*dir, component)?);
    }
    Ok(dir)
}

pub fn remove_entry(dir: &Dir, inp: &Entry)
    -> Result<(), Error>
{
    let me_err = &|e| {
        Error::Delete(recover_path(dir, inp.file_name()), e)
    };
    let me = dir.sub_dir(inp).map_err(me_err)?;
    for entry in me.list_dir(".").map_err(me_err)? {
        let entry = entry.map_err(me_err)?;
        // TODO(tailhook) fix absent type in entry
        match entry.simple_type().expect("fs is good") {
            SimpleType::Dir => {
                remove_entry(&me, &entry)?;
            }
            _ => {
                dir.remove_file(&entry).map_err(|e| {
                    Error::Delete(recover_path(&me, entry.file_name()), e)
                })?;
            }
        }
    }
    dir.remove_dir(inp).map_err(me_err);
    Ok(())
}

pub fn remove_dir_recursive(dir: &Dir, name: &str)
    -> Result<(), Error>
{
    let me_err = &|e| {
        Error::Delete(recover_path(dir, name), e)
    };
    let me = dir.sub_dir(name).map_err(me_err)?;
    for entry in me.list_dir(".").map_err(me_err)? {
        let entry = entry.map_err(me_err)?;
        // TODO(tailhook) fix absent type in entry
        match entry.simple_type().expect("fs is good") {
            SimpleType::Dir => {
                remove_entry(&me, &entry)?;
            }
            _ => {
                dir.remove_file(&entry).map_err(|e| {
                    Error::Delete(recover_path(&me, entry.file_name()), e)
                })?;
            }
        }
    }
    dir.remove_dir(name).map_err(me_err);
    Ok(())
}
