use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_cpupool::{CpuPool, CpuFuture};
use openat::Dir;

use openat;
use index::Index;
use config::Config;
use disk::{Init, Error};
use metadata::Meta;
use tracking::Block;
use disk::commit::commit_image;


#[derive(Clone)]
pub struct Disk {
    pool: CpuPool,
    config: Arc<Config>,
}

pub struct Image {
    parent: Dir,
    image_name: String,
    temporary_name: String,
    temporary: Dir,
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
            let mut dir = Dir::open(&base_dir)
                .map_err(|e| Error::OpenBase(base_dir.to_path_buf(), e))?;
            for name in parent.iter() {
                dir = open_subdir(&dir, name)?;
            }
            let tmp_name = format!(".tmp.{}", image_name);
            let temp_dir = open_subdir(&dir, &tmp_name)?;
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
            let mut dir_tmp;
            let parent = path.parent().expect("path is never root");
            let dir = if parent != Path::new("") {
                let mut pit = parent.iter();
                dir_tmp = open_subdir(&image.temporary, pit.next().unwrap())?;
                for component in pit {
                    dir_tmp = open_subdir(&dir_tmp, component)?;
                }
                &dir_tmp
            } else {
                &image.temporary
            };
            let fname = path.file_name()
                .expect("path has a filename");
            write_block(&dir, Path::new(fname), offset, block)
                .map_err(|e| Error::WriteFile(recover_path(&dir, fname), e))?;
            Ok(())
        })
    }
    pub fn commit_image(&self, image: Arc<Image>) -> CpuFuture<(), Error> {
        self.pool.spawn_fn(move || {
            debug!("Commiting {:?}", image.image_name);
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

pub fn open_subdir<P: AsRef<Path>>(dir: &Dir, name: P)
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

pub fn start(init: Init, metadata: &Meta) -> Result<(), Error> {
   Ok(())
}

fn recover_path<P: AsRef<Path>>(dir: &Dir, path: P) -> PathBuf {
    let mut result = dir.recover_path()
        .unwrap_or_else(|e| {
            warn!("Error recovering path {:?}: {}", dir, e);
            PathBuf::from("<unknown>")
        });
    result.push(path.as_ref());
    result
}
