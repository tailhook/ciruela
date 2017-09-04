use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write, BufReader, BufRead, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_cpupool::{CpuPool, CpuFuture};
use openat::{Dir, SimpleType};
use regex::Regex;

use ciruela::VPath;
use config::{Config, Directory};
use disk::commit::commit_image;
use disk::dir::{ensure_virtual_parent, ensure_path, open_path};
use disk::dir::{ensure_subdir, recover_path, DirBorrow};
use disk::dir::{remove_dir_recursive};
use disk::{Init, Error};
use tracking::Index;
use metadata::Meta;
use tracking::BlockData;


#[derive(Clone)]
pub struct Disk {
    pool: CpuPool,
    config: Arc<Config>,
}

pub struct Image {
    pub virtual_path: VPath,
    pub parent: Dir,
    pub temporary_name: String,
    pub temporary: Dir,
    pub index: Index,
    pub can_replace: bool,
}

fn check_exists(dir: &Dir, name: &str) -> Result<(), Error> {
    match dir.metadata(name) {
        Ok(ref m) if m.simple_type() == SimpleType::Dir => {
            // TODO(tailhook) verify image is okay
            Err(Error::AlreadyExists)
        }
        Ok(_) => Err(Error::ExistsNotADir(recover_path(&dir, name))),
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::OpenDir(recover_path(&dir, name), e)),
    }
}

impl Disk {
    pub fn new(num_threads: usize, config: &Arc<Config>)
        -> Result<(Disk, Init), Error>
    {
        Ok((Disk {
            pool: CpuPool::new(num_threads),
            config: config.clone(),
        }, Init {
        }))
    }
    pub fn start_image(&self, base_dir: PathBuf,
        index: Index, virtual_path: VPath, can_replace: bool)
        -> CpuFuture<Image, Error>
    {
        self.pool.spawn_fn(move || {
            let dir = Dir::open(&base_dir)
                .map_err(|e| Error::OpenBase(base_dir.to_path_buf(), e))?;
            let dir = match ensure_virtual_parent(&dir, &virtual_path)? {
                DirBorrow::Borrow(_) => dir,
                DirBorrow::Owned(dir) => dir,
            };
            if !can_replace {
                check_exists(&dir, virtual_path.final_name())?;
            }

            let tmp_name = format!(".tmp.{}", virtual_path.final_name());
            remove_dir_recursive(&dir, &tmp_name)?;
            let temp_dir = ensure_subdir(&dir, &tmp_name)?;
            Ok(Image {
                virtual_path: virtual_path,
                parent: dir,
                temporary_name: tmp_name,
                temporary: temp_dir,
                index: index,
                can_replace: can_replace,
            })
        })
    }
    pub fn remove_image(&self, config: &Arc<Directory>, path: PathBuf)
        -> CpuFuture<(), Error>
    {
        let cfg = config.clone();
        self.pool.spawn_fn(move || {
            let dir = Dir::open(&cfg.directory)
                .map_err(|e| Error::OpenBase(cfg.directory.clone(), e))?;
            let dir = open_path(&dir, path.parent().expect("valid parent"))?;
            let filename = path.file_name().and_then(|x| x.to_str())
                .expect("valid path");
            remove_dir_recursive(&dir, filename)?;
            Ok(())
        })
    }
    /// Fetch block at path and offset
    ///
    /// If `writing` is `true` then it looks in `.tmp.dirname`.
    ///
    /// Regardless of the `writing` flag the function is inherently racy,
    /// because dir can be replaced at any time. But this doesn't matter,
    /// because wrong or absend block will be tolerated on the other side.
    pub fn read_block(&self, vpath: &VPath, path: &PathBuf, offset: u64,
                      writing: bool)
        -> CpuFuture<Vec<u8>, Error>
    {
        assert!(path.is_absolute());
        let config = self.config.clone();
        let vpath = vpath.clone();
        let path = path.strip_prefix("/").expect("path is absolute")
            .to_path_buf();
        self.pool.spawn_fn(move || {
            trace!("Reading block {:?}/{:?}:{}", vpath, path, offset);

            let cfg = config.dirs.get(vpath.key())
                .ok_or_else(|| Error::NoDir(vpath.clone()))?;
            let dir = Dir::open(&cfg.directory)
                .map_err(|e| Error::OpenBase(cfg.directory.clone(), e))?;
            let dir = open_path(&dir, vpath.parent().suffix())?;
            let dir = if writing {
                let tmp_name = format!(".tmp.{}", vpath.final_name());
                open_path(&dir, tmp_name)?
            } else {
                open_path(&dir, vpath.final_name())?
            };
            let epath = &|| cfg.directory.join(vpath.suffix()).join(&path);
            let file = open_path(&dir,
                &path.parent().expect("parent should exist"))?;
            let mut file = file.open_file(
                path.file_name().expect("file name should exist"))
                .map_err(|e| Error::ReadFile(epath(), e))?;
            // TODO(tailhook) use size of the block
            let mut buf = vec![0u8; 32768];
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| Error::ReadFile(epath(), e))?;
            let n = file.read(&mut buf)
                .map_err(|e| Error::ReadFile(epath(), e))?;
            buf.truncate(n);
            Ok(buf)
        })
    }
    pub fn write_block(&self, image: Arc<Image>,
                       path: Arc<PathBuf>, offset: u64,
                       block: BlockData)
        -> CpuFuture<(), Error>
    {
        self.pool.spawn_fn(move || {
            debug!("Writing block {:?}:{}", path, offset);
            let path = path
                .strip_prefix("/")
                .expect("path is absolute");
            let parent = path.parent().expect("path is never root");
            let dir = ensure_path(&image.temporary, parent)?;
            let fname = path.file_name()
                .expect("path has a filename");
            write_block(&*dir, Path::new(fname), offset, block)
                .map_err(|e| Error::WriteFile(recover_path(&*dir, fname), e))?;
            Ok(())
        })
    }
    pub fn commit_image(&self, image: Arc<Image>) -> CpuFuture<(), Error> {
        self.pool.spawn_fn(move || {
            commit_image(image)
        })
    }
    pub fn read_keep_list(&self, dir: &Arc<Directory>)
        -> CpuFuture<Vec<PathBuf>, Error>
    {
        let dir = dir.clone();
        self.pool.spawn_fn(move || {
            if let Some(ref fpath) = dir.keep_list_file {
                File::open(fpath)
                .map(|f| BufReader::new(f))
                .and_then(|f| {
                    let mut result = Vec::new();
                    for line in f.lines() {
                        let line = line?;
                        let line = line.trim();
                        if line.starts_with('#') {
                            continue;
                        }
                        let line = line.trim_left_matches('/');
                        let p = PathBuf::from(line);
                        if p.iter().count() == dir.num_levels {
                            result.push(p);
                        } else {
                            warn!("invalid path {:?} in keep list {:?}",
                                p, fpath);
                        }
                    }
                    Ok(result)
                })
                .map_err(|e| Error::ReadKeepList(fpath.to_path_buf(), e))
            } else {
                Ok(Vec::new())
            }
        })
    }
    pub fn read_peer_list(&self, path: &Path) -> CpuFuture<Vec<String>, ()> {
        let path = path.to_path_buf();
        self.pool.spawn_fn(move || {
            let f = match File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    warn!("Can't read peers {:?}: {}", path, e);
                    return Ok(Vec::new());
                }
            };
            let host_re = Regex::new(r#"^[a-zA-Z0-9\._-]+$"#)
                .expect("regex compiles");
            let mut result = Vec::new();
            for line in BufReader::new(f).lines() {
                let line = match line {
                    Ok(line) => line,
                    Err(e) => {
                        warn!("Error reading peers {:?}: {}. \
                            Already read {}.", path, e, result.len());
                        return Ok(result);
                    }
                };
                let line = line.trim();
                if line.len() == 0 || line.starts_with('#') {
                    continue;
                }
                if !host_re.is_match(line) {
                    warn!("Invalid hostname: {:?}", line);
                    continue;
                }
                result.push(line.to_string());
            }
            Ok(result)
        })
    }
    pub fn check_exists(&self, config: &Arc<Directory>, path: PathBuf)
        -> CpuFuture<bool, Error>
    {
        let cfg = config.clone();
        self.pool.spawn_fn(move || {
            let dir = Dir::open(&cfg.directory)
                .map_err(|e| Error::OpenBase(cfg.directory.clone(), e))?;
            match open_path(&dir, path) {
                Ok(_) => Ok(true),
                Err(Error::OpenDir(_, ref e))
                if e.kind() == io::ErrorKind::NotFound
                => Ok(false),
                Err(e) => Err(e),
            }
        })
    }
}

fn write_block(dir: &Dir, filename: &Path, offset: u64, block: BlockData)
    -> io::Result<()>
{
    let mut file = dir.update_file(filename, 0o644)?;
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(&block[..])?;
    Ok(())
}

pub fn start(_: Init, _: &Meta) -> Result<(), Error> {
   Ok(())
}
