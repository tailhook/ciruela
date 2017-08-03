use std::path::{PathBuf};
use std::net::SocketAddr;
use std::sync::Arc;

use abstract_ns::{Router, Resolver};
use futures::{Future, Stream};
use futures::stream::iter;
use tk_easyloop::spawn;

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

pub fn start(me: PeersInit, _config: &Arc<Config>, disk: &Disk,
    router: &Router)
    -> Result<(), Box<::std::error::Error>>
{
    let router = router.clone();
    if let Some(peer_file) = me.peer_file {
        spawn(disk.read_peer_list(&peer_file)
            .and_then(move |lst| {
                if lst.len() == 0 {
                    info!("No other peers specified, \
                        running in astandalone mode");
                } else {
                    debug!("Read {} peers.", lst.len());
                }
                iter(lst.into_iter().map(Ok))
                    .map(move |host| router.resolve(&format!("{}:8379", host))
                        .then(move |res| {
                        match res {
                            Ok(addr) => match addr.pick_one() {
                                Some(addr) => Ok(Some(Peer {
                                    hostname: host.clone(),
                                    name: host.clone(),
                                    // TODO(tailhook) get rid of expect
                                    addr: addr,
                                })),
                                None => {
                                    error!("No address for {:?}", host);
                                    Ok(None)
                                }
                            },
                            Err(e) => {
                                error!("Can't resolve {:?}: {}", host, e);
                                Ok(None)
                            }
                        }
                    }))
                    .buffer_unordered(3)
                    .filter_map(|x| x)
                    .collect()
            })
            .map(|lst| {
                debug!("Host list {:#?}", lst);
            }));
    } else {
        // TODO(tailhook) cantal mode
    }
    Ok(())
}
