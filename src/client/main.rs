extern crate abstract_ns;
extern crate argparse;
extern crate ciruela;
extern crate dir_signature;
extern crate env_logger;
extern crate futures;
extern crate futures_cpupool;
extern crate minihttp;
extern crate ns_std_threaded;
extern crate ssh_keys;
extern crate tk_easyloop;
extern crate tokio_core;

#[macro_use] extern crate log;

mod global_options;
mod name;

// Commands
mod upload;

use std::env;
use std::io::{Write, stderr};
use argparse::{ArgumentParser, StoreOption, Collect};
use global_options::GlobalOptions;


fn main() {
    if let Err(_) = env::var("RUST_LOG") {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();

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
        ap.stop_on_first_argument(true);
        ap.parse_args_or_exit();
    }
    match cmd.as_ref().map(|x| &x[..]) {
        Some("upload") => {
            upload::cli(opt, args);
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
