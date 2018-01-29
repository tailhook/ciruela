use std::cmp::{min, max};
use std::net::SocketAddr;
use std::sync::{RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::{HashMap, HashSet};

use proto::{ReceivedImage, AbortedImage};
use cluster::future::UploadOk;
use cluster::error::ErrorKind;
use cluster::config::Config;

use machine_id::MachineId;

#[derive(Debug)]
struct Bookkeeping {
    accepted_ips: HashSet<SocketAddr>,
    discovered_ids: HashSet<MachineId>,
    done_ips: HashSet<SocketAddr>,
    done_ids: HashSet<MachineId>,
    done_hostnames: HashSet<String>,
    aborted_ips: HashMap<SocketAddr, String>,
    aborted_ids: HashMap<MachineId, String>,
    aborted_hostnames: HashMap<String, String>,
    rejected_ips: HashMap<SocketAddr, String>,
}

/// Current upload statistics
///
/// We're trying to be conservative of what can be published here so that
/// we don't have to break backwards compatibility in the future.
#[derive(Debug)]
pub struct Stats {
    weak_errors: bool,
    book: RwLock<Bookkeeping>,
    total_responses: AtomicUsize,
}


impl Stats {
    pub(crate) fn new(weak: bool) -> Stats {
        Stats {
            weak_errors: weak,
            book: RwLock::new(Bookkeeping {
                discovered_ids: HashSet::new(),
                accepted_ips: HashSet::new(),
                done_ips: HashSet::new(),
                done_ids: HashSet::new(),
                done_hostnames: HashSet::new(),
                aborted_ips: HashMap::new(),
                aborted_ids: HashMap::new(),
                aborted_hostnames: HashMap::new(),
                rejected_ips: HashMap::new(),
            }),
            total_responses: AtomicUsize::new(0),
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
        book.discovered_ids.insert(info.machine_id.clone());
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
        match (accepted, reject_reason.as_ref().map(|x| &x[..])) {
            (false, Some("already_exists"))
            | (false, Some("already_uploading_different_version"))
            if self.weak_errors => {
                warn!("Rejected because of {:?}, but that's fine",
                    reject_reason);
                let res = book.accepted_ips.insert(source);
                if res {
                    self.total_responses.fetch_add(1, Ordering::Relaxed);
                }
                book.done_ips.insert(source);
                // TODO(tailhook) mark other dicts as done too
            }
            (false, _) => {
                warn!("Rejected because of {:?} try {:?}",
                    reject_reason, hosts);
                let res = book.rejected_ips.insert(source,
                    reject_reason.unwrap_or_else(|| String::from("unknown")));
                if res.is_none() {
                    self.total_responses.fetch_add(1, Ordering::Relaxed);
                }
            }
            (true, _) => {
                debug!("Accepted from {}", source);
                book.discovered_ids.extend(hosts.keys().cloned());
                let res = book.accepted_ips.insert(source);
                if res {
                    self.total_responses.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
    pub(crate) fn total_responses(&self) -> u32 {
        self.total_responses.load(Ordering::Relaxed) as u32
    }
}

pub(crate) fn check(stats: &Stats, config: &Config, early_timeout: bool)
    -> Option<Result<UploadOk, ErrorKind>>
{
    let book = stats.book.read()
        .expect("bookkeeping is not poisoned");
    // TODO(tailhook) this is very simplistic preliminary check

    if book.done_ips.is_superset(&book.accepted_ips) {
        if early_timeout {
            let fract_hosts = (
                book.discovered_ids.len() as f32 *
                config.early_fraction).ceil() as usize;
            let hosts = min(book.discovered_ids.len(),
                max(config.early_hosts as usize, fract_hosts));
            debug!("Downloaded ids {}/{}/{}",
                book.done_ids.len(), hosts, book.discovered_ids.len());
            if book.done_ids.len() > hosts {
                // TODO(tailhook) check kinds of rejection
                if book.rejected_ips.len() > 0 {
                    return Some(Err(ErrorKind::Rejected));
                } else {
                    return Some(Ok(UploadOk::new()));
                }
            }
        } else if book.done_ids.is_superset(&book.discovered_ids) {
            debug!("Early downloaded ids {}/{}",
                book.done_ids.len(), book.discovered_ids.len());
            // TODO(tailhook) check kinds of rejection
            if book.rejected_ips.len() > 0 {
                return Some(Err(ErrorKind::Rejected));
            } else {
                return Some(Ok(UploadOk::new()));
            }
        }
    }
    return None;
}
