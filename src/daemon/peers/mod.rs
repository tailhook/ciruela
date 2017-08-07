mod file;
mod cantal;
mod config;

use std::path::{PathBuf};
use std::net::SocketAddr;
use std::sync::Arc;

use abstract_ns::{Router};
use crossbeam::sync::ArcCell;

use config::{Config};
use disk::Disk;


#[derive(Debug)]
pub struct Peer {
    addr: SocketAddr,
    hostname: String,
    name: String,
}

#[derive(Clone)]
pub struct Peers {
}

pub struct PeersInit {
    peer_file: Option<PathBuf>,
}

impl Peers {
    pub fn new(peer_file: Option<PathBuf>) -> (Peers, PeersInit) {
        (Peers {
        }, PeersInit {
            peer_file: peer_file,
        })
    }
}

pub fn start(me: PeersInit, config: &Arc<Config>, disk: &Disk,
    router: &Router)
    -> Result<(), Box<::std::error::Error>>
{
    let cell = Arc::new(ArcCell::new(Arc::new(Vec::new())));
    if let Some(peer_file) = me.peer_file {
        file::read_peers(&cell, peer_file, disk, router, config.port);
    } else {
        cantal::spawn_fetcher(&cell, config.port);
    }
    Ok(())
}
