use std::sync::Arc;

use futures::{Future};
use tk_easyloop::spawn;

use disk::{self, Image};
use tracking::{Subsystem, Downloading};
use tracking::fetch_blocks::FetchBlocks;


pub fn start(sys: &Subsystem, cmd: Downloading) {
    let cmd = Arc::new(cmd);
    sys.rescan_dir(cmd.virtual_path.parent());
    sys.state().in_progress.insert(cmd.clone());
    sys.peers.notify_progress(
        &cmd.virtual_path, &cmd.image_id, cmd.mask.get());
    let sys = sys.clone();
    spawn(sys.images.get(&sys.tracking, &cmd.virtual_path, &cmd.image_id)
    .and_then(|index| {
        debug!("Got index {:?}", cmd.image_id);
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
                        cmd.index_fetched(&img.index);
                        sys.peers.notify_progress(&cmd.virtual_path,
                            &cmd.image_id, cmd.mask.get());
                        hardlink_blocks(sys.clone(), img, cmd);
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
                        // TODO(tailhook) should we crash here?
                        //                the problem is we're
                        //                still occuping entry in `writing`
                    }
                }
                Ok(())
            }));
        Ok(())
    }).map_err(|e| {
        error!("Error fetching index: {}", e);
        // TODO(tailhook) should we crash here?
        //                the problem is we're still
        //                occuping entry in `writing`
    }));
}

fn hardlink_blocks(sys: Subsystem, image: Arc<Image>, cmd: Arc<Downloading>) {
    spawn(sys.meta.files_to_hardlink(&cmd.virtual_path, &image.index)
        .map(move |_sources| {
            //println!("Files {:#?}", sources);
            cmd.fill_blocks(&image.index);
            fetch_blocks(sys.clone(), image, cmd);
        })
        .map_err(|e| {
            error!("Error fetching hardlink sources: {}", e);
            panic!("Hardlink sources fetch error")
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
                // TODO(tailhook) drop image and start over?
                //                find out which error?
                //                wait and retry?
                ::std::process::exit(104);
            })
        })
        .map(move |()| {
            sys2.meta.dir_committed(&cmd.virtual_path);
            sys2.remote.notify_received_image(
                &cmd.image_id, &cmd.virtual_path);
        }));
}
