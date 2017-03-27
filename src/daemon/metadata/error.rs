use std::io;
use std::path::{PathBuf};
use serde_cbor;
use dir_signature::v1::{ParseError as IndexError};


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        InvalidPath {
            description("invalid path \
                (not absolute or has parents or invalid utf8)")
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
            cause(e)
        }
        ReadMeta(dir: PathBuf, e: io::Error) {
            description("can't open metadata file")
            display("can't open metadata file {:?}: {}", dir, e)
            cause(e)
        }
        WriteMeta(dir: PathBuf, e: io::Error) {
            description("can't write metadata file")
            display("can't write metadata file {:?}: {}", dir, e)
            cause(e)
        }
        SerializeError(e: serde_cbor::Error) {
            description("can't serialize metadata")
            display("can't serialize metadata: {}", e)
            cause(e)
            from()
        }
        BadIndex(path: PathBuf, e: IndexError) {
            description("error reading index")
            display("error reading index: {}", e)
            cause(e)
        }
        IndexNotFound {
            description("index not found")
        }
    }
}
