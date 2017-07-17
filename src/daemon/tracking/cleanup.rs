use std::sync::Arc;
use std::time::Duration;

use futures::{Future, Stream};
use futures::sync::mpsc::{UnboundedReceiver};
use tk_easyloop::{timeout, spawn};

use tracking::{Subsystem, BaseDir};


pub enum Command {
    Base(Arc<BaseDir>),
    Reschedule,
}

pub fn spawn_loop(rx: UnboundedReceiver<Command>, sys: &Subsystem) {
    let sys = sys.clone();
    spawn(rx
        .filter_map(move |x| {
            match x {
                Command::Base(ref dir) => {
                    Some(sys.meta.scan_dir(dir)
                        .and_then(|lst| {
                            // TODO(tailhook) do cleanup check
                            Ok(())
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
                    None
                }
            }
        })
        .map_err(|_| {
            unimplemented!();
        })
        .for_each(|f|
            f.and_then(|()| timeout(Duration::new(10, 0))
                            .map_err(|e| unreachable!()))
             .map_err(|_| unimplemented!())));
}
