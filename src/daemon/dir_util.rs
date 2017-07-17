use std::path::{Path, PathBuf};

use openat::Dir;


pub fn recover_path<P: AsRef<Path>>(dir: &Dir, path: P) -> PathBuf {
    let mut result = dir.recover_path()
        .unwrap_or_else(|e| {
            warn!("Error recovering path {:?}: {}", dir, e);
            PathBuf::from("<unknown>")
        });
    result.push(path.as_ref());
    result
}
