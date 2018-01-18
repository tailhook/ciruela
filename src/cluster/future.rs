use std::sync::Arc;

use cluster::error::UploadErr;
use failure::err_msg;
use futures::future::Shared;
use futures::sync::oneshot;
use futures::{Future, Async};


/// Future returned from `Upload::future`
#[derive(Debug)]
pub struct UploadFuture {
    pub(crate) inner: Shared<oneshot::Receiver<Result<UploadOk, Arc<UploadErr>>>>,
}

/// Result of the upload
#[derive(Debug, Clone)]
pub struct UploadOk {
    _private: ()
}

/// Error uploading image
#[derive(Debug, Fail)]
#[fail(display="Upload error: {}", err)]
pub struct UploadFail {
    err: Arc<UploadErr>,
}

impl Future for UploadFuture {
    type Item = UploadOk;
    type Error = UploadFail;
    fn poll(&mut self) -> Result<Async<UploadOk>, UploadFail> {
        match self.inner.poll() {
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(res)) => match *res {
                Ok(ref x) => Ok(Async::Ready(x.clone())),
                Err(ref e) => Err(UploadFail { err: e.clone() }),
            }
            Err(_) => {
                Err(UploadFail {
                    err: Arc::new(
                        UploadErr::Fatal(err_msg("uploader crashed")))
                })
            }
        }
    }
}

impl UploadOk {
    pub(crate) fn new() -> UploadOk {
        UploadOk {
            _private: (),
        }
    }
}
