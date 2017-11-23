mod cantal;
mod file;
mod gossip;
mod packets;
pub mod config;

use std::collections::{HashMap, HashSet};
use std::path::{PathBuf};
use std::net::SocketAddr;
use std::sync::Arc;

use crossbeam::sync::ArcCell;
use futures::Future;
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use ns_router::Router;
use tk_easyloop::spawn;

use index::{ImageId};
use {VPath};
use machine_id::{MachineId};
use config::{Config};
use disk::Disk;
use mask::Mask;
use named_mutex::{Mutex, MutexGuard};
use self::packets::Message;
use tracking::Tracking;
use peers::gossip::{HostData};


#[derive(Debug, Clone)]
pub struct Peer {
    id: MachineId,
    addr: SocketAddr,
    hostname: String,
    name: String,
}

#[derive(Clone)]
pub struct Peers {
    by_host: Arc<Mutex<HashMap<MachineId, HostData>>>,
    dir_peers: Arc<Mutex<HashMap<VPath, HashSet<MachineId>>>>,
    peers: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    messages: UnboundedSender<Message>,
}

pub struct PeersInit {
    machine_id: MachineId,
    cell: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    peer_file: Option<PathBuf>,
    by_host: Arc<Mutex<HashMap<MachineId, HostData>>>,
    dir_peers: Arc<Mutex<HashMap<VPath, HashSet<MachineId>>>>,
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
        let dir_peers = Arc::new(Mutex::new(HashMap::new(), "dir_peers"));
        (Peers {
            by_host: by_host.clone(),
            messages: tx,
            peers: cell.clone(),
            dir_peers: dir_peers.clone(),
        }, PeersInit {
            cell: cell,
            peer_file: peer_file,
            machine_id,
            by_host: by_host,
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
        for (mid, host) in self.by_host.lock().iter() {
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
    pub fn get_host_data(&self)
        -> MutexGuard<HashMap<MachineId, HostData>>
    {
        self.by_host.lock()
    }

    // check if this image is stalled across the whole cluster
    // Note: If there are no peers known for this directory, we consider
    // this as this is a sole owner of the directory. In the block fetching
    // code we don't drop the image in at least two minutes, it's expected
    // that two minutes is enought to syncrhonize `dir_peers`.
    pub fn check_stalled(&self, path: &VPath, image: &ImageId) -> bool {
        let needed_peers = self.dir_peers.lock()
            .get(&path.parent()).cloned().unwrap_or_else(HashSet::new);
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
    if let Some(peer_file) = me.peer_file {
        let tracking = tracking.clone();
        let id = me.machine_id.clone();
        let dw = me.by_host.clone();
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
            me.cell, me.by_host, me.dir_peers, me.messages, me.machine_id,
            tracking, HashMap::new())?;
    }
    Ok(())
}
