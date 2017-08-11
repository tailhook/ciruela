use std::path::PathBuf;
use std::net::SocketAddr;
use std::collections::HashMap;

use disk::Disk;

use abstract_ns::{Router, Resolver};
use futures::{Future, Stream};
use futures::stream::iter;


pub fn read_peers(peer_file: PathBuf, disk: &Disk,
    router: &Router, port: u16)
    -> Box<Future<Item=HashMap<SocketAddr, String>, Error=()>>
{
    let router = router.clone();
    Box::new(disk.read_peer_list(&peer_file)
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
                            Some(addr) => Ok(Some((addr, host))),
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
        .map(move |lst| {
            debug!("Peer list {:?}", lst);
            lst.into_iter().collect()
        })) as Box<_>
}
