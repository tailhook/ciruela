use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::collections::{HashSet, HashMap, BTreeMap, BTreeSet};

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
use config::Config;
use index::{ImageId};
use machine_id::{MachineId};
use mask::Mask;
use named_mutex::Mutex;
use peers::Peer;
use peers::packets::{Packet, Message, PacketRef, MessageRef};
use peers::two_way_map::ConfigMap;
use proto::Hash;
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
    pub watching: BTreeSet<VPath>,
    pub deleted: HashSet<(VPath, ImageId)>,
}

struct Gossip {
    socket: UdpSocket,
    tracking: Tracking,
    interval: Interval,
    machine_id: MachineId,
    config: Arc<Config>,
    config_hash: Hash,
    config_list: BTreeSet<VPath>,
    messages: UnboundedReceiver<Message>,
    peers: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    by_host: Arc<Mutex<HashMap<MachineId, HostData>>>,
    configs: Arc<Mutex<ConfigMap>>,
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
                if pkt.your_config != Some(self.config_hash) {
                    self.send_config(addr, &pkt.machine_id);
                }
                match pkt.message {
                    Message::BaseDirs { in_progress, watching, complete,
                                        deleted, base_dirs }
                    => {
                        for (vpath, hash) in base_dirs {
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
                        self.tracking.check_watched(&watching);
                        if in_progress.len() == 0 &&
                           deleted.len() == 0 &&
                           watching.len() == 0 &&
                           complete.len() == 0
                        {
                            self.by_host.lock().remove(&pkt.machine_id);
                        } else {
                            self.by_host.lock()
                                .insert(pkt.machine_id.clone(), HostData {
                                    deleted,
                                    complete,
                                    watching,
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
                                watching: BTreeSet::new(),
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
                                    watching: BTreeSet::new(),
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
                    Message::ConfigSync { paths } => {
                        self.configs.lock().set(pkt.machine_id, paths)
                    }
                    Message::Reconcile { path, hash, watches } => {
                        self.tracking.reconcile_dir(path, hash, addr,
                            pkt.machine_id.clone());
                        {
                            let mut hosts = self.by_host.lock();
                            for (path, ids) in watches {
                                for mid in ids {
                                    hosts.entry(mid)
                                        .or_insert_with(|| HostData {
                                            downloading: HashMap::new(),
                                            deleted: HashSet::new(),
                                            watching: BTreeSet::new(),
                                            complete: BTreeMap::new(),
                                        })
                                        .watching
                                        .insert(path.clone());
                                }
                            }
                        }
                    }
                }
            }
            if !self.socket.poll_read().is_ready() {
                break;
            }
        }
    }
    fn dest_for_packet(&self, msg: &Message) -> Vec<SocketAddr> {
        match *msg {
            Message::BaseDirs {..} => unreachable!(),
            Message::ConfigSync { .. } => unreachable!(),
            Message::Reconcile { ref path, .. } |
            Message::Downloading { ref path, .. } |
            Message::Complete { ref path, .. } => {
                let all = self.peers.get();
                let mut peers =
                    match self.configs.lock().by_dir(&path.key_vpath()) {
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
                return peers;
            }
        }
    }
    fn fill_msg(&mut self, msg: &mut Message) {
        match *msg {
            Message::Reconcile {ref path,  ref mut watches, .. } => {
                for (mid, host) in self.by_host.lock().iter() {
                    for hpath in &host.watching {
                        if &path.parent() == hpath {
                            watches.entry(hpath.clone())
                                .or_insert_with(HashSet::new)
                                .insert(mid.clone());
                        }
                    }
                }
                for mypath in self.tracking.get_watching() {
                    if &mypath.parent() == path {
                        watches.entry(mypath.clone())
                            .or_insert_with(HashSet::new)
                            .insert(self.machine_id.clone());
                    }
                }
            }
            _ => {}
        }
    }
    fn write_messages(&mut self) {
        while let Ok(Async::Ready(Some(mut msg))) = self.messages.poll() {
            let peers = self.dest_for_packet(&msg);
            self.fill_msg(&mut msg);
            let packet = Packet {
                machine_id: self.machine_id.clone(),
                your_config: Some(self.config_hash.clone()),
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
        let watching = self.tracking.get_watching();
        for (addr, _) in &self.future_peers {
            self.send_gossip(*addr, None, &ipr, &complete, &watching, &deleted);
        }
        let lst = self.peers.get();
        let hosts = sample_iter(&mut thread_rng(),
            lst.iter(), PACKETS_AT_ONCE)
            .unwrap_or_else(|v| v);
        for (id, host) in hosts {
            self.send_gossip(host.addr, Some(id), &ipr, &complete, &watching, &deleted);
        }
    }
    fn send_gossip(&self, addr: SocketAddr, id: Option<&MachineId>,
        in_progress: &BTreeMap<VPath, ShortProgress>,
        complete: &BTreeMap<VPath, ImageId>,
        watching: &BTreeSet<VPath>,
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
        let hash = id
            .and_then(|id| self.configs.lock().get(&id).map(|x| x.hash));
        let packet = PacketRef {
            machine_id: &self.machine_id,
            your_config: &hash,
            message: MessageRef::BaseDirs {
                in_progress: in_progress.iter()
                    .map(|(k, s)| {
                        (k, (&s.image_id, &s.mask, s.source, s.stalled))
                    })
                    .collect(),
                deleted, complete, watching,
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
    fn send_config(&self, addr: SocketAddr, id: &MachineId) {
        let mut buf = Vec::with_capacity(1400);

        let hash = self.configs.lock().get(id).map(|x| x.hash);
        let packet = PacketRef {
            machine_id: &self.machine_id,
            your_config: &hash,
            message: MessageRef::ConfigSync { paths: &self.config_list },
        };
        to_writer(&mut buf, &packet).expect("can serialize packet");
        debug!("Sending config to {} [{}]", addr, buf.len());
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
    configs: Arc<Mutex<ConfigMap>>,
    messages: UnboundedReceiver<Message>,
    machine_id: MachineId,
    config: Arc<Config>,
    tracking: &Tracking, future_peers: HashMap<SocketAddr, String>)
    -> Result<(), io::Error>
{
    let config_list = config.dirs.keys()
        .map(|x| VPath::from(format!("/{}", x)))
        .collect();
    let config_hash = Hash::for_object(&config_list);
    spawn(Gossip {
        interval: interval(Duration::from_millis(GOSSIP_INTERVAL)),
        socket: UdpSocket::bind(&addr, &handle())?,
        tracking: tracking.clone(),
        peers, machine_id, future_peers, by_host, messages,
        config, config_list, config_hash, configs,
    });
    Ok(())
}
