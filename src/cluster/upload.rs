use std::net::SocketAddr;
use std::sync::RwLock;
use std::collections::{HashMap, HashSet};

use proto::{ReceivedImage, AbortedImage};

use machine_id::MachineId;

#[derive(Debug)]
struct Bookkeeping {
    accepted_ips: HashSet<SocketAddr>,
    done_ips: HashSet<SocketAddr>,
    done_ids: HashSet<MachineId>,
    done_hostnames: HashSet<String>,
    aborted_ips: HashMap<SocketAddr, String>,
    aborted_ids: HashMap<MachineId, String>,
    aborted_hostnames: HashMap<String, String>,
    rejected_ips: HashMap<SocketAddr, String>,
}

#[derive(Debug)]
pub(crate) struct Stats {
    book: RwLock<Bookkeeping>,
}


impl Stats {
    pub(crate) fn new() -> Stats {
        Stats {
            book: RwLock::new(Bookkeeping {
                accepted_ips: HashSet::new(),
                done_ips: HashSet::new(),
                done_ids: HashSet::new(),
                done_hostnames: HashSet::new(),
                aborted_ips: HashMap::new(),
                aborted_ids: HashMap::new(),
                aborted_hostnames: HashMap::new(),
                rejected_ips: HashMap::new(),
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
    pub(crate) fn add_response(&self, source: SocketAddr,
        accepted: bool, reject_reason: Option<String>,
        hosts: HashMap<MachineId, String>)
    {
        let mut book = self.book.write()
            .expect("bookkeeping is not poisoned");
        warn!("Response {} {:?} {:?}", accepted, reject_reason, hosts);
        if !accepted {
            book.rejected_ips.insert(source,
                reject_reason.unwrap_or_else(|| String::from("unknown")));
        } else {
            book.accepted_ips.insert(source);
        }
    }
}
