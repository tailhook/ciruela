use std::net::SocketAddr;

use ciruela::{VPath, Hash};
use machine_id::{MachineId};
use tracking::Subsystem;

use futures::future::{Future, Loop, loop_fn};
use tk_easyloop::spawn;


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
    let ReconPush {
        path,
        hash,
        initial_addr: addr,
        initial_machine_id: mid
    } = info;
    // TODO(tailhook) allow Remote subsystem to pick a connection, so
    // it can choose one, already available when multiple choices are there
    spawn(loop_fn((addr, mid), move |(addr, mid)| {
        let sys = sys.clone();
        let pair = (path.clone(), hash); // TODO(tailhook) optimize?
        sys.remote.fetch_base_dir(addr, &pair.0).then(move |res| {
            let mut state = sys.state();
            match res {
                Ok(dir) => {
                    let dir_hash = Hash::for_object(&dir);
                    if dir_hash == hash {
                        // TODO(tailhook) isn't it too early to remove?
                        state.reconciling.remove(&pair);
                        return Ok(Loop::Break((addr, dir)))
                    } else {
                        debug!("Mismatching hash from {}:{:?}: {} != {}",
                            addr, pair.0, hash, dir_hash);
                    }
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
            if let Some(next_host) = next_host {
                return Ok(Loop::Continue(next_host))
            } else {
                // It's fine, probably all hosts have an updated hash already.
                // It might also be that there is some race condition, like
                // we tried to do request, and it failed temporarily
                // (keep-alive connection is dropping). But we didn't mark
                // this hash as visited, yet so on next ping we will retry.
                state.reconciling.remove(&pair);
                debug!("No next host for {:?}", pair);
                return Err(());
            }
        })
    })
    .map(|(addr, dir)| {
        debug!("Got dir {:?} from addr {}", dir, addr);
        unimplemented!();
    }));
}
