use std::sync::Arc;
use std::net::SocketAddr;
use std::collections::{VecDeque, HashMap};

use abstract_ns::{Name, Resolve, HostResolve};
use rand::{thread_rng, sample};
use futures::{Future, Async};
use futures::stream::{Stream, Fuse, FuturesUnordered};
use futures::sync::mpsc::{UnboundedReceiver};
use futures::sync::oneshot;

use {VPath};
use index::{GetIndex, ImageId};
use blocks::GetBlock;
use cluster::addr::AddrCell;
use cluster::config::Config;
use cluster::upload;
use cluster::error::UploadErr;
use cluster::future::UploadOk;
use proto::Client;

#[derive(Debug)]
pub enum Message {
    NewUpload(NewUpload),
}

#[derive(Debug)]
pub struct NewUpload {
    pub(crate) image_id: ImageId,
    pub(crate) path: VPath,
    pub(crate) stats: Arc<upload::Stats>,
    pub(crate) resolve: oneshot::Sender<Result<UploadOk, Arc<UploadErr>>>,
}

struct Upload {
    path: VPath,
    image_id: ImageId,
    stats: Arc<upload::Stats>,
    resolve: oneshot::Sender<Result<UploadOk, Arc<UploadErr>>>,
    connections: HashMap<SocketAddr, Client>,
}

pub struct ConnectionSet<R, I, B> {
    resolver: R,
    index_source: I,
    block_source: B,
    config: Arc<Config>,
    initial_addr: AddrCell,
    chan: Fuse<UnboundedReceiver<Message>>,
    uploads: VecDeque<Upload>,
    futures: FuturesUnordered<Box<Future<Item=(), Error=()>>>,
}

impl<R, I, B> ConnectionSet<R, I, B> {
    pub(crate) fn new(chan: UnboundedReceiver<Message>,
        initial_address: Vec<Name>, resolver: R,
        index_source: I, block_source: B, config: &Arc<Config>)
        -> ConnectionSet<R, I, B>
        where I: GetIndex + 'static,
              B: GetBlock + 'static,
              R: Resolve + HostResolve + 'static,
    {
        ConnectionSet {
            initial_addr: AddrCell::new(initial_address,
                                        config.port, &resolver),
            resolver,
            index_source,
            block_source,
            chan: chan.fuse(),
            config: config.clone(),
            uploads: VecDeque::new(),
            futures: FuturesUnordered::new(),
        }
    }
    fn read_messages(&mut self) {
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
            }
        }
    }

    fn start_upload(&mut self, up: NewUpload) {
        self.uploads.push_back(Upload {
            path: up.path,
            image_id: up.image_id,
            stats: up.stats,
            resolve: up.resolve,
            connections: HashMap::new(),
        });
    }

    fn new_conns(&mut self, up: &mut Upload) {
        if up.connections.len() >= self.config.initial_connections as usize {
            return;
        }
        let new_addresses = sample(&mut thread_rng(),
            self.initial_addr.get().addresses_at(0)
            .filter(|x| !up.connections.contains_key(x)), 3);
        for naddr in new_addresses {
            unimplemented!();
        }
    }

    fn poll_connect(&mut self) {
        for _ in 0..self.uploads.len() {
            let mut cur = self.uploads.pop_front().unwrap();
            self.new_conns(&mut cur);
            self.uploads.push_back(cur);
        }
    }
}

impl<R, I, B> Future for ConnectionSet<R, I, B> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        self.initial_addr.poll();
        self.read_messages();
        self.poll_connect();
        match self.futures.poll() {
            Ok(Async::Ready(Some(()))) => {}
            Ok(Async::Ready(None)) => {}
            Ok(Async::NotReady) => {}
            // reconnect?
            Err(()) => {}
        }
        if self.chan.is_done() && self.uploads.len() == 0 {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
    }
}
