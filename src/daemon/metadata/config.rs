use std::path::Path;
use std::sync::Arc;


use config::{Config, Directory};
use metadata::Error;


pub struct DirConfig<'a> {
    pub base: &'a Path,
    pub suffix: &'a Path,
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
                    // TODO(tailhook) check if suffix is non-empty!
                    suffix: suffix,
                    config: config,
                });
            }
        }
    }
    return Err(Error::PathNotFound(path.to_path_buf()));
}
