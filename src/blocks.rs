//! Block reading traits and implementations
//!
//! When uploading an image firsly you upload an index and then server
//! fetches individual blocks from the image.
//!
//! Server may not need to fetch some blocks, because it already have
//! them from other images. This is the reason why server request them.
//!
use std::io;
use std::fmt;
use std::path::PathBuf;

use futures::{Future, Async};
use futures_cpupool::{CpuPool, CpuFuture};
use block_id::{BlockHash};

/// A trait to fulfill block reading when uploading
pub trait GetBlock {
    /// A block data returned
    ///
    /// It's usually `Vec<u8>` but may also be an Arc'd container or a
    /// memory-mapped region.
    type Block: AsRef<[u8]>;
    /// Error returned by future
    ///
    /// This is used to print error and to send message to remote system
    type Error: fmt::Display;
    /// Future returned by `read_block`
    type Future: Future<Item=Self::Block, Error=Self::Error>;
    /// Read block by hash
    fn read_block(&self, hash: BlockHash) -> Self::Future;
}

/// A default threaded block reader
///
/// It starts 40 threads (by default) and reads every block requested from
/// server. The number of threads is large to allow disk subsystem to reorder
/// and optimize requests. Threads should be quite cheap in rust.
///
/// Note: no prefetching or caching blocks implemented because it's expected
/// that servers request different blocks. Also, we don't know which blocks
/// will be requested because in the average case most blocks are already
/// on server because of similar images. Also, OS file cache is good enough.
#[derive(Debug, Clone)]
pub struct ThreadedBlockReader {
    pool: CpuPool,
}

/// A future returned by `ThreadedBlockReader::read_block`
#[derive(Debug)]
pub struct FutureBlock(CpuFuture<Vec<u8>, ReadError>);

/// Error reading file or block not found
#[derive(Debug, Fail)]
#[fail(display="{}", internal)]
pub struct ReadError {
    internal: ReadErrorInt,
}

#[derive(Debug, Fail)]
enum ReadErrorInt {
    #[fail(display="error reading file {:?}: {}", _0, _1)]
    Fs(PathBuf, io::Error),
    #[fail(display="block {} not found", _0)]
    NotFound(BlockHash),
}

impl ThreadedBlockReader {
    /// Create a reader with default number of threads (40 at the moment)
    pub fn new() -> ThreadedBlockReader {
        ThreadedBlockReader {
            pool: CpuPool::new(40),
        }
    }
    /// Create a reader with specified number of threads
    pub fn new_num_threads(num: usize) -> ThreadedBlockReader {
        ThreadedBlockReader {
            pool: CpuPool::new(num),
        }
    }
}

impl GetBlock for ThreadedBlockReader {
    type Block = Vec<u8>;
    type Error = ReadError;
    type Future = FutureBlock;
    fn read_block(&self, hash: BlockHash) -> FutureBlock {
        unimplemented!();
    }
}

impl Future for FutureBlock {
    type Item = Vec<u8>;
    type Error = ReadError;
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        self.0.poll()
    }
}
