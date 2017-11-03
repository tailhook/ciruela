use std::io::BufReader;
use std::collections::BTreeMap;

use metadata::{Error, Dir};

use ciruela::database::signatures::State;
use serde_cbor::de::from_reader as read_cbor;


pub fn all_states(dir: &Dir)
    -> Result<BTreeMap<String, State>, Error>
{
    let mut res = BTreeMap::new();
    for mut name in dir.list_files(".state")? {
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
