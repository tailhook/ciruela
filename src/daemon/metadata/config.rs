use std::path::Path;
use std::sync::Arc;
use std::str::from_utf8;
use std::os::unix::ffi::OsStrExt;


use config::{Config, Directory};
use metadata::Error;


pub struct DirConfig<'a> {
    pub base: &'a Path,
    pub parent: &'a Path,
    pub image_name: &'a str,
    pub config: &'a Directory,
}


pub fn find_config_dir<'x>(cfg: &'x Arc<Config>, path: &'x Path)
    -> Result<DirConfig<'x>, Error>
{
    for (key, config) in &cfg.dirs {
        let keypath = Path::new(key);
        if let Ok(suffix) = path.strip_prefix(keypath) {
            let num_levels = suffix.components().count();
            if num_levels != config.num_levels {
                return Err(Error::LevelMismatch(
                    num_levels, config.num_levels));
            } else {
                return Ok(DirConfig {
                    base: keypath,
                    // all these unwraps are guaranteed by path
                    // and config checking (num_levels > 0) and check_path())
                    parent: suffix.parent().unwrap(),
                    image_name: from_utf8(
                        suffix.file_name().unwrap().as_bytes()).unwrap(),
                    config: config,
                });
            }
        }
    }
    return Err(Error::PathNotFound(path.to_path_buf()));
}
