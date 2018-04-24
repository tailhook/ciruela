use std::sync::Arc;
use std::time::Duration;

use abstract_ns::Name;
use failure::Error;
use futures::future::{join_all, Either};
use futures::{Future, Stream};
use tk_easyloop::{self, handle, interval};
use ns_env_config;

use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::{self, Config, Connection, UploadOk, UploadFail};

use sync::uploads::Upload;

pub fn upload_with_progress(up: cluster::Upload, progress_ivl: Duration)
    -> Box<Future<Item=UploadOk, Error=UploadFail>>
{
    let up2 = up.clone();
    Box::new(interval(progress_ivl)
        .for_each(move |()| {
            println!("{}", up2.stats().one_line_progress());
            Ok(())
        })
        .select2(up.future())
        .then(|x| match x {
            // interval doesn't exit or fails
            Ok(Either::A(_)) => unreachable!(),
            Err(Either::A(_)) => unreachable!(),
            Ok(Either::B((r, _))) => Ok(r),
            Err(Either::B((e, _))) => Err(e),
        }))
}

pub fn upload(config: Arc<Config>, clusters: Vec<Vec<Name>>,
    uploads: Vec<Upload>,
    indexes: &InMemoryIndexes, blocks: &ThreadedBlockReader,
    progress_interval: Duration)
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
                upload_with_progress(up, progress_interval)
            }))
        }))
    })?;
    for res in res.iter().flat_map(|x| x) {
        println!("{}", res);
    }
    Ok(())
}
