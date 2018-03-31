use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use cluster::error::{UploadErr, FetchErr};
use cluster::upload::Stats;
use cluster::download::RawIndex;
use failure::err_msg;
use futures::future::Shared;
use futures::sync::oneshot;
use futures::{Future, Async};


/// Future returned from `Upload::future`
#[derive(Debug)]
pub struct UploadFuture {
    pub(crate) inner: Shared<oneshot::Receiver<Result<UploadOk, Arc<UploadErr>>>>,
}

/// Future returned from `Connection::fetch_index`
#[derive(Debug)]
pub struct IndexFuture {
    pub(crate) inner: oneshot::Receiver<Result<RawIndex, FetchErr>>,
}


/// Result of the upload
#[derive(Debug, Clone)]
pub struct UploadOk {
    stats: Arc<Stats>,
    finished: Instant,
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
    pub(crate) fn new(stats: &Arc<Stats>) -> UploadOk {
        UploadOk {
            stats: stats.clone(),
            finished: Instant::now(),
        }
    }
}

impl fmt::Display for UploadOk {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Upload to ")?;
        let ref s = self.stats;
        let d = self.finished.duration_since(s.started);
        if self.stats.cluster_name.len() == 1 {
            f.write_str(self.stats.cluster_name[0].as_ref())?;
        } else {
            write!(f, "{} hosts", self.stats.cluster_name.len())?;
        }
        f.write_str(": ")?;
        s.fmt_downloaded(f)?;
        if d.as_secs() < 1 {
            write!(f, " in 0.{:03}s", d.subsec_nanos() / 1_000_000)?;
        } else if d.as_secs() < 10 {
            write!(f, " in {}.{:01}s", d.as_secs(),
                d.subsec_nanos() / 100_000_000)?;
        } else {
            write!(f, " in {}s", d.as_secs())?;
        }
        Ok(())
    }
}

impl Future for IndexFuture {
    type Item = RawIndex;
    type Error = FetchErr;
    fn poll(&mut self) -> Result<Async<RawIndex>, FetchErr> {
        match self.inner.poll() {
            Ok(Async::Ready(Ok(v))) => Ok(Async::Ready(v)),
            Ok(Async::Ready(Err(e))) => Err(e),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_) => Err(FetchErr::Fatal(
                format_err!("channel closed unexpectedly"))),
        }
    }
}
