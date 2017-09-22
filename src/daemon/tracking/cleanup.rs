use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use futures::{Future, Stream};
use futures::future::{Either, ok};
use futures::stream::iter_ok;
use futures::sync::mpsc::{UnboundedReceiver};
use tk_easyloop::{timeout, spawn};

use cleanup::{Image, sort_out};
use tracking::{Subsystem, BaseDir};
use ciruela::database::signatures::State;


pub enum Command {
    Base(Arc<BaseDir>),
    Reschedule,
}

fn find_unused(sys: &Subsystem, dir: &Arc<BaseDir>,
    all: BTreeMap<String, State>, keep_list: Vec<PathBuf>)
    -> Vec<Image>
{
    let images = all.into_iter().map(|(name, state)| {
        Image {
            path: dir.path.suffix().join(name),
            target_state: state,
        }
    }).collect();
    // TODO(tailhook) read keep list
    let sorted = sort_out(&dir.config, images, &keep_list);
    if sorted.unused.len() > 0 {
        info!("Sorted out {:?}, used {}, unused {}, keep_list: {}. {}",
            dir.path, sorted.used.len(), sorted.unused.len(), keep_list.len(),
            if sys.dry_cleanup() {
                "Dry run... \
                 Will issue a cleanup in 10 minutes after startup."
            } else {
                "Cleaning..."
            });
    } else {
        debug!("Sorted out {:?}, used {}, unused {}, keep_list: {}. {}",
            dir.path, sorted.used.len(), sorted.unused.len(), keep_list.len(),
            "Nothing to do.");
    }
    sorted.unused
}

fn boxerr<E: ::std::error::Error + Send + 'static>(e: E)
    -> Box<::std::error::Error + Send>
{
    Box::new(e) as Box<::std::error::Error + Send>
}

pub fn spawn_loop(rx: UnboundedReceiver<Command>, sys: &Subsystem) {
    let sys = sys.clone();
    spawn(rx
        .map(move |x| {
            match x {
                Command::Base(ref dir) => {
                    let dir1 = dir.clone();
                    let dir2 = dir.clone();
                    let dir3 = dir.clone();
                    let sys1 = sys.clone();
                    let sys2 = sys.clone();
                    let time = SystemTime::now();
                    Either::A(
                        sys.meta.scan_dir(&dir.path).map_err(boxerr)
                        .join(sys.disk.read_keep_list(&dir.config)
                              .map_err(boxerr))
                        .and_then(move |(lst, keep_list)| {
                            let u = find_unused(&sys1, &dir1, lst, keep_list);
                            iter_ok(u.into_iter())
                            .for_each(move |img| {
                                let vpath = dir1.path.join(
                                        &img.path.file_name()
                                        .expect("valid image path"));
                                warn!("Removing {:?}", vpath);
                                let cfg = dir2.config.clone();
                                let sys = sys2.clone();
                                sys.meta.remove_state_file(vpath, time)
                                .map_err(boxerr)
                                .and_then(move |()| {
                                    sys.disk.remove_image(&cfg, img.path)
                                    .map_err(boxerr)
                                })
                                // TODO(tailhook) clean the image itself
                            })
                        })
                        .then(move |result| {
                            match result {
                                Ok(()) => Ok(()),
                                Err(e) => {
                                    error!("cleanup error for {:?}: {}",
                                        dir3, e);
                                    Ok(())
                                }
                            }
                        }))
                }
                Command::Reschedule => {
                    let state = sys.state();
                    debug!("Rescheduling {} base dirs", state.base_dirs.len());
                    for dir in state.base_dirs.values() {
                        if dir.config.auto_clean {
                            sys.cleanup
                                .unbounded_send(Command::Base(dir.clone()))
                                .expect("can always send in cleanup channel");
                        }
                    }
                    sys.cleanup.unbounded_send(Command::Reschedule)
                        .expect("can always send in cleanup channel");
                    Either::B(ok(()))
                }
            }
        })
        .map_err(|()| {
            error!("Cleanup fatal error");
            // TODO(tailhook) sleep and retry?
            // or is it fatal?
            ::std::process::exit(103);
        })
        .for_each(|f|
            f.and_then(|()| timeout(Duration::new(10, 0))
                            .map_err(|_| unreachable!()))
             .map_err(|_| unimplemented!())));
}
