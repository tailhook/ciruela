use std::io::{stdout, stderr};
use std::process::exit;

use argparse::{ArgumentParser};

mod options;

use global_options::GlobalOptions;


pub fn cli(mut gopt: GlobalOptions, mut args: Vec<String>) {
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
    unimplemented!();
}
