mod cantal;
mod file;
mod gossip;
mod packets;
pub mod config;

use std::collections::{HashMap, BTreeMap, HashSet};
use std::path::{PathBuf};
use std::net::SocketAddr;
use std::sync::Arc;

use crossbeam::sync::ArcCell;
use futures::Future;
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use ns_router::Router;
use tk_easyloop::spawn;

use ciruela::{ImageId, VPath, MachineId};
use config::{Config};
use disk::Disk;
use mask::Mask;
use named_mutex::{Mutex, MutexGuard};
use self::packets::Message;
use tracking::Tracking;
use peers::gossip::Downloading;


#[derive(Debug, Clone)]
pub struct Peer {
    id: MachineId,
    addr: SocketAddr,
    hostname: String,
    name: String,
}

#[derive(Clone)]
pub struct Peers {
    downloading: Arc<Mutex<HashMap<MachineId, BTreeMap<VPath, Downloading>>>>,
    dir_peers: Arc<Mutex<HashMap<VPath, HashSet<MachineId>>>>,
    peers: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    messages: UnboundedSender<Message>,
}

pub struct PeersInit {
    machine_id: MachineId,
    cell: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    peer_file: Option<PathBuf>,
    downloading: Arc<Mutex<HashMap<MachineId, BTreeMap<VPath, Downloading>>>>,
    dir_peers: Arc<Mutex<HashMap<VPath, HashSet<MachineId>>>>,
    messages: UnboundedReceiver<Message>,
}

impl Peers {
    pub fn new(
        machine_id: MachineId,
        peer_file: Option<PathBuf>) -> (Peers, PeersInit)
    {
        let dw = Arc::new(Mutex::new(HashMap::new(), "peers_downloading"));
        let (tx, rx) = unbounded();
        let cell = Arc::new(ArcCell::new(Arc::new(HashMap::new())));
        let dir_peers = Arc::new(Mutex::new(HashMap::new(), "dir_peers"));
        (Peers {
            downloading: dw.clone(),
            messages: tx,
            peers: cell.clone(),
            dir_peers: dir_peers.clone(),
        }, PeersInit {
            cell: cell,
            peer_file: peer_file,
            machine_id,
            downloading: dw,
            dir_peers: dir_peers,
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
    pub fn addrs_by_mask(&self, vpath: &VPath,
        targ_id: &ImageId, targ_mask: Mask)
        -> Vec<SocketAddr>
    {
        let mut result = Vec::new();
        let peers = self.peers.get();
        for (mid, paths) in self.downloading.lock().iter() {
            if let Some(dw) = paths.get(vpath) {
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
        self.dir_peers.lock().get(vpath)
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
        self.dir_peers.lock().get(vpath)
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
    pub fn get_downloading(&self)
        -> MutexGuard<HashMap<MachineId, BTreeMap<VPath, Downloading>>>
    {
        self.downloading.lock()
    }

    // check if this image is stalled across the whole cluster
    // Note: If there are no peers known for this directory, we consider
    // this as this is a sole owner of the directory. In the block fetching
    // code we don't drop the image in at least two minutes, it's expected
    // that two minutes is enought to syncrhonize `dir_peers`.
    pub fn check_stalled(&self, path: &VPath, image: &ImageId) -> bool {
        let needed_peers = self.dir_peers.lock()
            .get(&path.parent()).cloned().unwrap_or_else(HashSet::new);
        let dw = self.downloading.lock();
        let mut stalled = 0;
        let mut sources = 0;
        let mut possibly_okay = 0;
        for pid in &needed_peers {
            if let Some(peer) = dw.get(&pid) {
                if let Some(dir) = peer.get(path) {
                    if &dir.image == image {
                        if dir.stalled {
                            stalled += 1;
                        } else {
                            possibly_okay += 1;
                        }
                        if dir.source {
                            sources += 1;
                        }
                    }
                } else {
                    possibly_okay += 1;
                }
            } else {
                possibly_okay += 1;
            }
        }
        info!("Stale check {} -> {:?}: {}/{} (okay: {}, sources: {})",
            image, path,
            stalled, needed_peers.len(), possibly_okay, sources);
        return stalled >= needed_peers.len() &&
               possibly_okay == 0 && sources == 0;
    }
}

pub fn start(me: PeersInit, addr: SocketAddr,
    config: &Arc<Config>, disk: &Disk,
    router: &Router, tracking: &Tracking)
    -> Result<(), Box<::std::error::Error>>
{
    if let Some(peer_file) = me.peer_file {
        let tracking = tracking.clone();
        let id = me.machine_id.clone();
        let dw = me.downloading.clone();
        let tx = me.messages;
        let dp = me.dir_peers;
        let cell = me.cell;
        spawn(
            file::read_peers(peer_file, disk, router, config.port)
            .and_then(move |fut| {
                gossip::start(addr, cell, dw, dp, tx, id, &tracking, fut)
                    .expect("can start gossip");
                Ok(())
            }));
    } else {
        cantal::spawn_fetcher(&me.cell, config.port);
        gossip::start(addr,
            me.cell, me.downloading, me.dir_peers, me.messages, me.machine_id,
            tracking, HashMap::new())?;
    }
    Ok(())
}
