use std::str::FromStr;
use std::path::PathBuf;

use argparse::{ArgumentParser, ParseOption, Collect};

#[derive(Clone, Debug)]
pub struct TargetUrl {
    pub host: String,
    pub path: String,
}


pub struct UploadOptions {
    pub source_directory: Option<PathBuf>,
    pub target_urls: Vec<TargetUrl>,
    pub identities: Vec<String>,
}

impl UploadOptions {
    pub fn new() -> UploadOptions {
        UploadOptions {
            source_directory: None,
            identities: Vec::new(),
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
