extern crate argparse;
extern crate ciruela;
extern crate env_logger;
extern crate futures;
extern crate futures_cpupool;
extern crate minihttp;
extern crate openat;
extern crate quire;
extern crate rustc_serialize;
extern crate scan_dir;
extern crate serde_cbor;
extern crate time;
extern crate tk_easyloop;
extern crate tokio_core;

#[macro_use] extern crate log;
#[macro_use] extern crate quick_error;

use std::env;
use std::error::Error;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;

use argparse::{ArgumentParser, Parse, Store};

mod http;
mod config;
mod websocket;
mod metadata;


fn main() {
    if let Err(_) = env::var("RUST_LOG") {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();

    let mut config_dir = PathBuf::from("/etc/ciruela");
    let mut db_dir = PathBuf::from("/var/lib/ciruela");
    let mut port: u16 = 24783;
    let mut limit: usize = 1000;
    let mut ip: IpAddr = "0.0.0.0".parse().unwrap();
    let mut metadata_threads: usize = 2;
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
        ap.parse_args_or_exit();
    }
    let addr = (ip, port).to_socket_addrs().unwrap().next().unwrap();
    let config = match config::read_dirs(&config_dir.join("configs")) {
        Ok(configs) => {
            Arc::new(config::Config {
                db_dir: db_dir,
                dirs: configs,
            })
        }
        Err(e) => {
            error!("Error reading configs: {}", e);
            exit(1);
        }
    };

    let meta = match metadata::Meta::new(metadata_threads, &config) {
        Ok(meta) => meta,
        Err(e) => {
            error!("Can't open metadata directory {:?}: {}",
                config.db_dir, e);
            exit(4);
        }
    };

    tk_easyloop::run_forever(|| -> Result<(), Box<Error>> {
        http::start(addr, &meta)?;
        Ok(())
    }).map_err(|e| {
        error!("Startup error: {}", e);
        exit(1);
    }).expect("looping forever");
}
