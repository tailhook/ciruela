use std::sync::Arc;
use std::path::PathBuf;

use tk_easyloop;
use futures::Future;
use futures::sync::oneshot::channel;

use config::Directory;
use ciruela::ImageId;
use tracking::Tracking;
use tracking::index::{Index, IndexData};


pub struct FetchDir {
    pub image_id: ImageId,
    pub base_dir: PathBuf,
    pub parent: PathBuf,
    pub image_name: String,
    pub config: Arc<Directory>,
}

pub fn start(tracking: &Tracking, cmd: FetchDir) {
    let t1 = tracking.clone();
    let mut state = &mut *tracking.state();
    let cached = state.images.get(&cmd.image_id)
        .and_then(|x| x.upgrade()).map(Index);
    if let Some(index) = cached {
        println!("Image {:?} is already cached", cmd.image_id);
        return;
    }
    let old_future = state.image_futures.get(&cmd.image_id).map(Clone::clone);
    let future = if let Some(future) = old_future {
        debug!("Image {:?} is already being fetched", cmd.image_id);
        future.clone()
    } else {
        let (tx, rx) = channel::<Index>();
        let future = rx.shared();
        state.image_futures.insert(cmd.image_id.clone(), future.clone());
        future
    };
    tk_easyloop::spawn(future
        .then(move |result| {
            match result {
                Ok(index) => {
                    println!("Got image {:?}", index.id);
                    t1.state().images.insert(cmd.image_id,
                        index.weak());
                }
                Err(e) => {
                    println!("Error getting image {:?}", e);
                    t1.state().image_futures.remove(&cmd.image_id);
                }
            }
            Ok(())
        }));
}
