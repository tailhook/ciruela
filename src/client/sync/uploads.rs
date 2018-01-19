use std::path::{Path, PathBuf};
use std::time::SystemTime;

use dir_signature::{v1, ScannerConfig, HashType};
use failure::{Error, err_msg, ResultExt};
use ssh_keys::PrivateKey;

use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::signature::{SignedUpload, sign_upload};
use ciruela::VPath;

use global_options::GlobalOptions;
use sync::SyncOptions;


pub enum Upload {
    Append(SignedUpload),
    WeakAppend(SignedUpload),
    Replace(SignedUpload),
}

fn split(cli: &str) -> Result<(PathBuf, VPath), Error> {
    let mut pair = cli.split(':');
    let src = PathBuf::from(pair.next().unwrap());
    // TODO(tailhook) this may crash
    let dest = VPath::from(pair.next()
        .ok_or_else(|| err_msg(format!("No destination directory specified, \
            must be `source:/dir/dest`, got {:?} instead", cli)))?);
    return Ok((src, dest));
}

fn scan(dir: &Path, threads: usize) -> Result<Vec<u8>, Error> {
    let mut cfg = ScannerConfig::new();
    cfg.threads(threads);
    cfg.hash(HashType::blake2b_256());
    cfg.add_dir(dir, "/");
    cfg.print_progress();
    let mut index_buf = Vec::new();
    v1::scan(&cfg, &mut index_buf)
        .context(format!("error indexing dir {:?}", dir))?;
    return Ok(index_buf);
}

pub(in sync) fn prepare(opts: &SyncOptions, keys: &Vec<PrivateKey>,
    gopt: &GlobalOptions,
    indexes: &InMemoryIndexes, blocks: &ThreadedBlockReader)
    -> Result<Vec<Upload>, Error>
{
    let mut result = Vec::new();

    let timestamp = SystemTime::now();

    for dir in &opts.append {
        let (src, dest) = split(dir)?;
        let index_buf = scan(&src, gopt.threads)?;
        let image_id = indexes.register_index(&index_buf)?;
        blocks.register_dir(&src, &index_buf)?;

        let upload = sign_upload(&dest, &image_id, timestamp, &keys);
        result.push(Upload::Append(upload));
    }

    for dir in &opts.append_weak {
        let (src, dest) = split(dir)?;
        let index_buf = scan(&src, gopt.threads)?;
        let image_id = indexes.register_index(&index_buf)?;
        blocks.register_dir(&src, &index_buf)?;

        let upload = sign_upload(&dest, &image_id, timestamp, &keys);
        result.push(Upload::WeakAppend(upload));
    }

    for dir in &opts.replace {
        let (src, dest) = split(dir)?;
        let index_buf = scan(&src, gopt.threads)?;
        let image_id = indexes.register_index(&index_buf)?;
        blocks.register_dir(&src, &index_buf)?;

        let upload = sign_upload(&dest, &image_id, timestamp, &keys);
        result.push(Upload::Replace(upload));
    }

    return Ok(result);
}
