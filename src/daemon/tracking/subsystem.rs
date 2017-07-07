use futures::Future;
use futures::Stream;
use futures::stream::iter;
use futures::sync::oneshot::channel;
use tk_easyloop::spawn;

use ciruela::{ImageId, Hash};
use index::Index;
use tracking::{Subsystem, Block};


impl Subsystem {
    pub fn commit_index_and_fetch(&self, image_id: &ImageId,
        index: &Index)
    {
        use dir_signature::v1::Entry::*;

        let sys = self.clone();
        let image_id = image_id.clone();
        self.state().images.insert(image_id.clone(), index.weak());
        // TODO(tailhook) implement cancellation and throttling
        // TODO(tailhook) maybe global BufferUnordered rather then per-image
        spawn(iter(
                index.entries.iter()
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
                    sys.remote.get_connection_for_index(&image_id)
                {
                    spawn(
                        conn.fetch_block(&h1)
                        .map_err(move |e| {
                            // TODO(tailhook) ask another connection
                            error!("Error fetching block {}: {}", h1, e);
                        })
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
                println!("Fetch block result {:?}", x);
                Ok(())
            })
            .map_err(|e| {
                // TODO(tailhook) figure out how to silence the future earlier
                error!("Shared future error: {:?}", e);
            }));
    }
}
