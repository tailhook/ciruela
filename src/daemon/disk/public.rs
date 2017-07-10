
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::sync::mpsc::{unbounded, UnboundedSender};
use futures_cpupool::{CpuPool, CpuFuture};
use openat::Dir;
use tk_easyloop;

use openat;
use index::Index;
use config::Config;
use disk::{Init, Error};
use metadata::Meta;


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
}

pub fn open_subdir<P: AsRef<Path>>(dir: &Dir, name: P)
    -> Result<Dir, Error>
{
    match dir.sub_dir(name.as_ref()) {
        Ok(dir) => Ok(dir),
        Err(e) => match dir.create_dir(name.as_ref(), 0o755) {
            Ok(()) => match dir.sub_dir(name.as_ref()) {
                Ok(dir) => Ok(dir),
                Err(e) => Err(Error::CreateDirRace(recover_path(dir, name), e)),
            },
            Err(e) => Err(Error::CreateDir(recover_path(dir, name), e)),
        },
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
