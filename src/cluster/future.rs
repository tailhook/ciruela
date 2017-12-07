use futures::{Future, Async};


/// Future returned from `Upload::future`
#[derive(Debug)]
pub struct UploadFuture {
}

/// Result of the upload
#[derive(Debug)]
pub struct UploadOk {
    _private: ()
}

/// Error uploading image
#[derive(Debug, Fail)]
#[fail(display="Upload error")]
pub struct UploadFail {
    _private: ()
}

impl Future for UploadFuture {
    type Item = UploadOk;
    type Error = UploadFail;
    fn poll(&mut self) -> Result<Async<UploadOk>, UploadFail> {
        unimplemented!();
    }
}
