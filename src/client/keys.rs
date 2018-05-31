use std::io::{self, Read};
use std::env::{self, home_dir};
use std::path::Path;
use std::fs::File;

use failure::{Error, ResultExt};
use ssh_keys::PrivateKey;
use ssh_keys::openssh::parse_private_key;


fn keys_from_file(filename: &Path, allow_non_existent: bool,
    res: &mut Vec<PrivateKey>)
    -> Result<(), Error>
{
    let mut f = match File::open(filename) {
        Ok(f) => f,
        Err(ref e)
        if e.kind() == io::ErrorKind::NotFound && allow_non_existent
        => {
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    let mut keybuf = String::with_capacity(1024);
    f.read_to_string(&mut keybuf)?;
    let keys = parse_private_key(&keybuf)?;
    res.extend(keys);
    Ok(())
}

fn keys_from_env(name: &str, allow_non_existent: bool,
    res: &mut Vec<PrivateKey>)
    -> Result<(), Error>
{
    let value = match env::var(name) {
        Ok(x) => x,
        Err(ref e)
        if matches!(e, &env::VarError::NotPresent) && allow_non_existent
        => {
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    let keys = parse_private_key(&value)?;
    res.extend(keys);
    Ok(())
}

pub fn read_keys(identities: &Vec<String>, key_vars: &Vec<String>)
    -> Result<Vec<PrivateKey>, Error>
{
    let mut private_keys = Vec::new();
    let no_default = identities.len() == 0 &&
        key_vars.len() == 0;
    if no_default {
        keys_from_env("CIRUELA_KEY", true, &mut private_keys)
            .context(format!("Can't read env key CIRUELA_KEY"))?;
        match home_dir() {
            Some(home) => {
                let path = home.join(".ssh/id_ed25519");
                keys_from_file(&path, true, &mut private_keys)
                    .context(format!("Can't read key file {:?}", path))?;
                let path = home.join(".ssh/id_ciruela");
                keys_from_file(&path, true, &mut private_keys)
                    .context(format!("Can't read key file {:?}", path))?;
                let path = home.join(".ciruela/id_ed25519");
                keys_from_file(&path, true, &mut private_keys)
                    .context(format!("Can't read key file {:?}", path))?;
            }
            None if private_keys.len() == 0 => {
                warn!("Cannot find home dir. \
                    Use `-i` or `-k` options to specify \
                    identity (private key) explicitly.");
            }
            None => {}  // fine if there is some key, say from env variable
        }
    } else {
        for ident in identities {
            keys_from_file(&Path::new(&ident), false,
                &mut private_keys)
            .context(format!("Can't read key file {:?}", ident))?;
        }
        for name in key_vars {
            keys_from_env(&name, false, &mut private_keys)
            .context(format!("Can't read env key {:?}", name))?;
        }
    };
    eprintln!("Read {} private keys, list:", private_keys.len());
    for key in &private_keys {
        eprintln!("  {}", key.public_key().to_string());
    }
    return Ok(private_keys)
}
