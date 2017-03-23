use std::sync::Arc;
use std::path::PathBuf;

use config::Directory;
use ciruela::ImageId;


pub struct FetchDir {
    pub image_id: ImageId,
    pub base_dir: PathBuf,
    pub parent: PathBuf,
    pub image_name: String,
    pub config: Arc<Directory>,
}

pub fn start(cmd: FetchDir) {
    unimplemented!();
}
