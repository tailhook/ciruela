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
            display("error writing {:?}: {}", path, e)
            cause(e)
        }
        RenameDir(path: PathBuf, e: io::Error) {
            description("error renaming dir")
            display("error renaming dir {:?}: {}", path, e)
            cause(e)
        }
        ReadFile(path: PathBuf, e: io::Error) {
            description("error reading file")
            display("error reading {:?}: {}", path, e)
            cause(e)
        }
        ReadKeepList(path: PathBuf, e: io::Error) {
            description("error reading keep-list file")
            display("error reading keep-list {:?}: {}", path, e)
            cause(e)
        }
        SetPermissions(path: PathBuf, e: io::Error) {
            description("error setting permissions")
            display("error setting permissions on {:?}: {}", path, e)
            cause(e)
        }
        CreateSymlink(path: PathBuf, e: io::Error) {
            description("error creating symlink")
            display("error creating symlink {:?}: {}", path, e)
            cause(e)
        }
        Checksum(path: PathBuf) {
            description("error verifing checksum")
            display("error verifing checksum {:?}", path)
        }
        Commit(path: PathBuf, e: io::Error) {
            description("error commiting dir")
            display("error commiting dir {:?}: {}", path, e)
        }
        Delete(path: PathBuf, e: io::Error) {
            description("error removing dir")
            display("error removing dir {:?}: {}", path, e)
        }
    }
}
