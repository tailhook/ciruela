use std::net::SocketAddr;
use std::sync::RwLock;
use std::collections::{HashMap, HashSet};

use proto::{ReceivedImage, AbortedImage};

use machine_id::MachineId;

#[derive(Debug)]
struct Bookkeeping {
    done_ips: HashSet<SocketAddr>,
    done_ids: HashSet<MachineId>,
    done_hostnames: HashSet<String>,
    aborted_ips: HashMap<SocketAddr, String>,
    aborted_ids: HashMap<MachineId, String>,
    aborted_hostnames: HashMap<String, String>,
}

#[derive(Debug)]
pub(crate) struct Stats {
    book: RwLock<Bookkeeping>,
}


impl Stats {
    pub(crate) fn new() -> Stats {
        Stats {
            book: RwLock::new(Bookkeeping {
                done_ips: HashSet::new(),
                done_ids: HashSet::new(),
                done_hostnames: HashSet::new(),
                aborted_ips: HashMap::new(),
                aborted_ids: HashMap::new(),
                aborted_hostnames: HashMap::new(),
            }),
        }
    }
    pub(crate) fn received_image(&self, addr: SocketAddr, info: &ReceivedImage)
    {
        let mut book = self.book.write()
            .expect("bookkeeping is not poisoned");
        if !info.forwarded {
            book.done_ips.insert(addr);
        }
        book.done_ids.insert(info.machine_id.clone());
        book.done_hostnames.insert(info.hostname.clone());
    }
    pub(crate) fn aborted_image(&self, addr: SocketAddr, info: &AbortedImage) {
        let mut book = self.book.write()
            .expect("bookkeeping is not poisoned");
        if !info.forwarded {
            book.aborted_ips.insert(addr, info.reason.clone());
        }
        book.aborted_ids.insert(
            info.machine_id.clone(), info.reason.clone());
        book.aborted_hostnames.insert(
            info.hostname.clone(), info.reason.clone());
    }
}
