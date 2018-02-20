mod outgoing;
pub mod websocket;

pub use remote::websocket::Connection;

use std::net::SocketAddr;
use std::collections::{HashSet, HashMap, BTreeSet};
use std::sync::{Arc};
use std::time::{Duration};

use proto::{ReceivedImage, AbortedImage};
use proto::{Registry};
use peers::Peer;
use index::{ImageId};
use {VPath};
use machine_id::{MachineId};
use failure_tracker::HostFailures;
use named_mutex::{Mutex, MutexGuard};
use remote::outgoing::connect;
use tk_http::websocket::{Config as WsConfig};
use tracking::Tracking;


pub struct Connections {
    // TODO(tailhook) optimize incoming and outgoing connections
    // but keep in mind that client connections should never be used as
    // outgoing (i.e. you can use `ciruela upload` on the same host as
    // `ciruela-server`)
    incoming: HashSet<Connection>,
    outgoing: HashMap<SocketAddr, Connection>,
    failures: HostFailures,
    declared_images: HashMap<ImageId, HashSet<Connection>>,
}

pub struct Token(Remote, Connection);

#[derive(Clone)]
pub struct Remote(Arc<RemoteState>);

struct RemoteState {
    websock_config: Arc<WsConfig>,
    conn: Mutex<Connections>,
    hostname: String,
    machine_id: MachineId,
}


impl Remote {
    pub fn new(my_hostname: &str, my_id: &MachineId)
        -> Remote
    {
        Remote(Arc::new(RemoteState {
            hostname: my_hostname.to_string(),
            machine_id: my_id.clone(),
            websock_config: WsConfig::new()
                .ping_interval(Duration::new(1200, 0)) // no pings
                .message_timeout(Duration::new(120, 0))
                .byte_timeout(Duration::new(5, 0))
                .max_packet_size(101 << 20)
                .done(),
            conn: Mutex::new(Connections {
                incoming: HashSet::new(),
                outgoing: HashMap::new(),
                failures: HostFailures::new_default(),
                declared_images: HashMap::new(),
            }, "remote_connections"),
        }))
    }
    pub fn websock_config(&self) -> &Arc<WsConfig> {
        &self.0.websock_config
    }
    fn inner(&self) -> MutexGuard<Connections> {
        self.0.conn.lock()
    }
    pub fn register_connection(&self, cli: &Connection) -> Token {
        self.inner().incoming.insert(cli.clone());
        return Token(self.clone(), cli.clone());
    }
    pub fn get_incoming_connection_for_image(&self, id: &ImageId)
        -> Option<Connection>
    {
        self.inner().declared_images.get(id)
            .and_then(|x| x.iter().cloned().next())
    }
    /// Splits the list into connected/not-connected list *and* removes
    /// recently failed addresses
    pub fn split_connected<I: Iterator<Item=SocketAddr>>(&self, inp: I)
        -> (Vec<Connection>, Vec<SocketAddr>)
    {
        let mut conn = Vec::new();
        let mut not_conn = Vec::new();
        let state = self.inner();
        for sa in inp {
            if let Some(con) = state.outgoing.get(&sa) {
                conn.push(con.clone());
            } else if state.failures.can_try(&sa) {
                not_conn.push(sa);
            }
        }
        return (conn, not_conn);
    }
    pub fn ensure_connected(&self, tracking: &Tracking, addr: SocketAddr)
        -> Connection
    {
        if let Some(conn) = self.inner().outgoing.get(&addr) {
            return conn.clone();
        }
        let reg = Registry::new();
        let (cli, rx) = Connection::outgoing(addr, &reg);
        self.inner().outgoing.insert(addr, cli.clone());
        let tok = Token(self.clone(), cli.clone());
        connect(self, tracking, &reg, cli.clone(), tok, addr, rx);
        cli.clone()
    }
    pub fn notify_received_image(&self, id: &ImageId, path: &VPath) {
        self._received_image(id, path, None);
    }
    pub fn forward_notify_received_image(&self, id: &ImageId, path: &VPath,
        peer: &Peer)
    {
        self._received_image(id, path, Some(peer));
    }
    pub fn get_watching(&self) -> BTreeSet<VPath> {
        let mut res = BTreeSet::new();
        for conn in self.inner().incoming.iter() {
            res.extend(conn.watches().iter().cloned());
        }
        return res;
    }
    pub fn _received_image(&self, id: &ImageId, path: &VPath,
        source: Option<&Peer>)
    {
        for conn in self.inner().incoming.iter() {
            if conn.has_image(id) {
                conn.notification(ReceivedImage {
                    id: id.clone(),
                    hostname:
                        source.map(|x| x.hostname.to_string())
                            .unwrap_or(self.0.hostname.clone()),
                    machine_id:
                        source.map(|x| x.id.clone())
                            .unwrap_or(self.0.machine_id.clone()),
                    forwarded: source.is_some(),
                    path: path.clone(),
                })
            }
        }
    }
    pub fn notify_aborted_image(&self, id: &ImageId, path: &VPath,
        reason: String)
    {
        for conn in self.inner().incoming.iter() {
            if conn.has_image(id) {
                conn.notification(AbortedImage {
                    id: id.clone(),
                    hostname: self.0.hostname.clone(),
                    machine_id: self.0.machine_id.clone(),
                    forwarded: false,
                    path: path.clone(),
                    reason: reason.clone(),
                })
            }
        }
    }
    pub fn has_image_source(&self, id: &ImageId) -> bool {
        self.inner().declared_images.contains_key(id)
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        let mut remote = self.0.inner();
        if self.1.hanging_requests() > 0 || !self.1.is_connected() {
            remote.failures.add_failure(self.1.addr());
        }
        if remote.incoming.remove(&self.1) {
            for img in self.1.images().iter() {
                let left = {
                    let mut item = remote.declared_images.get_mut(img);
                    item.as_mut().map(|x| x.remove(&self.1));
                    item.map(|x| x.len())
                };
                match left {
                    Some(0) => {
                        remote.declared_images.remove(img);
                    }
                    _ => {}
                }
            }
        }
        if remote.outgoing.get(&self.1.addr())
            .map(|x| *x == self.1).unwrap_or(false)
        {
            remote.outgoing.remove(&self.1.addr());
        }
    }
}
