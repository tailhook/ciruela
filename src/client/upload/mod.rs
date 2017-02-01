use std::io::{stdout, stderr, Write};
use std::error::Error;
use std::process::exit;

use argparse::{ArgumentParser};
use dir_signature::{ScannerConfig, HashType, v1};

mod options;

use global_options::GlobalOptions;


fn do_upload(_gopt: GlobalOptions, opt: options::UploadOptions)
    -> Result<(), Box<Error>>
{
    let dir = opt.source_directory.clone().unwrap();
    let mut cfg = ScannerConfig::new();
    cfg.hash(HashType::Blake2b_256);
    cfg.add_dir(&dir, "/");
    cfg.print_progress();

    let mut indexbuf = Vec::new();
    v1::scan(&cfg, &mut indexbuf)?;

    unimplemented!();
}


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
    if opt.source_directory.is_none() {
        writeln!(&mut stderr(),
            "Argument `-d` or `--directory` is required").ok();
        exit(1);
    };
    match do_upload(gopt, opt) {
        Ok(()) => {}
        Err(e) => {
            error!("Fatal error: {}", e);
            exit(2);
        }
    }
}
