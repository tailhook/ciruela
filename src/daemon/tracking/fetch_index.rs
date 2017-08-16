use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::collections::{HashSet, HashMap};
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::{Arc, Weak, Mutex, MutexGuard};
use std::time::Duration;

use futures::{Future, Async};
use futures::future::{Shared, loop_fn, Loop, Either, ok};
use futures::sync::oneshot::{channel, Receiver, Sender};

use ciruela::ImageId;
use ciruela::proto::{GetIndex, RequestFuture};
use index::{IndexData};
use tk_easyloop::{spawn, timeout};
use tokio_core::reactor::Timeout;
use metadata::{Meta, Error as MetaError};
use remote::{Remote};
use websocket::Connection;
use tracking::Tracking;


const RETRY_FOR: u64 = 3600_000;  // retry no more than an hour
const RETRY_TIMEOUT: u64 = 10000;


type Registry = HashMap<ImageId, IndexRef>;

#[derive(Clone)]
pub struct Index(Arc<Inner>);

struct Inner {
    data: IndexData,
    registry: Arc<Mutex<Registry>>,
}

struct InProgress {
    queue: VecDeque<SocketAddr>,
    tried: HashSet<SocketAddr>,
    wakeup: Option<Sender<()>>,
    future: Shared<Receiver<Index>>,
}

pub enum IndexFuture {
    Ready(Index),
    Future(Shared<Receiver<Index>>),
}

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        FutureClosed {
            description("future fetching index terminated abnormally")
        }
    }
}

enum IndexRef {
    Done(Weak<Inner>),
    InProgress(Box<InProgress>),
}

#[derive(Clone)]
pub struct Indexes {
    images: Arc<Mutex<Registry>>,
}

struct FetchIndex {
    id: ImageId,
    reg: Arc<Mutex<Registry>>,
    tracking: Tracking,
    tx: Option<Sender<Index>>,
    deadline: Timeout,
}

pub enum State {
    Start,
    Fetching(RequestFuture<GetIndexAck>),
    Waiting(Receiver<()>, Timeout),
}

impl FetchIndex {
    fn new(id: ImageId, reg: Arc<Mutex<Registry>>, tracking: Tracking,
           tx: Sender<Index>)
        -> FetchIndex
    {
        FetchIndex {
            id: id,
            reg: reg,
            tracking: tracking,
            tx: Some(tx),
            deadline: timeout(Duration::new(RETRY_FOR, 0)),
        }
    }
}

fn get_conn(id: &ImageId, tracking: &Tracking, inp: &mut InProgress)
    -> Option<Connection>
{
    for addr in &inp.queue {
        if let Some(conn) = tracking.0.remote.get_connection(*addr) {
            return Some(conn.clone());
        }
    }
    if let Some(conn) = tracking.0.remote
        .get_incoming_connection_for_index(id)
    {
        return Some(conn);
    }
    inp.queue.pop_front().map(|addr| {
        tracking.0.remote.ensure_connected(tracking, addr)
    })
}

fn next_attempt(id: &ImageId, tracking: &Tracking, inp: &mut InProgress)
    -> Option<Index>
{
    if let Some(conn) = get_conn(id, tracking, inp) {
        unimplemented!();
    } else {
        unimplemented!();
    }
}

impl Future for FetchIndex {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        let mut lock = self.reg.lock().expect("write");
        let res = match lock.get_mut(&self.id) {
            Some(&mut IndexRef::InProgress(ref mut inp)) => {
                next_attempt(&self.id, &self.tracking, inp)
            }
            _ => unreachable!(),
        };
        if let Some(image) = res {
            lock.insert(self.id.clone(),
                        IndexRef::Done(Arc::downgrade(&image.0)));
            self.tx.take().unwrap().send(image)
                .map_err(|_| warn!("nobody needs our image"))
                .ok();
            Ok(Async::Ready(()))
        } else {
            let tresult = self.deadline.poll()
                .expect("timeouts never fail");
            match tresult {
                Async::Ready(()) => {
                    error!("Deadline reached when fetching {}", self.id);
                    Ok(Async::Ready(()))
                }
                Async::NotReady => Ok(Async::NotReady),
            }
        }
    }
}

impl Deref for Index {
    type Target = IndexData;
    fn deref(&self) -> &IndexData {
        &self.0.data
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.registry.lock()
            .expect("image registry is not poisoned")
            .remove(&self.data.id);
    }
}


impl Indexes {
    pub fn new(meta: &Meta, remote: &Remote) -> Indexes {
        Indexes {
            images: Arc::new(Mutex::new(Registry::new())),
        }
    }
    fn lock(&self) -> MutexGuard<Registry> {
        self.images.lock().expect("images are not poisoned")
    }
    pub fn get(&self, tracking: &Tracking, index: &ImageId) -> IndexFuture
    {
        let images = self.images.clone();
        match self.lock().entry(index.clone()) {
            Entry::Occupied(mut e) => {
                let (fut, inp) = match *e.get() {
                    IndexRef::Done(ref x) => {
                        if let Some(im) = x.upgrade() {
                            info!("Image {:?} is already cached", index);
                            return IndexFuture::Ready(Index(im.clone()));
                        } else {
                            spawn_try_read(index, self.images.clone(),
                                tracking)
                        }
                    }
                    IndexRef::InProgress(ref x) => {
                        return IndexFuture::Future(x.future.clone())
                    }
                };
                *e.get_mut() = IndexRef::InProgress(Box::new(inp));
                IndexFuture::Future(fut)
            }
            Entry::Vacant(mut e) => {
                let (fut, inp) = spawn_try_read(index, self.images.clone(),
                    tracking);
                e.insert(IndexRef::InProgress(Box::new(inp)));
                IndexFuture::Future(fut)
            }
        }
    }
}

fn spawn_try_read(index: &ImageId, reg: Arc<Mutex<Registry>>,
    tracking: &Tracking)
    -> (Shared<Receiver<Index>>, InProgress)
{
    let (tx, rx) = channel();
    let rx = rx.shared();
    let inp = InProgress {
        queue: VecDeque::new(),
        tried: HashSet::new(),
        wakeup: None,
        future: rx.clone(),
    };
    let index = index.clone();
    let tracking = tracking.clone();
    spawn(tracking.0.meta.read_index(&index)
        .then(move |result| match result {
            Ok(index) => {
                tx.send(Index(Arc::new(Inner {
                        registry: reg,
                        data: index,
                    })))
                    .map_err(|_| debug!("Useless index read")).ok();
                Ok(())
            }
            Err(e) => {
                if matches!(e, MetaError::IndexNotFound) {
                    info!("Index {:?} can't be found in store", index);
                } else {
                    error!("Error reading index {:?}: {}. \
                            Will try to fetch... ",
                           index, e);
                }
                spawn(FetchIndex::new(index, reg, tracking, tx));
                Ok(())
            }
        }));
    return (rx, inp);
}

impl Future for IndexFuture {
    type Item = Index;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        match *self {
            IndexFuture::Ready(ref idx) => Ok(Async::Ready(idx.clone())),
            IndexFuture::Future(ref mut sh) => match sh.poll() {
                Ok(Async::Ready(val)) => Ok(Async::Ready(val.clone())),
                Ok(Async::NotReady) => Ok(Async::NotReady),
                Err(e) => Err(Error::FutureClosed),
            },
        }
    }
}

#[cfg(test)]
mod test {
    use std::mem::size_of;
    use super::*;

    #[test]
    #[cfg(target_arch="x86_64")]
    fn size() {
        assert_eq!(size_of::<IndexRef>(), 16);
    }

}
