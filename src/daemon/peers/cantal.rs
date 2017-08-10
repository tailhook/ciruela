use std::collections::HashMap;
use std::time::{Duration};
use std::net::SocketAddr;
use std::sync::Arc;

use crossbeam::sync::ArcCell;
use futures::{Future, Stream};
use machine_id::MachineId;
use tk_cantal;
use tk_easyloop::{spawn, interval, handle};

use peers::Peer;


pub fn spawn_fetcher(cell: &Arc<ArcCell<HashMap<MachineId, Peer>>>,
    port: u16)
{
    let conn = tk_cantal::connect_local(&handle());
    let cell = cell.clone();
    spawn(
        interval(Duration::new(5, 0))
        .map_err(|_| { unreachable!() })
        .for_each(move |_| {
            let cell = cell.clone();
            conn.get_peers()
            .and_then(move |peers| {
                let peers = peers.peers.into_iter()
                    .filter_map(|p| {
                        let addr = p.primary_addr
                            .and_then(|a| a.parse::<SocketAddr>().ok());
                        let machine_id: Result<MachineId, _> = p.id.parse();
                        match (addr, machine_id) {
                            (Some(addr), Ok(machine_id)) => Some((
                                machine_id.clone(),
                                Peer {
                                     id: machine_id,
                                     addr: SocketAddr::new(addr.ip(), port),
                                     hostname: p.hostname,
                                     name: p.name,
                                })),
                            (_, Err(e)) => {
                                info!("Invalid machine id {:?}: {}", p.id, e);
                                None
                            }
                            _ => None,
                        }
                    })
                    .collect::<HashMap<_, _>>();

                debug!("Peer list {:?}",
                    peers.iter().map(|(_, p)| &p.name).collect::<Vec<_>>());
                cell.set(Arc::new(peers));
                Ok(())
            })
            .map_err(|e| {
                error!("Error fetching cantal data: {}", e);
            })
        }));
}
