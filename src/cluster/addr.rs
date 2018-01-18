use abstract_ns::{HostResolve, Address, IpList, Name, Error};
use abstract_ns::addr::union;
use std::collections::VecDeque;

use futures::{Future, Async};
use void::{Void, unreachable};

pub(in cluster) enum AddrCell {
    Ready(Address),
    Fetching(Box<FutureAddr<Item=Address, Error=Void>>),
}

pub(in cluster) trait FutureAddr: Future + AsRef<Address> {
}

pub(in cluster) struct AddrFuture<F> {
    port: u16,
    futures: VecDeque<(Name, F)>,
    current: Address,
}

impl<F: Future<Item=IpList, Error=Error>> Future for AddrFuture<F> {
    type Item = Address;
    type Error = Void;
    fn poll(&mut self) -> Result<Async<Address>, Void> {
        for _ in 0..self.futures.len() {
            let (n, mut fut) = self.futures.pop_front().unwrap();
            match fut.poll() {
                Ok(Async::Ready(new)) => {
                    self.current = union(&[
                        self.current.clone(), new.with_port(self.port)
                    ]);
                }
                Ok(Async::NotReady) => {
                    self.futures.push_back((n, fut));
                }
                Err(e) => {
                    error!("Error resolving name {:?}: {}", n, e);
                }
            }
        }
        if self.futures.len() > 0 {
            return Ok(Async::NotReady);
        } else {
            return Ok(Async::Ready(self.current.clone()))
        }
    }
}

impl<F> AsRef<Address> for AddrFuture<F> {
    fn as_ref(&self) -> &Address {
        &self.current
    }
}

impl<F: Future<Item=IpList, Error=Error>> FutureAddr for AddrFuture<F> {}

impl AddrCell {
    pub fn new<R: HostResolve>(initial_address: Vec<Name>,
        port: u16, resolver: &R)
        -> AddrCell
        where R::HostFuture: 'static,
    {
        AddrCell::Fetching(Box::new(AddrFuture {
            port: port,
            futures: initial_address.iter()
                .map(|n| (n.clone(), resolver.resolve_host(n)))
                .collect(),
            current: [][..].into(),
        }))
    }
    pub fn poll(&mut self) {
        use self::AddrCell::*;
        let res = match *self {
            Ready(..) => return,
            Fetching(ref mut b) => match b.poll() {
                Ok(Async::Ready(a)) => a,
                Ok(Async::NotReady) => return,
                Err(e) => unreachable(e),
            },
        };
        *self = Ready(res);
    }
    pub fn get(&self) -> &Address {
        use self::AddrCell::*;
        match *self {
            Ready(ref a) => a,
            Fetching(ref b) => b.as_ref().as_ref(),
        }
    }
    pub fn is_done(&self) -> bool {
        matches!(self, &AddrCell::Ready(..))
    }
}
