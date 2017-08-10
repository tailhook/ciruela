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
        match dir.read_file(&name, |f| read_cbor(&mut BufReader::new(f))) {
            Ok(Some(state)) => {
                let nlen = name.len() - ".state".len();
                name.truncate(nlen);
                res.insert(name, state);
            }
            Ok(None) => {
                warn!("Scan error: {}",
                    Error::FileWasVanished(dir.path(&name)));
            }
            Err(e) => {
                warn!("Scan error: {}", Error::from(e));
            }
        }
    }
    Ok(res)
}
