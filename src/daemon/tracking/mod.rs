mod base_dir;
mod cleanup;
mod fetch_blocks;
mod fetch_dir;
mod fetch_index;
mod progress;
mod reconciliation;
mod rpc;

use std::collections::{HashMap, HashSet, BTreeMap};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc};
use std::time::{Instant, Duration};

use futures::{Future, Stream};
use futures::future::{Either, ok};
use futures::stream::iter_ok;
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use tk_easyloop::{spawn, timeout};

use proto::{AppendDir, Hash};
use id::{ImageId};
use virtual_path::{VPath};
use machine_id::{MachineId};
use config::{Config};
use disk::Disk;
use failure_tracker::{Failures, Policy};
use mask::{AtomicMask, Mask};
use metadata::{Meta};
use named_mutex::{Mutex, MutexGuard};
use peers::Peers;
use rand::{thread_rng, Rng};
use remote::{Remote, Connection};
use tracking::reconciliation::ReconPush;

pub use self::fetch_index::Index;
pub use self::progress::{Downloading, Slices};
pub use self::base_dir::BaseDir;

const DELETED_RETENTION: u64 = 300_000;  // 5 min
const AVOID_DOWNLOAD: u64 = 10_000;  // do not try delete image again in 2 min


pub type BlockData = Arc<Vec<u8>>;

pub struct State {
    in_progress: HashSet<Arc<Downloading>>,
    recently_deleted: HashMap<(VPath, ImageId), Instant>,

    base_dirs: HashMap<VPath, Arc<BaseDir>>,
    base_dir_list: Vec<Arc<BaseDir>>,
    // TODO(tailhook) remove SocketAddr, use gossip subsystem
    reconciling: HashMap<(VPath, Hash), HashSet<(SocketAddr, MachineId)>>,
    /// Holds hosts that we have just received reconciliation data from.
    /// This probably means that they have respective image. Unless their
    /// image is currently in-progress. We know in-progress data from
    /// gossip subsystem.
    // TODO(tailhook) cleanup them
    recently_received: HashMap<VPath, HashMap<SocketAddr, Instant>>,
}

pub struct ShortProgress {
    pub image_id: ImageId,
    pub mask: Mask,
    pub stalled: bool,
    pub source: bool,
}

#[derive(Clone)]
pub struct Tracking(Arc<Inner>);

struct Inner {
    images: fetch_index::Indexes,
    config: Arc<Config>,
    cmd_chan: UnboundedSender<Command>,
    rescan_chan: UnboundedSender<(VPath, Instant, bool)>,
    state: Arc<Mutex<State>>,
    meta: Meta,
    disk: Disk,
    remote: Remote,
    peers: Peers,
}

#[derive(Clone)]
pub struct Subsystem(Arc<Data>);

// Subsystem data, only here for Deref trait
pub struct Data {
    images: fetch_index::Indexes,
    state: Arc<Mutex<State>>,
    cleanup: UnboundedSender<cleanup::Command>,
    scan: UnboundedSender<(VPath, Instant, bool)>,
    config: Arc<Config>,
    meta: Meta,
    disk: Disk,
    remote: Remote,
    tracking: Tracking,
    peers: Peers,
    dry_cleanup: AtomicBool,
}

pub struct TrackingInit {
    cmd_chan: UnboundedReceiver<Command>,
    rescan_chan: UnboundedReceiver<(VPath, Instant, bool)>,
    tracking: Tracking,
}

pub enum Command {
    FetchDir(Downloading),
    Reconcile(ReconPush),
}

impl Tracking {
    pub fn new(config: &Arc<Config>, meta: &Meta, disk: &Disk,
        remote: &Remote, peers: &Peers)
        -> (Tracking, TrackingInit)
    {
        let (ctx, crx) = unbounded();
        let (rtx, rrx) = unbounded();
        let handler = Tracking(Arc::new(Inner {
            config: config.clone(),
            images: fetch_index::Indexes::new(),
            state: Arc::new(Mutex::new(State {
                in_progress: HashSet::new(),
                recently_deleted: HashMap::new(),
                base_dirs: HashMap::new(),
                base_dir_list: Vec::new(),
                reconciling: HashMap::new(),
                recently_received: HashMap::new(),
            }, "tracking_state")),
            meta: meta.clone(),
            disk: disk.clone(),
            remote: remote.clone(),
            peers: peers.clone(),
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
    pub fn fetch_dir(&self, path: VPath, image: ImageId, replace: bool) {
        let cfg = self.0.config.dirs.get(path.key())
            .expect("config should exist");
        self.send(Command::FetchDir(Downloading {
            replacing: replace,
            virtual_path: path,
            image_id: image,
            config: cfg.clone(),
            mask: AtomicMask::new(),
            slices: Slices::new(),
            index_fetched: AtomicBool::new(false),
            bytes_fetched: AtomicUsize::new(0),
            bytes_total: AtomicUsize::new(0),
            blocks_fetched: AtomicUsize::new(0),
            blocks_total: AtomicUsize::new(0),
            stalled: AtomicBool::new(false),
        }));
    }
    pub fn pick_random_dir(&self) -> Option<(VPath, Hash)> {
        let state = self.state();
        thread_rng().choose(&state.base_dir_list)
            .map(|d| (d.path.clone(), d.hash()))
    }
    fn send(&self, command: Command) {
        self.0.cmd_chan.unbounded_send(command)
            .expect("image tracking subsystem is alive")
    }
    fn state(&self) -> MutexGuard<State> {
        self.0.state.lock()
    }
    fn scan_dir(&self, path: VPath) {
        self.0.rescan_chan.unbounded_send((path, Instant::now(), true))
            .expect("rescan thread is alive");
    }
    pub fn remote(&self) -> &Remote {
        &self.0.remote
    }
    // only for http
    pub fn peers(&self) -> &Peers {
        &self.0.peers
    }
    pub fn get_in_progress(&self) -> BTreeMap<VPath, ShortProgress> {
        let mut res = BTreeMap::new();
        for inp in &self.state().in_progress {
            res.insert(inp.virtual_path.clone(), ShortProgress {
                image_id: inp.image_id.clone(),
                mask: inp.mask.get(),
                stalled: inp.is_stalled(),
                source: self.0.remote.has_image_source(&inp.image_id),
            });
        }
        return res;
    }
    pub fn get_deleted(&self) -> Vec<(VPath, ImageId)> {
        let retention = Duration::from_millis(DELETED_RETENTION);
        let mut state = self.state();
        state.recently_deleted
            .retain(|_, v| *v + retention > Instant::now());
        state.recently_deleted.keys().cloned().collect()
    }
    pub fn get_connection_by_mask<P: Policy>(&self,
        vpath: &VPath, id: &ImageId, mask: Mask,
        failures: &Failures<SocketAddr, P>)
        -> Option<Connection>
    {
        // First try hosts we certainly know has needed bit, but in our
        // local network
        let items = self.0.peers.addrs_by_mask(vpath, id, mask);
        let (conn, not_conn) = self.0.remote.split_connected(
            items.into_iter().filter(|a| failures.can_try(*a)));
        if conn.len() > 0 {
            return thread_rng().choose(&conn).cloned();
        }
        // The find incoming connection. It's tested later because it's
        // probably behind the slow network. Still it's already connected
        // and certainly has the data
        if let Some(conn) = self.0.remote.get_incoming_connection_for_image(id)
        {
            return Some(conn);
        }
        // Well then connect to a peer having data
        if not_conn.len() > 0 {
            return thread_rng().choose(&not_conn).map(|addr| {
                self.0.remote.ensure_connected(self, *addr)
            });
        }
        // If not such peers, get any peer that we had reconciled this path
        // from. First connected hosts, then any one
        if let Some(dict) = self.state().recently_received.get(vpath) {
            let (conn, not_conn) = self.0.remote.split_connected(
                dict.keys().filter(|addr| failures.can_try(**addr))
                .cloned());
            if conn.len() > 0 {
                return thread_rng().choose(&conn).cloned();
            }
            if not_conn.len() > 0 {
                return thread_rng().choose(&not_conn).map(|addr| {
                    self.0.remote.ensure_connected(self, *addr)
                });
            }
        }
        // Finally if nothing works, just ask random host having this base
        // dir. This works quite well, if this host was turned off for some
        // time but others are up.
        //
        // It could take some time to enumerate all hosts in 1000-node cluster,
        // (if there is only one host that needs data). Except every node
        // in the cluster picks 1 host on each attempt. And as quick as it
        // fetches the data it propagates that info to all the hosts. So it
        // should take just few roundtrips to find anything.
        //
        // All of this to say that actual time doesn't depend on the size
        // of the cluster (in simple words it's because the number of
        // probes over whole cluster is proportional to cluster size).
        //
        // TODO(tailhook) find out whether keeping split of `conn`/`not_conn`
        // makes sense for this specific case.
        let items = self.0.peers.addrs_by_basedir(&vpath.parent());
        let (conn, not_conn) = self.0.remote.split_connected(
            items.into_iter().filter(|addr| failures.can_try(*addr)));
        if conn.len() > 0 {
            return thread_rng().choose(&conn).cloned();
        }
        if not_conn.len() > 0 {
            return thread_rng().choose(&not_conn).map(|addr| {
                self.0.remote.ensure_connected(self, *addr)
            });
        }
        return None;
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
        self.state.lock()
    }
    fn start_cleanup(&self) {
        self.cleanup.unbounded_send(cleanup::Command::Reschedule)
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
        self.0.scan.unbounded_send((path, Instant::now(), false))
            .expect("can always send in rescan channel");
    }
    pub fn dir_deleted(&self, path: &VPath, image_id: &ImageId) {
        let mut state = self.state();
        state.recently_deleted
            .insert((path.clone(), image_id.clone()), Instant::now());
    }
    pub fn dir_aborted(&self, cmd: &Arc<Downloading>, reason: &'static str) {
        let mut state = self.state();
        state.recently_deleted
            .insert((cmd.virtual_path.clone(), cmd.image_id.clone()),
                    Instant::now());
        state.in_progress.remove(cmd);
        self.remote.notify_aborted_image(
            &cmd.image_id, &cmd.virtual_path, reason.into());
        self.rescan_dir(cmd.virtual_path.parent());
    }
    pub fn is_recently_deleted(&self, path: &VPath, image_id: &ImageId)
        -> bool
    {
        let state = self.state();
        let key = (path.clone(), image_id.clone());
        if let Some(ts) = state.recently_deleted.get(&key) {
            *ts + Duration::from_millis(AVOID_DOWNLOAD) >= Instant::now()
        } else {
            false
        }
    }
    fn dir_committed(&self, cmd: &Arc<Downloading>) {
        self.state().in_progress.remove(cmd);
        self.remote.notify_received_image(
            &cmd.image_id, &cmd.virtual_path);
        self.rescan_dir(cmd.virtual_path.clone());
    }
}

pub fn start(init: TrackingInit) -> Result<(), String> // actually void
{
    use metadata::Upload::*;
    use metadata::Accept::*;

    let (ctx, crx) = unbounded();
    let TrackingInit { cmd_chan, rescan_chan, tracking } = init;
    let sys = Subsystem(Arc::new(Data {
        images: tracking.0.images.clone(),
        config: tracking.0.config.clone(),
        meta: tracking.0.meta.clone(),
        disk: tracking.0.disk.clone(),
        state: tracking.0.state.clone(),
        remote: tracking.0.remote.clone(),
        peers: tracking.0.peers.clone(),
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
        .for_each(move |(path, time, first_time): (VPath, Instant, bool)| {
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
                base_dir::scan(&path, &config, &meta, &sys.disk)
                .then(move |res| match res {
                    Ok(bdir) => {
                        if first_time {
                            let dirs = bdir.dirs.clone();
                            BaseDir::commit_scan(bdir, &config, scan_time, &sys);
                            Either::B(iter_ok(dirs)
                                .for_each(move |(dir, mut state)| {
                                    let dir = path.join(dir);
                                    let sig = state.signatures.pop()
                                        .expect("checked in metadata reader");
                                    let image_id = state.image.clone();
                                    let cmd = AppendDir {
                                        path: dir.clone(),
                                        image: state.image,
                                        timestamp: sig.timestamp,
                                        signatures: vec![sig.signature],
                                    };
                                    let sys = sys.clone();
                                    sys.disk.check_exists(&config,
                                        PathBuf::from(cmd.path.suffix()))
                                    .then(move |result| match result {
                                        Ok(true) => Either::A(ok(())),
                                        Err(e) => {
                                            error!("Can't check {:?}: {}",
                                                cmd.path, e);
                                            Either::A(ok(()))
                                        }
                                        Ok(false) => {
                                            warn!("Resuming download of \
                                                {:?}: {}",
                                                cmd.path, cmd.image);
                                            Either::B(
                                                sys.meta.append_dir(cmd)
                                                .map(move |result| {
                                                    match result {
                                                        Accepted(New) => {
                                                            sys.tracking
                                                            .fetch_dir(
                                                                dir,
                                                                image_id,
                                                                false);
                                                        }
                                                        _ => {}
                                                    }
                                                })
                                                .map_err(|e| {
                                                    error!("Resume download \
                                                            failed: {}", e);
                                                }))
                                        }
                                    })
                                }))
                        } else {
                            BaseDir::commit_scan(
                                bdir, &config, scan_time, &sys);
                            Either::A(ok(()))
                        }
                    }
                    Err(e) => {
                        error!("Scaning {:?} failed: {}", path, e);
                        Either::A(ok(()))
                    }
                }))
        }));
    spawn(timeout(Duration::new(30, 0))
        .map(move |()| sys3.undry_cleanup())
        .map_err(|_| unreachable!()));
    Ok(())
}
