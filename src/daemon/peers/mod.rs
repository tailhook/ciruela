mod cantal;
mod file;
mod gossip;
mod packets;
pub mod config;

use std::collections::{HashMap, BTreeMap};
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
    messages: UnboundedSender<Message>,
}

pub struct PeersInit {
    machine_id: MachineId,
    peer_file: Option<PathBuf>,
    downloading: Arc<Mutex<
        HashMap<MachineId, BTreeMap<VPath, (ImageId, Mask)>>>>,
    messages: UnboundedReceiver<Message>,
}

impl Peers {
    pub fn new(
        machine_id: MachineId,
        peer_file: Option<PathBuf>) -> (Peers, PeersInit)
    {
        let dw = Arc::new(Mutex::new(HashMap::new(), "peers_downloading"));
        let (tx, rx) = unbounded();
        (Peers {
            downloading: dw.clone(),
            messages: tx,
        }, PeersInit {
            peer_file: peer_file,
            machine_id,
            downloading: dw,
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
}

pub fn start(me: PeersInit, addr: SocketAddr,
    config: &Arc<Config>, disk: &Disk,
    router: &Router, tracking: &Tracking)
    -> Result<(), Box<::std::error::Error>>
{
    let cell = Arc::new(ArcCell::new(Arc::new(HashMap::new())));
    if let Some(peer_file) = me.peer_file {
        let tracking = tracking.clone();
        let id = me.machine_id.clone();
        let dw = me.downloading.clone();
        let tx = me.messages;
        spawn(
            file::read_peers(peer_file, disk, router, config.port)
            .and_then(move |fut| {
                gossip::start(addr, cell, dw, tx, id, &tracking, fut)
                    .expect("can start gossip");
                Ok(())
            }));
    } else {
        cantal::spawn_fetcher(&cell, config.port);
        gossip::start(addr, cell, me.downloading, me.messages, me.machine_id,
            tracking, HashMap::new())?;
    }
    Ok(())
}
