use std::io;
use std::str::from_utf8;
use std::path::Path;
use std::os::unix::ffi::OsStrExt;

use openat::Dir;


use ciruela::proto::{AppendDir, AppendDirAck};
use metadata::{Meta, Error, find_config_dir};
use metadata::config::DirConfig;
use metadata::dir_ext::{DirExt, recover};


pub fn check_path(path: &Path) -> Result<&Path, Error> {
    use std::path::Component::Normal;
    let result = path.strip_prefix("/").map_err(|_| Error::InvalidPath)?;
    for cmp in result.components() {
        if let Normal(component) = cmp {
            if from_utf8(component.as_bytes()).is_ok() {
                continue;
            } else {
                return Err(Error::InvalidPath);
            }
        }
        return Err(Error::InvalidPath);
    }
    Ok(result)
}


pub fn start(params: AppendDir, meta: &Meta)
    -> Result<AppendDirAck, Error>
{
    let path = check_path(&params.path)?;

    let cfg = find_config_dir(&meta.config, path)?;
    info!("Directory {:?} has base {:?} and dir {:?} and name {:?}",
        params.path, cfg.base, cfg.parent, cfg.image_name);
    let dir = open_base_path(meta, &cfg)?;
    match dir.metadata(cfg.image_name) {
        Ok(_) => {
            Ok(AppendDirAck {
                accepted: false,
            })
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            let mut f = dir.create_meta_file(&cfg.image_name)?;
            unimplemented!();
        }
        Err(e) => {
            Err(Error::OpenMeta(recover(&dir, cfg.image_name), e))
        }
    }
}

fn open_dir(dir: &Dir, path: &Path) -> Result<Dir, Error> {
    match dir.sub_dir(path) {
        Ok(dir) => Ok(dir),
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                dir.create_meta_dir(path)?;
                dir.meta_sub_dir(path)
            } else {
                Err(Error::OpenMeta(recover(dir, path), e))
            }
        }
    }
}

pub fn open_base_path(meta: &Meta, cfg: &DirConfig) -> Result<Dir, Error> {
    let mut dir = open_dir(&meta.base_dir, &cfg.base)?;
    for cmp in cfg.parent.iter() {
        dir = open_dir(&dir, &Path::new(cmp))?;
    }
    return Ok(dir);
}

