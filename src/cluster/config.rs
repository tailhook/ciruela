use std::sync::Arc;
use std::time::Duration;

/// Configuration for clustered connection
///
/// More settings will be added as needed.
#[derive(Clone, Debug)]
pub struct Config {
    pub(crate) initial_connections: u32,
    pub(crate) port: u16,
    pub(crate) early_hosts: u32,
    pub(crate) early_fraction: f32,
    pub(crate) early_timeout: Duration,
    pub(crate) maximum_timeout: Duration,
}

impl Config {
    /// Create an initial config connecting to 127.0.0.1:24783
    pub fn new() -> Config {
        Config {
            initial_connections: 3,
            port: 24783,
            early_hosts: 3,
            early_fraction: 0.75,
            early_timeout: Duration::new(10, 0),
            maximum_timeout: Duration::new(30*60, 0),
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

    /// Define how to define early success of an upload
    ///
    /// Basically this means that if after ``timeout`` we couldn't get
    /// notifications that all hosts are downloaded the image, but this number
    /// of hosts done, we stop the upload tool and report success.
    ///
    /// The number of hosts is determined as the following:
    ///
    /// 1. Find out hosts that accept a directory,
    ///    ``ceil(accepting_hosts*fraction)``
    /// 2. If the number above is smaller than ``hosts`` we the latter instead
    /// 3. If ``accepting_hosts`` is smaller than ``hosts`` we use former
    ///
    /// Defaults are: hosts=3, fraction=0.75, duration=30 sec
    ///
    /// By default this provides resilience in the following ways:
    ///
    /// 1. Upload to at least three hosts (unless total hosts count is smaller)
    /// 2. Tolerate 25% of nodes currently failing
    /// 3. Do not wait full deadline if some hosts are failing because that
    ///    would make deploy too long
    ///
    /// (I.e. we need to keep the deadline huge, because 100% new/uncached
    /// image over a slow channel between CI and production might take lot
    /// of time to upload)
    pub fn early_upload(&mut self, hosts: u32, fraction: f32,
        timeout: Duration)
        -> &mut Self
    {
        self.early_hosts = hosts;
        self.early_fraction = fraction;
        self.early_timeout = timeout;
        self
    }

    /// Maximum time to wait before for upload before reporting failure
    ///
    /// Default: 30 minutes
    ///
    /// Thix timeout should be big, because 100% new/uncached image over a
    /// slow channel between CI and production might take lot of time to
    /// upload. Still default might be too pessimistic if you know your
    /// setup better.
    pub fn maximum_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.maximum_timeout = timeout;
        self
    }
    /// Finalize config and return an Arc of a config
    pub fn done(&mut self) -> Arc<Config> {
        Arc::new(self.clone())
    }
}
