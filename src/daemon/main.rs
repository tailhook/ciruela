extern crate time;
extern crate argparse;
extern crate futures;
extern crate env_logger;
extern crate tokio_core;
extern crate minihttp;
#[macro_use] extern crate log;

use std::env;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::PathBuf;

use futures::empty;
use tokio_core::reactor::Core;
use argparse::{ArgumentParser, Parse, Store};

mod http;


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
        ap.parse_args_or_exit();
    }
    let addr = (ip, port).to_socket_addrs().unwrap().next().unwrap();

    let mut lp = Core::new().unwrap();
    http::start(addr, &lp.handle());
    lp.run(empty::<(), ()>()).unwrap();
}
