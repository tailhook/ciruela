use std::io;
use std::path::{PathBuf};


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        InvalidPath {
            description("invalid path (not absolute or has parents)")
        }
        PathNotFound(path: PathBuf) {
            description("path not found")
            display("destination path for {:?} is not found", path)
        }
        LevelMismatch(has: usize, required: usize) {
            description("invalid directory level in upload path")
            display("expected path with {} components, but is {}",
                    required, has)
        }
        OpenMeta(dir: PathBuf, e: io::Error) {
            description("can't open metadata dir")
            display("can't open metadata dir {:?}: {}", dir, e)
        }
        CreateMeta(dir: PathBuf, e: io::Error) {
            description("can't create metadata dir")
            display("can't create metadata dir {:?}: {}", dir, e)
        }
    }
}
