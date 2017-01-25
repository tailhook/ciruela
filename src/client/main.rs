extern crate argparse;

mod global_options;

// Commands
mod upload;

use std::io::{Write, stderr};
use argparse::{ArgumentParser, StoreOption};
use global_options::GlobalOptions;


fn main() {
    let mut cmd = None::<String>;
    let mut opt = GlobalOptions::new();
    {
        let mut ap = ArgumentParser::new();
        opt.define(&mut ap);
        ap.refer(&mut cmd)
            .add_argument("command", StoreOption, r#"
                Command to run. Available commands: `upload`.
            "#);
        ap.stop_on_first_argument(false);
        ap.parse_args_or_exit();
    }
    match cmd.as_ref().map(|x| &x[..]) {
        Some("upload") => {
            upload::cli(opt);
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
