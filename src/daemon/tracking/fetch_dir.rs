use std::sync::Arc;

use futures::{Future, Stream};
use futures::stream::iter;
use futures::sync::oneshot::channel;
use tk_easyloop::spawn;

use ciruela::Hash;
use disk::{self, Image};
use tracking::{Subsystem, Downloading};
use tracking::fetch_blocks::FetchBlocks;


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
                        let img = Arc::new(img);
                        debug!("Created dir");
                        cmd.slices.start(&img.index);
                        fetch_blocks(sys.clone(), img, cmd);
                    }
                    Err(disk::Error::AlreadyExists) => {
                        sys.meta.dir_committed(&cmd.virtual_path);
                        sys.remote.notify_received_image(
                            &index.id, &cmd.virtual_path);
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

fn fetch_blocks(sys: Subsystem, image: Arc<Image>, cmd: Arc<Downloading>)
{
    let sys1 = sys.clone();
    let sys2 = sys.clone();
    spawn(FetchBlocks::new(&image, &cmd, &sys)
        .and_then(move |()| {
            sys1.disk.commit_image(image)
            .map_err(|e| {
                error!("Error commiting image: {}", e);
                unimplemented!();
            })
        })
        .map(move |()| {
            sys2.meta.dir_committed(&cmd.virtual_path);
            sys2.remote.notify_received_image(
                &cmd.image_id, &cmd.virtual_path);
        }));
}
