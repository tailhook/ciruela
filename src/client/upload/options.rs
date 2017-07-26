use std::env::{self, home_dir};
use std::io::{self, Read, Write, stderr};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use argparse::{ArgumentParser, ParseOption, Collect, StoreTrue};
use ssh_keys::PrivateKey;
use ssh_keys::openssh::parse_private_key;

#[derive(Clone, Debug)]
pub struct TargetUrl {
    pub host: String,
    pub path: String,
}


pub struct UploadOptions {
    pub source_directory: Option<PathBuf>,
    pub target_urls: Vec<TargetUrl>,
    pub identities: Vec<String>,
    pub key_vars: Vec<String>,
    pub private_keys: Vec<PrivateKey>,
    pub replace: bool,
}

fn keys_from_file(filename: &Path, allow_non_existent: bool,
    res: &mut Vec<PrivateKey>)
    -> Result<(), String>
{
    let mut f = match File::open(filename) {
        Ok(f) => f,
        Err(ref e)
        if e.kind() == io::ErrorKind::NotFound && allow_non_existent
        => {
            return Ok(());
        }
        Err(e) => return Err(format!("{}", e)),
    };
    let mut keybuf = String::with_capacity(1024);
    f.read_to_string(&mut keybuf)
        .map_err(|e| format!("{}", e))?;
    let keys = parse_private_key(&keybuf)
        .map_err(|e| format!("{}", e))?;
    res.extend(keys);
    Ok(())
}

fn keys_from_env(name: &str, allow_non_existent: bool,
    res: &mut Vec<PrivateKey>)
    -> Result<(), String>
{
    let value = match env::var(name) {
        Ok(x) => x,
        Err(ref e)
        if matches!(e, &env::VarError::NotPresent) && allow_non_existent
        => {
            return Ok(());
        }
        Err(e) => return Err(format!("{}", e)),
    };
    let keys = parse_private_key(&value)
        .map_err(|e| format!("{}", e))?;
    res.extend(keys);
    Ok(())
}

impl UploadOptions {
    pub fn new() -> UploadOptions {
        UploadOptions {
            source_directory: None,
            identities: Vec::new(),
            key_vars: Vec::new(),
            private_keys: Vec::new(),
            target_urls: Vec::new(),
            replace: false,
        }
    }
    pub fn define<'x, 'y>(&'x mut self, ap: &'y mut ArgumentParser<'x>) {
        ap.refer(&mut self.source_directory)
            .add_option(&["-d", "--directory"], ParseOption, "
                Directory to scan and upload
            ");
        ap.refer(&mut self.replace)
            .add_option(&["--replace"], StoreTrue, "
                Replace directory if not exists
            ");
        ap.refer(&mut self.target_urls)
            .add_argument("destination", Collect, "
                Upload image to the specified location. Location looks a
                lot like SSH url: `hostname:/path/to/target/dir`. Except
                path is not at the root, but first component is a virtual
                directory and other components resemble configured hierarchy.
            ");
        ap.refer(&mut self.identities)
            .add_option(&["-i", "--identity"], Collect, "
                Use the specified identity files (basically ssh-keys) to
                sign the upload. By default all supported keys in
                `$HOME/.ssh` and a key passed in environ variable `CIRUELA_KEY`
                are used. Note: multiple `-i` flags may be used.
            ");
        ap.refer(&mut self.key_vars)
            .add_option(&["-k", "--key-from-env"], Collect, "
                Use specified env variable to get identity (basically ssh-key).
                The environment variable contains actual key, not the file
                name. Multiple variables can be specified along with `-i`.
                If neither `-i` nor `-k` options present, default ssh keys
                and `CIRUELA_KEY` environment variable are used if present.
                Useful for CI systems.
            ");
    }
    pub fn finalize(mut self) -> Result<UploadOptions, i32> {
        if self.source_directory.is_none() {
            writeln!(&mut stderr(),
                "Argument `-d` or `--directory` is required").ok();
            return Err(1);
        };
        let no_default = self.identities.len() == 0 &&
            self.key_vars.len() == 0;
        if no_default {
            match home_dir() {
                Some(home) => {
                    let path = home.join(".ssh/id_ed25519");
                    keys_from_file(&path, true, &mut self.private_keys)
                        .map_err(|e| {
                            error!("Can't read key file {:?}: {}", path, e);
                            2
                        })?;
                    let path = home.join(".ssh/id_ciruela");
                    keys_from_file(&path, true, &mut self.private_keys)
                        .map_err(|e| {
                            error!("Can't read key file {:?}: {}", path, e);
                            2
                        })?;
                    let path = home.join(".ciruela/id_ed25519");
                    keys_from_file(&path, true, &mut self.private_keys)
                        .map_err(|e| {
                            error!("Can't read key file {:?}: {}", path, e);
                            2
                        })?;
                }
                None => {
                    warn!("Cannot find home dir. \
                        Use `-i` or `-k` options to specify \
                        identity (private key) explicitly.");
                }
            }
            keys_from_env("CIRUELA_KEY", true, &mut self.private_keys)
                .map_err(|e| {
                    error!("Can't read env key CIRUELA_KEY: {}", e);
                    2
                })?;
        } else {
            for ident in &self.identities {
                keys_from_file(&Path::new(&ident), false,
                    &mut self.private_keys)
                .map_err(|e| {
                    error!("Can't read key file {:?}: {}", ident, e);
                    2
                })?;
            }
            for name in &self.key_vars {
                keys_from_env(&name, false, &mut self.private_keys)
                .map_err(|e| {
                    error!("Can't read env key {:?}: {}", name, e);
                    2
                })?;
            }
        };
        info!("Read {} private keys", self.private_keys.len());
        Ok(self)
    }
}

impl FromStr for TargetUrl {
    type Err = String;
    fn from_str(s: &str) -> Result<TargetUrl, String> {
        if let Some(off) = s.find(":") {
            let host = &s[..off];
            let path = &s[off+1..];
            if host.len() == 0 {
                return Err(String::from("Host must not be empty"));
            }
            if !path.starts_with("/") {
                return Err(String::from("Path must start with slash"));
            }
            Ok(TargetUrl {
                host: host.to_string(),
                path: path.to_string(),
            })
        } else {
            Err(String::from("Target URL must be in format `host:/path`"))
        }
    }
}
