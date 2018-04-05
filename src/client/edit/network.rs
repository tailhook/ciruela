use std::sync::Arc;

use abstract_ns::Name;
use failure::{Error, ResultExt};
use futures::Future;
use futures::future::join_all;
use tk_easyloop::{self, handle};
use ns_env_config;

use {VPath};
use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::{Config, Connection};

use edit::EditOptions;

pub fn edit(config: Arc<Config>, clusters: Vec<Vec<Name>>,
    indexes: &InMemoryIndexes, blocks: &ThreadedBlockReader,
    opts: EditOptions)
    -> Result<(), Error>
{

    let data = tk_easyloop::run(|| {
        let ns = ns_env_config::init(&handle()).expect("init dns");
        let conn = Connection::new(clusters[0].clone(),
            ns, indexes.clone(), blocks.clone(), &config);
        conn.fetch_index(&VPath::from(&opts.dir))
        .then(|res| res.context("can't fetch index"))
        .and_then(|idx| idx.into_mut().context("can't parse index"))
        .and_then(move |idx| {
            conn.fetch_file(&idx, &opts.file)
            .then(|res| res.context("can't fetch file"))
        })
    })?;

    println!("READ {:?} BYTES", data.len());

    unimplemented!();
}
