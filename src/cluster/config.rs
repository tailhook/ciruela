use std::sync::Arc;
use std::time::Duration;

/// Configuration for clustered connection
///
/// More settings will be added as needed.
#[derive(Clone, Debug)]
pub struct Config {
    initial_connections: u32,
    name_resolution_timeout: Duration,
}

impl Config {
    /// Create an initial config connecting to 127.0.0.1:24783
    pub fn new() -> Config {
        Config {
            initial_connections: 3,
            name_resolution_timeout: Duration::new(30, 0),
        }
    }
    /// Set number of connections to initiate when accessing cluster
    ///
    /// Default: `3`.
    ///
    /// Ciruela picks `num` random addresses from the ones passed to
    /// connection constructor, and connects to them initially. Then if image
    /// is not accepted at this node or if the node fails, ciruela establishes
    /// connections to other nodes keeping max `num` connections for any
    /// specific image upload.
    ///
    /// In other words, this is a hint of how much simultaneous uploads might
    /// be going of any specific image. Rules of thumb:
    ///
    /// 1. Too large value means you're uploading data to more servers from
    ///    a client which presumably has slower connections than local
    ///    connection between nodes.
    /// 2. Small value like 1 is bad when node downloading image would fail
    /// 3. The default of 3 is just good enough
    ///
    /// Note, that big value (1) is also not a big deal because nodes will try
    /// top optimize download anyway. Also, `num=1` (2) is not big deal as
    /// ciruela will reconnect when node fails anyway. So technically any
    /// value is okay.
    pub fn initial_connections(&mut self, num: u32) -> &mut Self {
        self.initial_connections = num;
        self
    }
    /// Set maximum time for name resolution
    ///
    /// This value has two consequnces:
    ///
    /// 1. When some of the names can't be resolved we end up retrying this
    ///    number of seconds (even if upload proceeds much longer)
    /// 2. When no names could be resolved after the timeout we error upload
    pub fn name_resolution_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.name_resolution_timeout = timeout;
        self
    }
    /// Finalize config and return an Arc of a config
    pub fn done(&mut self) -> Arc<Config> {
        Arc::new(self.clone())
    }
}
