use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::collections::{HashSet, HashMap, BTreeMap};

use crossbeam::sync::ArcCell;
use futures::{Future, Stream, Async};
use futures::sync::mpsc::UnboundedReceiver;
use rand::{thread_rng, sample};
use tk_easyloop::{handle, spawn, interval};
use tokio_core::net::UdpSocket;
use tokio_core::reactor::Interval;
use serde::Deserialize;
use serde_cbor::de::Deserializer;

use ciruela::{VPath, ImageId};
use machine_id::MachineId;
use mask::Mask;
use named_mutex::Mutex;
use peers::Peer;
use peers::packets::{Packet, Message, PacketRef, MessageRef};
use serde_cbor::ser::to_writer;
use tracking::Tracking;

/// Size of the buffer for sending packet
pub const MAX_GOSSIP_PACKET: usize = 4096;

/// Maximum number of base dirs in single packet
pub const MAX_BASE_DIRS: usize = 10;

/// Interval at which send gossip packets
pub const GOSSIP_INTERVAL: u64 = 1000;

/// Number of packets to send on interval and on new fresh image
pub const PACKETS_AT_ONCE: usize = 4;


struct Gossip {
    socket: UdpSocket,
    tracking: Tracking,
    interval: Interval,
    machine_id: MachineId,
    messages: UnboundedReceiver<Message>,
    peers: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    dir_peers: HashMap<VPath, HashSet<MachineId>>,
    downloading: Arc<Mutex<
        HashMap<MachineId, BTreeMap<VPath, (ImageId, Mask)>>>>,
    /// A list of peers with unknown ids, read from file
    /// only used if we use `peers.txt` (not `--cantal`)
    future_peers: HashMap<SocketAddr, String>,
}

impl Gossip {
    fn poll_forever(&mut self) {
        self.write_messages();
        self.read_messages();
        while self.interval.poll().expect("interval never fails").is_ready()
        {
            self.send_gossips();
        }
    }
    fn read_messages(&mut self) {
        let mut buf = [0u8; MAX_GOSSIP_PACKET];
        loop {
            while let Ok((len, addr)) = self.socket.recv_from(&mut buf) {
                let mut buf = io::Cursor::new(&buf[..len]);
                let pkt: Packet = match Deserialize::deserialize(
                    &mut Deserializer::new(&mut buf))
                {
                    Ok(x) => x,
                    Err(e) => {
                        info!("Bad gossip packet from {:?}: {}", addr, e);
                        continue;
                    }
                };
                if pkt.machine_id == self.machine_id {
                    continue;
                }
                if let Some(name) = self.future_peers.remove(&addr) {
                    let mut peers = self.peers.get();
                    Arc::make_mut(&mut peers)
                        .insert(pkt.machine_id.clone(), Peer {
                            id: pkt.machine_id.clone(),
                            addr: addr,
                            hostname: name.clone(),
                            name: name.clone(),
                        });
                    self.peers.set(peers);
                }
                match pkt.message {
                    Message::BaseDirs { in_progress, base_dirs } => {
                        for (vpath, hash) in base_dirs {
                            self.dir_peers.entry(vpath.clone())
                                .or_insert_with(HashSet::new)
                                .insert(pkt.machine_id.clone());
                            self.tracking.reconcile_dir(vpath, hash, addr,
                                pkt.machine_id.clone());
                        }
                        if in_progress.len() == 0 {
                            self.downloading.lock().remove(&pkt.machine_id);
                        } else {
                            self.downloading.lock()
                                .insert(pkt.machine_id.clone(), in_progress);
                        }
                    }
                    Message::Downloading { path, image, mask } => {
                        self.downloading.lock()
                            .entry(pkt.machine_id.clone())
                            .or_insert_with(BTreeMap::new)
                            .insert(path, (image, mask));
                    }
                }
            }
            if !self.socket.poll_read().is_ready() {
                break;
            }
        }
    }
    fn write_messages(&mut self) {
        while let Ok(Async::Ready(Some(msg))) = self.messages.poll() {
            let peers = match msg {
                Message::BaseDirs {..} => unreachable!(),
                Message::Downloading { ref path, .. } => {
                    let all = self.peers.get();
                    let mut peers = match self.dir_peers.get(&path.parent()) {
                        Some(peers) => {
                            sample(&mut thread_rng(), peers, PACKETS_AT_ONCE)
                            .into_iter()
                            .filter_map(|id| all.get(id).map(|p| p.addr))
                            .collect()
                        }
                        None => Vec::new(),
                    };
                    if peers.len() < PACKETS_AT_ONCE {
                        peers.extend(sample(&mut thread_rng(),
                            all.values(), PACKETS_AT_ONCE)
                            .into_iter().map(|p| p.addr));
                    }
                    peers
                }
            };
            let packet = Packet {
                machine_id: self.machine_id.clone(),
                message: msg,
            };
            let mut buf = Vec::with_capacity(1400);
            to_writer(&mut buf, &packet).expect("can serialize packet");
            for addr in &peers {
                debug!("Sending progress to {}: {:?}",
                    addr, packet.message);
                self.socket.send_to(&buf, &addr)
                    .map_err(|e| {
                        warn!("Error sending message to {:?}: {}", addr, e)
                    }).ok();
            }
        }
    }
    fn send_gossips(&mut self) {
        // Need to ping future peers to find out addresses
        // We ping them until they respond, and are removed from future
        let ipr = self.tracking.get_in_progress();
        for (addr, _) in &self.future_peers {
            self.send_gossip(*addr, &ipr);
        }
        let lst = self.peers.get();
        debug!("PEERS {:?}", lst);
        let hosts = sample(&mut thread_rng(), lst.values(), PACKETS_AT_ONCE);
        for host in hosts {
            self.send_gossip(host.addr, &ipr);
        }
    }
    fn send_gossip(&self, addr: SocketAddr,
        in_progress: &BTreeMap<VPath, (ImageId, Mask)>)
    {
        let mut buf = Vec::with_capacity(1400);
        let mut base_dirs = BTreeMap::new();
        for _ in 0..MAX_BASE_DIRS {
            match self.tracking.pick_random_dir() {
                Some((vpath, hash)) => {
                    base_dirs.insert(vpath.clone(), hash.clone());
                }
                None => break,
            }
        }

        let packet = PacketRef {
            machine_id: &self.machine_id,
            message: MessageRef::BaseDirs {
                in_progress: &in_progress,
                base_dirs: &base_dirs,
            }
        };
        to_writer(&mut buf, &packet).expect("can serialize packet");
        debug!("Sending ping to {} [{}]", addr, buf.len());
        self.socket.send_to(&buf, &addr)
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
        Ok(Async::NotReady)
    }
}

pub fn start(addr: SocketAddr,
    peers: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    downloading: Arc<Mutex<
        HashMap<MachineId, BTreeMap<VPath, (ImageId, Mask)>>>>,
    messages: UnboundedReceiver<Message>,
    machine_id: MachineId,
    tracking: &Tracking, future_peers: HashMap<SocketAddr, String>)
    -> Result<(), io::Error>
{
    spawn(Gossip {
        interval: interval(Duration::from_millis(GOSSIP_INTERVAL)),
        socket: UdpSocket::bind(&addr, &handle())?,
        tracking: tracking.clone(),
        dir_peers: HashMap::new(),
        peers, machine_id, future_peers, downloading, messages,
    });
    Ok(())
}
