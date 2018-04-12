use std::collections::{HashMap};
use std::net::SocketAddr;
use std::time::Instant;

use cleanup::{sort_out};
use machine_id::MachineId;
use proto::Hash;
use proto::{BaseDirState, AppendDir, ReplaceDir, GetBaseDir};
use proto::{RequestClient};
use proto::Error;
use tracking::Subsystem;
use metrics::{Counter, Integer};
use {VPath};

use futures::future::{Future, Loop, loop_fn};
use tk_easyloop::spawn;
use metadata::{Upload, Accept};
use tracking::RECONCILE_BASEDIR_THROTTLE;


lazy_static! {
    pub static ref PROCESSING: Integer = Integer::new();
    pub static ref PROCESSED: Counter = Counter::new();
    pub static ref REPLACED: Counter = Counter::new();
    pub static ref APPENDED: Counter = Counter::new();
    pub static ref REPLACE_REJECTED: Counter = Counter::new();
    pub static ref APPEND_REJECTED: Counter = Counter::new();
}

pub struct ReconPush {
    pub path: VPath,
    pub hash: Hash,
    pub initial_addr: SocketAddr,
    pub initial_machine_id: MachineId,
}

pub fn start(sys: &Subsystem, info: ReconPush) {
    debug!("Reconciling {:?} to hash {} from {}/{}",
        info.path, info.hash, info.initial_addr, info.initial_machine_id);
    let sys = sys.clone();
    let sys2 = sys.clone();
    let sys3 = sys.clone();
    let sys_drop = sys.clone();
    let ReconPush {
        path,
        hash,
        initial_addr: addr,
        initial_machine_id: mid
    } = info;
    // TODO(tailhook) allow Remote subsystem to pick a connection, so
    // it can choose one, already available when multiple choices are there
    let pair = (path.clone(), hash);
    let pair2 = pair.clone();
    PROCESSING.incr(1);
    spawn(loop_fn((addr, mid, 0), move |(addr, mid, n)| {
        let sys = sys.clone();
        let pair = pair.clone(); // TODO(tailhook) optimize?
        sys.remote.ensure_connected(&sys.tracking, addr)
            .request(GetBaseDir { path: pair.0.clone() })
            .then(move |res| {
                let mut state = sys.state();
                match res {
                    Ok(dir) => {
                        let dir_state = BaseDirState {
                            path: pair.0.clone(),
                            config_hash: dir.config_hash,
                            keep_list_hash: dir.keep_list_hash,
                            dirs: dir.dirs,
                        };
                        let dir_hash = Hash::for_object(&dir_state.dirs);
                        if dir_hash == hash {
                            return Ok(Loop::Break((addr, dir_state)))
                        } else {
                            debug!("Mismatching hash from {}:{:?}: {} != {}",
                                addr, pair.0, hash, dir_hash);
                        }
                    }
                    // This error often happends as a part of race condition
                    // of sending request over keep-alived connection.
                    Err(Error::UnexpectedTermination) if n < 2 => {
                        return Ok(Loop::Continue((addr, mid, n+1)));
                    }
                    Err(e) => {
                        warn!("Error fetching {} from {}: {}", hash, addr, e);
                    }
                }
                let next_host = state.reconciling
                    .get_mut(&pair)
                    .and_then(|h| {
                        h.remove(&(addr, mid));
                        let item = h.iter().cloned().next();
                        item.as_ref().map(|pair| h.remove(&pair));
                        item
                    });
                if let Some((addr, mid)) = next_host {
                    return Ok(Loop::Continue((addr, mid, 0)));
                } else {
                    // It's fine, probably all hosts have an updated hash already.
                    // It might also be that there is some race condition, like
                    // we tried to do request, and it failed temporarily
                    // (keep-alive connection is dropping). But we didn't mark
                    // this hash as visited, yet so on next ping we will retry.
                    debug!("No next host for {:?}", pair);
                    return Err(());
                }
            })
    })
    .and_then(move |(addr, remote)| {
        let config = sys2.config.dirs.get(remote.path.key())
            .expect("only configured dirs are reconciled");
        let path = remote.path.clone();
        let p1 = remote.path.clone();
        let p2 = remote.path.clone();
        sys2.meta.scan_dir(&path)
            .map_err(move |e| error!("Scanning base-dir {:?}: {}", p1, e))
        .join(sys2.disk.read_keep_list(config)
            .map_err(move |e| error!("Reading keep_list {:?}: {}", p2, e)))
        .map(move |(local, keep_list)| (addr, remote, local, keep_list))
    })
    .map(move |(_addr, remote, mut local_dirs, keep_list)| {
        let config = sys3.config.dirs.get(remote.path.key())
            .expect("only configured dirs are reconciled");
        let path = remote.path.clone();
        let mut possible_dirs = local_dirs.iter().map(|(name, state)| {
            (path.suffix().join(name), state.clone())
        }).collect::<HashMap<_, _>>();

        for (name, rstate) in &remote.dirs {
            let vpath = path.join(name);
            if sys3.is_recently_deleted(&vpath, &rstate.image) {
                debug!("Not updating {:?} to {} because it was recently \
                    deleted", vpath, rstate.image);
                continue;
            }
            let ref image_id = rstate.image;
            // TODO(tailhook) consume multiple signatures
            let sig = match rstate.signatures.get(0) {
                Some(x) => x,
                None => {
                    warn!("Got image with no signatures: {:?}", vpath);
                    continue;
                }
            };
            if let Some(old_state) = local_dirs.get(name) {
                if &old_state.image == image_id {
                    // TODO(tailhook) maybe update timestamp
                    continue;
                }
                if old_state.signatures.iter()
                    .any(|old_s| old_s.timestamp >= sig.timestamp)
                {
                    continue;
                }
            }
            possible_dirs.insert(path.suffix().join(name), rstate.clone());
        }
        let sorted = sort_out(config,
            possible_dirs.into_iter().collect(), &keep_list);

        let mut count = sys3.state().downloading_in_basedir(&path);
        let mut sorted_remote = remote.dirs.into_iter().collect::<Vec<_>>();
        let mut full_reconciliation = true;
        sorted_remote.sort_unstable_by_key(|&(_, ref rstate)| {
            rstate.signatures.iter().map(|x| x.timestamp).max()
        });

        for (name, mut rstate) in sorted_remote.into_iter().rev() {
            let sub_path = path.suffix().join(&name);
            let vpath = path.join(&name);
            if !sorted.used.iter().any(|&(ref p, _)| p == &sub_path) {
                debug!("Not updating {:?} to {} it's going to be cleaned up \
                        again", vpath, rstate.image);
                continue;
            }
            let sys = sys3.clone();
            // TODO(tailhook) consume multiple signatures
            let sig = match rstate.signatures.drain(..).next() {
                Some(x) => x,
                None => {
                    warn!("Got image with no signatures: {:?}", vpath);
                    continue;
                }
            };
            let image_id = rstate.image;
            if let Some(old_state) = local_dirs.remove(&name) {
                if old_state.image == image_id {
                    // TODO(tailhook) maybe update timestamp
                    continue;
                }
                if old_state.signatures.iter()
                    .any(|old_s| old_s.timestamp >= sig.timestamp)
                {
                    trace!("Peer image {} is older than ours", image_id);
                    continue;
                }
                if count >= RECONCILE_BASEDIR_THROTTLE {
                    trace!("Throttling reconciliation of {:?}", path);
                    full_reconciliation = false;
                    break;
                }
                count += 1;
                {
                    let state = &mut *sys.state();
                    if let Some(items) = state.reconciling.get(&pair2) {
                        state.recently_received.entry(vpath.clone())
                            .or_insert_with(HashMap::new)
                            .extend(
                                items.iter()
                                .map(|&(addr, _)| {
                                    (addr, Instant::now())
                                }));
                    }
                }
                debug!("Replacing {:?} {:?} -> {:?}", name,
                    old_state, sig);
                spawn(
                    sys.meta.replace_dir(ReplaceDir {
                        path: vpath.clone(),
                        image: image_id.clone(),
                        old_image: Some(old_state.image),
                        timestamp: sig.timestamp,
                        signatures: vec![sig.signature],
                    }).then(move |result| {
                        match result {
                            Ok(Upload::Accepted(Accept::New)) => {
                                info!("Replacing {} -> {:?}", image_id, vpath);
                                REPLACED.incr(1);
                                sys.tracking.fetch_dir(
                                    vpath, image_id, true);
                            }
                            Ok(Upload::Accepted(Accept::InProgress)) |
                            Ok(Upload::Accepted(Accept::AlreadyDone)) => {}
                            Ok(Upload::Rejected(reason, _)) => {
                                REPLACE_REJECTED.incr(1);
                                error!("Error reconciling {:?}: {}",
                                    vpath, reason);
                            }
                            Err(e) => {
                                error!("Error reconciling {:?}: {}",
                                    vpath, e);
                            }
                        }
                        Ok(())
                    }));
            } else {
                if count >= RECONCILE_BASEDIR_THROTTLE {
                    trace!("Throttling reconciliation of {:?}", path);
                    full_reconciliation = false;
                    break;
                }
                count += 1;
                spawn(
                    sys.meta.append_dir(AppendDir {
                        path: vpath.clone(),
                        image: image_id.clone(),
                        timestamp: sig.timestamp,
                        signatures: vec![sig.signature],
                    }).then(move |result| {
                        match result {
                            Ok(Upload::Accepted(Accept::New)) => {
                                info!("Appending {} -> {:?}", image_id, vpath);
                                sys.tracking.fetch_dir(
                                    vpath, image_id, false);
                            }
                            Ok(Upload::Accepted(Accept::InProgress)) |
                            Ok(Upload::Accepted(Accept::AlreadyDone)) => {}
                            Ok(Upload::Rejected(reason, _)) => {
                                APPEND_REJECTED.incr(1);
                                error!("Error reconciling {:?}: {}",
                                    vpath, reason);
                            }
                            Err(e) => {
                                error!("Error reconciling {:?}: {}",
                                    vpath, e);
                            }
                        }
                        Ok(())
                    }));
            }
        }
        full_reconciliation
    })
    .then(move |res| -> Result<(), ()> {
        let mut state = sys_drop.state();
        if res == Ok(true) {
            state.base_dirs.get(&path).map(|b| b.add_parent_hash(hash));
        }
        state.reconciling.remove(&(path, hash));
        PROCESSING.decr(1);
        PROCESSED.incr(1);
        Ok(())
    }));
}
