use std::sync::Arc;

use futures::{Future};
use tk_easyloop::spawn;
use void::unreachable;

use disk::{Image};
use metrics::Counter;
use tracking::{Subsystem, Downloading, DOWNLOADING};
use tracking::fetch_blocks::FetchBlocks;


lazy_static! {
    pub static ref FAILURES: Counter = Counter::new();
}


pub fn start(sys: &Subsystem, cmd: Downloading) {
    let cmd = Arc::new(cmd);
    let cmd2 = cmd.clone();
    sys.rescan_dir(cmd.virtual_path.parent());
    {
        let mut state = sys.state();
        state.recently_deleted.remove(
            &(cmd.virtual_path.clone(), cmd.image_id.clone()));
        state.in_progress.insert(cmd.clone());
        state.base_dirs.get(&cmd.virtual_path.parent())
            .map(|b| b.incr_downloading());
        DOWNLOADING.set(state.in_progress.len() as i64);
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
        spawn(sys2.meta.dir_aborted(&cmd2.virtual_path)
            .map_err(|e| unreachable(e))
            .map(move |()| {
                sys2.dir_aborted(&cmd2, "cant_fetch_index")
            }));
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
                        spawn(sys.meta.dir_aborted(&cmd.virtual_path)
                            .map_err(|e| unreachable(e))
                            .map(move |()| {
                                sys.dir_aborted(&cmd, "cant_create_directory")
                            }));
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
            spawn(sys2.meta.dir_aborted(&cmd2.virtual_path)
                .map_err(|e| unreachable(e))
                .map(move |()| {
                    sys2.dir_aborted(&cmd2,
                        "internal_error_when_hardlinking")
                }));
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
    let sys4 = sys.clone();
    let cmd1 = cmd.clone();
    let cmd2 = cmd.clone();
    let cmd3 = cmd.clone();
    spawn(FetchBlocks::new(&image, &cmd, &sys)
        .map_err(move |()| {
            FAILURES.incr(1);
            // TODO(tailhook) remove temporary directory
            spawn(sys3.meta.dir_aborted(&cmd3.virtual_path)
                .map_err(|e| unreachable(e))
                .map(move |()| {
                    sys3.dir_aborted(&cmd3, "cluster_abort_no_file_source")
                }));
        })
        .and_then(move |()| {
            sys1.disk.commit_image(image)
            .map_err(move |e| {
                error!("Error commiting image: {}", e);
                // TODO(tailhook) remove temporary directory
                spawn(sys1.meta.dir_aborted(&cmd1.virtual_path)
                    .map_err(|e| unreachable(e))
                    .map(move |()| {
                        sys1.dir_aborted(&cmd1, "commit_error")
                    }));
            })
        })
        .and_then(move |()| {
            sys2.meta.dir_committed(&cmd2.virtual_path)
                .map_err(|e| unreachable(e))
        })
        .map(move |()| sys4.dir_committed(&cmd)));
}
