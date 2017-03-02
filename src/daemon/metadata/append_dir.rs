use std::io;
use std::path::{Path, PathBuf};

use openat::Dir;


use ciruela::proto::{AppendDir, AppendDirAck};
use metadata::{Meta, Error, find_config_dir};
use metadata::config::DirConfig;


pub fn check_path(path: &Path) -> Result<&Path, Error> {
    use std::path::Component::Normal;
    let result = path.strip_prefix("/").map_err(|_| Error::InvalidPath)?;
    for cmp in result.components() {
        if let Normal(_) = cmp {
            continue;
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
    info!("Directory {:?} has base {:?} and suffix {:?}",
        params.path, cfg.base, cfg.suffix);
    let dir = open_base_path(meta, &cfg)?;
    unimplemented!();
}

fn recover(dir: &Dir, path: &Path) -> PathBuf {
    let mut result = dir.recover_path()
        .unwrap_or_else(|e| {
            warn!("Error recovering path {:?}: {}", dir, e);
            PathBuf::from("<unknown>")
        });
    result.push(path);
    result
}

fn open_dir(dir: &Dir, path: &Path) -> Result<Dir, Error> {
    match dir.sub_dir(path) {
        Ok(dir) => Ok(dir),
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                dir.create_dir(path, 0o755)
                    .map_err(|e| Error::CreateMeta(recover(dir, path), e))?;
                match dir.sub_dir(path) {
                    Ok(dir) => Ok(dir),
                    Err(e) => Err(Error::OpenMeta(recover(dir, path), e)),
                }
            } else {
                Err(Error::OpenMeta(recover(dir, path), e))
            }
        }
    }
}

pub fn open_base_path(meta: &Meta, cfg: &DirConfig) -> Result<Dir, Error> {
    let mut dir = open_dir(&meta.base_dir, &cfg.base)?;
    for cmp in cfg.suffix.iter().take(cfg.config.num_levels - 1) {
        dir = open_dir(&dir, &Path::new(cmp))?;
    }
    return Ok(dir);
}

