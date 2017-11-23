//! Index serving traits and implementations
//!
//! When uploading an image (a directory). First it's indexed with
//! [`dir-signature`] crate getting basically recursive directory list with
//! a hash for each file.
//!
//! Then the hashsum of that file is recorded as an `ImageId` and sent to
//! the peer. Subsequently peer asks for the index data (if it have no index
//! cached already).
//!
//! [`dir-signature`]: https://crates.io/crates/dir-signature
use std::collections::HashMap;
use std::fmt;
use std::io;
use std::sync::{RwLock, Arc};

use failure::Backtrace;
use dir_signature::get_hash;
use futures::{Future};
use futures::future::{FutureResult, ok, err};

pub use id::ImageId;


/// A trait to fulfill index data when uploading
pub trait GetIndex {
    /// An index data returned
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
    fn read_index(&self, id: &ImageId) -> Self::Future;
}

fn _object_safe() {
    use futures::future::FutureResult;
    let _: Option<&GetIndex<Data=Vec<u8>, Error=String,
                   Future=FutureResult<Vec<u8>, String>>> = None;
}

/// A GetIndex implementation that serves indexes from memory
///
/// Usually this is what you want because index is small enough, and client
/// finishes work as quick as image is served
#[derive(Debug, Clone)]
pub struct InMemoryIndexes {
    indexes: Arc<RwLock<HashMap<ImageId, Arc<[u8]>>>>,
}

/// Error adding index
#[derive(Debug, Fail)]
pub enum IndexError {
    // Error parsing index passed as index data
    // TODO(tailhook) this is doc(hidden) because `get_hash` returns io::Error
    #[doc(hidden)]
    #[fail(display="error parsing index")]
    ParseError,
    #[doc(hidden)]
    #[fail(display="index lock was poisoned")]
    LockError(Backtrace),
    #[doc(hidden)]
    #[fail(display="non-existent-error")]
    __Nonexhaustive,
}

/// Index not found error
#[derive(Debug, Fail)]
pub enum ReadError {
    /// Index not found
    #[fail(display="index {} with not found", _0)]
    NotFound(ImageId),
    #[doc(hidden)]
    #[fail(display="index lock was poisoned")]
    LockError(Backtrace),
    #[doc(hidden)]
    #[fail(display="non-existent-error")]
    __Nonexhaustive,
}

impl InMemoryIndexes {
    /// New in-memory index collection
    pub fn new() -> InMemoryIndexes {
        InMemoryIndexes {
            indexes: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    /// Register index and return image id
    ///
    /// ImageId returned here for convenience and a bit microoptimization. You
    /// can get it from the image data on your own.
    pub fn register_index(&self, data: &[u8]) -> Result<ImageId, IndexError> {
        let image_id = ImageId::from(get_hash(&mut io::Cursor::new(&data))
            .map_err(|_| IndexError::ParseError)?);
        let mut indexes = self.indexes.write()
            .map_err(|_| IndexError::LockError(Backtrace::new()))?;
        indexes.insert(image_id.clone(), Arc::from(data));
        Ok(image_id)
    }
}

impl GetIndex for InMemoryIndexes {
    type Data = Arc<[u8]>;
    type Error = ReadError;
    /// Future returned by `read_block`
    type Future = FutureResult<Arc<[u8]>, ReadError>;
    /// Read block by hash
    fn read_index(&self, id: &ImageId) -> Self::Future {
        let data = match self.indexes.read() {
            Ok(data) => data,
            Err(_) => return err(ReadError::LockError(Backtrace::new())),
        };
        match data.get(id) {
            Some(data) => ok(data.clone()),
            None => err(ReadError::NotFound(id.clone())),
        }
    }
}
