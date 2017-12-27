use failure::Error;

#[derive(Debug, Fail)]
pub enum UploadErr {
    #[fail(display="{:?}", _0)]
    Fatal(Error),
}
