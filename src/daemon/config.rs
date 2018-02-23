use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use {VPath};
use machine_id::MachineId;
use scan_dir::ScanDir;
use quire::validate::{Directory as Dir, Structure, Numeric, Scalar, Sequence};
use quire::{parse_config, Options, ErrorList};


pub struct Config {
    pub machine_id: MachineId,
    pub hostname: String,
    pub port: u16,
    pub db_dir: PathBuf,
    pub config_dir: PathBuf,
    pub dirs: HashMap<String, Arc<Directory>>,
}


#[derive(Debug, Deserialize)]
pub struct Directory {
    pub directory: PathBuf,
    pub append_only: bool,
    pub num_levels: usize,
    pub upload_keys: Vec<String>,
    pub download_keys: Vec<String>,
    pub auto_clean: bool,
    pub keep_list_file: Option<PathBuf>,
    pub keep_min_directories: usize,
    pub keep_max_directories: usize,
    #[serde(with="::quire::duration")]
    pub keep_recent: Duration,
}

fn directory_validator<'x>() -> Structure<'x> {
    Structure::new()
    .member("directory", Dir::new())
    .member("append_only", Scalar::new())
    // the limit here is just arbitrary, maybe we will lift it later
    .member("num_levels", Numeric::new().min(1).max(16))
    .member("upload_keys", Sequence::new(Scalar::new()))
    .member("download_keys", Sequence::new(Scalar::new()))
    .member("auto_clean", Scalar::new().default(false))
    .member("keep_list_file", Scalar::new().optional())
    .member("keep_min_directories", Numeric::new().min(1).default(2))
    .member("keep_max_directories", Numeric::new().min(1).default(100))
    .member("keep_recent", Scalar::new().default("2 days"))
}

pub fn read_dirs(path: &Path)
    -> Result<HashMap<String, Arc<Directory>>, String>
{
    if !path.is_dir() {
        warn!("No directory {:?} found", path);
        return Ok(HashMap::new());
    }
    let validator = directory_validator();
    ScanDir::files().read(path, |iter| {
        let mut res = HashMap::new();
        let yamls = iter.filter(|&(_, ref name)| name.ends_with(".yaml"));
        for (entry, fname) in yamls {
            let name = fname[..fname.len() - 5].to_string();
            let config = parse_config(entry.path(),
                &validator, &Options::default())?;
            res.insert(name, Arc::new(config));
        }
        Ok::<_, ErrorList>(res)
    }).map_err(|e| e.to_string()).and_then(|v| v.map_err(|e| e.to_string()))
}

impl Config {
    pub fn is_valid_destination(&self, vpath: &VPath) -> bool {
        if let Some(ref cfg) = self.dirs.get(vpath.key()) {
            cfg.num_levels == vpath.level()
        } else {
            false
        }
    }
}
