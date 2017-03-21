use std::path::Path;

use config::{Directory};


pub struct DirConfig<'a> {
    pub base: &'a Path,
    pub parent: &'a Path,
    pub image_name: &'a str,
    pub config: &'a Directory,
}
