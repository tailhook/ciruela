use std::sync::Arc;
use std::time::Duration;

use digest_writer;
use blake2::{Blake2b, Digest};
use serde_cbor::ser::to_writer;
use typenum::U32;

use ciruela::serialize;
use config::Directory;


#[derive(Serialize)]
pub struct RemoteConfig {
    pub num_levels: usize,
    pub auto_clean: bool,
    pub keep_min_directories: usize,
    pub keep_max_directories: usize,
    #[serde(with="serialize::duration")]
    pub keep_recent: Duration,
}

pub fn get_hash(cfg: &Arc<Directory>) -> [u8; 64] {
    let mut result = [0u8; 64];
    let cfg = RemoteConfig {
        num_levels: cfg.num_levels,
        auto_clean: cfg.auto_clean,
        keep_min_directories: cfg.keep_min_directories,
        keep_max_directories: cfg.keep_max_directories,
        keep_recent: *cfg.keep_recent,
    };
    let mut dig = digest_writer::Writer::new(Blake2b::<U32>::new());
    to_writer(&mut dig, &cfg)
        .expect("can always serialize/hash structure");
    result[..].copy_from_slice(&dig.into_inner().result()[..]);
    return result;
}
