use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::{HashSet, HashMap};

use crossbeam::sync::ArcCell;
use futures::{Future, Stream, Async};
use rand::{thread_rng, sample, Rng};
use tk_easyloop::{handle, spawn, interval};
use tokio_core::net::UdpSocket;
use tokio_core::reactor::Interval;

use machine_id::MachineId;
use ciruela::VPath;
use ciruela::proto::Hash;
use peers::Peer;
use tracking::Tracking;
use serde_cbor::from_reader;
use serde_cbor::ser::to_writer;


/// Maximum size of gossip packet
///
/// Larger packets allow to carry more timestamps of directories at once,
/// which efficiently makes probability of syncing new directory erlier
/// much bigger.
///
/// Making it less than MTU, makes probability of loss much smaller. On
/// other hand 1500 MTU is on WAN and slower networks. Hopefully, we are
/// efficient enough with this packet size anyway.
pub const MAX_GOSSIP_PACKET: usize = 1400;

/// Interval at which send gossip packets
pub const GOSSIP_INTERVAL: u64 = 1000;

/// Number of packets to send on interval and on new fresh image
pub const PACKETS_AT_ONCE: usize = 4;


#[derive(Serialize, Deserialize)]
pub struct Head {
    pub id: MachineId,
}

struct Gossip {
    socket: UdpSocket,
    tracking: Tracking,
    interval: Interval,
    machine_id: MachineId,
    peers: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    dir_peers: HashMap<String, HashSet<MachineId>>,
    /// A list of peers with unknown ids, read from file
    /// only used if we use `peers.txt` (not `--cantal`)
    future_peers: HashMap<SocketAddr, String>,
}

impl Gossip {
    fn poll_forever(&mut self) {
        self.read_messages();
        if self.interval.poll().expect("interval never fails").is_ready() {
            self.send_gossips();
        }
    }
    fn read_messages(&mut self) {
        let mut buf = [0u8; MAX_GOSSIP_PACKET];
        loop {
            while let Ok((len, addr)) = self.socket.recv_from(&mut buf) {
                let mut buf = io::Cursor::new(&buf[..len]);
                let head: Head = match from_reader(&mut buf) {
                    Ok(x) => x,
                    Err(e) => {
                        info!("Bad gossip packet from {:?}: {}", addr, e);
                        continue;
                    }
                };
                if let Some(name) = self.future_peers.remove(&addr) {
                    let mut peers = self.peers.get();
                    Arc::make_mut(&mut peers).insert(head.id.clone(), Peer {
                        id: head.id.clone(),
                        addr: addr,
                        hostname: name.clone(),
                        name: name.clone(),
                    });
                }
                while buf.position() < len as u64 {
                    let pair = match from_reader(&mut buf) {
                        Ok(x) => x,
                        Err(e) => {
                            info!("Bad dir in gossip packet from {:?}: {}",
                                addr, e);
                            break;
                        }
                    };
                    let (vpath, hash): (VPath, Hash) = pair;
                    self.dir_peers.entry(vpath.key().to_string())
                        .or_insert_with(HashSet::new)
                        .insert(head.id.clone());
                    self.tracking.reconcile_dir(vpath, hash, addr,
                        self.machine_id.clone());
                }
            }
            if !self.socket.poll_read().is_ready() {
                break;
            }
        }
    }
    fn send_gossips(&mut self) {
        let lst = self.peers.get();
        let hosts = sample(&mut thread_rng(), lst.values(), PACKETS_AT_ONCE);
        for host in hosts {
            self.send_gossip(host.addr);
        }
    }
    fn send_gossip(&mut self, addr: SocketAddr) {
        let mut buf = [0u8; MAX_GOSSIP_PACKET];
        let packet_len = {
            let mut cur = io::Cursor::new(&mut buf[..]);
            to_writer(&mut cur, &Head {
                id: self.machine_id.clone(),
            }).expect("can serialize head");

            while let Some((vpath, hash)) = self.tracking.pick_random_dir() {
                let pos = cur.position();
                match to_writer(&mut cur, &(vpath, hash)) {
                    Ok(()) => {}
                    Err(e) => {
                        cur.set_position(pos);
                        break;
                    }
                }
            }
            cur.position() as usize
        };
        self.socket.send_to(&buf[..packet_len], &addr)
            .map_err(|e| {
                warn!("Error sending message to {:?}: {}", addr, e)
            }).ok();
    }
}

impl Future for Gossip {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        self.poll_forever();
        Ok(Async::Ready(()))
    }
}

pub fn start(addr: SocketAddr,
    peers: Arc<ArcCell<HashMap<MachineId, Peer>>>, machine_id: MachineId,
    tracking: &Tracking, future_peers: HashMap<SocketAddr, String>)
    -> Result<(), io::Error>
{
    spawn(Gossip {
        interval: interval(Duration::from_millis(GOSSIP_INTERVAL)),
        socket: UdpSocket::bind(&addr, &handle())?,
        tracking: tracking.clone(),
        dir_peers: HashMap::new(),
        peers, machine_id, future_peers,
    });
    Ok(())
}
