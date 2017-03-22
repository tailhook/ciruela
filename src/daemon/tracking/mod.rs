mod index;

pub use self::index::Index;

use std::sync::{Arc, Weak, Mutex, MutexGuard};
use std::collections::HashMap;
use std::path::PathBuf;

use futures::Future;
use futures::future::Shared;
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use futures::sync::oneshot::{channel, Receiver};
use tk_easyloop;

use metadata::Meta;
use remote::Remote;
use disk::Disk;
use ciruela::ImageId;
use dir_config::DirConfig;
use config::Directory;

type ImageFuture = Shared<Receiver<Index>>;

pub struct State {
    image_futures: HashMap<ImageId, ImageFuture>,
    images: HashMap<ImageId, Weak<Index>>,
}

#[derive(Clone)]
pub struct Tracking(Arc<TrackingInternal>);

struct TrackingInternal {
    chan: UnboundedSender<Command>,
    state: Mutex<State>,
}

pub struct TrackingInit {
    chan: UnboundedReceiver<Command>,
}

pub enum Command {
    FetchDir {
        image_id: ImageId,
        base_dir: PathBuf,
        parent: PathBuf,
        image_name: String,
        config: Arc<Directory>,
    },
}

impl Tracking {
    pub fn new() -> (Tracking, TrackingInit) {
        let (tx, rx) = unbounded();
        (Tracking(Arc::new(TrackingInternal {
            state: Mutex::new(State {
                image_futures: HashMap::new(),
                images: HashMap::new(),
            }),
            chan: tx,
         })),
         TrackingInit {
            chan: rx,
         })
    }
    pub fn fetch_dir(&self, image: &ImageId, cfg: DirConfig) {
        self.send(Command::FetchDir {
            image_id: image.clone(),
            base_dir: cfg.base.to_path_buf(),
            parent: cfg.parent.to_path_buf(),
            image_name: cfg.image_name.to_string(),
            config: cfg.config.clone(),
        });
    }
    /*
        let mut state = self.state();
        let cached = state.images.get(image)
            .and_then(|x| x.upgrade());
        if let Some(index) = cached {
            println!("Image {:?} is already cached", image);
        } else {
            let (tx, rx) = channel::<Index>();
            tk_easyloop::spawn(rx.shared()
                .map(|index| {
                    println!("Got image {:?}", index.id)
                })
                .map_err(|e| {
                    println!("Error getting image {:?}", e)
                }));
        }
    */
    fn state(&self) -> MutexGuard<State> {
        self.0.state.lock().expect("image tracking subsystem is not poisoned")
    }
    fn send(&self, command: Command) {
        self.0.chan.send(command).expect("image tracking subsystem is alive")
    }
}

pub fn start(init: TrackingInit, meta: &Meta, remote: &Remote, disk: &Disk)
    -> Result<(), String> // actually void
{
    //
    Ok(())
}
