use std::collections::{VecDeque, HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use abstract_ns::{Name, Resolve, HostResolve, Address, IpList, Error};
use dir_signature::v1::Hashes;
use rand::{thread_rng};
use rand::seq::sample_iter;
use futures::{Future, Async};
use futures::stream::{Stream, Fuse};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use futures::sync::oneshot;
use tk_easyloop::{spawn, timeout};
use tokio_core::reactor::Timeout;
use valuable_futures::Async as VAsync;

use {VPath};
use index::GetIndex;
use blocks::GetBlock;
use cluster::addr::AddrCell;
use cluster::config::Config;
use cluster::download::{RawIndex, Location, Pointer};
use cluster::upload;
use cluster::error::{UploadErr, FetchErr, ErrorKind};
use cluster::future::UploadOk;
use signature::SignedUpload;
use failure_tracker::{HostFailures, DnsFailures, SlowHostFailures};
use proto::{self, Client, ClientFuture, RequestClient, RequestFuture};
use proto::message::Notification;
use proto::{AppendDir, ReplaceDir};
use proto::{AppendDirAck, ReplaceDirAck};
use proto::{GetIndexAt, GetIndexAtResponse};
use proto::{GetBlockResponse};

#[derive(Debug)]
pub enum Message {
    NewUpload(NewUpload),
    FetchIndex(VPath, oneshot::Sender<Result<RawIndex, FetchErr>>),
    FetchFile {
        path: PathBuf,
        location: Location,
        hashes: Hashes,
        tx: oneshot::Sender<Result<Vec<u8>, FetchErr>>,
    },
    Notification(SocketAddr, Notification),
    Closed(SocketAddr),
}

struct Listener {
    addr: SocketAddr,
    chan: UnboundedSender<Message>,
}

#[derive(Debug)]
pub struct NewUpload {
    pub(crate) replace: bool,
    pub(crate) weak: bool,
    pub(crate) upload: SignedUpload,
    pub(crate) stats: Arc<upload::Stats>,
    pub(crate) resolve: oneshot::Sender<Result<UploadOk, Arc<UploadErr>>>,
}

struct Upload {
    replace: bool,
    weak: bool,
    upload: SignedUpload,
    stats: Arc<upload::Stats>,
    resolve: oneshot::Sender<Result<UploadOk, Arc<UploadErr>>>,
    connections: HashMap<SocketAddr, Client>,
    futures: HashMap<SocketAddr, RFuture>,
    early: Timeout,
    deadline: Timeout,
    candidate_hosts: HashSet<Name>,
}

enum RFuture {
    Append(RequestFuture<AppendDirAck>),
    Replace(RequestFuture<ReplaceDirAck>),
}

struct CurrentBlock {
    offset: usize,
    future: RequestFuture<GetBlockResponse>,
}

enum FRequest {
    Index {
        future: Option<RequestFuture<GetIndexAtResponse>>,
        resolve: oneshot::Sender<Result<RawIndex, FetchErr>>,
    },
    /*
    File {
        buf: Vec<u8>,
        future: Option<CurrentBlock>,
        resolve: oneshot::Sender<Result<Vec<u8>, FetchErr>>,
    },
    */
}

pub struct Fetch {
    location: Location,
    connection: Option<(SocketAddr, Client)>,
    req: FRequest,
}


pub struct ConnectionSet<R, I, B> {
    resolver: R,
    index_source: I,
    block_source: B,
    config: Arc<Config>,
    initial_addr: AddrCell,
    chan: Fuse<UnboundedReceiver<Message>>,
    chan_tx: UnboundedSender<Message>,
    failures: HostFailures,
    pending: HashMap<SocketAddr, ClientFuture>,
    active: HashMap<SocketAddr, Client>,
    uploads: VecDeque<Upload>,
    fetches: VecDeque<Fetch>,
    pending_addrs: HashMap<Name, Box<Future<Item=IpList, Error=Error>>>,
    failed_addrs: DnsFailures,
    addrs: HashMap<Name, Address>,
    retry: Timeout,
}

impl<R, I, B> ConnectionSet<R, I, B>
    where I: GetIndex + Clone + Send + 'static,
          B: GetBlock + Clone + Send + 'static,
          R: Resolve + HostResolve + 'static,
{
    pub(crate) fn spawn(initial_address: Vec<Name>, resolver: R,
        index_source: I, block_source: B, config: &Arc<Config>)
        -> UnboundedSender<Message>
        where I: GetIndex + 'static,
              B: GetBlock + 'static,
              R: Resolve + HostResolve + 'static,
    {
        let (tx, rx) = unbounded();
        spawn(ConnectionSet {
            initial_addr: AddrCell::new(initial_address,
                                        config.port, &resolver),
            resolver,
            index_source,
            block_source,
            chan: rx.fuse(),
            chan_tx: tx.clone(),
            config: config.clone(),
            uploads: VecDeque::new(),
            fetches: VecDeque::new(),
            failures: HostFailures::new_default(),
            pending: HashMap::new(),
            active: HashMap::new(),
            pending_addrs: HashMap::new(),
            failed_addrs: DnsFailures::new_default(),
            addrs: HashMap::new(),
            retry: timeout(Duration::new(1, 0)),
        });
        return tx;
    }
    fn read_messages(&mut self) {
        use proto::message::Notification::*;
        use self::Message::*;
        loop {
            let m = match self.chan.poll() {
                Ok(Async::Ready(Some(m))) => m,
                Ok(Async::Ready(None)) => break,
                Ok(Async::NotReady) => break,
                Err(e) => {
                    error!("Cluster set channel error: {:?}", e);
                    break;
                }
            };
            match m {
                NewUpload(up) => {
                    self.start_upload(up);
                }
                FetchIndex(path, tx) => {
                    self.start_fetch_index(path, tx);
                }
                FetchFile { .. } => {
                    unimplemented!();
                }
                Notification(addr, ReceivedImage(img)) => {
                    debug!("Host {}({}) received image {:?}[{}]",
                        img.hostname, addr, img.path, img.id);
                    for up in &mut self.uploads {
                        if up.upload.path == img.path &&
                            (up.weak || up.upload.path == img.path)
                        {
                            up.stats.received_image(addr, &img);
                        }
                    }
                }
                Notification(addr, AbortedImage(img)) => {
                    warn!("Host {}({}) aborted image {:?}[{}]: {}",
                        img.hostname, addr, img.path, img.id, img.reason);
                    for up in &mut self.uploads {
                        if up.upload.image_id == img.id &&
                            (up.weak || up.upload.path == img.path)
                        {
                            up.stats.aborted_image(addr, &img);
                        }
                    }
                }
                Notification(addr, n) => {
                    debug!("Host {} sent notification {:?}", addr, n);
                }
                Closed(addr) => {
                    self.active.remove(&addr).expect("duplicate close");
                }
            }
        }
    }

    fn start_upload(&mut self, up: NewUpload) {
        self.uploads.push_back(Upload {
            replace: up.replace,
            weak: up.weak,
            upload: up.upload,
            stats: up.stats,
            resolve: up.resolve,
            connections: HashMap::new(),
            futures: HashMap::new(),
            early: timeout(self.config.early_timeout),
            deadline: timeout(self.config.maximum_timeout),
            candidate_hosts: HashSet::new(),
        });
    }

    fn start_fetch_index(&mut self, vpath: VPath,
        tx: oneshot::Sender<Result<RawIndex, FetchErr>>)
    {
        self.fetches.push_back(Fetch {
            location: Pointer {
                vpath,
                candidate_hosts: HashSet::new(),
                failures: SlowHostFailures::new_slow(),
            }.into(),
            connection: None,
            req: FRequest::Index {
                future: None,
                resolve: tx,
            },
        });
    }
    fn new_conns_for_upload(&mut self, up: &mut Upload) {
        let initial = self.config.initial_connections as usize;
        if up.connections.len() >= initial {
            return;
        }
        let valid =
            self.pending.keys()
                .filter(|a| !up.stats.is_rejected(**a)).count() +
            self.active.keys()
                .filter(|a| !up.stats.is_rejected(**a)).count();
        if valid >= initial {
            return;
        }

        let mut rng = thread_rng();

        // first try already resolved discovered hosts
        let mut new_addresses = sample_iter(&mut rng,
            up.candidate_hosts.iter()
                .filter_map(|h| self.addrs.get(h).and_then(|a| a.pick_one()))
                .filter(|a| !up.stats.is_rejected(*a))
                .filter(|a| !self.pending.contains_key(a))
                .filter(|a| !self.active.contains_key(a))
                .filter(|a| self.failures.can_try(a)),
            initial)
            .unwrap_or_else(|v| v);
        let slots_left = initial.saturating_sub(new_addresses.len());
        if slots_left > 0 {

            // then start resolving some discovered hosts
            let to_resolve = sample_iter(&mut rng,
                up.candidate_hosts.iter()
                    .filter(|h| self.failed_addrs.can_try(h))
                    .filter(|h| self.addrs.get(h).is_none())
                    .filter(|h| self.pending_addrs.get(h).is_none()),
                slots_left)
                .unwrap_or_else(|v| v);
            for host in to_resolve {
                self.pending_addrs.insert(
                    host.clone(), Box::new(self.resolver.resolve_host(host)));
            }

            // and fallback to just initial addresses
            new_addresses.extend(sample_iter(&mut rng,
                self.initial_addr.get().addresses_at(0)
                .filter(|x| !up.connections.contains_key(x))
                .filter(|a| !up.stats.is_rejected(*a))
                .filter(|a| !self.pending.contains_key(a))
                .filter(|a| !self.active.contains_key(a))
                .filter(|a| self.failures.can_try(a)),
                slots_left)
                .unwrap_or_else(|v| v));
        }
        for naddr in &new_addresses {
            if !self.pending.contains_key(naddr) { // duplicate check
                self.pending.insert(*naddr, Client::spawn(*naddr,
                    "ciruela".to_string(),
                    self.block_source.clone(),
                    self.index_source.clone(),
                    Listener {
                        addr: *naddr,
                        chan: self.chan_tx.clone()
                    }));
            }
        }
    }
    fn new_conn_for_fetch(&mut self, fetch: &mut Fetch) {
        if fetch.connection.is_some() {
            return;
        }
        let mut rng = thread_rng();
        // first try already resolved discovered hosts
        let location = fetch.location.lock();
        let mut new_addresses = sample_iter(&mut rng,
            location.candidate_hosts.iter()
                .filter_map(|h| self.addrs.get(h).and_then(|a| a.pick_one()))
                .filter(|a| location.failures.can_try(a))
                .filter(|a| !self.pending.contains_key(a))
                .filter(|a| self.failures.can_try(a)),
            1)
            .unwrap_or_else(|v| v);
        if new_addresses.len() == 0 {
            let to_resolve = sample_iter(&mut rng,
                location.candidate_hosts.iter()
                    .filter(|h| self.failed_addrs.can_try(h))
                    .filter(|h| self.addrs.get(h).is_none())
                    .filter(|h| self.pending_addrs.get(h).is_none()),
                1)
                .unwrap_or_else(|v| v);
            for host in to_resolve {
                self.pending_addrs.insert(
                    host.clone(), Box::new(self.resolver.resolve_host(host)));
            }

            // and fallback to just initial addresses
            new_addresses = sample_iter(&mut rng,
                self.initial_addr.get().addresses_at(0)
                .filter(|a| !self.pending.contains_key(a))
                .filter(|a| self.failures.can_try(a))
                .filter(|a| location.failures.can_try(a)),
                1)
                .unwrap_or_else(|v| v);
            for naddr in &new_addresses {
                if !self.pending.contains_key(naddr) { // duplicate check
                    self.pending.insert(*naddr, Client::spawn(*naddr,
                        "ciruela".to_string(),
                        self.block_source.clone(),
                        self.index_source.clone(),
                        Listener {
                            addr: *naddr,
                            chan: self.chan_tx.clone()
                        }));
                }
            }
        }
    }

    fn poll_pending(&mut self) {
        let ref config = self.config;
        let ref mut active = self.active;
        let ref mut failures = self.failures;
        let ref mut failed_addrs = self.failed_addrs;
        let ref mut addrs = self.addrs;
        self.pending.retain(|addr, future| {
            match future.poll() {
                Ok(Async::NotReady) => true,
                Ok(Async::Ready(conn)) => {
                    active.insert(*addr, conn);
                    false
                }
                Err(()) => {
                    // error should have already logged
                    failures.add_failure(*addr);
                    false
                }
            }
        });
        self.pending_addrs.retain(|name, future| {
            match future.poll() {
                Ok(Async::NotReady) => true,
                Ok(Async::Ready(ip_list)) => {
                    addrs.insert(name.clone(),
                        ip_list.with_port(config.port));
                    false
                }
                Err(e) => {
                    info!("Can't resolve name {}: {}", name, e);
                    failed_addrs.add_failure(name.clone());
                    false
                }
            }
        });
    }

    fn poll_connect(&mut self) {
        for _ in 0..self.uploads.len() {
            let mut cur = self.uploads.pop_front().unwrap();
            self.new_conns_for_upload(&mut cur);
            self.uploads.push_back(cur);
        }
        for _ in 0..self.fetches.len() {
            let mut cur = self.fetches.pop_front().unwrap();
            self.new_conn_for_fetch(&mut cur);
            self.fetches.push_back(cur);
        }
    }

    fn wakeup_uploads(&mut self) {
        for up in &mut self.uploads {
            if up.connections.len() < self.config.initial_connections as usize
            {
                for (addr, conn) in &self.active {
                    // TODO(tailhook)
                    if !up.connections.contains_key(addr)
                        && !up.stats.is_rejected(*addr)
                    {
                        conn.register_index(&up.upload.image_id);
                        if up.replace {
                            up.futures.insert(*addr,
                                RFuture::Replace(conn.request(ReplaceDir {
                                    old_image: None,
                                    image: up.upload.image_id.clone(),
                                    timestamp: up.upload.timestamp.clone(),
                                    signatures: up.upload.signatures.clone(),
                                    path: up.upload.path.clone(),
                                })));
                        } else {
                            up.futures.insert(*addr,
                                RFuture::Append(conn.request(AppendDir {
                                    image: up.upload.image_id.clone(),
                                    timestamp: up.upload.timestamp.clone(),
                                    signatures: up.upload.signatures.clone(),
                                    path: up.upload.path.clone(),
                                })));
                        }
                        up.connections.insert(*addr, conn.clone());
                    }
                }
            }
        }
    }
    fn wakeup_fetches(&mut self) {
        for fetch in &mut self.fetches {
            if fetch.connection.is_none() {
                let location = fetch.location.lock();
                for (addr, conn) in &self.active {
                    // TODO(tailhook) optimize lock
                    if location.failures.can_try(addr) {
                        fetch.connection = Some((*addr, conn.clone()));
                        match fetch.req {
                            FRequest::Index { ref mut future, .. } => {
                                *future = Some(conn.request(GetIndexAt {
                                    path: location.vpath.clone(),
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    fn poll_uploads(&mut self) {
        for _ in 0..self.uploads.len() {
            let cur = self.uploads.pop_front().unwrap();
            match self.poll_upload(cur) {
                VAsync::Ready(()) => {},
                VAsync::NotReady(cur) => self.uploads.push_back(cur),
            }
        }
    }

    fn poll_fetches(&mut self) {
        for _ in 0..self.fetches.len() {
            let cur = self.fetches.pop_front().unwrap();
            match self.poll_fetch(cur) {
                VAsync::Ready(()) => {},
                VAsync::NotReady(cur) => self.fetches.push_back(cur),
            }
        }
    }

    fn poll_upload(&mut self, mut up: Upload) -> VAsync<(), Upload> {
        {
            let ref stats = up.stats;
            let ref mut candidates = up.candidate_hosts;
            let ref mut connections = up.connections;
            up.futures.retain(|addr, capsule| {
                match capsule {
                    &mut RFuture::Append(ref mut fut) => {
                        match fut.poll() {
                            Ok(Async::NotReady) => true,
                            Ok(Async::Ready(resp)) => {
                                candidates
                                    .extend(resp.hosts.iter().filter_map(
                                        |(_, h)| h.parse().ok()));
                                let accepted = stats.add_response(
                                    *addr,
                                    resp.accepted,
                                    resp.reject_reason,
                                    resp.hosts);
                                if !accepted {
                                    connections.remove(addr);
                                }
                                false
                            }
                            Err(e) => {
                                error!("AppendDir error at {}: {}", addr, e);
                                false
                            }
                        }
                    }
                    &mut RFuture::Replace(ref mut fut) => {
                        match fut.poll() {
                            Ok(Async::NotReady) => true,
                            Ok(Async::Ready(resp)) => {
                                candidates
                                    .extend(resp.hosts.iter().filter_map(
                                        |(_, h)| h.parse().ok()));
                                let accepted = stats.add_response(
                                    *addr,
                                    resp.accepted,
                                    resp.reject_reason,
                                    resp.hosts);
                                if !accepted {
                                    connections.remove(addr);
                                }
                                false
                            }
                            Err(e) => {
                                error!("ReplaceDir error at {}: {}", addr, e);
                                false
                            }
                        }
                    }
                }
            });
        }

        let early_timeout = up.early.poll()
            .expect("timeout is infallible").is_ready();

        // Never exit when request is hanging. This is still bounded because
        // we have timeouts in connection inself.
        trace!("Pending futures: {}, responses: {}", up.futures.len(),
               up.stats.total_responses());
        if up.futures.len() == 0 && up.stats.total_responses() > 0 {
            match upload::check(&up.stats, &self.config, early_timeout) {
                Some(Ok(result)) => {
                    up.resolve.send(Ok(result)).ok();
                    return VAsync::Ready(())
                }
                Some(Err(kind)) => {
                    up.resolve.send(Err(Arc::new(
                        UploadErr::NetworkError(kind, up.stats.clone())
                    ))).ok();
                    return VAsync::Ready(())
                }
                None => {}
            }
        }

        if up.deadline.poll().expect("timeout is infallible").is_ready() {
            error!("Error uploading {:?}[{}]: deadline reached. \
                Current stats {:?}",
                up.upload.path, up.upload.image_id, up.stats);
            match upload::check(&up.stats, &self.config, early_timeout) {
                Some(Ok(result)) => {
                    up.resolve.send(Ok(result)).ok();
                    return VAsync::Ready(())
                }
                Some(Err(kind)) => {
                    up.resolve.send(Err(Arc::new(
                        UploadErr::NetworkError(kind, up.stats.clone())
                    ))).ok();
                    return VAsync::Ready(())
                }
                None => {
                    up.resolve.send(Err(Arc::new(
                        UploadErr::NetworkError(ErrorKind::DeadlineReached,
                                                up.stats.clone())
                    ))).ok();
                }
            }
            return VAsync::Ready(());
        }
        VAsync::NotReady(up)
    }

    fn poll_fetch(&mut self, mut fetch: Fetch) -> VAsync<(), Fetch> {
        let req = match fetch.req {
            FRequest::Index { future: Some(mut fut), resolve } => {
                match fut.poll() {
                    Ok(Async::NotReady) => {}
                    Ok(Async::Ready(result)) => {
                        if let Some(data) = result.data {
                            resolve.send(Ok(RawIndex {
                                data: data.into(),
                                location: fetch.location.clone(),
                            })).ok();
                            return VAsync::Ready(());
                        } else {
                            if let Some((addr, _)) = fetch.connection.take() {
                                debug!("No data at {}", addr);
                                fetch.location.lock().failures.add_failure(addr);
                            }
                            // TODO(tailhook) optimize lock?
                            fetch.location.lock().candidate_hosts
                                .extend(result.hosts.iter().filter_map(
                                    |(_, h)| h.parse().ok()));
                        }
                    }
                    Err(e) => {
                        debug!("Fetch index error: {}", e);
                        if let Some((addr, _)) = fetch.connection.take() {
                            fetch.location.lock().failures.add_failure(addr);
                        }
                    }
                }
                FRequest::Index { future: Some(fut), resolve }
            }
            req @ FRequest::Index { future: None, .. } => req
        };
        fetch.req = req;
        let cancel = match fetch.req {
            FRequest::Index { ref mut resolve, .. } => resolve.poll_cancel(),
        };
        match cancel {
            Ok(Async::Ready(())) => return VAsync::Ready(()),
            Ok(Async::NotReady) => {}
            Err(()) => unreachable!(),
        }
        VAsync::NotReady(fetch)
    }
}

impl<R, I, B> Future for ConnectionSet<R, I, B>
    where I: GetIndex + Clone + Send + 'static,
          B: GetBlock + Clone + Send + 'static,
          R: Resolve + HostResolve + 'static,
{
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        self.initial_addr.poll();
        self.read_messages();
        let mut repeat = true;
        while repeat {
            repeat = false;
            match self.retry.poll().expect("timeout never fails") {
                Async::Ready(()) if self.uploads.len() > 0 => {
                    // failed node will be okay in a second
                    self.retry = timeout(Duration::new(1, 0));
                    repeat = true;
                }
                Async::Ready(()) => {}  // don't wakeup if no active uploads
                Async::NotReady => {}
            }

            let pending = self.pending.len();
            self.poll_connect();
            repeat = repeat || pending != self.pending.len();

            let pending = self.pending.len();
            let paddrs = self.pending_addrs.len();
            self.poll_pending();
            repeat = repeat || pending != self.pending.len();
            repeat = repeat || paddrs != self.pending_addrs.len();

            self.wakeup_uploads();
            self.wakeup_fetches();

            let uploads = self.uploads.len();
            self.poll_uploads();
            repeat = repeat || uploads != self.uploads.len();

            let fetches = self.fetches.len();
            self.poll_fetches();
            repeat = repeat || fetches != self.fetches.len();
        }
        // TODO(tailhook) set reconnect timer
        if self.chan.is_done() && self.uploads.len() == 0 {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
    }
}

impl proto::Listener for Listener {
    fn notification(&self, n: Notification) {
        self.chan.unbounded_send(Message::Notification(self.addr, n)).ok();
    }
    fn closed(&self) {
        debug!("Connection to {} closed", self.addr);
        self.chan.unbounded_send(Message::Closed(self.addr)).ok();
    }
}
