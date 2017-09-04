mod cantal;
mod file;
mod gossip;
mod packets;
pub mod config;

use std::collections::{HashMap, BTreeMap, HashSet};
use std::path::{PathBuf};
use std::net::SocketAddr;
use std::sync::Arc;

use abstract_ns::{Router};
use crossbeam::sync::ArcCell;
use futures::Future;
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use tk_easyloop::spawn;

use ciruela::{ImageId, VPath};
use config::{Config};
use disk::Disk;
use machine_id::MachineId;
use mask::Mask;
use named_mutex::Mutex;
use self::packets::Message;
use tracking::Tracking;


#[derive(Debug, Clone)]
pub struct Peer {
    id: MachineId,
    addr: SocketAddr,
    hostname: String,
    name: String,
}

#[derive(Clone)]
pub struct Peers {
    downloading: Arc<Mutex<
        HashMap<MachineId, BTreeMap<VPath, (ImageId, Mask)>>>>,
    dir_peers: Arc<Mutex<HashMap<VPath, HashSet<MachineId>>>>,
    peers: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    messages: UnboundedSender<Message>,
}

pub struct PeersInit {
    machine_id: MachineId,
    cell: Arc<ArcCell<HashMap<MachineId, Peer>>>,
    peer_file: Option<PathBuf>,
    downloading: Arc<Mutex<
        HashMap<MachineId, BTreeMap<VPath, (ImageId, Mask)>>>>,
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
    pub fn notify_progress(&self, path: &VPath, image_id: &ImageId, mask: Mask)
    {
        self.messages.unbounded_send(Message::Downloading {
            path: path.clone(),
            image: image_id.clone(),
            mask: mask,
        }).expect("gossip subsystem crashed");
    }
    pub fn addrs_by_mask(&self, vpath: &VPath,
        targ_id: &ImageId, targ_mask: Mask)
        -> Vec<SocketAddr>
    {
        let mut result = Vec::new();
        let peers = self.peers.get();
        for (mid, paths) in self.downloading.lock().iter() {
            if let Some(&(ref id, ref mask)) = paths.get(vpath) {
                if id == targ_id && mask.is_superset_of(targ_mask) {
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
