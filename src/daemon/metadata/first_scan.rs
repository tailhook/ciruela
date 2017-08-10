use std::path::{PathBuf, Path};

use serde_cbor::ser::to_writer;

use ciruela::{VPath, Hash, HashBuilder};
use ciruela::proto::{BaseDirState};
use metadata::dir::Dir;
use metadata::scan;
use metadata::{Meta, Error};
use peers::config::get_hash;



fn scan_dir(cfg_hash: Hash, dir: &Dir, virtual_path: PathBuf, level: usize,
            result: &mut Vec<(VPath, Hash, usize)>)
    -> Result<(), Error>
{
    if level > 1 {
        for item in dir.list_dirs()? {
            if let Some(sub_dir) = dir.dir_if_exists(&item)? {
                scan_dir(cfg_hash, &sub_dir,
                    virtual_path.join(item), level-1, result)?;
            }
        }
    } else {
        let mut dig = Hash::builder();
        let states = scan::all_states(dir)?;
        let state = BaseDirState {
            path: virtual_path.into(),
            config_hash: cfg_hash,
            dirs: states,
        };
        dig.object(&state);
        let hash = dig.done();
        debug!("Found path {:?} with {} states, hash: {}",
            state.path, state.dirs.len(), hash);
        result.push((state.path, hash, state.dirs.len()));
    }
    Ok(())
}

pub fn scan(meta: &Meta) -> Result<Vec<(VPath, Hash, usize)>, Error> {
    let mut result = Vec::new();
    // TODO(tailhook) throttle maybe?
    let root = meta.signatures()?;
    for (virtual_name, ref cfg) in &meta.config.dirs {
        if let Some(dir) = root.dir_if_exists(virtual_name)? {
            let vpath = Path::new("/").join(virtual_name);
            let cfg_hash = get_hash(&cfg);
            scan_dir(cfg_hash, &dir, vpath, cfg.num_levels, &mut result)?;
        }
    }
    Ok(result)
}
