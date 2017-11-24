use std::collections::{HashMap, HashSet};
use std::io::{stdout, stderr};
use std::net::SocketAddr;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, Duration};

use abstract_ns::HostResolve;
use argparse::{ArgumentParser};
use dir_signature::{ScannerConfig, HashType, v1};
use futures::future::{Future, Either, join_all, ok};
use futures::sync::oneshot::{channel, Sender};
use tk_easyloop;

mod options;

use name;
use ciruela::blocks;
use ciruela::index;
use machine_id::{MachineId};
use time_util::to_ms;
use proto::{SigData, sign};
use global_options::GlobalOptions;
use proto::{Client, AppendDir, ReplaceDir};
use proto::RequestClient;
use proto::{Listener};
use proto::message::Notification;
use {VPath};

struct Progress {
    started: SystemTime,
    hosts_done: HashMap<MachineId, String>,
    ips_needed: HashSet<SocketAddr>,
    ids_needed: HashMap<MachineId, String>,
    hosts_errored: HashSet<SocketAddr>,
    done: Option<Sender<()>>,
}

struct Tracker(SocketAddr, Arc<Mutex<Progress>>);

fn duration_float(d: Duration) -> f64 {
    d.as_secs() as f64 + d.subsec_nanos() as f64 / 1_000_000_000.
}


impl Progress {
    fn hosts_done(&self) -> String {
        self.hosts_done.values().map(|x| &x[..]).collect::<Vec<_>>().join(", ")
    }
    fn add_ids(&mut self, hosts: HashMap<MachineId, String>) {
        for (id, hostname) in hosts {
            if !self.hosts_done.contains_key(&id) {
                self.ids_needed.insert(id, hostname);
            }
        }
    }
    fn check_status(&mut self) {
        if self.ips_needed.len() == 0 {
            info!("Fetched from {}", self.hosts_done());
            eprintln!("Fetched from all required hosts. {} total. \
                Done in {:.3} seconds.",
                self.hosts_done.len(),
                duration_float(
                    SystemTime::now().duration_since(self.started)
                    .unwrap()));
            self.done.take().map(|chan| {
                chan.send(()).ok();
            });
        } else {
            eprint!("Fetched from ({}/{}) {}\r",
                self.hosts_done.len(),
                self.hosts_done.len() +
                    self.ids_needed.len() + self.ips_needed.len(),
                self.hosts_done());
        }
    }
}

impl Listener for Tracker {
    fn notification(&self, n: Notification) {
        use proto::message::Notification::*;
        match n {
            ReceivedImage(img) => {
                // TODO(tailhook) check image id and path
                let mut pro = self.1.lock().expect("progress is not poisoned");
                pro.hosts_done.insert(img.machine_id, img.hostname);
                if !img.forwarded {
                    pro.ips_needed.remove(&self.0);
                }
                pro.check_status()
            }
            AbortedImage(img) => {
                error!("Image download from {} aborted: {}", self.0,
                    img.reason);
                // TODO(tailhook) check image id and path
                let mut pro = self.1.lock().expect("progress is not poisoned");
                pro.hosts_errored.insert(self.0);
                if !img.forwarded {
                    pro.ips_needed.remove(&self.0);
                }
                pro.check_status()
            }
            _ => {}
        }
    }
    fn closed(&self) {
        // TODO(tailhook) reconnect
        let mut pro = self.1.lock().expect("progress is not poisoned");
        if pro.done.is_some() {
            error!("Connection to {} is closed", self.0);
            pro.ips_needed.remove(&self.0);
            pro.hosts_errored.insert(self.0);
            pro.check_status();
        }
    }
}

fn is_ok(pro: &Arc<Mutex<Progress>>) -> bool {
    pro.lock().expect("progress is ok").hosts_errored.len() == 0
}

fn is_conflict_reason(reason: &Option<String>) -> bool {
    match reason.as_ref().map(|x| &x[..]) {
        Some("already_exists") => true,
        Some("already_uploading_different_version") => true,
        _ => false,
    }
}

fn do_upload(gopt: GlobalOptions, opt: options::UploadOptions)
    -> Result<bool, ()>
{
    let dir = opt.source_directory.clone().unwrap();
    let mut cfg = ScannerConfig::new();
    cfg.threads(gopt.threads);
    cfg.hash(HashType::blake2b_256());
    cfg.add_dir(&dir, "/");
    cfg.print_progress();

    let mut indexbuf = Vec::new();
    v1::scan(&cfg, &mut indexbuf)
        .map_err(|e| error!("Error scanning {:?}: {}", dir, e))?;

    if indexbuf.len() > 100 << 20 {
        error!("Index is too large: {} with the limit of {}",
            indexbuf.len(), 100 << 20);
        return Err(());
    }


    let indexes = index::InMemoryIndexes::new();
    let blocks = blocks::ThreadedBlockReader::new_num_threads(gopt.threads);
    let image_id = indexes.register_index(&indexbuf)
        .expect("register index failed");
    eprintln!("Uploading image {} [index_size: {}]",
        image_id, indexbuf.len());


    let timestamp = SystemTime::now();
    let mut signatures = HashMap::new();
    for turl in &opt.target_urls {
        if signatures.contains_key(&turl.path[..]) {
            continue;
        }
        blocks.register_dir(&dir, &indexbuf)
            .expect("register blocks failed");
        signatures.insert(&turl.path[..], sign(SigData {
            path: &turl.path,
            image: image_id.as_ref(),
            timestamp: to_ms(timestamp),
        }, &opt.private_keys));
    }
    let signatures = Arc::new(signatures);
    let (done_tx, done_rx) = channel();
    let done_rx = done_rx.shared();
    let progress = Arc::new(Mutex::new(Progress {
        started: SystemTime::now(),
        hosts_done: HashMap::new(),
        ips_needed: HashSet::new(),
        ids_needed: HashMap::new(),
        hosts_errored: HashSet::new(),
        done: Some(done_tx),
    }));
    let replace = opt.replace;
    let fail_on_conflict = opt.fail_on_conflict;
    let mut keep_resolver = None;
    let port = gopt.destination_port;

    tk_easyloop::run(|| {
        let resolver = name::resolver(&tk_easyloop::handle());
        keep_resolver = Some(resolver.clone());

        join_all(
            opt.target_urls.iter()
            .map(move |turl| {
                let host2 = turl.host.clone();
                let host4 = turl.host.clone();
                let image_id = image_id.clone();
                let indexes = indexes.clone();
                let blocks = blocks.clone();
                let signatures = signatures.clone();
                let done_rx = done_rx.clone();
                let progress = progress.clone();
                resolver.resolve_host(&turl.host)
                .then(move |res| match res {
                    Ok(addr) => {
                        let addr = addr.with_port(port);
                        Ok(name::pick_hosts(&host2, addr)
                            .unwrap_or(Vec::new()))
                    }
                    Err(e) => {
                        error!("Error resolving host {}: {}", host2, e);
                        Ok(Vec::new())
                    }
                })
                .and_then(move |addrs| {
                    let done_rx = done_rx.clone();
                    let progress = progress.clone();
                    progress.lock().expect("progress is ok")
                        .ips_needed.extend(&addrs);
                    join_all(
                        addrs.iter()
                        .map(move |&addr| {
                            let turl = turl.clone();
                            let host = host4.clone();
                            let signatures = signatures.clone();
                            let image_id = image_id.clone();
                            let indexes = indexes.clone();
                            let blocks = blocks.clone();
                            let progress = progress.clone();
                            let progress2 = progress.clone();
                            let progress3 = progress.clone();
                            let tracker = Tracker(addr,
                                                  progress.clone());
                            let done_rx = done_rx.clone();
                            let host_port = format!("{}:{}", host4, port);
                            Client::spawn(addr, host_port,
                                blocks.clone(), indexes.clone(), tracker)
                            .and_then(move |mut cli| {
                                info!("Connected to {}", addr);
                                cli.register_index(&image_id);
                                if replace {
                                    Either::A(cli.request(ReplaceDir {
                                        image: image_id.clone(),
                                        timestamp: timestamp,
                                        old_image: None,
                                        signatures: signatures
                                            .get(&turl.path[..]).unwrap()
                                            .clone(),
                                        path: VPath::from(turl.path),
                                    })
                                    .map(move |resp| {
                                        info!("Response from {}: {:?}",
                                            addr, resp.accepted);
                                        progress3.lock()
                                            .expect("progress is ok")
                                            .add_ids(resp.hosts);
                                        (resp.accepted, resp.reject_reason)
                                    })
                                    .map_err(|e|
                                        error!("Request error: {}", e)))
                                } else {
                                    Either::B(cli.request(AppendDir {
                                        image: image_id.clone(),
                                        timestamp: timestamp,
                                        signatures: signatures
                                            .get(&turl.path[..]).unwrap()
                                            .clone(),
                                        path: VPath::from(turl.path),
                                    })
                                    .map(move |resp| {
                                        info!("Response from {}: {:?}",
                                            addr, resp.accepted);
                                        progress3.lock()
                                            .expect("progress is ok")
                                            .add_ids(resp.hosts);
                                        (resp.accepted, resp.reject_reason)
                                    })
                                    .map_err(|e|
                                        error!("Request error: {}", e)))
                                }
                            })
                            .and_then(move |(accepted, reason)| {
                                if !accepted {
                                    progress.lock().expect("progress is ok")
                                        .ips_needed.remove(&addr);
                                    if !fail_on_conflict &&
                                       is_conflict_reason(&reason)
                                    {
                                        eprintln!("Upload rejected by \
                                            {} / {}: {}, but that's okay.",
                                            host, addr,
                                            reason.as_ref().map(|x| &x[..])
                                                .unwrap_or("(???)"));
                                        Either::A(ok(true))
                                    } else {
                                        error!("Upload rejected by {} / {}: {}",
                                            host, addr,
                                            reason.as_ref().map(|x| &x[..])
                                                .unwrap_or("(???)"));
                                        Either::A(ok(false))
                                    }
                                } else {
                                    Either::B(done_rx.clone()
                                        .then(move |v| match v {
                                            Ok(_) => Ok(is_ok(&progress)),
                                            Err(_) => {
                                                debug!("Connection closed \
                                                        before all \
                                                        notifications \
                                                        are received");
                                                Ok(false)
                                            }
                                        }))
                                }
                            })
                            .then(move |res| match res {
                                Ok(x) => Ok(x),
                                Err(()) => {
                                    progress2.lock().expect("progress is ok")
                                        .ips_needed.remove(&addr);
                                    // Always succeed, for now, so join will
                                    // drive all the futures, even if one
                                    // failed
                                    error!("Address {} has failed.", addr);
                                    Ok(false)
                                }
                            })
                        })
                        .collect::<Vec<_>>() // to unrefer "names"
                    )
                    .map(|results: Vec<bool>| results.iter().any(|x| *x))
                })
            })
            .collect::<Vec<_>>()
        )
        .map(|results:Vec<bool>| results.iter().all(|x| *x))
    })
}


pub fn cli(mut gopt: GlobalOptions, mut args: Vec<String>) -> ! {
    let mut opt = options::UploadOptions::new();
    {
        let mut ap = ArgumentParser::new();
        opt.define(&mut ap);
        gopt.define(&mut ap);
        args.insert(0, String::from("ciruela upload"));
        match ap.parse(args, &mut stdout(), &mut stderr()) {
            Ok(()) => {}
            Err(code) => exit(code),
        }
    }
    let opt = match opt.finalize() {
        Ok(opt) => opt,
        Err(code) => exit(code),
    };
    match do_upload(gopt, opt) {
        Ok(true) => exit(0),
        Ok(false) => exit(1),
        Err(()) => exit(2),
    }
}
