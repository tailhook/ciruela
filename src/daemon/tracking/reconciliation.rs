use std::net::SocketAddr;

use ciruela::{VPath, Hash};
use machine_id::{MachineId};
use tracking::Subsystem;


pub struct ReconPush {
    pub path: VPath,
    pub hash: Hash,
    pub initial_addr: SocketAddr,
    pub initial_machine_id: MachineId,
}

pub fn start(sys: &Subsystem, info: ReconPush) {
    debug!("Reconciling {:?} to hash {} from {}/{}",
        info.path, info.hash, info.initial_addr, info.initial_machine_id);
}
