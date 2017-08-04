use std::error::Error;
use std::time::{Duration};
use std::net::SocketAddr;

use futures::{Future, Stream};
use tk_cantal;
use tk_easyloop::{spawn, interval, handle};

use peers::Peer;


pub fn spawn_fetcher(port: u16)
    -> Result<(), Box<Error>>
{
    let conn = tk_cantal::connect_local(&handle());
    spawn(
        interval(Duration::new(30, 0))
        .map_err(|_| { unreachable!() })
        .for_each(move |_| {
            conn.get_peers()
            .and_then(move |peers| {
                let peers = peers.peers.into_iter()
                    .filter_map(|p| {
                        let addr = p.primary_addr
                            .and_then(|a| a.parse::<SocketAddr>().ok());
                        if let Some(addr) = addr {
                            Some(Peer {
                                addr: SocketAddr::new(addr.ip(), port),
                                hostname: p.hostname,
                                name: p.name,
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                println!("Peers {:#?}", peers);
                Ok(())
            })
            .map_err(|e| {
                error!("Error fetching cantal data: {}", e);
            })
        }));
    Ok(())
}
