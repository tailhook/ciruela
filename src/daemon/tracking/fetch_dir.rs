use std::io::Cursor;
use std::sync::Arc;
use std::path::PathBuf;

use tk_easyloop;
use futures::Future;
use futures::sync::oneshot::channel;
use quick_error::ResultExt;

use ciruela::ImageId;
use config::Directory;
use index::Index;
use tracking::{Subsystem};
use metadata::Error;


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
    let cmd = Arc::new(cmd);
    if let Some(index) = cached {
        println!("Image {:?} is already cached", cmd.image_id);
        return;
    }
    let old_future = state.image_futures.get(&cmd.image_id).map(Clone::clone);
    let future = if let Some(future) = old_future {
        debug!("Index {:?} is already being fetched", cmd.image_id);
        future.clone()
    } else {
        let (tx, rx) = channel::<Index>();
        let future = rx.shared();
        let cmd = cmd.clone();
        let sys = sys.clone();
        state.image_futures.insert(cmd.image_id.clone(), future.clone());
        tk_easyloop::spawn(sys.meta.read_index(&cmd.image_id)
            .then(move |result| {
                match result {
                    Ok(index) => {
                        info!("Index {:?} is read from store", index.id);
                        unimplemented!();
                    }
                    Err(e) => {
                        if matches!(e, Error::IndexNotFound) {
                            info!("Index {:?} can't be found in store",
                                cmd.image_id);
                        } else {
                            error!("Error reading index {:?}: {}. \
                                    Will try to fetch... ",
                                   cmd.image_id, e);
                        }
                        let conn_opt = sys.remote.get_connection_for_index(
                            &cmd.image_id);
                        if let Some(conn) = conn_opt {
                            // TODO(tailhook) also set timeout?
                            conn.fetch_index(&cmd.image_id)
                            .and_then(move |response| {
                                Index::parse(&cmd.image_id,
                                    Cursor::new(response.data))
                                .context(&cmd.image_id)
                                .map_err(|e| e.into())
                            })
                            .map(|idx| {
                                tx.send(idx)
                                .map_err(|_| debug!("Useless index fetch"))
                                .ok();
                            })
                            // TODO(tailhok) check another connection on error
                            .map_err(|e| error!("Error fetching index: {}", e))
                            .map_err(|()| unimplemented!())
                        } else {
                            unimplemented!();
                        }
                    }
                }
            }));
        future
    };
    tk_easyloop::spawn(future
        .then(move |result| {
            match result {
                Ok(index) => {
                    println!("Got image {:?}", index.id);
                    sys1.state().images.insert(cmd.image_id.clone(),
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
