extern crate dir_signature;
extern crate ciruela;
extern crate tk_easyloop;
#[macro_use] extern crate log;

use std::process::exit;

use dir_signature::{v1, ScannerConfig, HashType};
use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::{Connection, Config};

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
    let indexes = InMemoryIndexes::new();
    let block_reader = ThreadedBlockReader::new();
    indexes.register_index(&indexbuf)
        .expect("register is okay");
    block_reader.register_dir(DIR, &indexbuf)
        .expect("register is okay");
    let config = Config::new().done();
    tk_easyloop::run(|| {
        //Connection::new(vec!["localhost".parse().unwrap()],
        //    resolver, indexes, block_reader, &config);
        unimplemented!();
        Ok(true)
    })
}
