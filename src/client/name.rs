
use abstract_ns::{Router, RouterBuilder};
use futures_cpupool::CpuPool;
use ns_std_threaded::ThreadedResolver;


pub fn resolver() -> Router {
    let mut router = RouterBuilder::new();
    router.add_default(ThreadedResolver::new(CpuPool::new(1)));
    return router.into_resolver();
}
