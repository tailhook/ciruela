use std::io;
use std::path::{Path, PathBuf};


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        DirOpen(path: PathBuf, err: io::Error) {
            description("error opening directory")
            context(path: &'a Path, err: io::Error)
                -> (path.to_path_buf(), err)
            context(path: &'a PathBuf, err: io::Error)
                -> (path.clone(), err)
        }
    }
}
