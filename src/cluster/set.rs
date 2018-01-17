use std::sync::Arc;
use std::net::SocketAddr;
use std::collections::{VecDeque, HashMap};

use abstract_ns::{Name, Resolve, HostResolve};
use rand::{thread_rng, sample};
use futures::{Future, Async};
use futures::stream::{Stream, Fuse};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use futures::sync::oneshot;
use tk_easyloop::{spawn, timeout};
use tokio_core::reactor::Timeout;
use valuable_futures::Async as VAsync;

use {VPath};
use index::{GetIndex, ImageId};
use blocks::GetBlock;
use cluster::addr::AddrCell;
use cluster::config::Config;
use cluster::upload;
use cluster::error::UploadErr;
use cluster::future::UploadOk;
use signature::SignedUpload;
use failure_tracker::HostFailures;
use proto::{self, Client, ClientFuture, RequestClient, RequestFuture};
use proto::message::Notification;
use proto::{AppendDir, ReplaceDir};
use proto::{AppendDirAck, ReplaceDirAck};

#[derive(Debug)]
pub enum Message {
    NewUpload(NewUpload),
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
    pub(crate) upload: SignedUpload,
    pub(crate) stats: Arc<upload::Stats>,
    pub(crate) resolve: oneshot::Sender<Result<UploadOk, Arc<UploadErr>>>,
}

struct Upload {
    replace: bool,
    upload: SignedUpload,
    stats: Arc<upload::Stats>,
    resolve: oneshot::Sender<Result<UploadOk, Arc<UploadErr>>>,
    connections: HashMap<SocketAddr, Client>,
    futures: HashMap<SocketAddr, RFuture>,
    early: Timeout,
    deadline: Timeout,
}

enum RFuture {
    Append(RequestFuture<AppendDirAck>),
    Replace(RequestFuture<ReplaceDirAck>),
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
}

impl<R, I, B> ConnectionSet<R, I, B>
    where I: GetIndex + Clone + Send + 'static,
          B: GetBlock + Clone + Send + 'static,
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
            failures: HostFailures::new_default(),
            pending: HashMap::new(),
            active: HashMap::new(),
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
                Notification(addr, ReceivedImage(img)) => {
                    debug!("Host {}({}) received image {:?}[{}]",
                        img.hostname, addr, img.path, img.id);
                    for up in &mut self.uploads {
                        if up.upload.image_id == img.id &&
                            up.upload.path == img.path
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
                            up.upload.path == img.path
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
            upload: up.upload,
            stats: up.stats,
            resolve: up.resolve,
            connections: HashMap::new(),
            futures: HashMap::new(),
            early: timeout(self.config.early_timeout),
            deadline: timeout(self.config.maximum_timeout),
        });
    }

    fn new_conns(&mut self, up: &mut Upload) {
        let initial = self.config.initial_connections as usize;
        if up.connections.len() >= initial
            || self.pending.len() + self.active.len() >= initial
        {
            return;
        }
        let new_addresses = sample(&mut thread_rng(),
            self.initial_addr.get().addresses_at(0)
            .filter(|x| !up.connections.contains_key(x)), 3);
        for naddr in &new_addresses {
            if !self.pending.contains_key(naddr)
               && !self.active.contains_key(naddr)
               && self.failures.can_try(*naddr)
            {
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

    fn poll_pending(&mut self) {
        let ref mut active = self.active;
        let ref mut failures = self.failures;
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
    }

    fn poll_connect(&mut self) {
        for _ in 0..self.uploads.len() {
            let mut cur = self.uploads.pop_front().unwrap();
            self.new_conns(&mut cur);
            self.uploads.push_back(cur);
        }
    }

    fn wakeup_uploads(&mut self) {
        for up in &mut self.uploads {
            if up.connections.len() < self.config.initial_connections as usize
            {
                for (addr, conn) in &self.active {
                    if !up.connections.contains_key(addr) {
                        if up.replace {
                            unimplemented!();
                        }
                        conn.register_index(&up.upload.image_id);
                        up.futures.insert(*addr,
                            RFuture::Append(conn.request(AppendDir {
                                image: up.upload.image_id.clone(),
                                timestamp: up.upload.timestamp.clone(),
                                signatures: up.upload.signatures.clone(),
                                path: up.upload.path.clone(),
                            })));
                        up.connections.insert(*addr, conn.clone());
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

    fn poll_upload(&mut self, mut up: Upload) -> VAsync<(), Upload> {
        if up.deadline.poll().expect("timeout is infallible").is_ready() {
            error!("Error uploading {:?}[{}]: deadline reached. \
                Current stats {:?}",
                up.upload.path, up.upload.image_id, up.stats);
            up.resolve.send(Err(
                Arc::new(UploadErr::DeadlineReached)))
                .ok();
            return VAsync::Ready(());
        }
        VAsync::NotReady(up)
    }
}

impl<R, I, B> Future for ConnectionSet<R, I, B>
    where I: GetIndex + Clone + Send + 'static,
          B: GetBlock + Clone + Send + 'static,
{
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        self.initial_addr.poll();
        self.read_messages();
        let mut repeat = true;
        while repeat {
            repeat = false;

            let pending = self.pending.len();
            self.poll_connect();
            repeat = repeat || pending != self.pending.len();

            let pending = self.pending.len();
            self.poll_pending();
            repeat = repeat || pending != self.pending.len();

            self.wakeup_uploads();

            let uploads = self.uploads.len();
            self.poll_uploads();
            repeat = repeat || uploads != self.uploads.len();
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
