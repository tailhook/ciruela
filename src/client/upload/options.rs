use std::env::home_dir;
use std::io::{Read, Write, stderr};
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;

use argparse::{ArgumentParser, ParseOption, Collect};
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
    pub private_keys: Vec<PrivateKey>,
}

impl UploadOptions {
    pub fn new() -> UploadOptions {
        UploadOptions {
            source_directory: None,
            identities: Vec::new(),
            private_keys: Vec::new(),
            target_urls: Vec::new(),
        }
    }
    pub fn define<'x, 'y>(&'x mut self, ap: &'y mut ArgumentParser<'x>) {
        ap.refer(&mut self.source_directory)
            .add_option(&["-d", "--directory"], ParseOption, "
                Directory to scan and upload
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
                `$HOME/.ssh` are used.
            ");
    }
    pub fn finalize(mut self) -> Result<UploadOptions, i32> {
        if self.source_directory.is_none() {
            writeln!(&mut stderr(),
                "Argument `-d` or `--directory` is required").ok();
            return Err(1);
        };
        let identities = if self.identities.len() == 0 {
            let home = home_dir()
                .ok_or_else(|| {
                    error!("Cannot find home dir, use `-i` option to specify \
                        identity (private key) explicitly");
                    2
                })?;
            vec![home.join(".ssh/id_ed25519")]
        } else {
            self.identities.iter().map(PathBuf::from).collect()
        };
        let mut keybuf = String::with_capacity(1024);
        for filename in &identities {
            File::open(filename)
                .and_then(|mut f| f.read_to_string(&mut keybuf))
                .map_err(|e| {
                    error!("Error reading private key {:?}: {}", filename, e);
                    2
                })?;
            let keys = parse_private_key(&keybuf)
                .map_err(|e| {
                    error!("Error reading private key {:?}: {}", filename, e);
                    2
                })?;
            self.private_keys.extend(keys);
        }
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
