use std::sync::Arc;
use std::time::Duration;

use serialize;
use proto::Hash;
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

pub fn get_hash(cfg: &Arc<Directory>) -> Hash {
    Hash::for_object(&RemoteConfig {
        num_levels: cfg.num_levels,
        auto_clean: cfg.auto_clean,
        keep_min_directories: cfg.keep_min_directories,
        keep_max_directories: cfg.keep_max_directories,
        keep_recent: cfg.keep_recent,
    })
}
