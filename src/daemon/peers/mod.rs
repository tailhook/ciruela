mod cantal;
mod file;
mod gossip;
mod packets;
mod two_way_map;
pub mod config;

use std::collections::{HashMap, HashSet, BTreeMap};
use std::path::{PathBuf};
use std::net::SocketAddr;
use std::sync::Arc;

use crossbeam::sync::ArcCell;
use futures::Future;
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use ns_router::Router;
use tk_easyloop::spawn;

use config::{Config};
use disk::Disk;
use index::{ImageId};
use machine_id::{MachineId};
use mask::Mask;
use metrics::{List, Metric, Integer};
use named_mutex::{Mutex, MutexGuard};
use peers::gossip::{HostData};
use peers::two_way_map::ConfigMap;
use proto::Hash;
use self::packets::Message;
use tracking::Tracking;
use {VPath};


lazy_static! {
    static ref PEERS: Integer = Integer::new();
}

#[derive(Debug, Clone, Serialize)]
pub struct Peer {
    pub id: MachineId,
    pub addr: SocketAddr,
    pub hostname: String,
    pub name: String,
}

#[derive(Clone)]
pub struct Peers {
    by_host: Arc<Mutex<HashMap<MachineId, HostData>>>,
    configs: Arc<Mutex<ConfigMap>>,
    peers: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    messages: UnboundedSender<Message>,
}

pub struct PeersInit {
    machine_id: MachineId,
    cell: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    peer_file: Option<PathBuf>,
    by_host: Arc<Mutex<HashMap<MachineId, HostData>>>,
    configs: Arc<Mutex<ConfigMap>>,
    messages: UnboundedReceiver<Message>,
}

impl Peers {
    pub fn new(
        machine_id: MachineId,
        peer_file: Option<PathBuf>) -> (Peers, PeersInit)
    {
        let by_host = Arc::new(Mutex::new(HashMap::new(),
            "peers_host_data"));
        let (tx, rx) = unbounded();
        let cell = Arc::new(ArcCell::new(Arc::new(HashMap::new())));
        let configs = Arc::new(Mutex::new(ConfigMap::new(), "dir_peers"));
        (Peers {
            by_host: by_host.clone(),
            messages: tx,
            peers: cell.clone(),
            configs: configs.clone(),
        }, PeersInit {
            cell, peer_file, by_host, configs,
            machine_id,
            messages: rx,
        })
    }
    pub fn notify_progress(&self, path: &VPath,
        image_id: &ImageId, mask: Mask, source: bool)
    {
        self.messages.unbounded_send(Message::Downloading {
            path: path.clone(),
            image: image_id.clone(),
            mask, source,
        }).expect("gossip subsystem crashed");
    }
    pub fn notify_basedir(&self, path: &VPath, hash: &Hash) {
        self.messages.unbounded_send(Message::Reconcile {
            path: path.clone(),
            hash: hash.clone(),
            watches: BTreeMap::new(), // will fill it in in the gossip
        }).expect("gossip subsystem crashed");
    }
    pub fn notify_complete(&self, path: &VPath, image_id: &ImageId) {
        self.messages.unbounded_send(Message::Complete {
            path: path.clone(),
            image: image_id.clone(),
        }).expect("gossip subsystem crashed");
    }
    pub fn addrs_by_mask(&self, vpath: &VPath,
        targ_id: &ImageId, targ_mask: Mask)
        -> Vec<SocketAddr>
    {
        let mut result = Vec::new();
        let peers = self.peers.get();
        for (mid, host) in self.by_host.lock().iter() {
            // First return complete, they are less loaded probably
            // But they are less likely to contain image in FS cache, so...
            // TODO(tailhook) use another check when premature notification
            // is fixed
            if let Some(img) = host.complete.get(vpath) {
                if img == targ_id {
                    if let Some(peer) = peers.get(mid) {
                        result.push(peer.addr)
                    }
                }
            }
            // first return then partially downloaded
            if let Some(dw) = host.downloading.get(vpath) {
                if &dw.image == targ_id && dw.mask.is_superset_of(targ_mask) {
                    if let Some(peer) = peers.get(mid) {
                        result.push(peer.addr)
                    }
                }
            }
        }
        return result;
    }
    pub fn addrs_by_basedir(&self, vpath: &VPath)
        -> Vec<SocketAddr>
    {
        self.configs.lock().by_dir(&vpath.key_vpath())
        .map(|ids| {
            let peers = self.peers.get();
            ids.iter()
                .filter_map(|id| peers.get(id).map(|x| x.addr))
                .collect()
        })
        .unwrap_or_else(Vec::new)
    }
    pub fn servers_by_basedir(&self, vpath: &VPath)
        -> HashMap<MachineId, String>
    {
        self.configs.lock().by_dir(&vpath.key_vpath())
        .map(|ids| {
            let peers = self.peers.get();
            ids.iter()
                .filter_map(|id| peers.get(id)
                    .map(|x| (id.clone(), x.name.clone())))
                .collect()
        })
        .unwrap_or_else(HashMap::new)
    }
    // this is only for calling from UI, probably not safe in different threads
    pub fn get_host_data(&self)
        -> MutexGuard<HashMap<MachineId, HostData>>
    {
        self.by_host.lock()
    }

    // this is only for calling from UI, probably not safe in different threads
    pub fn get_configs(&self)
        -> MutexGuard<ConfigMap>
    {
        self.configs.lock()
    }
    pub fn get_peers(&self) -> Arc<HashMap<MachineId, Peer>> {
        self.peers.get()
    }

    // this is only for calling from UI, probably not safe in different threads
    pub fn get_all_watching(&self) -> HashSet<VPath> {
        let mut result = HashSet::new();
        for host in self.by_host.lock().values() {
            result.extend(host.downloading.keys().cloned());
            result.extend(host.watching.iter().cloned());
        }
        result
    }

    // check if this image is stalled across the whole cluster
    // Note: If there are no peers known for this directory, we consider
    // this as this is a sole owner of the directory. In the block fetching
    // code we don't drop the image in at least two minutes, it's expected
    // that two minutes is enought to syncrhonize `dir_peers`.
    pub fn check_stalled(&self, path: &VPath, image: &ImageId) -> bool {
        let needed_peers = self.configs.lock()
            .by_dir(&path.key_vpath()).cloned().unwrap_or_else(HashSet::new);
        let hdata = self.by_host.lock();
        let mut stalled = 0;
        let mut deleted = 0;
        let mut sources = 0;
        let mut possibly_okay = 0;
        let mut wrong_image = 0;
        let pair = (path.clone(), image.clone());
        for pid in &needed_peers {
            if let Some(peer) = hdata.get(&pid) {
                if let Some(dir) = peer.downloading.get(path) {
                    if &dir.image == image {
                        if dir.stalled {
                            stalled += 1;
                        } else {
                            possibly_okay += 1;
                        }
                        if dir.source {
                            sources += 1;
                        }
                    } else {
                        wrong_image += 1;
                    }
                } else if peer.deleted.contains(&pair) {
                    deleted += 1;
                } else {
                    possibly_okay += 1;
                }
            } else {
                possibly_okay += 1;
            }
        }
        info!("Stale check {} -> {:?}: {}+{}/{} \
            (okay: {}, sources: {}, wrong: {})",
            image, path,
            stalled, deleted, needed_peers.len(),
            possibly_okay, sources, wrong_image);
        return stalled+deleted >= needed_peers.len() &&
               possibly_okay == 0 && sources == 0;
    }
}

pub fn start(me: PeersInit, addr: SocketAddr,
    config: &Arc<Config>, disk: &Disk,
    router: &Router, tracking: &Tracking)
    -> Result<(), Box<::std::error::Error>>
{
    let config = config.clone();
    if let Some(peer_file) = me.peer_file {
        let tracking = tracking.clone();
        let id = me.machine_id.clone();
        let dw = me.by_host.clone();
        let tx = me.messages;
        let cfgs = me.configs;
        let cell = me.cell;
        spawn(
            file::read_peers(peer_file, disk, router, config.port)
            .and_then(move |fut| {
                gossip::start(addr, cell, dw, cfgs, tx, id,
                    config, &tracking, fut)
                    .expect("can start gossip");
                Ok(())
            }));
    } else {
        cantal::spawn_fetcher(&me.cell, config.port);
        gossip::start(addr,
            me.cell, me.by_host, me.configs, me.messages, me.machine_id,
            config, tracking, HashMap::new())?;
    }
    Ok(())
}

pub fn metrics() -> List {
    let gossip = "gossip";
    vec![
        (Metric(gossip, "known_peers"), &*PEERS),
    ]
}
