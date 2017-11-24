//! Block reading traits and implementations
//!
//! When uploading an image firsly you upload an index and then server
//! fetches individual blocks from the image.
//!
//! Server may not need to fetch some blocks, because it already have
//! them from other images. This is the reason why server request them.
//!
use std::cmp::min;
use std::collections::HashMap;
use std::io::{self, Seek, SeekFrom, Read};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, Arc};
use std::fs::{File};

use failure::Backtrace;
use futures::{Future, Async};
use futures_cpupool::{CpuPool, CpuFuture};
use dir_signature::v1::{self, ParseError};
use virtual_path::VPath;

pub use block_id::{BlockHash};

/// A trait to fulfill block reading when uploading
pub trait GetBlock {
    /// A block data returned
    ///
    /// It's usually `Vec<u8>` but may also be an Arc'd container or a
    /// memory-mapped region.
    type Data: AsRef<[u8]>;
    /// Error returned by future
    ///
    /// This is used to print error and to send message to remote system
    type Error: fmt::Display;
    /// Future returned by `read_block`
    type Future: Future<Item=Self::Data, Error=Self::Error> + 'static;
    /// Read block by hash
    fn read_block(&self, hash: BlockHash, hint: BlockHint) -> Self::Future;
}

/// Hint where to find block
///
/// It's currently empty, will grow methods in future
#[derive(Debug)]
pub struct BlockHint {
    hint: Option<(VPath, PathBuf, u64)>,
}

trait Assert: Send + Sync + 'static {}
impl Assert for BlockHint {}
impl Assert for BlockHash {}

fn _object_safe() {
    use futures::future::FutureResult;
    let _: Option<&GetBlock<Data=Vec<u8>, Error=String,
                   Future=FutureResult<Vec<u8>, String>>> = None;
}

#[derive(Debug, Clone)]
struct BlockPointer {
    path: Arc<PathBuf>,
    offset: u64,
    size: usize,
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
    blocks: Arc<RwLock<HashMap<BlockHash, BlockPointer>>>,
}

/// A future returned by `ThreadedBlockReader::read_block`
#[derive(Debug)]
pub struct FutureBlock(CpuFuture<Vec<u8>, ReadError>);


/// Error reading block
#[derive(Debug, Fail)]
pub enum ReadError {
    /// Filesystem read error
    #[fail(display="error reading file {:?}: {}", _0, _1)]
    Fs(PathBuf, io::Error, Backtrace),
    /// Block not found
    #[fail(display="block {} not found", _0)]
    NotFound(BlockHash),
    #[doc(hidden)]
    #[fail(display="block lock was poisoned")]
    LockError(Backtrace),
    #[doc(hidden)]
    #[fail(display="non-existent-error")]
    __Nonexhaustive,
}

/// Error adding directory
#[derive(Debug, Fail)]
pub enum DirError {
    /// Error parsing index passed as index data
    #[fail(display="error parsing index: {}", _0)]
    ParseError(ParseError),
    #[doc(hidden)]
    #[fail(display="blocks lock was poisoned")]
    LockError(Backtrace),
    #[doc(hidden)]
    #[fail(display="hash size is unsupported")]
    HashSize(Backtrace),
    #[doc(hidden)]
    #[fail(display="non-existent-error")]
    __Nonexhaustive,
}

impl ThreadedBlockReader {
    /// Create a reader with default number of threads (40 at the moment)
    pub fn new() -> ThreadedBlockReader {
        ThreadedBlockReader {
            pool: CpuPool::new(40),
            blocks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    /// Create a reader with specified number of threads
    pub fn new_num_threads(num: usize) -> ThreadedBlockReader {
        ThreadedBlockReader {
            pool: CpuPool::new(num),
            blocks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    /// Register a filesystem directory to respond to get_block request from
    pub fn register_dir<P: AsRef<Path>>(&self, dir: P, index_data: &[u8])
        -> Result<(), DirError>
    {
        self._register_dir(dir.as_ref(), index_data)
    }
    fn _register_dir(&self, dir: &Path, index_data: &[u8])
        -> Result<(), DirError>
    {
        let ref mut cur = io::Cursor::new(&index_data);
        let mut parser = v1::Parser::new(cur).map_err(DirError::ParseError)?;
        let header = parser.get_header();
        let block_size = header.get_block_size();
        let mut blocks = self.blocks.write()
            .map_err(|_| DirError::LockError(Backtrace::new()))?;
        // TODO(tailhook) maybe we don't need to know all block hashes?
        for entry in parser.iter() {
            match entry.expect("just created index is valid") {
                v1::Entry::File { ref path, ref hashes, size, .. } => {
                    let path = Arc::new(dir.join(
                        path.strip_prefix("/").expect("paths are absolute")
                    ));
                    let mut left = size;
                    for (idx, hash) in hashes.iter().enumerate() {
                        let id = BlockHash::from_bytes(hash)
                            .ok_or_else(||
                                DirError::HashSize(Backtrace::new()))?;
                        blocks.insert(id, BlockPointer {
                            path: path.clone(),
                            offset: idx as u64 * block_size,
                            size: min(left, block_size) as usize,
                        });
                        left = left.saturating_sub(block_size);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl GetBlock for ThreadedBlockReader {
    type Data = Vec<u8>;
    type Error = ReadError;
    type Future = FutureBlock;
    fn read_block(&self, hash: BlockHash, _hint: BlockHint) -> FutureBlock {
        let blocks = self.blocks.clone();
        FutureBlock(self.pool.spawn_fn(move || {
            let blocks = blocks.read()
                .map_err(|_| ReadError::LockError(Backtrace::new()))?;
            let pointer = blocks.get(&hash)
                .ok_or_else(|| ReadError::NotFound(hash))?;
            let mut result = vec![0u8; pointer.size];
            let mut file = File::open(&*pointer.path)
                .map_err(|e| ReadError::Fs(pointer.path.to_path_buf(), e,
                                           Backtrace::new()))?;
            file.seek(SeekFrom::Start(pointer.offset))
                .map_err(|e| ReadError::Fs(pointer.path.to_path_buf(), e,
                                           Backtrace::new()))?;
            let bytes = file.read(&mut result[..])
                .map_err(|e| ReadError::Fs(pointer.path.to_path_buf(), e,
                                           Backtrace::new()))?;
            result.truncate(bytes);
            assert_eq!(result.len(), pointer.size);
            Ok(result)
        }))
    }
}

impl Future for FutureBlock {
    type Item = Vec<u8>;
    type Error = ReadError;
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        self.0.poll()
    }
}

impl BlockHint {
    /// Create an empty hint
    ///
    /// This assumes that block reader is able to find block b hash
    pub fn empty() -> BlockHint {
        BlockHint {
            hint: None,
        }
    }
}
