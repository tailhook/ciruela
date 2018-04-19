mod uploads;
mod network;

use std::process::exit;
use std::mem;
use std::time::Duration;

use abstract_ns::Name;
use failure::{Error, ResultExt};
use structopt::StructOpt;

use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::Config;

use keys::read_keys;
use global_options::GlobalOptions;


#[derive(StructOpt, Debug)]
#[structopt(name="ciruela sync", about="
    A tool for bulk-uploading a set of directories to a set
    of clusters *each* having a single name on a command-line
    as an entry point (but see `-m`)

    Executes a set of operations (uploads) to each mentioned
    cluster. Cluster dns name (ENTRY_POINT) should resolve
    to a multiple (e.g. at least three) ip addresses of
    servers for reliability.

    All uploading is done in parallel. Command
    returns when all uploads are done or rejected.
")]
struct SyncOptions {
    #[structopt(name="ENTRY_POINT",
        help="domain names used as entry points to a cluster")]
    clusters: Vec<String>,

    #[structopt(short="m", long="multiple", help="
        Multiple hosts per cluster mode. By default each hostname specified
        on the command-line is a separate cluster (and is expected to be
        resolved to multiple IP addresses). With this option set, hostnames
        are treated as single cluster. You may still upload to a multiple
        clusters by putting `--` at the start and use `--` as a separator.
    ")]
    multiple: bool,

    #[structopt(name="A_SOURCE:DEST", long="append",
                raw(number_of_values="1"),
                help="Append a directory \
                   (skip if already exists and same contents)")]
    append: Vec<String>,

    #[structopt(name="W_SOURCE:DEST", long="append-weak",
                raw(number_of_values="1"),
                help="Append a directory \
                   (skip if already exists even if different contents)")]
    append_weak: Vec<String>,

    #[structopt(name="R_SOURCE:DEST", long="replace",
                raw(number_of_values="1"),
                help="Replace a directory \
                    (this should be allowed in server config)")]
    replace: Vec<String>,

    #[structopt(short="i", long="identity", name="FILENAME",
                raw(number_of_values="1"),
                help="
        Use the specified identity files (basically ssh-keys) to
        sign the upload. By default all supported keys in
        `$HOME/.ssh` and a key passed in environ variable `CIRUELA_KEY`
        are used. Note: multiple `-i` flags may be used.
    ")]
    identity: Vec<String>,

    #[structopt(short="k", long="key-from-env", name="ENV_VAR",
                raw(number_of_values="1"),
                help="
        Use specified env variable to get identity (basically ssh-key).
        The environment variable contains actual key, not the file
        name. Multiple variables can be specified along with `-i`.
        If neither `-i` nor `-k` options present, default ssh keys
        and `CIRUELA_KEY` environment variable are used if present.
        Useful for CI systems.
    ")]
    key_from_env: Vec<String>,

    #[structopt(short="e", long="early-timeout", name="EARLY_TIMEO",
                parse(try_from_str="::humantime::parse_duration"),
                default_value="30s",
                help="
        Report successful exit after this timeout even if not all hosts
        received directories as long as most of them done
        (format: http://bit.ly/durationf).
    ")]
    early_timeout: Duration,

    #[structopt(long="early-fraction", name="EARLY_FACTION",
                default_value="0.75",
                help="
        Report successful exit after early timeout if this fraction if known
        hosts are done.
    ")]
    early_fraction: f32,

    #[structopt(long="early-hosts", name="EARLY_HOSTS",
                default_value="3",
                help="
        Report successful exit after early timeout after at least this number
        of hosts are done, if this number is larger than
        known-hosts*EARLY_FRACTION. If after early timeout number of known
        hosts is less than this number the 100% of hosts are used as a measure.
    ")]
    early_hosts: u32,

    #[structopt(short="t", long="deadline", name="DEADLINE",
                parse(try_from_str="::humantime::parse_duration"),
                default_value="30min",
                help="
        Maximum time ciruela sync is allowed to run. If not all hosts are
        done and early exit conditions are not met utility will exit with
        non-zero status (format: http://bit.ly/durationf).
    ")]
    deadline: Duration,
}


pub fn convert_clusters(src: &Vec<String>, multi: bool)
    -> Result<Vec<Vec<Name>>, Error>
{
    if multi {
        let mut result = Vec::new();
        let mut cur = Vec::new();
        for name in src {
            if name == "--" {
                if !cur.is_empty() {
                    result.push(mem::replace(&mut cur, Vec::new()));
                }
            } else {
                let name = name.parse::<Name>()
                    .context(format!("bad name {:?}", name))?;
                cur.push(name);
            }
        }
        if !cur.is_empty() {
            result.push(cur);
        }
        return Ok(result);
    } else {
        return Ok(src.iter().map(|name| {
            name.parse::<Name>().map(|n| vec![n])
            .context(format!("bad name {:?}", name))
        }).collect::<Result<_, _>>()?);
    }
}


pub fn cli(gopt: GlobalOptions, mut args: Vec<String>) -> ! {
    args.insert(0, String::from("ciruela sync"));  // temporarily
    let opts = SyncOptions::from_iter(args);

    let keys = match read_keys(&opts.identity, &opts.key_from_env) {
        Ok(keys) => keys,
        Err(e) => {
            error!("{}", e);
            warn!("Images haven't started to upload.");
            exit(2);
        }
    };
    let clusters = match convert_clusters(&opts.clusters, opts.multiple) {
        Ok(names) => names,
        Err(e) => {
            error!("{}", e);
            warn!("Images haven't started to upload.");
            exit(2);
        }
    };

    let indexes = InMemoryIndexes::new();
    let block_reader = ThreadedBlockReader::new();

    let uploads = match
        uploads::prepare(&opts, &keys, &gopt, &indexes, &block_reader)
    {
        Ok(uploads) => uploads,
        Err(e) => {
            error!("{}", e);
            warn!("Images haven't started to upload.");
            exit(1);
        }
    };

    let config = Config::new()
        .port(gopt.destination_port)
        .early_upload(opts.early_hosts, opts.early_fraction,
                      opts.early_timeout)
        .maximum_timeout(opts.deadline)
        .done();
    match
        network::upload(config, clusters, uploads, &indexes, &block_reader)
    {
        Ok(()) => {}
        Err(e) => {
            error!("{}", e);
            exit(3);
        }
    }
    exit(0);
}
