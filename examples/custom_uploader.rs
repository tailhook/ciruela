extern crate env_logger;
extern crate dir_signature;
extern crate ciruela;
extern crate failure;
extern crate tk_easyloop;
extern crate ns_env_config;

use std::process::exit;

use failure::{Error, ResultExt};
use tk_easyloop::handle;
use dir_signature::{v1, ScannerConfig, HashType};
use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::{Connection, Config};
use ciruela::VPath;

const DIR: &str = "./src";

fn main() {
    env_logger::init();
    match run() {
        Ok(()) => exit(0),
        Err(err) => {
            eprintln!("Error: {}", err);
            exit(1);
        }
    }
}

fn run() -> Result<(), Error> {
    let mut cfg = ScannerConfig::new();
    cfg.auto_threads();
    cfg.hash(HashType::blake2b_256());
    cfg.add_dir(&DIR, "/");
    cfg.print_progress();
    let mut indexbuf = Vec::new();
    v1::scan(&cfg, &mut indexbuf).context(DIR)?;
    let indexes = InMemoryIndexes::new();
    let block_reader = ThreadedBlockReader::new();
    let image_id = indexes.register_index(&indexbuf)
        .expect("register is okay");
    block_reader.register_dir(DIR, &indexbuf)
        .expect("register is okay");
    let config = Config::new().done();
    tk_easyloop::run(|| {
        let ns = ns_env_config::init(&handle()).expect("init dns");
        let conn = Connection::new(vec!["localhost".parse().unwrap()],
            ns, indexes, block_reader, &config);
        let up = conn.append(&image_id, &VPath::from("/virtual-dir/sub-dir"));
        up.future()
    })?;
    Ok(())
}
