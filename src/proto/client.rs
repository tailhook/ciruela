use futures::{Async, Future};


pub struct Client {

}

impl Client {
}

impl Future for Client {
    type Item = ();
    type Error =();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        unimplemented!();
    }
}
