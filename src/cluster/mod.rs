//! Client connection manager
//!
//! By cluster we mean just one or more ciruela servers which see each other
//! and have same directory namespace (no specific software known as
//! ciruela-cluster exists).
//!
//! We might expose individual server connections later, but now we only
//! have higher level API.
use std::path::Path;
use std::usize;

mod addr;
mod config;
mod set;
mod upload;
mod download;
mod future;
mod error;

pub use cluster::config::Config;
pub use cluster::upload::{Stats, ProgressOneLiner};
pub use cluster::download::{RawIndex, MutableIndex, MaterializedIndex};
pub use cluster::download::{IndexParseError, IndexUpdateError};
pub use cluster::future::{UploadFuture, UploadOk, UploadFail};
pub use cluster::error::{UploadErr, ErrorKind};

use std::sync::Arc;

use abstract_ns::{Name, Resolve, HostResolve};
use futures::sync::mpsc::{UnboundedSender};
use futures::future::{Future, Shared};
use futures::sync::oneshot;

use {VPath};
use id::ImageId;
use index::GetIndex;
use blocks::GetBlock;
use cluster::set::{Message, NewUpload};
use signature::SignedUpload;
pub use cluster::future::{IndexFuture, FileFuture};

/// Connection to a server or cluster of servers
///
/// Ciruela automatically manages a number of connections according to
/// configs and the operations over connection (i.e. images currently
/// uploading).
#[derive(Debug, Clone)]
pub struct Connection {
    cluster_name: Vec<Name>,
    config: Arc<Config>,
    chan: UnboundedSender<Message>,
}

/// This structure represents upload
///
/// You can introspect current upload progress through it and also make a
/// future which resolves to `true` if upload is okay or `false` if it was
/// rejected by all nodes.
#[derive(Debug, Clone)]
pub struct Upload {
    stats: Arc<upload::Stats>,
    future: Shared<oneshot::Receiver<Result<UploadOk, Arc<UploadErr>>>>,
}

impl Connection {
    /// Create a connection pool object
    ///
    /// **Warning**: constructor should run on loop provieded by
    /// ``tk_easyloop``. In future, tokio will provide implicit loop reference
    /// on it's own.
    ///
    /// The actual underlying connections are established when specific
    /// operation is requested. Also you don't need to specify all node name
    /// in the cluster, they will be discovered.
    ///
    /// There are two common patterns:
    ///
    /// 1. [preferred] Use a DNS name that resolves to a full list of IP
    ///    addresses. Common DNS servers randomize them and only spill few
    ///    of the adressses because of DNS package size limit, but that's
    ///    fine, as we only need 3 or so of them.
    /// 2. Specify 3-5 server names and leave the discover to ciruela itself.
    ///
    /// While you can specify a name that refers to only one address, it's not
    /// a very good idea (unless you really have one server) because the server
    /// you're connecting to may fail.
    pub fn new<R, I, B>(initial_address: Vec<Name>, resolver: R,
        index_source: I, block_source: B, config: &Arc<Config>)
        -> Connection
        where I: GetIndex + Clone + Send + 'static,
              B: GetBlock + Clone + Send + 'static,
              R: Resolve + HostResolve + Clone + Send + 'static,
    {
        let tx = set::ConnectionSet::spawn(initial_address.clone(), resolver,
            index_source, block_source, config);
        return Connection {
            cluster_name: initial_address,
            config: config.clone(),
            chan: tx,
        }
    }

    /// Initiate a new upload (appending a directory)
    ///
    /// # Panics
    ///
    /// If connection set is already closed
    pub fn append(&self, upload: SignedUpload) -> Upload {
        self._upload(false, false, upload, None)
    }
    /// Initiate a new upload (appending a directory, if not exists)
    ///
    /// This is basically same as 'append()` but ignores errors when directory
    /// already exists (or currently downloading) but has different contents.
    ///
    /// # Panics
    ///
    /// If connection set is already closed
    pub fn append_weak(&self, upload: SignedUpload) -> Upload {
        self._upload(false, true, upload, None)
    }
    /// Initiate a new upload (replacing a directory)
    ///
    /// # Panics
    ///
    /// If connection set is already closed
    pub fn replace(&self, upload: SignedUpload) -> Upload {
        self._upload(true, false, upload, None)
    }

    /// Initiate a new upload (replacing if directory hash matches)
    ///
    /// # Panics
    ///
    /// If connection set is already closed
    pub fn replace_if_matches(&self, upload: SignedUpload, old_image: ImageId)
        -> Upload
    {
        self._upload(true, false, upload, Some(old_image))
    }
    fn _upload(&self, replace: bool, weak: bool, upload: SignedUpload,
        old_image: Option<ImageId>)
        -> Upload
    {
        let (tx, rx) = oneshot::channel();
        let stats = Arc::new(upload::Stats::new(
            &self.cluster_name, &upload.path, weak));
        self.chan.unbounded_send(Message::NewUpload(NewUpload {
            replace, upload, weak, old_image: old_image,
            stats: stats.clone(),
            resolve: tx,
        })).expect("connection set is not closed");
        Upload {
            stats,
            future: rx.shared(),
        }
    }

    /// Fetch index of a directory that is currently on the server
    pub fn fetch_index(&self, vpath: &VPath) -> IndexFuture {
        let (tx, rx) = oneshot::channel();
        self.chan.unbounded_send(Message::FetchIndex(vpath.clone(), tx))
            .expect("connection set is not closed");
        IndexFuture {
            inner: rx,
        }
    }

    /// Fetch file relative to the index
    ///
    /// # Panics
    ///
    /// Panics if there is no such file in the index
    pub fn fetch_file<I, P>(&self, idx: &I, path: P)
        -> FileFuture
        where P: AsRef<Path>,
              I: MaterializedIndex,
    {
        let (tx, rx) = oneshot::channel();
        let path = path.as_ref().to_path_buf();
        let (_, size, hashes) = idx.get_file(&path).expect("file must exist");
        assert!(size < usize::MAX as u64, "file must fit memory");
        assert_eq!(
            (size + hashes.block_size()-1) / hashes.block_size(),
            hashes.len() as u64,
            "valid number of hashes");
        self.chan.unbounded_send(Message::FetchFile {
            location: idx.get_location(),
            size: size as usize, hashes, path, tx,
        }).expect("connection set is not closed");
        FileFuture {
            inner: rx,
        }
    }
}

impl Upload {
    /// Return a future that will be resolved when upload is complete according
    /// to the configuration.
    pub fn future(&self) -> UploadFuture {
        UploadFuture {
            inner: self.future.clone()
        }
    }

    /// Return a real-time stats for upload
    pub fn stats(&self) -> &Stats {
        &*self.stats
    }
}
