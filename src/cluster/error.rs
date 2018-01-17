use failure::Error;


/// Error when uploading image
#[derive(Debug, Fail)]
pub enum UploadErr {
    /// Unexpected fatal error happened
    #[fail(display="{:?}", _0)]
    Fatal(Error),
    /// Deadline reached
    // TODO(tailhook) maybe make stats here
    #[fail(display="deadline reached")]
    DeadlineReached,
}
