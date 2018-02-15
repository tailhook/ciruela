use std::sync::Arc;

use abstract_ns::Name;
use failure::Error;
use futures::future::join_all;
use tk_easyloop::{self, handle};
use ns_env_config;

use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::{Config, Connection};

use sync::uploads::Upload;


pub fn upload(config: Arc<Config>, clusters: Vec<Vec<Name>>,
    uploads: Vec<Upload>,
    indexes: &InMemoryIndexes, blocks: &ThreadedBlockReader)
    -> Result<(), Error>
{
    let res = tk_easyloop::run(move || {
        let ns = ns_env_config::init(&handle()).expect("init dns");
        join_all(clusters.into_iter().map(move |names| {
            let ns = ns.clone();
            let indexes = indexes.clone();
            let blocks = blocks.clone();
            let config = config.clone();
            let conn = Connection::new(names,
                ns, indexes.clone(), blocks.clone(), &config);
            join_all(uploads.clone().into_iter().map(move |upload| {
                let up = match upload {
                    Upload::Append(a) => conn.append(a.clone()),
                    Upload::Replace(r) => conn.replace(r.clone()),
                    Upload::WeakAppend(a) => conn.append_weak(a.clone()),
                };
                up.future()
            }))
        }))
    })?;
    for res in res.iter().flat_map(|x| x) {
        println!("{}", res);
    }
    Ok(())
}
