use std::cmp::{min, max};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use abstract_ns::Name;
use proto::{ReceivedImage, AbortedImage};
use cluster::future::UploadOk;
use cluster::error::ErrorKind;
use cluster::config::Config;

use VPath;
use machine_id::MachineId;

#[derive(Debug)]
struct Bookkeeping {
    accepted_ips: HashSet<SocketAddr>,
    discovered_ids: HashSet<MachineId>,
    discovered_hosts: HashSet<String>,
    done_ips: HashSet<SocketAddr>,
    done_ids: HashSet<MachineId>,
    done_hostnames: HashSet<String>,
    aborted_ips: HashMap<SocketAddr, String>,
    aborted_ids: HashMap<MachineId, String>,
    aborted_hostnames: HashMap<String, String>,
    rejected_no_config: HashSet<SocketAddr>,
    rejected_ips: HashMap<SocketAddr, String>,
}

/// Current upload statistics
///
/// We're trying to be conservative of what can be published here so that
/// we don't have to break backwards compatibility in the future.
#[derive(Debug)]
pub struct Stats {
    pub(crate) cluster_name: Vec<Name>,
    pub(crate) started: Instant,
    pub(crate) path: VPath,
    weak_errors: bool,
    book: RwLock<Bookkeeping>,
    total_responses: AtomicUsize,
}

impl Stats {
    pub(crate) fn new(cluster_name: &Vec<Name>, path: &VPath, weak: bool)
        -> Stats
    {
        Stats {
            cluster_name: cluster_name.clone(),
            started: Instant::now(),
            path: path.clone(),
            weak_errors: weak,
            book: RwLock::new(Bookkeeping {
                discovered_ids: HashSet::new(),
                discovered_hosts: HashSet::new(),
                accepted_ips: HashSet::new(),
                done_ips: HashSet::new(),
                done_ids: HashSet::new(),
                done_hostnames: HashSet::new(),
                aborted_ips: HashMap::new(),
                aborted_ids: HashMap::new(),
                aborted_hostnames: HashMap::new(),
                rejected_ips: HashMap::new(),
                rejected_no_config: HashSet::new(),
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
            (false, Some("no_config")) => {
                warn!("Info {} rejects directory", source);
                book.rejected_no_config.insert(source);
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
                let res = book.accepted_ips.insert(source);
                if res {
                    self.total_responses.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        for (id, val) in hosts {
            book.discovered_ids.insert(id);
            book.discovered_hosts.insert(val);
        }
    }
    pub(crate) fn total_responses(&self) -> u32 {
        self.total_responses.load(Ordering::Relaxed) as u32
    }
    pub(crate) fn is_rejected(&self, addr: SocketAddr) -> bool {
        let book = self.book.read()
            .expect("bookkeeping is not poisoned");
        return book.rejected_ips.contains_key(&addr) ||
            book.rejected_no_config.contains(&addr);
    }
    pub(crate) fn fmt_downloaded(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        let book = self.book.read()
            .expect("bookkeeping is not poisoned");
        f.write_str("fetched from ")?;
        if book.done_ids.len() > 3 {
            write!(f, "{} hosts", book.done_ids.len())?;
        } else {
            fmt_iter(f, &book.done_hostnames)?;
        }
        if book.discovered_ids.len() > book.done_ids.len() {
            write!(f, " (out of {} hosts)", book.discovered_ids.len())?;
        }
        if !book.rejected_ips.is_empty() {
            f.write_str(", rejected by")?;
            fmt_iter(f,
                book.rejected_ips.iter()
                .map(|(k, v)| format!("{}: {}", k, v)))?;
        }
        if !book.aborted_ids.is_empty() {
            f.write_str(", aborted by")?;
            fmt_iter(f,
                book.aborted_hostnames.iter()
                .map(|(k, v)| format!("{}: {}", k, v)))?;
        }
        Ok(())
    }
}

fn fmt_iter<I: IntoIterator>(f: &mut fmt::Formatter, iter: I) -> fmt::Result
    where I::Item: fmt::Display,
{
    let mut iter = iter.into_iter();
    match iter.next() {
        Some(x) => write!(f, "{}", x)?,
        None => return Ok(()),
    }
    for item in iter {
        write!(f, ", {}", item)?;
    }
    Ok(())
}

pub(crate) fn check(stats: &Arc<Stats>, config: &Config, early_timeout: bool)
    -> Option<Result<UploadOk, ErrorKind>>
{
    let book = stats.book.read()
        .expect("bookkeeping is not poisoned");
    trace!("Current state {:?}{}", book,
        if early_timeout { " early" } else { "" });
    if book.done_ips.is_superset(&book.accepted_ips) {
        if early_timeout {
            let fract_hosts = (
                book.discovered_ids.len() as f32 *
                config.early_fraction).ceil() as usize;
            let hosts = min(book.discovered_ids.len(),
                max(config.early_hosts as usize, fract_hosts));
            debug!("Downloaded ids {}/{}/{}",
                book.done_ids.len(), hosts, book.discovered_ids.len());
            if book.done_ids.len() >= hosts {
                if book.rejected_ips.len() > 0 || book.aborted_ids.len() > 0 {
                    return Some(Err(ErrorKind::Rejected));
                } else {
                    return Some(Ok(UploadOk::new(stats)));
                }
            }
        } else if book.done_ids.is_superset(&book.discovered_ids) {
            debug!("Early downloaded ids {}/{}",
                book.done_ids.len(), book.discovered_ids.len());
            if book.rejected_ips.len() > 0 || book.aborted_ips.len() > 0 {
                return Some(Err(ErrorKind::Rejected));
            } else {
                return Some(Ok(UploadOk::new(stats)));
            }
        }
    }
    return None;
}
