use std::io;
use std::str::from_utf8;
use std::path::Path;
use std::os::unix::ffi::OsStrExt;

use openat::Dir;
use serde::Serialize;
use serde_cbor::ser::Serializer as Cbor;

use ciruela::proto::{AppendDir, AppendDirAck};
use ciruela::database::signatures::{State, SignatureEntry};
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
    let state_file = format!("{}.state", cfg.image_name);
    match dir.metadata(&state_file[..]) {
        Ok(_) => {
            // TODO(tailhook) check whether current directory is the same
            // which should make request idempotent
            Ok(AppendDirAck {
                accepted: false,
            })
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            let timestamp = params.timestamp;
            let state = State {
                image: params.image,
                signatures: params.signatures.into_iter()
                    .map(|sig| SignatureEntry {
                        timestamp: timestamp,
                        signature: sig,
                    }).collect(),
            };
            let tmpname = format!("{}.state.tmp", cfg.image_name);
            let mut f = io::BufWriter::new(dir.create_meta_file(&tmpname)?);
            state.serialize(&mut Cbor::new(&mut f))?;
            drop(f);
            dir.rename_meta(&tmpname, &state_file)?;
            // TODO(tailhook) send message to image tracking subsystem
            // to start image syncing
            Ok(AppendDirAck {
                accepted: true,
            })
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

