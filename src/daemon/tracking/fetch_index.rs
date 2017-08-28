use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::collections::{HashSet, HashMap};
use std::io::Cursor;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::time::Duration;

use futures::{self, Future as FutureTrait};
use futures::future::{Shared};
use futures::sync::oneshot::{channel, Receiver, Sender};
use valuable_futures::{Supply, Async, StateMachine};

use ciruela::ImageId;
use ciruela::proto::{GetIndex, GetIndexResponse};
use ciruela::proto::{RequestFuture, RequestClient};
use index::{IndexData};
use metadata::{Meta, Error as MetaError};
use named_mutex::{Mutex, MutexGuard};
use remote::{Remote};
use tk_easyloop::{spawn, timeout};
use tokio_core::reactor::Timeout;
use tracking::Tracking;
use remote::websocket::Connection;


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

struct FetchContext {
    id: ImageId,
    reg: Arc<Mutex<Registry>>,
    tracking: Tracking,
}

struct FetchBase(Sender<Index>, Timeout, State);

pub enum State {
    Start,
    Fetching(RequestFuture<GetIndexResponse>),
    Waiting(Receiver<()>, Timeout),
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
    while let Some(addr) = inp.queue.pop_front() {
        if inp.tried.insert(addr) {
            return Some(tracking.0.remote.ensure_connected(tracking, addr))
        }
    }
    return None;
}

fn poll_state(mut state: State, id: &ImageId, tracking: &Tracking,
    inp: &mut InProgress)
    -> Result<Async<IndexData, State>, ()>
{
    use self::State::*;
    loop {
        state = match state {
            Start => {
                if let Some(conn) = get_conn(id, tracking, inp) {
                    inp.wakeup.take();
                    Fetching(conn.request(GetIndex { id: id.clone() }))
                } else {
                    let (tx, rx) = channel();
                    inp.wakeup = Some(tx);
                    Waiting(rx, timeout(Duration::new(RETRY_TIMEOUT, 0)))
                }
            }
            Fetching(mut fut) => {
                match fut.poll() {
                    Err(e) => {
                        info!("Failed to fetch index: {}", e);
                        Start
                    }
                    Ok(futures::Async::Ready(v)) => {
                        let res = IndexData::parse(id, Cursor::new(v.data));
                        match res {
                            Ok(x) => return Ok(Async::Ready(x)),
                            Err(e) => {
                                error!("Error parsing index: {}",
                                    e);
                                Start
                            }
                        }
                    }
                    Ok(futures::Async::NotReady) => {
                        return Ok(Async::NotReady(Fetching(fut)));
                    }
                }
            }
            Waiting(mut rx, mut timeo) => {
                let res = timeo.poll()
                    .expect("timeout never fails");
                if res.is_ready() {
                    let ref mut inp = &mut *inp;
                    inp.queue.extend(inp.tried.drain());
                    Start
                } else if rx.poll().map(|x| x.is_ready())
                    .unwrap_or(true)
                {
                    Start
                } else {
                    return Ok(Async::NotReady(Waiting(rx, timeo)));
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
                poll_state(state, &ctx.id, &ctx.tracking, inp)?
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
                tx.send(idx).map_err(|_| warn!("nobody needs our image")).ok();
                Ok(Async::Ready(()))
            }
            Async::NotReady(state) => {
                if dline.poll().expect("timeouts never fail").is_ready() {
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
        self.registry.lock().remove(&self.data.id);
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
    pub fn get(&self, tracking: &Tracking, index: &ImageId) -> IndexFuture
    {
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
            Entry::Vacant(e) => {
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
                Ok(Async::Ready(val)) => Ok(Async::Ready(val.clone())),
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
    let retry_timeout = Duration::new(RETRY_FOR, 0);
    spawn(Supply::new(FetchContext {
            id: id,
            reg: reg,
            tracking: tracking,
        }, FetchBase(tx, timeout(retry_timeout), State::Start)));
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
