use std::fmt;
use std::collections::hash_map::Entry;
use std::collections::{HashMap};
use std::io::Cursor;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::time::Duration;

use futures::{self, Future as FutureTrait};
use futures::future::{Shared};
use futures::sync::oneshot::{channel, Receiver, Sender};
use valuable_futures::{Supply, Async, StateMachine};
use rand::{thread_rng, Rng};

use index::{ImageId};
use {VPath};
use proto::{GetIndex, GetIndexResponse};
use proto::{RequestFuture, RequestClient};
use index_cache::{IndexData};
use failure_tracker::HostFailures;
use mask::Mask;
use metadata::{Error as MetaError};
use metrics::{Integer, Counter};
use named_mutex::{Mutex, MutexGuard};
use tk_easyloop::{spawn, timeout};
use tokio_core::reactor::Timeout;
use tracking::Tracking;


/// Retry just 90 seconds
///
/// In case there is no place to download image it's safe to cancel it as
/// there's not much work already started. If image appears again it will be
/// uploaded by client or pushed by reconciliation algorithm again.
const RETRY_FOR: Duration = Duration::from_secs(90);
const RETRY_TIMEOUT: Duration = Duration::from_secs(1);

lazy_static! {
    pub static ref INDEXES: Integer = Integer::new();
    pub static ref FETCHING: Integer = Integer::new();
    pub static ref FETCHED: Counter = Counter::new();
    pub static ref FAILURES: Counter = Counter::new();
}


type Registry = HashMap<ImageId, IndexRef>;

#[derive(Clone)]
pub struct Index(Arc<Inner>);

struct Inner {
    data: IndexData,
    registry: Arc<Mutex<Registry>>,
}

struct InProgress {
    paths: Vec<VPath>,
    failures: HostFailures,
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

struct FetchContext {
    id: ImageId,
    reg: Arc<Mutex<Registry>>,
    tracking: Tracking,
}

struct FetchBase(Sender<Index>, Timeout, State);

pub enum State {
    Start,
    Fetching(SocketAddr, RequestFuture<GetIndexResponse>),
    Waiting(Receiver<()>, Timeout),
}

fn poll_state(mut state: State, id: &ImageId, tracking: &Tracking,
    inp: &mut InProgress)
    -> Async<IndexData, State>
{
    use self::State::*;
    loop {
        state = match state {
            Start => {
                // TODO(tailhook) this isn't strictly good, because of
                // failure tracking. But because of randomized nature of
                // everything, it should work good enough
                let path = thread_rng().choose(&inp.paths)
                    .expect("at least one path should be there");
                if let Some(conn) = tracking.get_connection_by_mask(path,
                    id, Mask::index_bit(), &inp.failures)
                {
                    inp.wakeup.take();
                    Fetching(conn.addr(), conn.request(GetIndex {
                        id: id.clone(),
                        hint: Some(path.clone()),
                    }))
                } else {
                    let (tx, rx) = channel();
                    inp.wakeup = Some(tx);
                    info!("No host for {}. Waiting...", id);
                    Waiting(rx, timeout(RETRY_TIMEOUT))
                }
            }
            Fetching(addr, mut fut) => {
                match fut.poll() {
                    Err(e) => {
                        info!("Failed to fetch index: {}", e);
                        inp.failures.add_failure(addr);
                        Start
                    }
                    Ok(futures::Async::Ready(v)) => {
                        let res = IndexData::parse(id, Cursor::new(&v.data));
                        match res {
                            Ok(x) => {
                                tracking.0.meta.store_index(id, v.data);
                                return Async::Ready(x);
                            }
                            Err(e) => {
                                error!("Error parsing index: {}",
                                    e);
                                inp.failures.add_failure(addr);
                                Start
                            }
                        }
                    }
                    Ok(futures::Async::NotReady) => {
                        return Async::NotReady(Fetching(addr, fut));
                    }
                }
            }
            Waiting(mut rx, mut timeo) => {
                let res = timeo.poll()
                    .expect("timeout never fails");
                if res.is_ready() {
                    Start
                } else if rx.poll().map(|x| x.is_ready())
                    .unwrap_or(true)
                {
                    Start
                } else {
                    return Async::NotReady(Waiting(rx, timeo));
                }
            }
        }
    }
}

impl StateMachine for FetchBase {
    type Supply = FetchContext;
    type Item = ();
    type Error = ();
    fn poll(self, ctx: &mut FetchContext) -> Result<Async<(), Self>, ()> {
        let FetchBase(tx, mut dline, state) = self;

        let mut lock = ctx.reg.lock();
        let res = match lock.get_mut(&ctx.id) {
            Some(&mut IndexRef::InProgress(ref mut inp)) => {
                poll_state(state, &ctx.id, &ctx.tracking, inp)
            }
            _ => unreachable!(),
        };
        match res {
            Async::Ready(data) => {
                let idx = Index(Arc::new(Inner {
                    data: data,
                    registry: ctx.reg.clone(),
                }));
                lock.insert(ctx.id.clone(),
                            IndexRef::Done(Arc::downgrade(&idx.0)));
                FETCHING.decr(1);
                FETCHED.incr(1);
                INDEXES.set(lock.len() as i64);
                tx.send(idx).map_err(|_| warn!("nobody needs our image")).ok();
                Ok(Async::Ready(()))
            }
            Async::NotReady(state) => {
                if dline.poll().expect("timeouts never fail").is_ready() {
                    lock.remove(&ctx.id);
                    INDEXES.set(lock.len() as i64);
                    FETCHING.decr(1);
                    FAILURES.incr(1);
                    error!("Deadline reached when fetching {}", ctx.id);
                    Ok(Async::Ready(()))
                } else {
                    Ok(Async::NotReady(FetchBase(tx, dline, state)))
                }
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
        let mut lock = self.registry.lock();
        lock.remove(&self.data.id);
        INDEXES.set(lock.len() as i64);
    }
}


impl Indexes {
    pub fn new() -> Indexes {
        Indexes {
            images: Arc::new(Mutex::new(Registry::new(), "image_registry")),
        }
    }
    fn lock(&self) -> MutexGuard<Registry> {
        self.images.lock()
    }
    pub(in tracking) fn get(&self, tracking: &Tracking, vpath: &VPath, index: &ImageId)
        -> IndexFuture
    {
        let mut lock = self.lock();
        let fut = match lock.entry(index.clone()) {
            Entry::Occupied(mut e) => {
                let (fut, inp) = match *e.get_mut() {
                    IndexRef::Done(ref x) => {
                        if let Some(im) = x.upgrade() {
                            info!("Image {:?} is already cached", index);
                            return IndexFuture::Ready(Index(im.clone()));
                        } else {
                            spawn_try_read(vpath.clone(), index,
                                self.images.clone(), tracking)
                        }
                    }
                    IndexRef::InProgress(ref mut x) => {
                        if !x.paths.contains(vpath) {
                            x.paths.push(vpath.clone());
                        }
                        return IndexFuture::Future(x.future.clone())
                    }
                };
                *e.get_mut() = IndexRef::InProgress(Box::new(inp));
                FETCHING.incr(1);
                IndexFuture::Future(fut)
            }
            Entry::Vacant(e) => {
                let (fut, inp) = spawn_try_read(vpath.clone(),
                    index, self.images.clone(), tracking);
                e.insert(IndexRef::InProgress(Box::new(inp)));
                FETCHING.incr(1);
                IndexFuture::Future(fut)
            }
        };
        INDEXES.set(lock.len() as i64);
        fut
    }
}

fn spawn_try_read(path: VPath, index: &ImageId, reg: Arc<Mutex<Registry>>,
    tracking: &Tracking)
    -> (Shared<Receiver<Index>>, InProgress)
{
    let (tx, rx) = channel();
    let rx = rx.shared();
    let inp = InProgress {
        paths: vec![path],
        failures: HostFailures::new_default(),
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
                    debug!("Index {:?} can't be found in store", index);
                } else {
                    error!("Error reading index {:?}: {}. \
                            Will try to fetch... ",
                           index, e);
                }
                spawn_fetcher(index, reg, tracking, tx);
                Ok(())
            }
        }));
    return (rx, inp);
}

impl futures::Future for IndexFuture {
    type Item = Index;
    type Error = Error;

    fn poll(&mut self) -> Result<futures::Async<Self::Item>, Self::Error> {
        use futures::Async;
        match *self {
            IndexFuture::Ready(ref idx) => Ok(Async::Ready(idx.clone())),
            IndexFuture::Future(ref mut sh) => match sh.poll() {
                Ok(Async::Ready(val)) => Ok(Async::Ready((*val).clone())),
                Ok(Async::NotReady) => Ok(Async::NotReady),
                // TODO(tailhook) should we log this error
                Err(_) => Err(Error::FutureClosed),
            },
        }
    }
}
fn spawn_fetcher(id: ImageId, reg: Arc<Mutex<Registry>>, tracking: Tracking,
       tx: Sender<Index>)
{
    spawn(Supply::new(FetchContext {
            id: id,
            reg: reg,
            tracking: tracking,
        }, FetchBase(tx, timeout(RETRY_FOR), State::Start)));
}

impl fmt::Debug for IndexRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IndexRef::Done(..) => f.write_str("IndexRef::Done"),
            IndexRef::InProgress(..) => f.write_str("IndexRef::InProgress"),
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
