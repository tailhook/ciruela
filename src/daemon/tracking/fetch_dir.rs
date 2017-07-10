use std::io::Cursor;
use std::sync::Arc;
use std::path::PathBuf;

use futures::{Future, Stream};
use futures::future::{ok, Either};
use futures::stream::iter;
use futures::sync::oneshot::channel;
use quick_error::ResultExt;
use tk_easyloop::spawn;

use ciruela::{ImageId, Hash};
use config::Directory;
use disk::Image;
use index::Index;
use metadata::Error;
use tracking::{Subsystem, Block};


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
        info!("Image {:?} is already cached", cmd.image_id);
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
        spawn(sys.meta.read_index(&cmd.image_id)
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
                            // TODO(tailhook) check another connection on error
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
    spawn(future
        .then(move |result| {
            match result {
                Ok(index) => {
                    sys1.state().images
                        .insert(cmd.image_id.clone(), index.weak());
                    // TODO(tailhook) sys1.meta.store_index()
                    Either::A(sys1.disk.start_image(
                        cmd.config.directory.clone(),
                        cmd.parent.clone(),
                        cmd.image_name.clone(),
                        index.clone())
                    .map(move |img| {
                        debug!("Created dir");
                        fetch_blocks(sys1, img);
                    })
                    .map_err(move |e| {
                        error!("Can't start image {:?}: {}",
                            cmd.image_name, e);
                    }))
                }
                Err(e) => {
                    error!("Error getting image {:?}", e);
                    sys1.state().image_futures.remove(&cmd.image_id);
                    Either::B(ok(()))
                }
            }
        }));
}

fn fetch_blocks(sys: Subsystem, image: Image)
{
    use dir_signature::v1::Entry::*;

    let sys = sys.clone();
    // TODO(tailhook) implement cancellation and throttling
    // TODO(tailhook) maybe global BufferUnordered rather then per-image
    spawn(iter(
            image.index.entries.iter()
            .flat_map(|entry| {
                match *entry {
                    Dir(..) => Vec::new(),
                    Link(..) => Vec::new(),
                    File { ref hashes, .. } => {
                        let mut result = Vec::new();
                        for hash in hashes.iter() {
                            let hash = Hash::new(hash);
                            result.push(Ok(hash))
                        }
                        result
                    }
                }
            }).collect::<Vec<_>>()
        ).map(move |hash| {
            let h1 = hash.clone();
            let h2 = hash.clone();
            let sys = sys.clone();
            let mut state = sys.state();
            if let Some(fut) = state.block_futures.get(&hash) {
                return fut.clone();
            }
            let (tx, rx) = channel::<Block>();
            let fut = rx.shared();
            state.block_futures.insert(hash, fut.clone());
            if let Some(conn) =
                sys.remote.get_connection_for_index(&image.index.id)
            {
                spawn(
                    conn.fetch_block(&h1)
                    .map_err(move |e| {
                        // TODO(tailhook) ask another connection
                        error!("Error fetching block {}: {}", h1, e);
                    })
                    // TODO(tailhook) check hashsum
                    .and_then(move |block| tx.send(block.data)
                        .map_err(|_| {
                            debug!("Block future for {} is dropped before \
                                    resolving", h2);
                    })));
            } else {
                unimplemented!();
            }
            fut
        })
        .buffer_unordered(10)
        .for_each(|x| {
            println!("Fetch block result {:?}", *x);
            Ok(())
        })
        .map_err(|e| {
            // TODO(tailhook) figure out how to silence the future earlier
            error!("Shared future error: {:?}", e);
        }));
}
