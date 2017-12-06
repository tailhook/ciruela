use futures::{Future, Async};


/// Future returned from `Upload::future`
#[derive(Debug)]
pub struct UploadFuture {
}

impl Future for UploadFuture {
    type Item = ();  // TODO(tailhook) some success value?
    type Error = ::failure:: Error;  // TODO(tailhook) more specific failure
    fn poll(&mut self) -> Result<Async<()>, ::failure::Error> {
        unimplemented!();
    }
}
