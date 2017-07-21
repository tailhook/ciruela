use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write, BufReader, BufRead};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_cpupool::{CpuPool, CpuFuture};
use openat::Dir;

use ciruela::VPath;
use config::{Config, Directory};
use disk::commit::commit_image;
use disk::dir::{ensure_virtual_parent, ensure_path, open_path};
use disk::dir::{ensure_subdir, recover_path, DirBorrow};
use disk::dir::{remove_dir_recursive};
use disk::{Init, Error};
use index::Index;
use metadata::Meta;
use tracking::Block;


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
    pub fn write_block(&self, image: Arc<Image>,
                       path: Arc<PathBuf>, offset: u64,
                       block: Block)
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
}

fn write_block(dir: &Dir, filename: &Path, offset: u64, block: Block)
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
