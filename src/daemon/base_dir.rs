use std::sync::Arc;
use std::collections::BTreeSet;

use futures::Future;

use ciruela::{VPath, Hash};
use ciruela::proto::BaseDirState;
use config::Directory;
use disk::{self, Disk};
use metadata::{self, Meta};
use peers::config::get_hash;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Meta(err: metadata::Error) {
            description(err.description())
            display("{}", err)
            from(err)
        }
        Disk(err: disk::Error) {
            description(err.description())
            display("{}", err)
            from(err)
        }
    }
}


pub fn scan(path: &VPath, config: &Arc<Directory>, meta: &Meta, disk: &Disk)
    -> Box<Future<Item=BaseDirState, Error=Error>>
{
    let path = path.clone();
    let config = config.clone();
    Box::new(meta.scan_dir(&path).map_err(Error::Meta)
        .join(disk.read_keep_list(&config).map_err(Error::Disk))
        .map(move |(dirs, keep_list)| {
            let kl: BTreeSet<String> = keep_list.into_iter()
                .filter_map(|p| {
                    p.strip_prefix(path.suffix()).ok()
                    .and_then(|name| name.to_str())
                    .map(|name| {
                        assert!(name.find('/').is_none());
                        name.to_string()
                    })
                }).collect();
            BaseDirState {
                path: path,
                config_hash: get_hash(&config),
                keep_list_hash: Hash::for_object(&kl),
                dirs: dirs,
            }
        }))
}
