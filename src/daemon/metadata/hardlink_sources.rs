use std::io;
use std::time::UNIX_EPOCH;
use std::collections::{HashSet, BTreeMap};

use ciruela::VPath;
use index::IndexData;
use metadata::{Meta, Error};
use metadata::{read_index, scan};


pub fn read_indexes(dir: VPath, meta: Meta)
    -> Result<Vec<(VPath, IndexData)>, Error>
{
    let all_states = match meta.signatures()?.open_vpath(&dir) {
        Ok(dir) => scan::all_states(&dir)?,

        Err(Error::Open(_, ref e))
        if e.kind() == io::ErrorKind::NotFound
        => BTreeMap::new(),

        Err(e) => return Err(e.into()),
    };
    // deduplicate, assuming all images really exist
    let mut vec = all_states.into_iter().collect::<Vec<_>>();
    vec.sort_by_key(|&(_, ref s)| {
        s.signatures.iter().map(|s| s.timestamp).max().unwrap_or(UNIX_EPOCH)
    });
    let mut visited = HashSet::new();
    let mut selected = Vec::new();
    for (dir, s) in vec {
        if visited.contains(&s.image) {
            continue;
        }
        visited.insert(s.image.clone());
        selected.push((dir, s));
        if selected.len() > 36 {  // TODO(tailhook) make tweakable
            break;
        }
    }
    let mut result = Vec::new();
    for (dir_name, s) in selected {
        // TODO(tailhook) look in cache
        match read_index::read(&s.image, &meta) {
            Ok(index) => {
                result.push((dir.join(dir_name), index));
            }
            Err(ref e) => {
                info!("Error reading index {:?} from file: {}", s.image, e);
            }
        }
    }
    Ok(result)
}
