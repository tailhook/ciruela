use std::net::SocketAddr;

use abstract_ns::{Name, Address, Resolve, HostResolve};
use ns_router::{Router, Config};
use futures_cpupool::CpuPool;
use ns_std_threaded::ThreadedResolver;

use tokio_core::reactor::Handle;


pub fn resolver(h: &Handle) -> Router {
    return Router::from_config(&Config::new()
        .set_fallthrough(ThreadedResolver::use_pool(CpuPool::new(1))
            .null_service_resolver()
            .frozen_subscriber())
        .done(), h)
}

pub fn pick_hosts(name: &Name, addr: Address) -> Result<Vec<SocketAddr>, ()> {
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

        println!("Host {} resolves to {} addresses, \
            picking: {:?}", name, num, addresses);
        Ok(addresses)
    } else {
        let addresses = set.addresses()
            .collect::<Vec<_>>();
        println!("Host {} resolves to {:?}",
            name, addresses);
        Ok(addresses)
    }
}
