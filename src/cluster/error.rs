use std::sync::Arc;

use failure::Error;

use cluster::upload::Stats;

/// Network error happened
///
/// Network error means we couldn't upload data to all peers we wanted, either
/// because they are inaccessible or some kind of timeout happened, or they
/// refused to accept the image
#[derive(Debug, Fail, Clone)]
pub enum ErrorKind {
    /// Deadline reached when doing upload
    #[fail(display="deadline reached")]
    DeadlineReached,
    /// Some hosts rejected the download
    #[fail(display="some hosts rejected the download")]
    Rejected,
    #[doc(hidden)]
    #[fail(display="undefined error")]
    __Nonexhaustive,
}

/// Error when uploading image
#[derive(Debug, Fail)]
pub enum UploadErr {
    /// Unexpected fatal error happened
    #[fail(display="{:?}", _0)]
    Fatal(Error),
    /// Deadline reached
    // TODO(tailhook) maybe make stats here
    #[fail(display="network error: {}", _0)]
    NetworkError(ErrorKind, Arc<Stats>),
    #[doc(hidden)]
    #[fail(display="undefined error")]
    __Nonexhaustive,
}
