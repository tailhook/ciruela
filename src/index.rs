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
use std::fmt;

use futures::{Future, Async};

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
    fn read_index(&self, id: ImageId) -> Self::Future;
}

fn _object_safe() {
    use futures::future::FutureResult;
    let _: Option<&GetIndex<Data=Vec<u8>, Error=String,
                   Future=FutureResult<Vec<u8>, String>>> = None;
}
