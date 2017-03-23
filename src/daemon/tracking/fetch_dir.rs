use std::sync::Arc;
use std::path::PathBuf;

use tk_easyloop;
use futures::Future;
use futures::sync::oneshot::channel;

use ciruela::ImageId;
use config::Directory;
use index::{Index, IndexData};
use tracking::{Tracking, Subsystem};


pub struct FetchDir {
    pub image_id: ImageId,
    pub base_dir: PathBuf,
    pub parent: PathBuf,
    pub image_name: String,
    pub config: Arc<Directory>,
}

pub fn start(sys: &Subsystem, cmd: FetchDir) {
    let sys1 = sys.clone();
    let mut state = &mut *sys.state();
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
        tk_easyloop::spawn(sys.meta.read_index(&cmd.image_id)
            .then(|result| {
                println!("Metadata result {:?}", result.is_ok());
                Ok(())
            }));
        future
    };
    tk_easyloop::spawn(future
        .then(move |result| {
            match result {
                Ok(index) => {
                    println!("Got image {:?}", index.id);
                    sys1.state().images.insert(cmd.image_id,
                        index.weak());
                }
                Err(e) => {
                    println!("Error getting image {:?}", e);
                    sys1.state().image_futures.remove(&cmd.image_id);
                }
            }
            Ok(())
        }));
}
