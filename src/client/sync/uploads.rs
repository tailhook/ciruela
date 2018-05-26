use std::path::{Path, PathBuf};
use std::time::SystemTime;

use dir_signature::{v1, ScannerConfig, HashType};
use failure::{Error, err_msg, ResultExt};
use ssh_keys::PrivateKey;

use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::{InMemoryIndexes, ImageId};
use ciruela::signature::{SignedUpload, sign_upload};
use ciruela::VPath;

use global_options::GlobalOptions;
use sync::SyncOptions;


#[derive(Debug, Clone)]
pub enum Upload {
    Append(SignedUpload),
    WeakAppend(SignedUpload),
    Replace(SignedUpload),
    ReplaceIfMatches(SignedUpload, ImageId),
}

fn split(cli: &str) -> Result<(PathBuf, VPath), Error> {
    let mut pair = cli.split(':');
    let src = PathBuf::from(pair.next().unwrap());
    let dest = pair.next().and_then(|x| VPath::try_from(x).ok())
        .ok_or_else(|| err_msg(format!("Destination directory is invalid, \
            must be `source:/dir/dest`, got {:?} instead", cli)))?;
    return Ok((src, dest));
}

fn split_replace(cli: &str)
    -> Result<(PathBuf, VPath, Option<ImageId>), Error>
{
    let mut triple = cli.split(':');
    let src = PathBuf::from(triple.next().unwrap());
    // TODO(tailhook) this may crash
    let dest = triple.next().and_then(|x| VPath::try_from(x).ok())
        .ok_or_else(|| err_msg(format!("Destination directory is invalid, \
            must be `source:/dir/dest`, got {:?} instead", cli)))?;
    let image_id = triple.next().map(|x| x.parse())
        .map_or(Ok(None), |v| v.map(Some))
        .map_err(|e| format_err!("error parsing image_id: {}", e))?;
    return Ok((src, dest, image_id));
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
        let (src, dest, old_image) = split_replace(dir)?;
        let index_buf = scan(&src, gopt.threads)?;
        let image_id = indexes.register_index(&index_buf)?;
        blocks.register_dir(&src, &index_buf)?;

        let upload = sign_upload(&dest, &image_id, timestamp, &keys);
        if let Some(old) = old_image {
            result.push(Upload::ReplaceIfMatches(upload, old));
        } else {
            result.push(Upload::Replace(upload));
        }
    }

    return Ok(result);
}
