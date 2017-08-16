use std::io::{self, BufReader, BufRead};
use std::sync::Arc;

use ssh_keys::PublicKey;
use ssh_keys::openssh::parse_public_key;

use config::Directory;
use openat::Dir;
use metadata::{Error, Meta};


fn read_keys(dir: &Dir, name: &str, keys: &mut Vec<PublicKey>) {
    let f = match dir.open_file(name) {
        Ok(f) => f,
        Err(e) => {
            error!("Can't read key {:?}: {}", name, e);
            return;
        }
    };
    for line in BufReader::new(f).lines() {
        let line = match line {
            Ok(ref line) => line.trim(),
            Err(e) => {
                error!("Can't read key {:?}: {}", name, e);
                return;
            }
        };
        if line == "" || line.starts_with("#") {
            continue;
        }
        match parse_public_key(line) {
            Ok(key) => keys.push(key),
            Err(e) => {
                error!("Can't parse key {:?}: {}", name, e);
                return;
            }
        };
    }
}

pub fn read_upload_keys(cfg: &Arc<Directory>, meta: &Meta)
    -> Result<Vec<PublicKey>, Error>
{
    let mut res = Vec::new();
    let cfg_dir = Dir::open(&meta.0.config.config_dir)
        .map_err(|e| Error::ReadKey(meta.0.config.config_dir.clone(), e))?;
    read_keys(&cfg_dir, "master.key", &mut res);
    match cfg_dir.sub_dir("keys") {
        Ok(dir) => {
            for key in &cfg.upload_keys {
                read_keys(&dir, &format!("{}.key", key), &mut res);
            }
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            for key in &cfg.upload_keys {
                error!("Can't read key {:?}: no such file", key);
            }
        }
        Err(e) => {
            return Err(Error::ReadKey(
                meta.0.config.config_dir.join("keys"), e));
        }
    }
    Ok(res)
}
