use std::path::{PathBuf, Path};

use ciruela::VPath;
use metadata::dir::Dir;
use metadata::{Meta, Error};


fn scan_dir(dir: &Dir, virtual_path: PathBuf, level: usize,
            result: &mut Vec<(VPath, usize)>)
    -> Result<(), Error>
{
    if level > 1 {
        for item in dir.list_dirs()? {
            if let Some(sub_dir) = dir.dir_if_exists(&item)? {
                scan_dir(&sub_dir,
                    virtual_path.join(item), level-1, result)?;
            }
        }
    } else {
        let num = dir.list_files(".state")?.len();
        debug!("Found path {:?} with {} states", virtual_path, num);
        result.push((virtual_path.into(), num));
    }
    Ok(())
}

pub fn scan(meta: &Meta) -> Result<Vec<(VPath, usize)>, Error> {
    let mut result = Vec::new();
    // TODO(tailhook) throttle maybe?
    let root = meta.signatures()?;
    for (virtual_name, ref cfg) in &meta.config.dirs {
        if let Some(dir) = root.dir_if_exists(virtual_name)? {
            let vpath = Path::new("/").join(virtual_name);
            scan_dir(&dir, vpath, cfg.num_levels, &mut result)?;
        }
    }
    Ok(result)
}
