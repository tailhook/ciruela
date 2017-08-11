use std::net::SocketAddr;
use std::collections::{HashSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

use futures::Future;

use websocket::Connection;
use ciruela::{ImageId, VPath};
use ciruela::proto::{ReceivedImage, BaseDirState, RequestFuture};


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
pub struct Remote(Arc<Mutex<Connections>>);


impl Remote {
    pub fn new() -> Remote {
        Remote(Arc::new(Mutex::new(Connections {
            incoming: HashSet::new(),
            outgoing: HashMap::new(),
        })))
    }
    fn inner(&self) -> MutexGuard<Connections> {
        self.0.lock().expect("remote interface poisoned")
    }
    pub fn register_connection(&self, cli: &Connection) -> Token {
        self.inner().incoming.insert(cli.clone());
        return Token(self.clone(), cli.clone());
    }
    pub fn get_connection_for_index(&self, id: &ImageId) -> Option<Connection>
    {
        for conn in self.inner().incoming.iter() {
            if conn.has_image(id) {
                return Some(conn.clone());
            }
        }
        return None;
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
    pub fn fetch_base_dir(&self, addr: SocketAddr, path: &VPath)
        -> RequestFuture<BaseDirState>
    {
        unimplemented!();
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
