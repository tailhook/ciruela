use std::collections::HashMap;
use std::sync::Arc;
use std::path::Path;
use std::sync::atomic::{AtomicUsize};

use futures::Future;
use tk_easyloop::spawn;

use tracking::{Subsystem, BaseDir};


pub fn spawn_scan(sys: &Subsystem) {
    let sys = sys.clone();
    spawn(sys.meta.scan_base_dirs()
         .and_then(move |items| {
            let state = &mut *sys.state();
            let mut downloading = HashMap::new();
            for dw in &state.in_progress {
                *downloading.entry(
                    dw.virtual_path.parent()
                    .expect("virtual path always has parent")
                ).or_insert(0) += 1;
            }
            let mut sum = 0;
            let mut ndown = 0;
            state.base_dirs.extend(items.into_iter().map(|(dir, num)| {
                let cur_down = downloading.remove(dir.as_path()).unwrap_or(0);
                sum += num;
                ndown += cur_down;
                Arc::new(BaseDir {
                    virtual_path: dir,
                    num_subdirs: AtomicUsize::new(num),
                    num_downloading: AtomicUsize::new(cur_down),
                })
            }));
            if downloading.len() > 0 {
                error!("Downloadinging in non-existing base dirs {:?}",
                    downloading);
            }
            info!("Initial scan complete. Base dirs: {}, having {} states. \
                And {} downloads in progress.",
                state.base_dirs.len(), sum, ndown);
            Ok(())
         })
         .map_err(|e| {
            panic!("First scan failed: {}", e);
         }));
}
