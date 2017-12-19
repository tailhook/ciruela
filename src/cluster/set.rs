use std::sync::Arc;

use abstract_ns::{Name, Resolve, HostResolve};
use futures::{Future, Async};
use futures::sync::mpsc::{UnboundedReceiver};

use index::GetIndex;
use blocks::GetBlock;
use cluster::addr::AddrCell;
use cluster::config::Config;

pub struct ConnectionSet<R, I, B> {
    resolver: R,
    index_source: I,
    block_source: B,
    config: Arc<Config>,
    initial_addr: AddrCell,
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
            initial_addr: AddrCell::new(initial_address,
                                        config.port, &resolver),
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
        self.initial_addr.poll();
        unimplemented!();
    }
}
