use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write, BufReader, BufRead, Read};
use std::os::unix::fs::{PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::{Future, Stream};
use futures::stream::iter_ok;
use futures_cpupool::{self, CpuPool, CpuFuture};
use openat::{Dir, SimpleType, hardlink};
use regex::Regex;
use self_meter_http::Meter;
use void::Void;

use {VPath};
use config::{Config, Directory};
use disk::commit::commit_image;
use disk::dir::{ensure_virtual_parent, ensure_path, open_path};
use disk::dir::{ensure_subdir, recover_path, DirBorrow};
use disk::dir::{remove_dir_recursive};
use disk::{Init, Error};
use tracking::Index;
use metadata::{Meta, Hardlink};
use tracking::BlockData;
use metrics::Counter;



lazy_static! {
    pub static ref HARDLINKED_FILES: Counter = Counter::new();
    pub static ref HARDLINKED_BYTES: Counter = Counter::new();
}


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
    pub fn new(num_threads: usize, config: &Arc<Config>, meter: &Meter)
        -> Result<(Disk, Init), Error>
    {
        let m1 = meter.clone();
        let m2 = meter.clone();
        Ok((Disk {
            pool: futures_cpupool::Builder::new()
                .pool_size(num_threads)
                .name_prefix("disk-")
                .after_start(move || m1.track_current_thread_by_name())
                .before_stop(move || m2.untrack_current_thread())
                .create(),
            config: config.clone(),
        }, Init {
        }))
    }
    pub fn start_image(&self, base_dir: PathBuf,
        index: Index, virtual_path: VPath)
        -> CpuFuture<Image, Error>
    {
        self.pool.spawn_fn(move || {
            let dir = Dir::open(&base_dir)
                .map_err(|e| Error::OpenBase(base_dir.to_path_buf(), e))?;
            let dir = match ensure_virtual_parent(&dir, &virtual_path)? {
                DirBorrow::Borrow(_) => dir,
                DirBorrow::Owned(dir) => dir,
            };

            let tmp_name = format!(".tmp.{}", virtual_path.final_name());
            remove_dir_recursive(&dir, &tmp_name)?;
            let temp_dir = ensure_subdir(&dir, &tmp_name)?;
            Ok(Image {
                virtual_path: virtual_path,
                parent: dir,
                temporary_name: tmp_name,
                temporary: temp_dir,
                index: index,
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
            let dir = match Dir::open(&cfg.directory) {
                Ok(dir) => dir,
                Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                    return Ok(false);
                }
                Err(e) => {
                    return Err(Error::OpenBase(cfg.directory.clone(), e));
                }
            };
            match open_path(&dir, path) {
                Ok(_) => Ok(true),
                Err(Error::OpenDir(_, ref e))
                if e.kind() == io::ErrorKind::NotFound
                => Ok(false),
                Err(e) => Err(e),
            }
        })
    }
    pub fn check_and_hardlink(&self, hardlinks: Vec<Hardlink>,
        image: &Arc<Image>)
        -> Box<Future<Item=HashSet<PathBuf>, Error=Void>>
    {
        let pool = self.pool.clone();
        let config = self.config.clone();
        let image = image.clone();
        Box::new(iter_ok(hardlinks)
            .map(move |hlink| {
                let config = config.clone();
                let image = image.clone();
                pool.spawn_fn(move || {
                    Ok(try_hardlink(&config, &hlink, &image)
                        .map_err(|e| error!("Error hardlinking: {}", e))
                        .map(move |()| hlink.path))
                })
            })
            .buffer_unordered(8)
            .filter_map(|x| x.ok())
            .collect().map(|x| x.into_iter().collect()))
    }
}

fn try_hardlink(config: &Arc<Config>, hlink: &Hardlink, img: &Arc<Image>)
    -> Result<(), Error>
{
    let cfg = match config.dirs.get(hlink.source.key()) {
        Some(cfg) => cfg,
        None => return Err(Error::NoDir(hlink.source.clone())),
    };
    let dir = Dir::open(&cfg.directory)
        .map_err(|e| Error::OpenBase(cfg.directory.clone(), e))?;
    let dir = open_path(&dir, hlink.source.suffix())?;
    let parent = hlink.path.parent().expect("path is never root");
    let dir = open_path(&dir, parent)?;
    let epath = &|| cfg.directory.join(hlink.source.suffix()).join(&hlink.path);
    let file_name = hlink.path.file_name().expect("file name should exist");
    let mut file = dir.open_file(file_name)
        .map_err(|e| Error::ReadFile(epath(), e))?;
    let ok = hlink.hashes.check_file(&mut file)
        .map_err(|e| Error::ReadFile(epath(), e))?;
    if !ok {
        return Err(Error::Checksum(epath()));
    }
    let meta = file.metadata()
        .map_err(|e| Error::ReadFile(epath(), e))?;
    let target_perm = if hlink.exe { 0o755 } else { 0o644 };
    // Throwing out S_IFREG flag
    if meta.permissions().mode() & 0o777 != target_perm {
        // note: entry permissions are checked when comparing, so this check
        // is as useful as checksum check: i.e. if file was modified on
        // the disk
        return Err(Error::Checksum(epath()));
    }
    debug!("Hardlinking {:?}/{:?} -> {:?}", hlink.source, hlink.path, epath());
    let dest = ensure_path(&img.temporary, parent)?;
    hardlink(&dir, file_name, &dest, file_name)
        .map_err(|e| Error::Hardlink(epath(), e))?;
    HARDLINKED_FILES.incr(1);
    HARDLINKED_BYTES.incr(hlink.size);
    Ok(())
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
