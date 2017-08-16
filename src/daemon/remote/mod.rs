mod outgoing;

use std::net::SocketAddr;
use std::collections::{HashSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use config::Config;
use websocket::Connection;
use ciruela::{ImageId, VPath};
use ciruela::proto::{ReceivedImage, Request, RequestFuture, RequestClient};
use ciruela::proto::{GetBaseDir, GetBaseDirResponse};
use remote::outgoing::connect;
use tk_http::websocket::{Config as WsConfig};
use metadata::Meta;
use disk::Disk;
use tracking::Tracking;


pub struct Connections {
    // TODO(tailhook) optimize incoming and outgoing connections
    // but keep in mind that client connections should never be used as
    // outgoing (i.e. you can use `ciruela upload` on the same host as
    // `ciruela-server`)
    incoming: HashSet<Connection>,
    outgoing: HashMap<SocketAddr, Connection>,
}

pub struct Token(Remote, Connection);

#[derive(Clone)]
pub struct Remote(Arc<RemoteState>);

struct RemoteState {
    websock_config: Arc<WsConfig>,
    conn: Mutex<Connections>,
}


impl Remote {
    pub fn new()
        -> Remote
    {
        Remote(Arc::new(RemoteState {
            websock_config: WsConfig::new()
                .ping_interval(Duration::new(1200, 0)) // no pings
                .inactivity_timeout(Duration::new(5, 0))
                .done(),
            conn: Mutex::new(Connections {
                incoming: HashSet::new(),
                outgoing: HashMap::new(),
            }),
        }))
    }
    pub fn websock_config(&self) -> &Arc<WsConfig> {
        &self.0.websock_config
    }
    fn inner(&self) -> MutexGuard<Connections> {
        self.0.conn.lock().expect("remote interface poisoned")
    }
    pub fn register_connection(&self, cli: &Connection) -> Token {
        self.inner().incoming.insert(cli.clone());
        return Token(self.clone(), cli.clone());
    }
    pub fn get_incoming_connection_for_index(&self, id: &ImageId)
        -> Option<Connection>
    {
        for conn in self.inner().incoming.iter() {
            if conn.has_image(id) {
                return Some(conn.clone());
            }
        }
        return None;
    }
    pub fn get_connection(&self, addr: SocketAddr) -> Option<Connection> {
        self.inner().outgoing.get(&addr).cloned()
    }
    pub fn ensure_connected(&self, tracking: &Tracking, addr: SocketAddr)
        -> Connection
    {
        self.inner().outgoing.entry(addr)
            .or_insert_with(move || {
                let (cli, rx) = Connection::outgoing(addr);
                let tok = Token(self.clone(), cli.clone());
                connect(self, tracking, cli.clone(), tok, addr, rx);
                cli
            }).clone()
    }
    pub fn notify_received_image(&self, ref id: ImageId, path: &VPath) {
        for conn in self.inner().incoming.iter() {
            if conn.has_image(id) {
                conn.notification(ReceivedImage {
                    id: id.clone(),
                    // TODO(tailhook)
                    hostname: String::from("localhost"),
                    forwarded: false,
                    path: path.clone(),
                })
            }
        }
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        let mut remote = self.0.inner();
        remote.incoming.remove(&self.1);
        if remote.outgoing.get(&self.1.addr())
            .map(|x| *x == self.1).unwrap_or(false)
        {
            remote.outgoing.remove(&self.1.addr());
        }
    }
}
