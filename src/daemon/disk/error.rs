use std::io;
use std::path::{PathBuf};


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        OpenBase(dir: PathBuf, e: io::Error) {
            description("can't open base dir")
            display("can't open base dir {:?}: {}", dir, e)
            cause(e)
        }
        OpenDir(dir: PathBuf, e: io::Error) {
            description("can't open dir")
            display("can't open dir {:?}: {}", dir, e)
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
        WriteFile(path: PathBuf, e: io::Error) {
            description("error writing file")
            display("erorr writing {:?}: {}", path, e)
            cause(e)
        }
    }
}
