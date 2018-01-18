#![allow(dead_code)]  // temporarily
extern crate abstract_ns;
extern crate argparse;
extern crate base64;
extern crate blake2;
extern crate ciruela;
extern crate crypto;
extern crate digest_writer;
extern crate dir_signature;
extern crate env_logger;
extern crate failure;
extern crate futures;
extern crate futures_cpupool;
extern crate gumdrop;
extern crate hex;
extern crate ns_router;
extern crate ns_std_threaded;
extern crate serde;
extern crate serde_bytes;
extern crate serde_cbor;
extern crate ssh_keys;
extern crate tk_bufstream;
extern crate tk_easyloop;
extern crate tk_http;
extern crate tokio_core;
extern crate tokio_io;
extern crate void;

#[macro_use] extern crate gumdrop_derive;
#[macro_use] extern crate log;
#[macro_use] extern crate matches;
#[macro_use] extern crate mopa;
#[macro_use] extern crate quick_error;
#[macro_use] extern crate serde_derive;

mod global_options;
mod name;

// Commands
mod upload;
mod sync;

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
pub use ciruela::signature as signature;

use std::env;
use std::io::{Write, stderr};
use argparse::{ArgumentParser, StoreOption, Collect, Print};
use global_options::GlobalOptions;


fn main() {
    if let Err(_) = env::var("RUST_LOG") {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init();

    let mut cmd = None::<String>;
    let mut opt = GlobalOptions::new();
    let mut args = Vec::new();
    {
        let mut ap = ArgumentParser::new();
        opt.define(&mut ap);
        ap.refer(&mut cmd)
            .add_argument("command", StoreOption, r#"
                Command to run. Available commands: `upload`.
            "#);
        ap.refer(&mut args)
            .add_argument("args", Collect, r#"
                Arguments and options to the command
            "#);
        ap.add_option(&["--version"],
            Print(env!("CARGO_PKG_VERSION").to_string()),
            "Show version");
        ap.stop_on_first_argument(true);
        ap.parse_args_or_exit();
    }
    match cmd.as_ref().map(|x| &x[..]) {
        Some("upload") => {
            upload::cli(opt, args);
        }
        Some("sync") => {
            sync::cli(opt, args);
        }
        None => {
            writeln!(&mut stderr(), "\
                Command argument required. Try:\n\
                \n\
                  ciruela upload\n\
            ").ok();
        }
        Some(cmd) => {
            writeln!(&mut stderr(), "\
                Unknown command {:?}. Try:\n\
                \n\
                  ciruela upload\n\
            ", cmd).ok();
        }
    }
}
