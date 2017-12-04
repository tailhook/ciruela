use std::sync::Arc;

use abstract_ns::{Name, Resolve, HostResolve};
use futures::{Future, Async};
use futures::sync::mpsc::{UnboundedReceiver};

use index::GetIndex;
use blocks::GetBlock;
use cluster::config::Config;

pub struct ConnectionSet<R, I, B> {
    resolver: R,
    index_source: I,
    block_source: B,
    config: Arc<Config>,
    chan: UnboundedReceiver<()>,
}

impl<R, I, B> ConnectionSet<R, I, B> {
    pub fn new(chan: UnboundedReceiver<()>,
        initial_address: Vec<Name>, resolver: R,
        index_source: I, block_source: B, config: &Arc<Config>)
        -> ConnectionSet<R, I, B>
        where I: GetIndex + 'static,
              B: GetBlock + 'static,
              R: Resolve + HostResolve + 'static,
    {
        ConnectionSet {
            resolver,
            index_source,
            block_source,
            chan,
            config: config.clone(),
        }
    }
}

impl<R, I, B> Future for ConnectionSet<R, I, B> {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        unimplemented!();
    }
}
