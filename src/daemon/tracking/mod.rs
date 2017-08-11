mod base_dir;
mod fetch_dir;
mod progress;
mod cleanup;
mod reconciliation;

use std::net::SocketAddr;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak, Mutex, MutexGuard};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Instant, Duration};

use futures::{Future, Stream};
use futures::future::{Shared, Either, ok};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use futures::sync::oneshot::{Receiver};
use tk_easyloop::{spawn, timeout};

use ciruela::{ImageId, Hash, VPath};
use config::{Config, Directory};
use disk::Disk;
use index::{Index, IndexData};
use machine_id::MachineId;
use metadata::Meta;
use rand::{thread_rng, Rng};
use remote::Remote;
use self::reconciliation::ReconPush;

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
    reconciling: HashMap<(VPath, Hash), HashSet<(SocketAddr, MachineId)>>,
}

#[derive(Clone)]
pub struct Tracking {
    config: Arc<Config>,
    cmd_chan: UnboundedSender<Command>,
    rescan_chan: UnboundedSender<(VPath, Instant)>,
    state: Arc<Mutex<State>>,
}

#[derive(Clone)]
pub struct Subsystem(Arc<Data>);

// Subsystem data, only here for Deref trait
pub struct Data {
    state: Arc<Mutex<State>>,
    cleanup: UnboundedSender<cleanup::Command>,
    scan: UnboundedSender<(VPath, Instant)>,
    config: Arc<Config>,
    meta: Meta,
    disk: Disk,
    remote: Remote,
    dry_cleanup: AtomicBool,
}

pub struct TrackingInit {
    cmd_chan: UnboundedReceiver<Command>,
    rescan_chan: UnboundedReceiver<(VPath, Instant)>,
    tracking: Tracking,
}

pub enum Command {
    FetchDir(Downloading),
    Reconcile(ReconPush),
}

impl Tracking {
    pub fn new(config: &Arc<Config>) -> (Tracking, TrackingInit) {
        let (ctx, crx) = unbounded();
        let (rtx, rrx) = unbounded();
        let handler = Tracking {
            config: config.clone(),
            state: Arc::new(Mutex::new(State {
                image_futures: HashMap::new(),
                images: HashMap::new(),
                block_futures: HashMap::new(),
                in_progress: HashSet::new(),
                base_dirs: HashMap::new(),
                base_dir_list: Vec::new(),
                reconciling: HashMap::new(),
            })),
            cmd_chan: ctx,
            rescan_chan: rtx,
        };
        (handler.clone(),
         TrackingInit {
            cmd_chan: crx,
            rescan_chan: rrx,
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
        if self.config.dirs.get(path.key()).is_none() {
            return;
        }
        let mut state = self.state();
        if state.base_dirs
           .get(&path).map(|x| !x.is_superset_of(hash))
           .unwrap_or(true)
        {
            let pair = (path, hash);
            if state.reconciling.contains_key(&pair) {
                debug!("Already reconciling {:?}", pair);
                state.reconciling.get_mut(&pair).unwrap()
                    .insert((peer_addr, peer_id));
            } else {
                state.reconciling.insert(pair.clone(), {
                    let mut hash = HashSet::new();
                    hash.insert((peer_addr, peer_id.clone()));
                    hash
                });
                self.send(Command::Reconcile(ReconPush {
                    path: pair.0,
                    hash: pair.1,
                    initial_addr: peer_addr,
                    initial_machine_id: peer_id,
                }));
            }
        }
    }
    pub fn pick_random_dir(&self) -> Option<(VPath, Hash)> {
        let state = self.state();
        thread_rng().choose(&state.base_dir_list)
            .map(|d| (d.path.clone(), d.hash()))
    }
    fn send(&self, command: Command) {
        self.cmd_chan.send(command)
            .expect("image tracking subsystem is alive")
    }
    fn state(&self) -> MutexGuard<State> {
        self.state.lock().expect("image tracking subsystem is not poisoned")
    }
    pub fn scan_dir(&self, path: VPath) {
        self.rescan_chan.send((path, Instant::now()));
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
        info!("Okay, now real cleanup routines will start");
        self.0.dry_cleanup.store(false, Ordering::Relaxed);
    }
    fn dry_cleanup(&self) -> bool {
        self.0.dry_cleanup.load(Ordering::Relaxed)
    }
    fn rescan_dir(&self, path: VPath) {
        self.0.scan.send((path, Instant::now()));
    }
}

pub fn start(init: TrackingInit, meta: &Meta, remote: &Remote, disk: &Disk)
    -> Result<(), String> // actually void
{
    let (ctx, crx) = unbounded();
    let TrackingInit { cmd_chan, rescan_chan, tracking } = init;
    let sys = Subsystem(Arc::new(Data {
        config: tracking.config.clone(),
        meta: meta.clone(),
        disk: disk.clone(),
        state: tracking.state.clone(),
        remote: remote.clone(),
        cleanup: ctx,
        scan: tracking.rescan_chan.clone(),
        dry_cleanup: AtomicBool::new(true),  // start with no cleanup
    }));
    let sys2 = sys.clone();
    let sys3 = sys.clone();
    let meta = meta.clone();

    spawn(meta.scan_base_dirs(&tracking)
        .map_err(|e| error!("Error during first scan: {}", e)));
    cleanup::spawn_loop(crx, &sys);
    spawn(cmd_chan
        .for_each(move |command| {
            use self::Command::*;
            match command {
                FetchDir(info) => fetch_dir::start(&sys2, info),
                Reconcile(info) => reconciliation::start(&sys2, info),
            }
            Ok(())
        }));
    spawn(rescan_chan
        .for_each(move |(path, time): (VPath, Instant)| {
            if sys.state().base_dirs.get(&path)
                .map(|x| x.last_scan() > time).unwrap_or(false)
            {
                // race condition between scan and enqueue. just skip it
                return Either::A(ok(()))
            }
            let config = sys.config.dirs.get(path.key())
                .expect("only scan configured dirs");
            let sys = sys.clone();
            let scan_time = Instant::now();
            Either::B(
                meta.scan_dir(&path).map_err(boxerr)
                .join(sys.disk.read_keep_list(config).map_err(boxerr))
                .then(move |res| match res {
                    Ok((states, keep_list)) => {
                        let kl = keep_list.into_iter()
                            .filter_map(|p| {
                                p.strip_prefix(path.suffix()).ok()
                                .and_then(|name| name.to_str())
                                .map(|name| {
                                    assert!(name.find('/').is_none());
                                    name.to_string()
                                })
                            }).collect();
                        BaseDir::commit_scan(path, states, kl,
                            scan_time, &sys);
                        Ok(())
                    }
                    Err(e) => {
                        error!("Scaning {:?} failed: {}", path, e);
                        Ok(())
                    }
                }))
        }));
    spawn(timeout(Duration::new(30, 0))
        .map(move |()| sys3.undry_cleanup())
        .map_err(|_| unreachable!()));
    Ok(())
}

fn boxerr<E: ::std::error::Error + Send + 'static>(e: E)
    -> Box<::std::error::Error + Send>
{
    Box::new(e) as Box<::std::error::Error + Send>
}
