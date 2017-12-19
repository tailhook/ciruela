use std::sync::Arc;

/// Configuration for clustered connection
///
/// More settings will be added as needed.
#[derive(Clone, Debug)]
pub struct Config {
    pub(crate) initial_connections: u32,
    pub(crate) port: u16,
}

impl Config {
    /// Create an initial config connecting to 127.0.0.1:24783
    pub fn new() -> Config {
        Config {
            initial_connections: 3,
            port: 24783,
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
    /// Set ciruela server port
    ///
    /// This port is used both for initial connections and when redirecting
    /// connections to other host names.
    pub fn port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }
    /// Finalize config and return an Arc of a config
    pub fn done(&mut self) -> Arc<Config> {
        Arc::new(self.clone())
    }
}
