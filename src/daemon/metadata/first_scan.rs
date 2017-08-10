use std::path::{PathBuf, Path};

use metadata::dir::Dir;
use metadata::{Meta, Error};
use tracking::Tracking;



fn scan_dir(dir: &Dir, virtual_path: PathBuf, level: usize,
            tracking: &Tracking)
    -> Result<(), Error>
{
    if level > 1 {
        for item in dir.list_dirs()? {
            if let Some(sub_dir) = dir.dir_if_exists(&item)? {
                scan_dir(&sub_dir,
                    virtual_path.join(item), level-1, tracking)?;
            }
        }
    } else {
        tracking.scan_dir(virtual_path.into());
    }
    Ok(())
}

pub fn scan(meta: &Meta, tracking: &Tracking)
    -> Result<(), Error>
{
    // TODO(tailhook) throttle maybe?
    let root = meta.signatures()?;
    for (virtual_name, ref cfg) in &meta.config.dirs {
        if let Some(dir) = root.dir_if_exists(virtual_name)? {
            let vpath = Path::new("/").join(virtual_name);
            scan_dir(&dir, vpath, cfg.num_levels, tracking)?;
        }
    }
    Ok(())
}
