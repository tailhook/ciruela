use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::path::PathBuf;

use futures::{Future, Stream};
use futures::future::{Either, ok};
use futures::stream::iter;
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
    all: Vec<(String, State)>, keep_list: Vec<PathBuf>)
    -> Vec<Image>
{
    let images = all.into_iter().map(|(name, state)| {
        Image {
            path: dir.virtual_path.suffix().join(name),
            target_state: state,
        }
    }).collect();
    // TODO(tailhook) read keep list
    let sorted = sort_out(&dir.config, images, &keep_list);
    info!("sorted out {:?}, used {}, unused {}, keep_list: {}. {}",
        dir.virtual_path,
        sorted.used.len(), sorted.unused.len(), keep_list.len(),
        if sorted.unused.len() > 0 {
            if sys.dry_cleanup() {
                "Dry run... \
                 Will issue a cleanup in 10 minutes after startup."
            } else {
                "Cleaning..."
            }
        } else {
            "Nothing to do."
        });
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
                    let sys1 = sys.clone();
                    let sys2 = sys.clone();
                    let time = SystemTime::now();
                    Either::A(
                        sys.meta.scan_dir(dir).map_err(boxerr)
                        .join(sys.disk.read_keep_list(&dir.config)
                              .map_err(boxerr))
                        .and_then(move |(lst, keep_list)| {
                            let u = find_unused(&sys1, &dir1, lst, keep_list);
                            iter(u.into_iter().map(Ok))
                            .for_each(move |img| {
                                sys2.meta.remove_state_file(
                                    dir1.virtual_path.join(img.path), time)
                                .map_err(boxerr)
                                // TODO(tailhook) clean the image itself
                            })
                        })
                        .then(move |result| {
                            match result {
                                Ok(()) => Ok(()),
                                Err(e) => {
                                    error!("cleanup error for {:?}: {}",
                                        dir2, e);
                                    Ok(())
                                }
                            }
                        }))
                }
                Command::Reschedule => {
                    let state = sys.state();
                    debug!("Rescheduling {} base dirs", state.base_dirs.len());
                    for dir in &state.base_dirs {
                        sys.cleanup.send(Command::Base(dir.clone()))
                            .expect("can always send in cleanup channel");
                    }
                    sys.cleanup.send(Command::Reschedule)
                        .expect("can always send in cleanup channel");
                    Either::B(ok(()))
                }
            }
        })
        .map_err(|_| {
            unimplemented!();
        })
        .for_each(|f|
            f.and_then(|()| timeout(Duration::new(10, 0))
                            .map_err(|_| unreachable!()))
             .map_err(|_| unimplemented!())));
}
