use std::io::{Write, stderr};
use std::path::{PathBuf};
use std::str::FromStr;

use abstract_ns::Name;
use argparse::{ArgumentParser, ParseOption, Collect, StoreTrue, StoreFalse};
use ssh_keys::PrivateKey;

use keys::read_keys;


#[derive(Clone, Debug)]
pub struct TargetUrl {
    pub host: Name,
    pub path: String,
}


pub struct UploadOptions {
    pub source_directory: Option<PathBuf>,
    pub target_urls: Vec<TargetUrl>,
    pub identities: Vec<String>,
    pub key_vars: Vec<String>,
    pub private_keys: Vec<PrivateKey>,
    pub replace: bool,
    pub fail_on_conflict: bool,
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
            fail_on_conflict: true,
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
        ap.refer(&mut self.fail_on_conflict)
            .add_option(&["-x", "--no-fail-on-conflict"], StoreFalse, "
                If reason of rejection is `already_exists` or
                `already_uploading_different_version` treat this reason as
                successful upload rather than error. This is mean for the
                cases where you rebuild images with right version (directory
                name) but it has different hash of contents (build is not
                entirely reproducible). Usually it's fine having any of the
                version for this directory (instead of failing).

                Note: this should not be used if you don't have version
                number/hash in directory name, this is why it's not default.
            ");
    }
    pub fn finalize(mut self) -> Result<UploadOptions, i32> {
        if self.source_directory.is_none() {
            writeln!(&mut stderr(),
                "Argument `-d` or `--directory` is required").ok();
            return Err(1);
        };
        self.private_keys = match read_keys(&self.identities, &self.key_vars) {
            Ok(keys) => keys,
            Err(e) => {
                error!("{}", e);
                return Err(2);
            }
        };
        Ok(self)
    }
}

impl FromStr for TargetUrl {
    type Err = String;
    fn from_str(s: &str) -> Result<TargetUrl, String> {
        if let Some(off) = s.find(":") {
            let path = &s[off+1..];
            let host = match s[..off].parse::<Name>() {
                Ok(name) => name,
                Err(e) => return Err(e.to_string())
            };
            if !path.starts_with("/") {
                return Err(String::from("Path must start with slash"));
            }
            Ok(TargetUrl {
                host: host,
                path: path.to_string(),
            })
        } else {
            Err(String::from("Target URL must be in format `host:/path`"))
        }
    }
}
