use std::path::{PathBuf};
use std::net::SocketAddr;
use std::sync::Arc;

use futures::Future;
use tk_easyloop::spawn;

use config::{Config};
use disk::Disk;


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

pub fn start(me: PeersInit, _config: &Arc<Config>, disk: &Disk)
    -> Result<(), Box<::std::error::Error>>
{
    if let Some(peer_file) = me.peer_file {
        spawn(disk.read_peer_list(&peer_file)
            .and_then(|lst| {
                if lst.len() == 0 {
                    info!("No other peers specified, \
                        running in astandalone mode");
                } else {
                    debug!("Read {} peers.", lst.len());
                }
                // TODO(tailhook) resolve names
                Ok(())
            }));
    } else {
        // TODO(tailhook) cantal mode
    }
    Ok(())
}
