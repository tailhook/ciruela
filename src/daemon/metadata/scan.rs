use std::io::BufReader;

use metadata::{Meta, Error};

use ciruela::VPath;
use ciruela::database::signatures::State;
use serde_cbor::de::from_reader as read_cbor;


pub fn all_states(path: &VPath, meta: &Meta)
    -> Result<Vec<(String, State)>, Error>
{
    let mut res = Vec::new();
    let dir = meta.signatures()?.open_vpath(path)?;
    for mut name in dir.list_files(".state")? {
        let state = dir.read_file(&name, |f| {
            read_cbor(&mut BufReader::new(f))
        })?;
        let nlen = name.len() - ".state".len();
        name.truncate(nlen);
        match state {
            Some(state) => res.push((name, state)),
            None => return Err(Error::FileWasVanished(dir.path(&name))),
        }
    }
    Ok(res)
}
