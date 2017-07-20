use std::io::{BufReader};
use std::fs::File;
use std::sync::Arc;
use std::collections::hash_map::Entry;

use serde_cbor::de::from_reader as read_cbor;
use serde_cbor::error::Error as CborError;

use ciruela::proto::{AppendDir, AppendDirAck};
use ciruela::database::signatures::{State, SignatureEntry};
use metadata::{Meta, Error};


fn append_signatures(state: &mut State, new: Vec<SignatureEntry>) {
    for sig in new.into_iter() {
        if state.signatures.iter().find(|&x| *x == sig).is_none() {
            state.signatures.push(sig);
        }
    }
}

fn read_state(f: File) -> Result<State, CborError> {
    read_cbor(&mut BufReader::new(f))
}

pub fn start(params: AppendDir, meta: &Meta)
    -> Result<AppendDirAck, Error>
{
    let vpath = params.path;
    let config = if let Some(cfg) = meta.config.dirs.get(vpath.key()) {
        if vpath.level() != cfg.num_levels {
            return Err(Error::LevelMismatch(vpath.level(), cfg.num_levels));
        }
        cfg
    } else {
        return Err(Error::PathNotFound(vpath));
    };
    let dir = meta.signatures()?.ensure_dir(vpath.parent())?;

    let timestamp = params.timestamp;
    let signatures = params.signatures.into_iter()
        .map(|sig| SignatureEntry {
            timestamp: timestamp,
            signature: sig,
        }).collect::<Vec<_>>();
    let state_file = format!("{}.state", vpath.final_name());

    let mut writing = meta.writing();
    let state = match writing.entry(vpath.clone()) {
        Entry::Vacant(e) => {
            let final_name = vpath.final_name();
            if let Some(mut state) = dir.read_file(final_name, read_state)? {
                if state.image == params.image {
                    append_signatures(&mut state, signatures);
                    let state = Arc::new(state);
                    e.insert(state.clone());
                    state
                } else {
                    return Ok(AppendDirAck {
                        accepted: false,
                    });
                }
            } else {
                let state = Arc::new(State {
                    image: params.image.clone(),
                    signatures: signatures,
                });
                e.insert(state.clone());
                state
            }
        }
        Entry::Occupied(mut e) => {
            let old_state = e.get_mut();
            if old_state.image == params.image {
                if signatures != old_state.signatures {
                    let nstate = Arc::make_mut(old_state);
                    append_signatures(nstate, signatures);
                }
                old_state.clone()
            } else {
                return Ok(AppendDirAck {
                    accepted: false,
                });
            }
        }
    };
    dir.replace_file(&state_file, &*state)?;
    meta.tracking.fetch_dir(&params.image, vpath, config);
    Ok(AppendDirAck {
        accepted: true,
    })
}
