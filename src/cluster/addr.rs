use tokio_core::reactor::Timeout;

use tk_easyloop::timeout;


pub struct InitAddrFuture {
    queue: Vec<Name>,
    timeout: Timeout,
    current: Address,
}

impl AddrFuture {
    pub fn new(initial_address: Vec<Name>, cfg: &Arc<Config>)
        -> InitAddrFuture
    {
        InitAddrFuture {
            queue: initial_address,
            timeout: timeout(Duration::new(30, 0)),
        }
    }
}

impl Future for AddrFuture {

}
