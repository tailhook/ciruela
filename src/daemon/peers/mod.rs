mod file;
mod cantal;
mod config;
mod gossip;

use std::collections::HashMap;
use std::path::{PathBuf};
use std::net::SocketAddr;
use std::sync::Arc;

use abstract_ns::{Router};
use crossbeam::sync::ArcCell;
use futures::Future;
use tk_easyloop::spawn;

use config::{Config};
use disk::Disk;
use machine_id::MachineId;
use tracking::Tracking;


#[derive(Debug, Clone)]
pub struct Peer {
    id: MachineId,
    addr: SocketAddr,
    hostname: String,
    name: String,
}

#[derive(Clone)]
pub struct Peers {
}

pub struct PeersInit {
    machine_id: MachineId,
    peer_file: Option<PathBuf>,
}

impl Peers {
    pub fn new(
        machine_id: MachineId,
        peer_file: Option<PathBuf>) -> (Peers, PeersInit)
    {
        (Peers {
        }, PeersInit {
            peer_file: peer_file,
            machine_id,
        })
    }
}

pub fn start(me: PeersInit, addr: SocketAddr,
    config: &Arc<Config>, disk: &Disk,
    router: &Router, tracking: &Tracking)
    -> Result<(), Box<::std::error::Error>>
{
    let cell = Arc::new(ArcCell::new(Arc::new(HashMap::new())));
    if let Some(peer_file) = me.peer_file {
        let tracking = tracking.clone();
        let id = me.machine_id.clone();
        spawn(
            file::read_peers(&cell, peer_file, disk, router, config.port)
            .and_then(move |fut| {
                gossip::start(addr, cell, id, &tracking, fut)
                    .expect("can start gossip");
                Ok(())
            }));
    } else {
        cantal::spawn_fetcher(&cell, config.port);
        gossip::start(addr, cell, me.machine_id, tracking, HashMap::new())?;
    }
    Ok(())
}
