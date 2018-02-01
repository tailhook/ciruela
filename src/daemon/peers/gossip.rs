use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::collections::{HashSet, HashMap, BTreeMap};

use crossbeam::sync::ArcCell;
use futures::{Future, Stream, Async};
use futures::sync::mpsc::UnboundedReceiver;
use rand::{thread_rng};
use rand::seq::sample_iter;
use tk_easyloop::{handle, spawn, interval};
use tokio_core::net::UdpSocket;
use tokio_core::reactor::Interval;
use serde::Deserialize;
use serde_cbor::de::Deserializer;

use {VPath};
use index::{ImageId};
use machine_id::{MachineId};
use mask::Mask;
use named_mutex::Mutex;
use peers::Peer;
use peers::packets::{Packet, Message, PacketRef, MessageRef};
use serde_cbor::ser::to_writer;
use tracking::{Tracking, ShortProgress};

/// Size of the buffer for sending packet
/// TODO(tailhook) it's now absolute maximum for UDP packets, we need to figure
/// out a way to make it smaller (i.e. to guarantee making it smaller)
pub const MAX_GOSSIP_PACKET: usize = 65536;

/// Maximum number of base dirs in single packet
pub const MAX_BASE_DIRS: usize = 10;

/// Interval at which send gossip packets
pub const GOSSIP_INTERVAL: u64 = 1000;

/// Number of packets to send on interval and on new fresh image
pub const PACKETS_AT_ONCE: usize = 4;

pub struct Downloading {
    pub image: ImageId,
    pub mask: Mask,
    pub source: bool,
    pub stalled: bool,
}

pub struct HostData {
    pub downloading: HashMap<VPath, Downloading>,
    pub complete: BTreeMap<VPath, ImageId>,
    pub deleted: HashSet<(VPath, ImageId)>,
}

struct Gossip {
    socket: UdpSocket,
    tracking: Tracking,
    interval: Interval,
    machine_id: MachineId,
    messages: UnboundedReceiver<Message>,
    peers: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    dir_peers: Arc<Mutex<HashMap<VPath, HashSet<MachineId>>>>,
    by_host: Arc<Mutex<HashMap<MachineId, HostData>>>,
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
                    &mut Deserializer::from_reader(&mut buf))
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
                    Message::BaseDirs { in_progress, complete,
                                        deleted, base_dirs }
                    => {
                        for (vpath, hash) in base_dirs {
                            self.dir_peers.lock().entry(vpath.clone())
                                .or_insert_with(HashSet::new)
                                .insert(pkt.machine_id.clone());
                            self.tracking.reconcile_dir(vpath, hash, addr,
                                pkt.machine_id.clone());
                        }
                        match self.peers.get().get(&pkt.machine_id) {
                            Some(peer) => {
                                for (path, image) in &complete {
                                    self.tracking.remote()
                                        .forward_notify_received_image(
                                            &image, &path, peer);
                                }
                            }
                            None => {}
                        }
                        if in_progress.len() == 0 && deleted.len() == 0 {
                            self.by_host.lock().remove(&pkt.machine_id);
                        } else {
                            self.by_host.lock()
                                .insert(pkt.machine_id.clone(), HostData {
                                    deleted,
                                    complete,
                                    downloading:
                                        in_progress.into_iter().map(|(k, v)| {
                                            let (
                                                image, mask, source, stalled
                                            ) = v;
                                            (k, Downloading {
                                                image, mask, source, stalled,
                                            })
                                        }).collect(),
                                });
                        }
                    }
                    Message::Downloading { path, image, mask, source } => {
                        self.by_host.lock()
                            .entry(pkt.machine_id.clone())
                            .or_insert_with(|| HostData {
                                downloading: HashMap::new(),
                                deleted: HashSet::new(),
                                complete: BTreeMap::new(),
                            })
                            .downloading.insert(path, Downloading {
                                image: image,
                                mask: mask,
                                source: source,
                                stalled: false,
                            });
                    }
                    Message::Complete { path, image } => {
                        {
                            let mut hosts = self.by_host.lock();
                            let host = hosts.entry(pkt.machine_id.clone())
                                .or_insert_with(|| HostData {
                                    downloading: HashMap::new(),
                                    deleted: HashSet::new(),
                                    complete: BTreeMap::new(),
                                });
                            host.downloading.remove(&path);
                            host.complete.insert(path.clone(), image.clone());
                        }
                        match self.peers.get().get(&pkt.machine_id) {
                            Some(peer) => {
                                self.tracking.remote()
                                    .forward_notify_received_image(
                                        &image, &path, peer);
                            }
                            None => {}
                        }
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
            let peers = {
                let hint_path = match msg {
                    Message::BaseDirs {..} => unreachable!(),
                    Message::Downloading { ref path, .. } => path,
                    Message::Complete { ref path, .. } => path,
                };
                let all = self.peers.get();
                let mut peers =
                    match self.dir_peers.lock().get(&hint_path.parent()) {
                        Some(peers) => {
                            sample_iter(&mut thread_rng(),
                                peers, PACKETS_AT_ONCE)
                            .unwrap_or_else(|v| v)
                            .into_iter()
                            .filter_map(|id| all.get(id).map(|p| p.addr))
                            .collect()
                        }
                        None => Vec::new(),
                    };
                if peers.len() < PACKETS_AT_ONCE {
                    peers.extend(sample_iter(&mut thread_rng(),
                            all.values(), PACKETS_AT_ONCE)
                        .unwrap_or_else(|v| v)
                        .into_iter().map(|p| p.addr));
                }
                peers
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
        let deleted = self.tracking.get_deleted();
        let complete = self.tracking.get_complete();
        for (addr, _) in &self.future_peers {
            self.send_gossip(*addr, &ipr, &complete, &deleted);
        }
        let lst = self.peers.get();
        let hosts = sample_iter(&mut thread_rng(),
            lst.values(), PACKETS_AT_ONCE)
            .unwrap_or_else(|v| v);
        for host in hosts {
            self.send_gossip(host.addr, &ipr, &complete, &deleted);
        }
    }
    fn send_gossip(&self, addr: SocketAddr,
        in_progress: &BTreeMap<VPath, ShortProgress>,
        complete: &BTreeMap<VPath, ImageId>,
        deleted: &Vec<(VPath, ImageId)>)
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
                in_progress: in_progress.iter()
                    .map(|(k, s)| {
                        (k, (&s.image_id, &s.mask, s.source, s.stalled))
                    })
                    .collect(),
                deleted, complete,
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
    by_host: Arc<Mutex<HashMap<MachineId, HostData>>>,
    dir_peers: Arc<Mutex<HashMap<VPath, HashSet<MachineId>>>>,
    messages: UnboundedReceiver<Message>,
    machine_id: MachineId,
    tracking: &Tracking, future_peers: HashMap<SocketAddr, String>)
    -> Result<(), io::Error>
{
    spawn(Gossip {
        interval: interval(Duration::from_millis(GOSSIP_INTERVAL)),
        socket: UdpSocket::bind(&addr, &handle())?,
        tracking: tracking.clone(),
        peers, machine_id, future_peers, by_host, dir_peers, messages,
    });
    Ok(())
}
