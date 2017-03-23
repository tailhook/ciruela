use std::sync::Arc;
use std::path::PathBuf;

use tk_easyloop;
use futures::Future;
use futures::sync::oneshot::channel;

use config::Directory;
use ciruela::ImageId;
use tracking::Tracking;
use tracking::Index;


pub struct FetchDir {
    pub image_id: ImageId,
    pub base_dir: PathBuf,
    pub parent: PathBuf,
    pub image_name: String,
    pub config: Arc<Directory>,
}

pub fn start(tracking: &Tracking, cmd: FetchDir) {
    let mut state = tracking.state();
    let cached = state.images.get(&cmd.image_id)
        .and_then(|x| x.upgrade());
    if let Some(index) = cached {
        println!("Image {:?} is already cached", cmd.image_id);
    } else {
        let (tx, rx) = channel::<Index>();
        tk_easyloop::spawn(rx.shared()
            .map(|index| {
                println!("Got image {:?}", index.id)
            })
            .map_err(|e| {
                println!("Error getting image {:?}", e)
            }));
    }
}
