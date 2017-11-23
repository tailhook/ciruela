#![allow(dead_code)]  // temporarily
#![recursion_limit="100"]
extern crate abstract_ns;
extern crate argparse;
extern crate atomic;
extern crate base64;
extern crate blake2;
extern crate ciruela;
extern crate crossbeam;
extern crate crypto;
extern crate digest_writer;
extern crate dir_signature;
extern crate env_logger;
extern crate failure;
extern crate futures;
extern crate futures_cpupool;
extern crate hex;
extern crate hostname;
extern crate libc;
extern crate ns_router;
extern crate ns_std_threaded;
extern crate openat;
extern crate quire;
extern crate rand;
extern crate regex;
extern crate scan_dir;
extern crate self_meter_http;
extern crate serde;
extern crate serde_bytes;
extern crate serde_cbor;
extern crate serde_json;
extern crate ssh_keys;
extern crate time;
extern crate tk_bufstream;
extern crate tk_cantal;
extern crate tk_easyloop;
extern crate tk_http;
extern crate tk_listen;
extern crate tokio_core;
extern crate tokio_io;
extern crate typenum;
extern crate valuable_futures;
extern crate void;

#[macro_use] extern crate log;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate matches;
#[macro_use] extern crate mopa;
#[macro_use] extern crate quick_error;
#[macro_use] extern crate serde_derive;

#[cfg(test)] extern crate humantime;

use std::env;
use std::error::Error;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;

use abstract_ns::{HostResolve, Resolve};
use argparse::{ArgumentParser, Parse, Store, Print, StoreTrue, StoreOption};
use ns_std_threaded::ThreadedResolver;
use ns_router::{Router, Config};

use machine_id::MachineId;

mod cleanup;
mod config;
mod dir_util;
mod disk;
mod failure_tracker;
mod http;
mod index_cache;
mod mask;
mod metadata;
mod named_mutex;
mod peers;
mod remote;
mod tracking;

// common modules for lib and daemon, we don't expose them in the lib because
// that would mean keep backwards compatibility
#[path="../database/mod.rs"] mod database;
#[path="../machine_id.rs"] mod machine_id;
#[path="../proto/mod.rs"] mod proto;
#[path="../serialize/mod.rs"] mod serialize;
#[path="../time_util.rs"] mod time_util;
#[path="../hexlify.rs"] mod hexlify;
pub use ciruela::{VPath};
pub use ciruela::blocks as blocks;
pub use ciruela::index as index;


fn init_logging(mid: MachineId, log_mid: bool) {
    let format = move |record: &log::LogRecord| {
        if log_mid {
            format!("{} {} [{}] {}: {}", mid, time::now_utc().rfc3339(),
                record.location().module_path(),
                record.level(), record.args())
        } else {
            format!("{} [{}] {}: {}", time::now_utc().rfc3339(),
                record.location().module_path(),
                record.level(), record.args())
        }
    };

    let mut builder = env_logger::LogBuilder::new();
    builder.format(format).filter(None, log::LogLevelFilter::Warn);

    if let Ok(value) = env::var("RUST_LOG") {
       builder.parse(&value);
    }

    builder.init()
        .expect("can always initialize logging subsystem");
}

fn main() {

    let mut config_dir = PathBuf::from("/etc/ciruela");
    let mut db_dir = PathBuf::from("/var/lib/ciruela");
    let mut port: u16 = 24783;
    let mut limit: usize = 1000;
    let mut ip: IpAddr = "0.0.0.0".parse().unwrap();
    let mut metadata_threads: usize = 2;
    let mut disk_threads: usize = 8;
    let mut machine_id = None::<MachineId>;
    let mut hostname = hostname::get_hostname();
    let mut log_machine_id = false;
    let mut cantal: bool = false;
    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut config_dir)
            .add_option(&["-c", "--config-base-dir"], Parse,
                "A directory with configuration files (default /etc/ciruela)");
        ap.refer(&mut db_dir)
            .add_option(&["--db-dir"], Parse,
                "A directory where to keep indexes of all directories and
                 other files needed to operate server
                 (default /var/lib/ciruela)");
        ap.refer(&mut ip)
            .add_option(&["--host"], Store,
                "A ip address to listen to (default 0.0.0.0)");
        ap.refer(&mut port)
            .add_option(&["--port"], Store,
                "A port to listen to (default 24783). Note it's used both for
                 TCP and UDP");
        ap.refer(&mut limit)
            .add_option(&["--max-connections"], Store,
                "A maximum number of TCP connections (default 1000).
                 Note: this limit isn't related to maximum size of cluster we
                 can support. More likely it's a number of users can
                 upload data simultaneously minus 10 or so connections for
                 clusteting.");
        ap.refer(&mut metadata_threads)
            .add_option(&["--metadata-threads"], Store,
                "A threads for reading/writing metadata (default 2)");
        ap.refer(&mut disk_threads)
            .add_option(&["--disk-threads"], Store,
                "A threads for reading/writing disk data (default 8)");
        ap.refer(&mut cantal)
            .add_option(&["--cantal"], StoreTrue,
                "Connect to cantal to fetch/update peer list");
        ap.refer(&mut machine_id)
            .add_option(&["--override-machine-id"], StoreOption, "
                Overrides machine id. Do not use in production, put the
                file `/etc/machine-id` instead. This should only be used
                for tests which run multiple nodes in single filesystem
                image");
        ap.refer(&mut hostname)
            .add_option(&["--override-hostname"], StoreOption, "
                Overrides host name, instead of one provided by the system");
        ap.refer(&mut log_machine_id)
            .add_option(&["--log-machine-id"], StoreTrue, "
                Adds machine id to the logs, useful for local multi-node
                testing such as `vagga trio`.");
        ap.add_option(&["--version"],
            Print(env!("CARGO_PKG_VERSION").to_string()),
            "Show version");
        ap.parse_args_or_exit();
    }
    let machine_id = if let Some(machine_id) = machine_id {
        machine_id
    } else {
        match MachineId::read() {
            Ok(x) => x,
            Err(e) => {
                eprintln!("Error reading machine-id: {}", e);
                exit(1);
            }
        }
    };
    let hostname = hostname.unwrap_or_else(|| String::from("localhost"));
    init_logging(machine_id.clone(), log_machine_id);
    warn!("Starting version {}, id {}",
        env!("CARGO_PKG_VERSION"), machine_id);

    let addr = (ip, port).to_socket_addrs().unwrap().next().unwrap();
    let config = match config::read_dirs(&config_dir.join("configs")) {
        Ok(configs) => {
            Arc::new(config::Config {
                port: port,
                db_dir: db_dir,
                config_dir: config_dir,
                dirs: configs,
            })
        }
        Err(e) => {
            error!("Error reading configs: {}", e);
            exit(1);
        }
    };

    let meter = self_meter_http::Meter::new();

    let (disk, disk_init) = match
        disk::Disk::new(disk_threads, &config, &meter)
    {
        Ok(pair) => pair,
        Err(e) => {
            error!("Can't start disk subsystem: {}", e);
            exit(4);
        }
    };

    let meta = match
        metadata::Meta::new(metadata_threads, &config, &meter)
    {
        Ok(meta) => meta,
        Err(e) => {
            error!("Can't open metadata directory {:?}: {}",
                config.db_dir, e);
            exit(4);
        }
    };

    let remote = remote::Remote::new(&hostname, &machine_id);

    let (peers, peers_init) = peers::Peers::new(
        machine_id.clone(),
        if cantal { None } else {
            Some(config.config_dir.join("peers.txt"))
        });

    let (tracking, tracking_init) = tracking::Tracking::new(&config,
        &meta, &disk, &remote, &peers);

    let mut keep_router = None;

    tk_easyloop::run_forever(|| -> Result<(), Box<Error>> {
        meter.spawn_scanner(&tk_easyloop::handle());

        let m1 = meter.clone();
        let m2 = meter.clone();
        let router = Router::from_config(&Config::new()
            .set_fallthrough(ThreadedResolver::use_pool(
                    futures_cpupool::Builder::new()
                    .pool_size(2)
                    .name_prefix("ns-resolver-")
                    .after_start(move || m1.track_current_thread_by_name())
                    .before_stop(move || m2.untrack_current_thread())
                    .create())
                .null_service_resolver()
                .frozen_subscriber())
            .done(), &tk_easyloop::handle());
        keep_router = Some(router.clone());

        http::start(addr, &tracking, &meter)?;
        disk::start(disk_init, &meta)?;
        tracking::start(tracking_init)?;
        peers::start(peers_init, addr, &config, &disk, &router, &tracking)?;

        Ok(())
    }).map_err(|e| {
        error!("Startup error: {}", e);
        exit(1);
    }).expect("looping forever");
}
