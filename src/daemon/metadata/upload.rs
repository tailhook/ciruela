use std::collections::hash_map::Entry;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::sync::Arc;

use serde::Serialize;
use serde_cbor::de::from_reader as read_cbor;
use serde_cbor::error::Error as CborError;
use serde_cbor::ser::Serializer as Cbor;

use ciruela::database::signatures::{State, SignatureEntry};
use ciruela::proto::{AppendDir, AppendDirAck};
use ciruela::proto::{ReplaceDir, ReplaceDirAck};
use ciruela::proto::{SigData, Signature, verify};
use config::Directory;
use metadata::keys::read_upload_keys;
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

fn check_keys(sigdata: &SigData, signatures: &Vec<Signature>,
              config: &Arc<Directory>, meta: &Meta)
    -> Result<bool, Error>
{
    let keys = read_upload_keys(config, meta)?;
    // If at least one key is allowed we keep all signatures for the key
    // some server needs them
    debug!("Keys {:?} resulted into {} keys read",
        config.upload_keys, keys.len());
    Ok(signatures.iter().any(|sig| verify(sigdata, sig, &keys)))
}

pub fn start_append(params: AppendDir, meta: &Meta)
    -> Result<AppendDirAck, Error>
{
    let vpath = params.path.clone();
    let config = if let Some(cfg) = meta.config.dirs.get(vpath.key()) {
        if vpath.level() != cfg.num_levels {
            return Err(Error::LevelMismatch(vpath.level(), cfg.num_levels));
        }
        cfg
    } else {
        return Err(Error::PathNotFound(vpath));
    };

    if !check_keys(&params.sig_data(), &params.signatures, config, meta)? {
        warn!("{:?} has no valid signatures. Upload-keys: {:?}",
              params, config.upload_keys);
        return Ok(AppendDirAck {
            accepted: false,
        });
    }

    let dir = meta.signatures()?.ensure_dir(vpath.parent_rel())?;

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
            if let Some(mut state) = dir.read_file(&state_file, read_state)? {
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
    dir.replace_file(&state_file, |file| {
        state.serialize(&mut Cbor::new(BufWriter::new(file)))
    })?;
    meta.tracking.fetch_dir(&params.image, vpath, false, config);
    Ok(AppendDirAck {
        accepted: true,
    })
}

pub fn start_replace(params: ReplaceDir, meta: &Meta)
    -> Result<ReplaceDirAck, Error>
{
    let vpath = params.path.clone();
    let config = if let Some(cfg) = meta.config.dirs.get(vpath.key()) {
        if vpath.level() != cfg.num_levels {
            return Err(Error::LevelMismatch(vpath.level(), cfg.num_levels));
        }
        cfg
    } else {
        return Err(Error::PathNotFound(vpath));
    };
    if config.append_only {
        return Ok(ReplaceDirAck {
            accepted: false,
        });
    }

    if !check_keys(&params.sig_data(), &params.signatures, config, meta)? {
        return Ok(ReplaceDirAck {
            accepted: false,
        });
    }

    let dir = meta.signatures()?.ensure_dir(vpath.parent_rel())?;

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
            if let Some(mut state) = dir.read_file(&state_file, read_state)? {
                if state.image == params.image {
                    append_signatures(&mut state, signatures);
                    let state = Arc::new(state);
                    e.insert(state.clone());
                    state
                } else {
                    let state = Arc::new(State {
                        image: params.image.clone(),
                        signatures: signatures,
                    });
                    e.insert(state.clone());
                    state
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
                // TODO(tailhook) stop fetching image, delete and
                // start replacing
                warn!("Replace is rejected because already in progress");
                return Ok(ReplaceDirAck {
                    accepted: false,
                });
            }
        }
    };
    dir.replace_file(&state_file, |file| {
        state.serialize(&mut Cbor::new(BufWriter::new(file)))
    })?;
    meta.tracking.fetch_dir(&params.image, vpath, true, config);
    Ok(ReplaceDirAck {
        accepted: true,
    })
}
