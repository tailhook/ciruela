use disk::Disk;
use std::path::PathBuf;

use abstract_ns::{Router, Resolver};
use futures::{Future, Stream};
use futures::stream::iter;
use tk_easyloop::spawn;

use peers::Peer;

pub fn read_peers(peer_file: PathBuf, disk: &Disk,
    router: &Router, port: u16)
{
    let router = router.clone();
    spawn(disk.read_peer_list(&peer_file)
        .and_then(move |lst| {
            if lst.len() == 0 {
                info!("No other peers specified, \
                    running in astandalone mode");
            } else {
                debug!("Read {} peers.", lst.len());
            }
            iter(lst.into_iter().map(Ok))
                .map(move |host|
                    router.resolve(&format!("{}:{}", host, port))
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
}
