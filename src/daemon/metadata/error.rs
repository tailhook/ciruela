use std::io;
use std::path::{PathBuf};
use serde_cbor;
use serde_cbor::error::Error as CborError;

use ciruela::VPath;


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        InvalidPath {
            description("invalid path \
                (not absolute or has parents or invalid utf8)")
        }
        PathNotFound(path: VPath) {
            description("path not found")
            display("destination path for {:?} is not found", path)
        }
        FileWasVanished(path: PathBuf) {
            description("file was vanished while scanning")
            display("file {:?} was vanished while scanning", path)
        }
        CleanupCanceled(path: VPath) {
            description("cleanup canceled because dir was updated \
                or is currently being written to")
            display("cleanup of {:?} canceled because dir was updated \
                or is currently being written to", path)
        }
        LevelMismatch(has: usize, required: usize) {
            description("invalid directory level in upload path")
            display("expected path with {} components, but is {}",
                    required, has)
        }
        OpenRoot(dir: PathBuf, e: io::Error) {
            description("can't open root metadata dir")
            display("can't open root metadata dir {:?}: {}", dir, e)
            cause(e)
        }
        CreateDirRace(dir: PathBuf, e: io::Error) {
            description("race condition when creating metadata dir")
            display("race condition when creating metadata dir {:?}: {}",
                    dir, e)
            cause(e)
        }
        Open(dir: PathBuf, e: io::Error) {
            description("can't open metadata dir")
            display("can't open metadata dir {:?}: {}", dir, e)
            cause(e)
        }
        Read(dir: PathBuf, e: io::Error) {
            description("can't open metadata file")
            display("can't open metadata file {:?}: {}", dir, e)
            cause(e)
        }
        Encode(dir: PathBuf, e: CborError) {
            description("can't encode metadata file")
            display("can't encode metadata file {:?}: {}", dir, e)
            cause(e)
        }
        Decode(dir: PathBuf, e: Box<::std::error::Error + Send>) {
            description("can't decode metadata file")
            display("can't decode metadata file {:?}: {}", dir, e)
            cause(&**e)
        }
        ListDir(dir: PathBuf, e: io::Error) {
            description("can't list metadata dir")
            display("can't list metadata dir {:?}: {}", dir, e)
            cause(e)
        }
        CreateDir(dir: PathBuf, e: io::Error) {
            description("can't create metadata dir")
            display("can't create metadata dir {:?}: {}", dir, e)
            cause(e)
        }
        WriteMeta(dir: PathBuf, e: io::Error) {
            description("can't write metadata file")
            display("can't write metadata file {:?}: {}", dir, e)
            cause(e)
        }
        Remove(path: PathBuf, e: io::Error) {
            description("can't remove metadata file")
            display("can't remove metadata file {:?}: {}", path, e)
            cause(e)
        }
        SerializeError(e: serde_cbor::Error) {
            description("can't serialize metadata")
            display("can't serialize metadata: {}", e)
            cause(e)
            from()
        }
        IndexNotFound {
            description("index not found")
        }
    }
}
