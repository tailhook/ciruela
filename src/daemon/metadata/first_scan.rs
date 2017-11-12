use std::path::{PathBuf, Path};

use virtual_path::VPath;
use metadata::dir::Dir;
use metadata::{Meta, Error};



fn scan_dir<F>(dir: &Dir, virtual_path: PathBuf, level: usize,
    add_dir: &mut F)
    -> Result<(), Error>
    where F: FnMut(VPath),
{
    if level > 1 {
        for item in dir.list_dirs()? {
            if let Some(sub_dir) = dir.dir_if_exists(&item)? {
                scan_dir(&sub_dir,
                    virtual_path.join(item), level-1, add_dir)?;
            }
        }
    } else {
        add_dir(virtual_path.into());
    }
    Ok(())
}

pub fn scan<F>(meta: &Meta, mut add_dir: F)
    -> Result<(), Error>
    where F: FnMut(VPath),
{
    // TODO(tailhook) throttle maybe?
    let root = meta.signatures()?;
    for (virtual_name, ref cfg) in &meta.0.config.dirs {
        if let Some(dir) = root.dir_if_exists(virtual_name)? {
            let vpath = Path::new("/").join(virtual_name);
            scan_dir(&dir, vpath, cfg.num_levels, &mut add_dir)?;
        }
    }
    Ok(())
}
