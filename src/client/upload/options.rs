use std::path::PathBuf;

use argparse::{ArgumentParser, ParseOption, Collect};


pub struct UploadOptions {
    source_directory: Option<PathBuf>,
    target_urls: Vec<String>,
    identities: Vec<String>,
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

