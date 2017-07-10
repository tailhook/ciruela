use std::io;
use std::path::{Path, PathBuf};


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        OpenBase(dir: PathBuf, e: io::Error) {
            description("can't open base dir")
            display("can't open base dir {:?}: {}", dir, e)
            cause(e)
        }
        CreateDir(dir: PathBuf, e: io::Error) {
            description("can't create dir")
            display("can't create dir {:?}: {}", dir, e)
            cause(e)
        }
        CreateDirRace(dir: PathBuf, e: io::Error) {
            description("race condition when creating dir")
            display("race condition when creating dir {:?}: {}", dir, e)
            cause(e)
        }
    }
}
