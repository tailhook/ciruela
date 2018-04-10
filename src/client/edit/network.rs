use std::sync::Arc;

use abstract_ns::Name;
use failure::{Error, ResultExt};
use futures::Future;
use tk_easyloop::{self, handle};
use ns_env_config;

use {VPath};
use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::{Config, Connection};

use edit::EditOptions;
use edit::editor;

pub fn edit(config: Arc<Config>, clusters: Vec<Vec<Name>>,
    indexes: &InMemoryIndexes, blocks: &ThreadedBlockReader,
    opts: EditOptions)
    -> Result<(), Error>
{
    let file = opts.file.file_name()
        .and_then(|x| x.to_str()).map(|x| x.to_string())
        .ok_or(format_err!("path {:?} should have filename", opts.file))?;
    tk_easyloop::run(|| {
        let ns = ns_env_config::init(&handle()).expect("init dns");
        let conn = Connection::new(clusters[0].clone(),
            ns, indexes.clone(), blocks.clone(), &config);
        conn.fetch_index(&VPath::from(&opts.dir))
        .then(|res| res.context("can't fetch index").map_err(Error::from))
        .and_then(|idx| idx.into_mut()
            .context("can't parse index").map_err(Error::from))
        .and_then(move |idx| {
            conn.fetch_file(&idx, &opts.file)
            .then(|res| res.context("can't fetch file").map_err(Error::from))
        })
        .and_then(move |data| {
            editor::run(&file, data)
        })
        .map(|ndata| {
            if let Some(ndata) = ndata {
                println!("New data {} bytes", ndata.len());
            } else {
                warn!("File is unchanged")
            }
        })
    })
}
