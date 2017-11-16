use std::sync::Arc;

use futures::{Future};
use tk_easyloop::spawn;
use void::unreachable;

use disk::{Image};
use tracking::{Subsystem, Downloading};
use tracking::fetch_blocks::FetchBlocks;


pub fn start(sys: &Subsystem, cmd: Downloading) {
    let cmd = Arc::new(cmd);
    let cmd2 = cmd.clone();
    sys.rescan_dir(cmd.virtual_path.parent());
    {
        let mut state = sys.state();
        state.recently_deleted.remove(
            &(cmd.virtual_path.clone(), cmd.image_id.clone()));
        state.in_progress.insert(cmd.clone());
    }
    sys.peers.notify_progress(
        &cmd.virtual_path, &cmd.image_id, cmd.mask.get(),
        sys.remote.has_image_source(&cmd.image_id));
    let sys = sys.clone();
    let sys2 = sys.clone();
    spawn(sys.images.get(&sys.tracking, &cmd.virtual_path, &cmd.image_id)
    .map_err(move |e| {
        error!("Error fetching index: {}. \
            We abort downloading of {} to {:?}", e,
            &cmd2.image_id, &cmd2.virtual_path);
        sys2.dir_deleted(&cmd2.virtual_path, &cmd2.image_id);
        sys2.state().in_progress.remove(&cmd2);
        sys2.meta.dir_aborted(&cmd2.virtual_path);
        sys2.remote.notify_aborted_image(
            &cmd2.image_id, &cmd2.virtual_path,
            "cant_fetch_index".into());
    })
    .and_then(|index| {
        debug!("Got index {:?}", cmd.image_id);
        spawn(sys.disk.start_image(
                cmd.config.directory.clone(),
                index.clone(),
                cmd.virtual_path.clone())
            .then(move |res| -> Result<(), ()> {
                match res {
                    Ok(img) => {
                        let img = Arc::new(img);
                        debug!("Created dir");
                        cmd.index_fetched(&img.index);
                        sys.peers.notify_progress(&cmd.virtual_path,
                            &cmd.image_id, cmd.mask.get(),
                            sys.remote.has_image_source(&cmd.image_id));
                        hardlink_blocks(sys.clone(), img, cmd);
                    }
                    Err(e) => {
                        error!("Can't start image {:?}: {}",
                            cmd.virtual_path, e);
                        sys.dir_deleted(&cmd.virtual_path, &cmd.image_id);
                        sys.state().in_progress.remove(&cmd);
                        sys.meta.dir_aborted(&cmd.virtual_path);
                        sys.remote.notify_aborted_image(
                            &cmd.image_id, &cmd.virtual_path,
                            "cant_create_directory".into());
                    }
                }
                Ok(())
            }));
        Ok(())
    }));
}

fn hardlink_blocks(sys: Subsystem, image: Arc<Image>, cmd: Arc<Downloading>) {
    let sys2 = sys.clone();
    let sys3 = sys.clone();
    let cmd2 = cmd.clone();
    let image2 = image.clone();
    spawn(sys.meta.files_to_hardlink(&cmd.virtual_path, &image.index)
        .map_err(move |e| {
            error!("Error fetching hardlink sources: {}", e);
            // TODO(tailhook) remove temporary directory
            sys2.dir_deleted(&cmd2.virtual_path, &cmd2.image_id);
            sys2.state().in_progress.remove(&cmd2);
            sys2.meta.dir_aborted(&cmd2.virtual_path);
            sys2.remote.notify_aborted_image(
                &cmd2.image_id, &cmd2.virtual_path,
                "internal_error_when_hardlinking".into());
        })
        .and_then(move |sources| {
            sys3.disk.check_and_hardlink(sources, &image2)
                .map_err(|e| unreachable(e))
        })
        .map(move |hardlink_paths| {
            cmd.fill_blocks(&image.index, hardlink_paths);
            fetch_blocks(sys.clone(), image, cmd);
        })
        );
}

fn fetch_blocks(sys: Subsystem, image: Arc<Image>, cmd: Arc<Downloading>)
{
    let sys1 = sys.clone();
    let sys2 = sys.clone();
    let sys3 = sys.clone();
    let cmd1 = cmd.clone();
    let cmd3 = cmd.clone();
    spawn(FetchBlocks::new(&image, &cmd, &sys)
        .map_err(move |()| {
            // TODO(tailhook) remove temporary directory
            sys3.dir_deleted(&cmd3.virtual_path, &cmd3.image_id);
            sys3.state().in_progress.remove(&cmd3);
            sys3.meta.dir_aborted(&cmd3.virtual_path);
            sys3.remote.notify_aborted_image(
                &cmd3.image_id, &cmd3.virtual_path,
                "cluster_abort_no_file_source".into());
        })
        .and_then(move |()| {
            sys1.disk.commit_image(image)
            .map_err(move |e| {
                error!("Error commiting image: {}", e);
                // TODO(tailhook) remove temporary directory
                sys1.dir_deleted(&cmd1.virtual_path, &cmd1.image_id);
                sys1.state().in_progress.remove(&cmd1);
                sys1.meta.dir_aborted(&cmd1.virtual_path);
                sys1.remote.notify_aborted_image(
                    &cmd1.image_id, &cmd1.virtual_path,
                    "commit_error".into());
            })
        })
        .map(move |()| {
            sys2.state().in_progress.remove(&cmd);
            sys2.meta.dir_committed(&cmd.virtual_path);
            sys2.remote.notify_received_image(
                &cmd.image_id, &cmd.virtual_path);
        }));
}
