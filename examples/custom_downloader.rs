extern crate env_logger;
extern crate ciruela;
extern crate failure;
extern crate futures;
extern crate tk_easyloop;
extern crate ns_env_config;
extern crate ssh_keys;

use std::process::exit;

use failure::{Error, ResultExt};
use futures::Future;
use tk_easyloop::handle;
use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::{Connection, Config};
use ciruela::VPath;


const VPATH: &str = "/dir1/a/1";
const FILE: &str = "/daemon/metrics.rs";


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
    let indexes = InMemoryIndexes::new();
    let block_reader = ThreadedBlockReader::new();

    let config = Config::new().done();
    let data = tk_easyloop::run(|| {
        let ns = ns_env_config::init(&handle()).expect("init dns");
        let conn = Connection::new(vec!["localhost".parse().unwrap()],
            ns, indexes, block_reader, &config);
        conn.fetch_index(&VPath::from(VPATH))
        .then(|res| res.context("can't fetch index"))
        .and_then(|idx| idx.into_mut().context("can't parse index"))
        .and_then(move |idx| {
            conn.fetch_file(&idx, FILE)
            .then(|res| res.context("can't fetch file"))
        })
    })?;
    println!("--- file data ---\n{}", &String::from_utf8_lossy(&data));
    Ok(())
}
