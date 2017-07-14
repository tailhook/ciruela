mod base_dir;
mod fetch_dir;
mod progress;
mod first_scan;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak, Mutex, MutexGuard};
use std::sync::atomic::{AtomicBool, AtomicUsize};

use futures::{Stream};
use futures::future::Shared;
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use futures::sync::oneshot::{Receiver};
use tk_easyloop;

use ciruela::{ImageId, Hash};
use config::Directory;
use dir_config::DirConfig;
use disk::Disk;
use index::{Index, IndexData};
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

    base_dirs: HashSet<Arc<BaseDir>>,
}

#[derive(Clone)]
pub struct Tracking {
    chan: UnboundedSender<Command>,
    state: Arc<Mutex<State>>,
}

#[derive(Clone)]
pub struct Subsystem {
    state: Arc<Mutex<State>>,
    meta: Meta,
    disk: Disk,
    remote: Remote,
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
                base_dirs: HashSet::new(),
            })),
            chan: tx,
        };
        (handler.clone(),
         TrackingInit {
            chan: rx,
            tracking: handler,
         })
    }
    pub fn fetch_dir(&self, image: &ImageId, cfg: DirConfig) {
        self.send(Command::FetchDir(Downloading {
            virtual_path: cfg.virtual_path.to_path_buf(),
            image_id: image.clone(),
            base_dir: cfg.base.to_path_buf(),
            parent: cfg.parent.to_path_buf(),
            image_name: cfg.image_name.to_string(),
            config: cfg.config.clone(),
            index_fetched: AtomicBool::new(false),
            bytes_fetched: AtomicUsize::new(0),
            bytes_total: AtomicUsize::new(0),
            blocks_fetched: AtomicUsize::new(0),
            blocks_total: AtomicUsize::new(0),
        }));
    }
    fn send(&self, command: Command) {
        self.chan.send(command).expect("image tracking subsystem is alive")
    }
}

impl Subsystem {
    fn state(&self) -> MutexGuard<State> {
        self.state.lock().expect("image tracking subsystem is not poisoned")
    }
}

pub fn start(init: TrackingInit, meta: &Meta, remote: &Remote, disk: &Disk)
    -> Result<(), String> // actually void
{
    let TrackingInit { chan, tracking } = init;
    let sys = Subsystem {
        meta: meta.clone(),
        disk: disk.clone(),
        state: tracking.state.clone(),
        remote: remote.clone(),
    };
    first_scan::spawn_scan(&sys);
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
