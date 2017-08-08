mod base_dir;
mod fetch_dir;
mod progress;
mod first_scan;
mod cleanup;

use std::net::SocketAddr;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak, Mutex, MutexGuard};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use futures::{Stream};
use futures::future::Shared;
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use futures::sync::oneshot::{Receiver};
use tk_easyloop;

use ciruela::{ImageId, Hash, VPath};
use config::{Config, Directory};
use disk::Disk;
use index::{Index, IndexData};
use machine_id::MachineId;
use metadata::Meta;
use remote::Remote;

pub use self::progress::Downloading;
pub use self::base_dir::BaseDir;


pub type Block = Arc<Vec<u8>>;
type ImageFuture = Shared<Receiver<Index>>;
type BlockFuture = Shared<Receiver<Block>>;

pub struct State {

    image_futures: HashMap<ImageId, ImageFuture>,
    images: HashMap<ImageId, Weak<IndexData>>,

    block_futures: HashMap<Hash, BlockFuture>,

    in_progress: HashSet<Arc<Downloading>>,

    base_dirs: HashMap<VPath, Arc<BaseDir>>,
    base_dir_list: Vec<Arc<BaseDir>>,
}

#[derive(Clone)]
pub struct Tracking {
    chan: UnboundedSender<Command>,
    state: Arc<Mutex<State>>,
}

#[derive(Clone)]
pub struct Subsystem(Arc<Data>);

// Subsystem data, only here for Deref trait
pub struct Data {
    state: Arc<Mutex<State>>,
    cleanup: UnboundedSender<cleanup::Command>,
    config: Arc<Config>,
    meta: Meta,
    disk: Disk,
    remote: Remote,
    dry_cleanup: AtomicBool,
}

pub struct TrackingInit {
    chan: UnboundedReceiver<Command>,
    tracking: Tracking,
}

pub enum Command {
    FetchDir(Downloading),
}

impl Tracking {
    pub fn new() -> (Tracking, TrackingInit) {
        let (tx, rx) = unbounded();
        let handler = Tracking {
            state: Arc::new(Mutex::new(State {
                image_futures: HashMap::new(),
                images: HashMap::new(),
                block_futures: HashMap::new(),
                in_progress: HashSet::new(),
                base_dirs: HashMap::new(),
                base_dir_list: Vec::new(),
            })),
            chan: tx,
        };
        (handler.clone(),
         TrackingInit {
            chan: rx,
            tracking: handler,
         })
    }
    pub fn fetch_dir(&self, image: &ImageId, vpath: VPath, replace: bool,
                     config: &Arc<Directory>)
    {
        self.send(Command::FetchDir(Downloading {
            replacing: replace,
            virtual_path: vpath,
            image_id: image.clone(),
            config: config.clone(),
            index_fetched: AtomicBool::new(false),
            bytes_fetched: AtomicUsize::new(0),
            bytes_total: AtomicUsize::new(0),
            blocks_fetched: AtomicUsize::new(0),
            blocks_total: AtomicUsize::new(0),
        }));
    }
    pub fn reconcile_dir(&self, path: VPath, hash: Hash,
        peer_addr: SocketAddr, peer_id: MachineId)
    {
        unimplemented!();
    }
    pub fn pick_random_dir(&self) -> Option<(VPath, Hash)> {
        unimplemented!();
    }
    fn send(&self, command: Command) {
        self.chan.send(command).expect("image tracking subsystem is alive")
    }
}

impl ::std::ops::Deref for Subsystem {
    type Target = Data;
    fn deref(&self) -> &Data {
        &self.0
    }
}

impl Subsystem {
    fn state(&self) -> MutexGuard<State> {
        self.state.lock().expect("image tracking subsystem is not poisoned")
    }
    fn start_cleanup(&self) {
        self.cleanup.send(cleanup::Command::Reschedule)
            .expect("can always send in cleanup channel");
    }
    fn undry_cleanup(&self) {
        self.0.dry_cleanup.store(false, Ordering::Relaxed);
    }
    fn dry_cleanup(&self) -> bool {
        self.0.dry_cleanup.load(Ordering::Relaxed)
    }
}

pub fn start(init: TrackingInit, config: &Arc<Config>,
    meta: &Meta, remote: &Remote, disk: &Disk)
    -> Result<(), String> // actually void
{
    let (ctx, crx) = unbounded();
    let TrackingInit { chan, tracking } = init;
    let sys = Subsystem(Arc::new(Data {
        config: config.clone(),
        meta: meta.clone(),
        disk: disk.clone(),
        state: tracking.state.clone(),
        remote: remote.clone(),
        cleanup: ctx,
        dry_cleanup: AtomicBool::new(true),  // start with no cleanup
    }));
    first_scan::spawn_scan(&sys);
    cleanup::spawn_loop(crx, &sys);
    tk_easyloop::spawn(chan
        .for_each(move |command| {
            use self::Command::*;
            match command {
                FetchDir(info) => fetch_dir::start(&sys, info),
            }
            Ok(())
        }));
    Ok(())
}
