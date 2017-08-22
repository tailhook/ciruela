use std::collections::VecDeque;
use std::sync::Arc;

use futures::{Future, Async};
use futures_cpupool::CpuFuture;
use valuable_futures::{StateMachine, Async as VAsync};

use ciruela::proto::{RequestFuture, GetBlockResponse};
use tracking::progress::{Downloading, Block};
use tracking::Subsystem;
use disk::{self, Image};


pub struct FetchBlocks {
    sys: Subsystem,
    downloading: Arc<Downloading>,
    image: Arc<Image>,
    futures: VecDeque<FetchBlock>,
}

pub enum FetchBlock {
    Fetching(Block, usize, RequestFuture<GetBlockResponse>),
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
                            // TODO(tailhook) note failure somehow
                            //ctx.downloading.slices.lock()[slice]
                            //    .add_block(blk);
                            // TODO(tailhook) better message
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
        if self.futures.len() >= 10 {
            return Ok(Async::NotReady);
        }
        unimplemented!();
        if self.futures.len() == 0 {
            return Ok(Async::Ready(()));
        } else {
            return Ok(Async::NotReady);
        }
    }
}
