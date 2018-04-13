use std::thread::sleep;
use std::io::{self, BufReader};
use std::time::Duration;
use std::path::{Path, PathBuf};

use serde_cbor::de::from_reader as read_cbor;

use VPath;
use database::signatures::State;
use metadata::dir::Dir;
use metadata::{Meta, Error};


fn collect_dir(dir: &Dir, meta: &Meta) -> Result<(), Error> {
    for mut name in dir.list_files(".state")? {
        let read: Option<State>;
        read = dir.read_file(&name, |f| read_cbor(&mut BufReader::new(f)))?;
        if let Some(state) = read {
            meta.mark_used(&state.image);
        }
        sleep(Duration::from_millis(1));
    }
    Ok(())
}

fn scan_dir(meta: &Meta, dir: &Dir, virtual_path: PathBuf, level: usize)
    -> Result<(), Error>
{
    if level > 1 {
        for item in dir.list_dirs()? {
            if let Some(sub_dir) = dir.dir_if_exists(&item)? {
                scan_dir(meta, &sub_dir, virtual_path.join(item), level-1)?;
            }
        }
    } else {
        match meta.signatures()?.open_vpath(&VPath::from(virtual_path)) {
            Ok(dir) => {
                collect_dir(&dir, meta)?;
            }
            Err(Error::Open(_, ref e))
            if e.kind() == io::ErrorKind::NotFound
            => {}
            Err(e) => Err(e)?,
        }
        sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn scan(meta: &Meta) -> Result<(), Error>
{
    let root = meta.signatures()?;
    for (virtual_name, ref cfg) in &meta.0.config.dirs {
        if let Some(dir) = root.dir_if_exists(virtual_name)? {
            let vpath = Path::new("/").join(virtual_name);
            scan_dir(meta, &dir, vpath, cfg.num_levels)?;
        }
    }
    Ok(())
}

pub fn full_collection(meta: &Meta) -> Result<(), Error> {
    scan(meta)?;
    info!("Found {} used images",
        meta.0.collecting.lock().as_ref().map(|x| x.len()).unwrap_or(0));
    let root = meta.indexes()?;
    for prefix in root.list_dirs()? {
        if let Some(sub_dir) = root.dir_if_exists(&prefix)? {
            for file_name in sub_dir.list_files(".ds1")? {
                let image_id = match file_name[..file_name.len()-4].parse() {
                    Ok(x) => x,
                    Err(_) => {
                        error!("bad filename {:?} in indexes", file_name);
                        continue;
                    }
                };
                let is_marked = match *meta.0.collecting.lock() {
                    Some(ref set) => set.contains(&image_id),
                    None => return Err(Error::IndexGcInterrupted),
                };
                if !is_marked {
                    match sub_dir.remove_file(&file_name) {
                        Ok(()) => {
                            info!("Deleted index {:?}", image_id);
                        }
                        Err(e) => {
                            error!("Error deleting index {:?}: {}",
                                image_id, e);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
