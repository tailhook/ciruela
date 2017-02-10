use std::net::SocketAddr;

use abstract_ns::{Address, Router, RouterBuilder};
use futures_cpupool::CpuPool;
use ns_std_threaded::ThreadedResolver;


pub fn resolver() -> Router {
    let mut router = RouterBuilder::new();
    router.add_default(ThreadedResolver::new(CpuPool::new(1)));
    return router.into_resolver();
}

pub fn pick_hosts(name: &str, addr: Address) -> Result<Vec<SocketAddr>, ()> {
    let set = addr.at(0);
    // TODO(tailhook)
    let num = set.addresses().count();
    if num == 0 {
        error!("Host {:?}, has no address", name);
        Err(())
    } else if num > 3 {

        // TODO(tailhook) better algorithm
        let mut addresses = (0..3)
            .filter_map(|_| set.pick_one())
            .collect::<Vec<_>>();
        addresses.dedup();

        println!("Host {:?} resolves to {} addresses, \
            picking: {:?}", name, num, addresses);
        Ok(addresses)
    } else {
        let addresses = set.addresses()
            .collect::<Vec<_>>();
        println!("Host {:?} resolves to {:?}",
            name, addresses);
        Ok(addresses)
    }
}
