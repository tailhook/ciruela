mod base_dir;
mod fetch_dir;
mod progress;
mod cleanup;
mod fetch_index;
mod reconciliation;
mod rpc;

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
use ciruela::proto::{AppendDir, AppendDirAck};
use config::{Config, Directory};
use disk::Disk;
use index::{IndexData};
use machine_id::MachineId;
use metadata::{Meta, Error as MetaError};
use rand::{thread_rng, Rng};
use remote::Remote;
use tracking::reconciliation::ReconPush;

pub use self::fetch_index::Index;
pub use self::progress::Downloading;
pub use self::base_dir::BaseDir;


pub type Block = Arc<Vec<u8>>;
type ImageFuture = Shared<Receiver<Index>>;
type BlockFuture = Shared<Receiver<Block>>;

pub struct State {
    block_futures: HashMap<Hash, BlockFuture>,

    in_progress: HashSet<Arc<Downloading>>,

    base_dirs: HashMap<VPath, Arc<BaseDir>>,
    base_dir_list: Vec<Arc<BaseDir>>,
    reconciling: HashMap<(VPath, Hash), HashSet<(SocketAddr, MachineId)>>,
}

#[derive(Clone)]
pub struct Tracking(Arc<Inner>);

struct Inner {
    images: fetch_index::Indexes,
    config: Arc<Config>,
    cmd_chan: UnboundedSender<Command>,
    rescan_chan: UnboundedSender<(VPath, Instant)>,
    state: Arc<Mutex<State>>,
    meta: Meta,
    disk: Disk,
    remote: Remote,
}

#[derive(Clone)]
pub struct Subsystem(Arc<Data>);

// Subsystem data, only here for Deref trait
pub struct Data {
    images: fetch_index::Indexes,
    state: Arc<Mutex<State>>,
    cleanup: UnboundedSender<cleanup::Command>,
    scan: UnboundedSender<(VPath, Instant)>,
    config: Arc<Config>,
    meta: Meta,
    disk: Disk,
    remote: Remote,
    tracking: Tracking,
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
    pub fn new(config: &Arc<Config>, meta: &Meta, disk: &Disk,
        remote: &Remote)
        -> (Tracking, TrackingInit)
    {
        let (ctx, crx) = unbounded();
        let (rtx, rrx) = unbounded();
        let handler = Tracking(Arc::new(Inner {
            config: config.clone(),
            images: fetch_index::Indexes::new(meta, remote),
            state: Arc::new(Mutex::new(State {
                block_futures: HashMap::new(),
                in_progress: HashSet::new(),
                base_dirs: HashMap::new(),
                base_dir_list: Vec::new(),
                reconciling: HashMap::new(),
            })),
            meta: meta.clone(),
            disk: disk.clone(),
            remote: remote.clone(),
            cmd_chan: ctx,
            rescan_chan: rtx,
        }));
        (handler.clone(),
         TrackingInit {
            cmd_chan: crx,
            rescan_chan: rrx,
            tracking: handler,
         })
    }
    pub fn reconcile_dir(&self, path: VPath, hash: Hash,
        peer_addr: SocketAddr, peer_id: MachineId)
    {
        if self.0.config.dirs.get(path.key()).is_none() {
            return;
        }
        let mut state = self.state();
        if state.base_dirs
           .get(&path).map(|x| !x.is_superset_of(hash))
           .unwrap_or(true)
        {
            let pair = (path, hash);
            if state.reconciling.contains_key(&pair) {
                trace!("Already reconciling {:?}", pair);
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
        self.0.cmd_chan.send(command)
            .expect("image tracking subsystem is alive")
    }
    fn state(&self) -> MutexGuard<State> {
        self.0.state.lock()
            .expect("image tracking subsystem is not poisoned")
    }
    pub fn scan_dir(&self, path: VPath) {
        self.0.rescan_chan.send((path, Instant::now()))
            .expect("rescan thread is alive");
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
        self.0.scan.send((path, Instant::now()))
            .expect("can always send in rescan channel");
    }
}

pub fn start(init: TrackingInit, disk: &Disk)
    -> Result<(), String> // actually void
{
    let (ctx, crx) = unbounded();
    let TrackingInit { cmd_chan, rescan_chan, tracking } = init;
    let sys = Subsystem(Arc::new(Data {
        images: tracking.0.images.clone(),
        config: tracking.0.config.clone(),
        meta: tracking.0.meta.clone(),
        disk: disk.clone(),
        state: tracking.0.state.clone(),
        remote: tracking.0.remote.clone(),
        tracking: tracking.clone(),
        cleanup: ctx,
        scan: tracking.0.rescan_chan.clone(),
        dry_cleanup: AtomicBool::new(true),  // start with no cleanup
    }));
    let sys2 = sys.clone();
    let sys3 = sys.clone();
    let sys4 = sys.clone();
    let meta = tracking.0.meta.clone();
    let trk1 = tracking.clone();

    spawn(meta.scan_base_dirs(move |dir| trk1.scan_dir(dir))
        .map(move |()| sys4.start_cleanup())
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
                .expect("only scan configured dirs")
                .clone();
            let sys = sys.clone();
            let scan_time = Instant::now();
            Either::B(
                ::base_dir::scan(&path, &config, &meta, &sys.disk)
                .then(move |res| match res {
                    Ok(bdir) => {
                        BaseDir::commit_scan(bdir, &config, scan_time, &sys);
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
