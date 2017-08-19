use std::io::Cursor;
use std::sync::Arc;

use futures::{Future, Stream};
use futures::future::{ok, Either};
use futures::stream::iter;
use futures::sync::oneshot::channel;
use quick_error::ResultExt;
use tk_easyloop::spawn;

use ciruela::Hash;
use disk::{self, Image};
use index::IndexData;
use metadata::Error;
use tracking::{Subsystem, Block, Downloading};


pub fn start(sys: &Subsystem, cmd: Downloading) {
    let cmd = Arc::new(cmd);
    sys.rescan_dir(cmd.virtual_path.parent());
    sys.state().in_progress.insert(cmd.clone());
    let sys = sys.clone();
    spawn(sys.images.get(&sys.tracking, &cmd.image_id).and_then(|index| {
        info!("Image {:?} is already cached", cmd.image_id);
        spawn(sys.disk.start_image(
                cmd.config.directory.clone(),
                index.clone(),
                cmd.virtual_path.clone(),
                cmd.replacing)
            .then(move |res| -> Result<(), ()> {
                match res {
                    Ok(img) => {
                        debug!("Created dir");
                        fetch_blocks(sys.clone(), img, cmd);
                    }
                    Err(disk::Error::AlreadyExists) => {
                        sys.meta.dir_committed(&cmd.virtual_path);
                        sys.remote.notify_received_image(index.id.clone(),
                            &cmd.virtual_path);
                        info!("Image already exists {:?}", cmd);
                    }
                    Err(e) => {
                        error!("Can't start image {:?}: {}",
                            cmd.virtual_path, e);
                    }
                }
                Ok(())
            }));
        Ok(())
    }).map_err(|e| {
        error!("Error fetching index: {}", e);
    }));
}

fn fetch_blocks(sys: Subsystem, image: Image, cmd: Arc<Downloading>)
{
    use dir_signature::v1::Entry::*;

    let image = Arc::new(image);
    let image2 = image.clone();
    let image_id = image.index.id.clone();
    let sys = sys.clone();
    let sys2 = sys.clone();
    let sys3 = sys.clone();
    let cmd1 = cmd.clone();
    // TODO(tailhook) implement cancellation and throttling
    // TODO(tailhook) maybe global BufferUnordered rather then per-image
    spawn(iter(
            image.index.entries.iter()
            .flat_map(|entry| {
                match *entry {
                    Dir(..) => Vec::new(),
                    Link(..) => Vec::new(),
                    File { ref hashes, ref path, .. } => {
                        let arc = Arc::new(path.clone());
                        let mut result = Vec::new();
                        for (i, hash) in hashes.iter().enumerate() {
                            let hash = Hash::new(hash);
                            result.push(Ok((
                                hash,
                                arc.clone(),
                                (i as u64)*image.index.block_size,
                            )))
                        }
                        result
                    }
                }
            }).collect::<Vec<_>>()
        ).map(move |(hash, path, offset)| {
            let h1 = hash.clone();
            let h2 = hash.clone();
            let h3 = hash.clone();
            let sys = sys.clone();
            let sys1 = sys.clone();
            let sys2 = sys.clone();
            let cmd1 = cmd1.clone();
            let image = image.clone();
            let mut state = sys.state();
            let fut = state.block_futures.get(&hash).cloned();
            let fut = if let Some(fut) = fut {
                fut
            } else {
                let (tx, rx) = channel::<Block>();
                let fut = rx.shared();
                state.block_futures.insert(hash, fut.clone());
                if let Some(conn) =
                    sys.remote.get_incoming_connection_for_index(&image.index.id)
                {
                    spawn(
                        conn.fetch_block(&h1)
                        .map_err(move |e| {
                            // TODO(tailhook) ask another connection
                            error!("Error fetching block {}: {}", h1, e);
                        })
                        // TODO(tailhook) check hashsum
                        .and_then(move |block| {
                            tx.send(Arc::new(block.data))
                            .map_err(|_| {
                                debug!("Block future for {} is dropped before \
                                        resolving", h2);
                            })
                        }));
                    fut
                } else {
                    unimplemented!();
                }
            };
            fut
            .and_then(move |block| {
                sys1.disk.write_block(image, path, offset, block.clone())
                .map(move |r| {
                    cmd1.report_block(&*block);
                    r
                })
                .map_err(|e| {
                    error!("Error writing block: {}", e);
                    unimplemented!();
                })
            })
            .map(move |()| {
                sys2.state().block_futures.remove(&h3);
            })
        })
        .buffer_unordered(10)
        .map_err(|e| {
            error!("Error fetching block: {}", *e);
            unimplemented!();
        })
        .for_each(|()| {
            Ok(())
        })
        .and_then(move |()| {
            sys2.disk.commit_image(image2)
        })
        .map(move |()| {
            sys3.meta.dir_committed(&cmd.virtual_path);
            sys3.remote.notify_received_image(image_id, &cmd.virtual_path);
        })
        .map_err(|e| {
            error!("Error commiting image: {}", e);
            unimplemented!();
        }));
}
