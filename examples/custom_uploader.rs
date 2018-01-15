extern crate env_logger;
extern crate dir_signature;
extern crate ciruela;
extern crate failure;
extern crate tk_easyloop;
extern crate ns_env_config;
extern crate ssh_keys;
extern crate rand;

use std::fs::File;
use std::io::Read;
use std::process::exit;
use std::time::SystemTime;

use failure::{Error, ResultExt};
use tk_easyloop::handle;
use dir_signature::{v1, ScannerConfig, HashType};
use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::{Connection, Config};
use ciruela::VPath;
use ciruela::signature::sign_upload;
use rand::Rng;
use ssh_keys::openssh::parse_private_key;
use ssh_keys::PrivateKey;


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

fn read_keys() -> Result<Vec<PrivateKey>, Error> {
    let mut f = File::open("ciruela-example.key")
        .context("error reading ciruela-example.key")?;
    let mut keybuf = String::with_capacity(1024);
    f.read_to_string(&mut keybuf)
        .context("error reading ciruela-example.key")?;
    let keys = parse_private_key(&keybuf)
        .context("error reading ciruela-example.key")?;
    return Ok(keys);
}

fn run() -> Result<(), Error> {

    let dest = VPath::from(
        format!("/dir1/custom_uploader/{}",
        rand::thread_rng().gen::<u32>()));

    let mut cfg = ScannerConfig::new();
    cfg.auto_threads();
    cfg.hash(HashType::blake2b_256());
    cfg.add_dir(&DIR, "/");
    cfg.print_progress();
    let mut indexbuf = Vec::new();
    v1::scan(&cfg, &mut indexbuf).context(DIR)?;

    let keys = read_keys()?;

    let indexes = InMemoryIndexes::new();
    let block_reader = ThreadedBlockReader::new();
    let image_id = indexes.register_index(&indexbuf)
        .expect("register is okay");
    block_reader.register_dir(DIR, &indexbuf)
        .expect("register is okay");

    let timestamp = SystemTime::now();
    let upload = sign_upload(&dest, &image_id, timestamp, &keys);

    let config = Config::new().done();
    tk_easyloop::run(|| {
        let ns = ns_env_config::init(&handle()).expect("init dns");
        let conn = Connection::new(vec!["localhost".parse().unwrap()],
            ns, indexes, block_reader, &config);
        let up = conn.append(upload);
        up.future()
    })?;
    Ok(())
}
