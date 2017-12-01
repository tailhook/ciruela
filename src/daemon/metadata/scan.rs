use std::io::{BufReader};
use std::collections::BTreeMap;

use metadata::{Meta, Error, Dir};

use {VPath};
use database::signatures::State;
use serde_cbor::de::from_reader as read_cbor;


pub fn all_states(meta: &Meta, vpath: &VPath, dir: &Dir)
    -> Result<BTreeMap<String, State>, Error>
{
    let mut res = BTreeMap::new();
    for mut name in dir.list_files(".state")? {
        if name.ends_with(".new.state") {
            let nlen = name.len() - ".new.state".len();
            if !meta.writing().contains_key(&vpath.join(&name[..nlen])) {
                match dir.remove_file(&name) {
                    Ok(()) => {
                        info!("Removed stale file {:?} in {:?}",
                            name, vpath);
                    }
                    Err(e) => warn!("Error removing stale file: {}", e),
                }
                continue;
            }
        }
        let read: Result<Option<State>, _>;
        read = dir.read_file(&name, |f| read_cbor(&mut BufReader::new(f)));
        match read {
            Ok(Some(ref state)) if state.signatures.len() < 1 => {
                dir.rename_broken_file(&name,
                    format_args!("Scan error: state has no signatures"));
            }
            Ok(Some(state)) => {
                let nlen = name.len() - ".state".len();
                name.truncate(nlen);
                if name.ends_with(".new") {
                    let nlen = nlen - 4;
                    name.truncate(nlen);
                    res.insert(name, state);
                } else if !res.contains_key(&name) {
                    // We must not replace `.new` file, if it visited earlier
                    // Since the only way to have duplicate entries is to
                    // have both `.new` and non-new file, we replace when
                    // visit `.new` and insert if not exists for non-new file
                    res.insert(name, state);
                }
            }
            Ok(None) => {
                warn!("Scan error: {}",
                    Error::FileWasVanished(dir.path(&name)));
            }
            Err(e @ Error::Decode(..)) => {
                dir.rename_broken_file(&name,
                    format_args!("Scan error: {}", e));
            }
            Err(e) => {
                error!("Scan error: {}", e);
            }
        }
    }
    Ok(res)
}
