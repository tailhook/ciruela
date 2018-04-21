use std::cmp::Reverse;
use std::collections::{HashSet, BTreeMap};
use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use dir_signature::v1::merge::MergedSignatures;
use dir_signature::v1::{Entry, EntryKind, Hashes, Parser};

use {VPath};
use metadata::{Meta, Error};
use metadata::{read_index, scan};
use metadata::upload;
use tracking::Index;


#[derive(Debug)]
pub struct Hardlink {
    pub source: VPath,
    pub path: PathBuf,
    pub exe: bool,
    pub size: u64,
    pub hashes: Hashes,
}

pub fn replace_mode(index: Index, path: VPath, meta: Meta)
    -> Result<Vec<Hardlink>, Error>
{
    let dir = meta.signatures()?.ensure_dir(path.parent_rel())?;
    let old_image = if let Some(state) = dir.read_file(
        &format!("{}.state", path.final_name()),
        upload::read_state)?
    {
        state.image
    } else {
        // No old dir
        // TODO(tailhook) maybe make normal scan like in append_mode?
        //                are there any real use-cases where it's useful?
        debug!("no old dir for {:?}", path);
        return Ok(Vec::new());
    };
    let reader = match read_index::open(&old_image, &meta) {
        Ok(index) => index,
        Err(ref e) => {
            warn!("Error reading index {:?} from file: {}", old_image, e);
            return Ok(Vec::new());
        }
    };
    return scan_links(index, vec![(path, reader)]);
}

pub fn append_mode(index: Index, base_dir: VPath, meta: Meta)
    -> Result<Vec<Hardlink>, Error>
{
    let all_states = match meta.signatures()?.open_vpath(&base_dir) {
        Ok(open_dir) => scan::all_states(&meta, &base_dir, &open_dir)?,

        Err(Error::Open(_, ref e))
        if e.kind() == io::ErrorKind::NotFound
        => BTreeMap::new(),

        Err(e) => return Err(e.into()),
    };
    // deduplicate, assuming all images really exist
    let mut vec = all_states.into_iter().collect::<Vec<_>>();
    vec.sort_unstable_by_key(|&(_, ref s)| {
        Reverse(
            s.signatures.iter()
            .map(|s| s.timestamp).max()
            .unwrap_or(UNIX_EPOCH))
    });
    let mut visited = HashSet::new();
    let mut selected = Vec::new();
    {
        let writing = meta.writing();
        for (cur_dir, s) in vec {
            if visited.contains(&s.image) {
                continue;
            }
            if writing.contains_key(&base_dir.join(&cur_dir)) {
                // skip all currently writing images
                continue;
            }
            visited.insert(s.image.clone());
            selected.push((cur_dir, s));
            if selected.len() > 36 {  // TODO(tailhook) make tweakable
                break;
            }
        }
    }
    let mut files = Vec::new();
    for (dir_name, s) in selected {
        // TODO(tailhook) look in cache
        match read_index::open(&s.image, &meta) {
            Ok(index) => {
                files.push((base_dir.join(dir_name), index));
            }
            Err(ref e) => {
                warn!("Error reading index {:?} from file: {}", s.image, e);
            }
        }
    }
    return scan_links(index, files);
}

fn scan_links(index: Index, files: Vec<(VPath, Parser<BufReader<File>>)>)
    -> Result<Vec<Hardlink>, Error>
{
    let mut result = Vec::new();
    let mut msig = MergedSignatures::new(files)?;
    let mut msig_iter = msig.iter();
    for entry in index.entries.iter() {
        match entry {
            &Entry::File {
                path: ref lnk_path,
                exe: lnk_exe,
                size: lnk_size,
                hashes: ref lnk_hashes,
            } => {
                for tgt_entry in msig_iter.advance(&EntryKind::File(lnk_path))
                {
                    match tgt_entry {
                        (vpath,
                         Ok(Entry::File {
                             path: ref tgt_path,
                             exe: tgt_exe,
                             size: tgt_size,
                             hashes: ref tgt_hashes
                        }))
                        if lnk_exe == tgt_exe &&
                            lnk_size == tgt_size &&
                            lnk_hashes == tgt_hashes
                        => {
                            debug_assert_eq!(tgt_path, lnk_path);
                            result.push(Hardlink {
                                source: vpath.clone(),
                                path: lnk_path.clone(),
                                exe: lnk_exe,
                                size: lnk_size,
                                hashes: lnk_hashes.clone(),
                            });
                            break;
                        },
                        _ => continue,
                    }
                }
            },
            _ => {},
        }
    }
    Ok(result)
}
