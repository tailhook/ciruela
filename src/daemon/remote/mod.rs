use std::sync::{Arc, Mutex, MutexGuard};
use std::collections::HashSet;

use websocket::Connection;
use ciruela::ImageId;


pub struct Connections {
    connections: HashSet<Connection>,
}

pub struct Token(Remote, Connection);

#[derive(Clone)]
pub struct Remote(Arc<Mutex<Connections>>);


impl Remote {
    pub fn new() -> Remote {
        Remote(Arc::new(Mutex::new(Connections {
            connections: HashSet::new(),
        })))
    }
    fn inner(&self) -> MutexGuard<Connections> {
        self.0.lock().expect("remote interface poisoned")
    }
    pub fn register_connection(&self, cli: &Connection) -> Token {
        self.inner().connections.insert(cli.clone());
        return Token(self.clone(), cli.clone());
    }
    pub fn get_connection_for_index(&self, id: &ImageId) -> Option<Connection>
    {
        for conn in self.inner().connections.iter() {
            if conn.has_image(id) {
                return Some(conn.clone());
            }
        }
        return None;
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        self.0.inner().connections.remove(&self.1);
    }
}
