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
    spawn(loop_fn((addr, mid), move |(addr, mid)| {
        let sys = sys.clone();
        let pair = (path.clone(), hash); // TODO(tailhook) optimize?
        sys.remote.fetch_base_dir(addr, &pair.0).then(move |res| {
            let mut state = sys.state();
            match res {
                Ok(dir) => {
                    let dir_hash = Hash::for_object(&dir);
                    if dir_hash == hash {
                        state.reconciling.remove(&pair);
                        return Ok(Loop::Break((addr, dir)))
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
                // It's fine, probably all hosts have an updated hash already
                state.reconciling.remove(&pair);
                return Err(());
            }
        })
    })
    .map(|(addr, dir)| {
        debug!("Got dir {:?} from addr {}", dir, addr);
        unimplemented!();
    }));
}
