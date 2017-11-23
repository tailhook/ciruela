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
pub use id::ImageId;
