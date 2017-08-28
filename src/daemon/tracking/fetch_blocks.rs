use std::collections::VecDeque;

use std::sync::Arc;
use std::time::Duration;

use futures::{Future, Async};
use futures_cpupool::CpuFuture;
use valuable_futures::{StateMachine, Async as VAsync};
use tokio_core::reactor::Timeout;
use tk_easyloop::timeout;

use ciruela::proto::{RequestFuture, GetBlock, GetBlockResponse};
use ciruela::proto::{RequestClient};
use tracking::progress::{Downloading, Block};
use tracking::Subsystem;
use disk::{self, Image};


const FETCH_DEADLINE: u64 = 3600; // one hour
const RETRY_INTERVAL: u64 = 2000; // 2 seconds
const CONCURRENCY: usize = 10;


pub struct FetchBlocks {
    sys: Subsystem,
    downloading: Arc<Downloading>,
    image: Arc<Image>,
    futures: VecDeque<FetchBlock>,
    retry_timeout: Option<Timeout>,
    deadline: Timeout,
}

pub enum FetchBlock {
    Fetching(Block, u8, RequestFuture<GetBlockResponse>),
    Writing(CpuFuture<(), disk::Error>),
}


impl FetchBlocks {
    pub fn new(image: &Arc<Image>, down: &Arc<Downloading>,
            sys: &Subsystem)
        -> FetchBlocks
    {
        FetchBlocks {
            sys: sys.clone(),
            image: image.clone(),
            downloading: down.clone(),
            futures: VecDeque::new(),
            retry_timeout: None,
            deadline: timeout(Duration::new(FETCH_DEADLINE, 0)),
        }
    }
}

impl StateMachine for FetchBlock {
    type Supply = FetchBlocks;
    type Item = ();
    type Error = ();
    fn poll(self, ctx: &mut FetchBlocks) -> Result<VAsync<(), Self>, ()> {
        use self::FetchBlock::*;
        let mut state = self;
        loop {
            state = match state {
                Fetching(blk, slice, mut f) => {
                    match f.poll() {
                        Ok(Async::Ready(data)) => {
                            for s in ctx.downloading.slices().iter_mut() {
                                if s.index == slice {
                                    s.in_progress -= 1;
                                }
                            }
                            Writing(ctx.sys.disk.write_block(
                                ctx.image.clone(),
                                blk.path.clone(),
                                blk.offset,
                                Arc::new(data.data)))
                        }
                        Ok(Async::NotReady) => {
                            return Ok(VAsync::NotReady(
                                Fetching(blk, slice, f)));
                        }
                        Err(e) => {
                            for s in ctx.downloading.slices().iter_mut() {
                                if s.index == slice {
                                    s.in_progress -= 1;
                                    s.blocks.push_back(blk);
                                    break;
                                }
                            }
                            error!("Block fetch error: {}", e);
                            return Err(());
                        }
                    }
                }
                Writing(mut f) => {
                    match f.poll() {
                        Ok(Async::Ready(())) => {
                            // TODO(tailhook) mark it
                            return Ok(VAsync::Ready(()));
                        }
                        Ok(Async::NotReady) => {
                            return Ok(VAsync::NotReady(Writing(f)));
                        }
                        Err(e) => {
                            // TODO(tailhook) better message
                            error!("Block fetch error: {}", e);
                            // TODO(tailhook) sleep and retry?
                            // or is it fatal?
                            unimplemented!();
                            return Err(());
                        }
                    }
                }
            }
        }
    }
}


impl Future for FetchBlocks {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        use self::FetchBlock::Fetching;

        self.retry_timeout.take();
        'outer: loop {
            for _ in 0 .. self.futures.len() {
                match self.futures.pop_front() {
                    Some(f) => match f.poll(self) {
                        Ok(VAsync::NotReady(x)) => self.futures.push_back(x),
                        Ok(VAsync::Ready(())) => {}
                        Err(()) => {}
                    },
                    None => unreachable!(),
                }
            }
            if self.futures.len() == 0 &&
                self.downloading.slices().len() == 0
            {
                return Ok(Async::Ready(()));
            } else if self.deadline.poll().expect("timers don't fail")
                .is_ready()
            {
                error!("Deadline reached while fetching blocks");
                return Err(());
            } else if self.futures.len() >= CONCURRENCY {
                return Ok(Async::NotReady);
            }
            let mut new = 0;
            self.downloading.slices().retain(|s| {
                s.in_progress > 0 || s.blocks.len() > 0
            });
            for s in self.downloading.slices().iter_mut() {
                while let Some(blk) = s.blocks.pop_front() {
                    // TODO(tailhook) Try peer connections
                    let conn = self.sys.remote
                        .get_incoming_connection_for_index(
                            &self.downloading.image_id);
                    if let Some(conn) = conn {
                        let req = conn.request(GetBlock {
                            hash: blk.hash,
                        });
                        let f = Fetching(blk, s.index, req);
                        new += 1;
                        self.futures.push_back(f);
                        s.in_progress += 1;
                        if self.futures.len() > CONCURRENCY {
                            continue 'outer;
                        }
                    } else {
                        s.blocks.push_back(blk);
                        break;
                    }
                }
            }
            if new > 0 {
                // must poll new futures
                continue;
            }
            if self.futures.len() == 0 {
                info!("Nowhere to fetch some chunks of {}. Waiting...",
                    self.downloading.image_id);
                let mut t = timeout(Duration::from_millis(RETRY_INTERVAL));
                match t.poll().expect("timeout never fails") {
                    Async::Ready(()) => continue,
                    Async::NotReady => {}
                }
                self.retry_timeout = Some(t);
            }
            return Ok(Async::NotReady);
        }
    }
}
