use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_cpupool::{CpuPool, CpuFuture};
use openat::Dir;

use config::Config;
use disk::commit::commit_image;
use disk::dir::{ensure_path, ensure_subdir, recover_path, DirBorrow};
use disk::{Init, Error};
use index::Index;
use metadata::Meta;
use openat;
use tracking::Block;


#[derive(Clone)]
pub struct Disk {
    pool: CpuPool,
    config: Arc<Config>,
}

pub struct Image {
    pub parent: Dir,
    pub image_name: String,
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
        parent: PathBuf, image_name: String,
        index: Index)
        -> CpuFuture<Image, Error>
    {
        self.pool.spawn_fn(move || {
            let dir = Dir::open(&base_dir)
                .map_err(|e| Error::OpenBase(base_dir.to_path_buf(), e))?;
            let dir = match ensure_path(&dir, parent)? {
                DirBorrow::Borrow(_) => dir,
                DirBorrow::Owned(dir) => dir,
            };
            let tmp_name = format!(".tmp.{}", image_name);
            let temp_dir = ensure_subdir(&dir, &tmp_name)?;
            Ok(Image {
                parent: dir,
                image_name: image_name.to_string(),
                temporary_name: tmp_name,
                temporary: temp_dir,
                index: index,
            })
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
}

fn write_block(dir: &Dir, filename: &Path, offset: u64, block: Block)
    -> io::Result<()>
{
    let mut file = dir.update_file(filename, 0o644)?;
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(&block[..])?;
    Ok(())
}

pub fn start(init: Init, metadata: &Meta) -> Result<(), Error> {
   Ok(())
}

