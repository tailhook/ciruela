use std::io;
use std::path::{PathBuf, Path};

use openat::{Dir, SimpleType};

use metadata::{Meta, Error};
use metadata::dir_ext::{recover, DirExt};

fn scan_dir(dir: &Dir, virtual_path: PathBuf, level: usize,
            result: &mut Vec<(PathBuf, usize)>)
    -> Result<(), Error>
{
    let err = &|e| {
        Error::ListDir(recover(dir, Path::new("")), e)
    };
    let list = dir.list_dir(".").map_err(err)?;
    if level > 1 {
        for item in list {
            let item = item.map_err(err)?;
            let name = match item.file_name().to_str() {
                Some(s) => s,
                None => {
                    warn!("Bad directory {:?}",
                        recover(dir, &Path::new(item.file_name())));
                    continue;
                }
            };
            if item.simple_type() != Some(SimpleType::Dir) {
                // TODO(tailhook) support filesystem which don't report file
                // type on read_dir
                warn!("Path {:?} is not a directory {}",
                    recover(dir, name), level);
                continue;
            }
            let sub_dir = dir.meta_sub_dir(name)?;
            scan_dir(&sub_dir, virtual_path.join(name), level-1, result)?;
        }
    } else {
        let mut num = 0;
        for item in list {
            let item = item.map_err(err)?;
            let name = match item.file_name().to_str() {
                Some(s) => s,
                None => {
                    warn!("Bad filename {:?}",
                        recover(dir, &Path::new(item.file_name())));
                    continue;
                }
            };
            if item.simple_type() != Some(SimpleType::File) {
                // TODO(tailhook) support filesystem which don't report file
                // type on read_dir
                warn!("Path {:?} is not a file", recover(dir, name));
                continue;
            }
            if !name.ends_with(".state") {
                warn!("Path {:?} is not a state file", recover(dir, name));
                continue;
            }
            num += 1;
        }
        debug!("Found path {:?} with {} states", virtual_path, num);
        result.push((virtual_path, num));
    }
    Ok(())
}

pub fn scan(meta: &Meta) -> Result<Vec<(PathBuf, usize)>, Error> {
    let mut result = Vec::new();
    // TODO(tailhook) throttle maybe?
    for (virtual_name, ref cfg) in &meta.config.dirs {
        let dir = match meta.base_dir.sub_dir(virtual_name) {
            Ok(dir) => dir,
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(Error::OpenMeta(
                    recover(&meta.base_dir, virtual_name), e));
            }
        };
        let vpath = Path::new("/").join(virtual_name);
        scan_dir(&dir, vpath, cfg.num_levels, &mut result)?;
    }
    Ok(result)
}
