use std::sync::Arc;
use std::fmt;

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
    Fatal(Error),
    /// Deadline reached
    // TODO(tailhook) maybe make stats here
    NetworkError(ErrorKind, Arc<Stats>),
    #[doc(hidden)]
    __Nonexhaustive,
}

impl fmt::Display for UploadErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::UploadErr::*;
        match *self {
            Fatal(ref e) => write!(f, "fatal error: {:?}", e),
            NetworkError(ref kind, ref stats) => {
                write!(f, "network error: {}. Overall progress: {}",
                    kind, stats.one_line_progress())
            }
            __Nonexhaustive => write!(f, "undefined error"),
        }
    }
}

/// Error when downloading index or block
#[derive(Debug, Fail)]
pub enum FetchErr {
    /// Unexpected fatal error happened
    #[fail(display="{:?}", _0)]
    Fatal(Error),
    #[doc(hidden)]
    #[fail(display="undefined error")]
    __Nonexhaustive,
}
