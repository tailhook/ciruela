//! Client connection manager
//!
//! By cluster we mean just one or more ciruela servers which see each other
//! and have same directory namespace (no specific software known as
//! ciruela-cluster exists).
//!
//! We might expose individual server connections later, but now we only
//! have higher level API.

mod addr;
mod config;
mod set;
mod upload;
mod future;
mod error;

pub use cluster::config::Config;
pub use cluster::future::{UploadFuture, UploadOk, UploadFail};
pub use cluster::error::UploadErr;

use std::sync::Arc;

use abstract_ns::{Name, Resolve, HostResolve};
use futures::sync::mpsc::{unbounded, UnboundedSender};
use futures::future::{Future, Shared};
use futures::sync::oneshot;
use tk_easyloop::spawn;

use index::{GetIndex, ImageId};
use blocks::GetBlock;
use VPath;

/// Connection to a server or cluster of servers
///
/// Ciruela automatically manages a number of connections according to
/// configs and the operations over connection (i.e. images currently
/// uploading).
#[derive(Debug)]
pub struct Connection {
    config: Arc<Config>,
    chan: UnboundedSender<()>,
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
        where I: GetIndex + 'static,
              B: GetBlock + 'static,
              R: Resolve + HostResolve + 'static,
    {
        let (tx, rx) = unbounded();
        spawn(set::ConnectionSet::new(rx, initial_address, resolver,
            index_source, block_source, config));
        return Connection {
            config: config.clone(),
            chan: tx,
        }
    }

    /// Initiate a new upload
    pub fn upload(&self, image_id: &ImageId, path: &VPath) -> Upload {
        let (tx, rx) = oneshot::channel();
        Upload {
            stats: Arc::new(upload::Stats {
            }),
            future: rx.shared(),
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
}
