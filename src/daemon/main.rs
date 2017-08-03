#![recursion_limit="100"]
extern crate abstract_ns;
extern crate argparse;
extern crate ciruela;
extern crate crossbeam;
extern crate dir_signature;
extern crate env_logger;
extern crate futures;
extern crate futures_cpupool;
extern crate hex;
extern crate ns_std_threaded;
extern crate openat;
extern crate quire;
extern crate regex;
extern crate rustc_serialize;
extern crate scan_dir;
extern crate serde;
extern crate serde_cbor;
extern crate ssh_keys;
extern crate time;
extern crate tk_bufstream;
extern crate tk_easyloop;
extern crate tk_http;
extern crate tk_listen;
extern crate tokio_core;

#[macro_use] extern crate log;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate matches;
#[macro_use] extern crate quick_error;

#[cfg(test)] extern crate humantime;
#[cfg(test)] extern crate rand;

use std::env;
use std::error::Error;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;

use abstract_ns::RouterBuilder;
use argparse::{ArgumentParser, Parse, Store, Print, StoreTrue};
use futures_cpupool::CpuPool;
use ns_std_threaded::ThreadedResolver;


mod cleanup;
mod config;
mod dir_util;
mod disk;
mod http;
mod index;
mod metadata;
mod peers;
mod remote;
mod tracking;
mod websocket;

fn init_logging() {
    let format = |record: &log::LogRecord| {
        format!("{} [{}] {}: {}", time::now_utc().rfc3339(),
            record.location().module_path(),
            record.level(), record.args())
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
    init_logging();

    let mut config_dir = PathBuf::from("/etc/ciruela");
    let mut db_dir = PathBuf::from("/var/lib/ciruela");
    let mut port: u16 = 24783;
    let mut limit: usize = 1000;
    let mut ip: IpAddr = "0.0.0.0".parse().unwrap();
    let mut metadata_threads: usize = 2;
    let mut disk_threads: usize = 8;
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
        ap.add_option(&["--version"],
            Print(env!("CARGO_PKG_VERSION").to_string()),
            "Show version");
        ap.parse_args_or_exit();
    }
    let addr = (ip, port).to_socket_addrs().unwrap().next().unwrap();
    let config = match config::read_dirs(&config_dir.join("configs")) {
        Ok(configs) => {
            Arc::new(config::Config {
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

    let mut router = RouterBuilder::new();
    router.add_default(ThreadedResolver::new(CpuPool::new(1)));
    let router = router.into_resolver();

    let (tracking, tracking_init) = tracking::Tracking::new();

    let (disk, disk_init) = match disk::Disk::new(disk_threads, &config) {
        Ok(pair) => pair,
        Err(e) => {
            error!("Can't start disk subsystem: {}", e);
            exit(4);
        }
    };

    let meta = match
        metadata::Meta::new(metadata_threads, &config, &tracking)
    {
        Ok(meta) => meta,
        Err(e) => {
            error!("Can't open metadata directory {:?}: {}",
                config.db_dir, e);
            exit(4);
        }
    };

    let remote = remote::Remote::new();

    let (peers, peers_init) = peers::Peers::new(
        if cantal { None } else {
            Some(config.config_dir.join("peers.txt"))
        });

    tk_easyloop::run_forever(|| -> Result<(), Box<Error>> {
        http::start(addr, &meta, &remote)?;
        disk::start(disk_init, &meta)?;
        tracking::start(tracking_init, &config, &meta, &remote, &disk)?;
        peers::start(peers_init, &config, &disk, &router)?;
        Ok(())
    }).map_err(|e| {
        error!("Startup error: {}", e);
        exit(1);
    }).expect("looping forever");
}
