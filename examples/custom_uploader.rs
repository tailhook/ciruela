extern crate dir_signature;
extern crate ciruela;
#[macro_use] extern crate log;

use std::process::exit;

use dir_signature::{v1, ScannerConfig, HashType};
use ciruela::blocks::ThreadedBlockReader;
use ciruela::VPath;

const DIR: &str = "./src";

fn main() {
    match run() {
        Ok(true) => exit(0),
        Ok(false) => exit(1),
        Err(()) => exit(2),
    }
}

fn run() -> Result<bool, ()> {
    let mut cfg = ScannerConfig::new();
    cfg.auto_threads();
    cfg.hash(HashType::blake2b_256());
    cfg.add_dir(&DIR, "/");
    cfg.print_progress();
    let mut indexbuf = Vec::new();
    v1::scan(&cfg, &mut indexbuf)
        .map_err(|e| error!("Error scanning {:?}: {}", DIR, e))?;
    let block_reader = ThreadedBlockReader::new();
    block_reader.register_dir(DIR, &VPath::from("/dest/src"), &indexbuf)
        .expect("register is okay");
    unimplemented!();
}
