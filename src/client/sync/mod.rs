mod uploads;
mod network;

use std::process::exit;
use std::mem;

use abstract_ns::Name;
use failure::{Error, ResultExt};
use gumdrop::Options;

use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::Config;

use keys::read_keys;
use global_options::GlobalOptions;


#[derive(Options, Debug, Default)]
#[options(no_short)]
struct SyncOptions {
    #[options(help="Print help message and exit")]
    help: bool,

    #[options(free)]
    clusters: Vec<String>,

    #[options(short="m", help="
        Multiple hosts per cluster mode. By default each hostname specified
        on the command-line is a separate cluster (and is expected to be
        resolved to multiple IP addresses). With this option set, hostnames
        are treated as single cluster. You may still upload to a multiple
        clusters by putting `--` at the start and use `--` as a separator.
    ")]
    multiple: bool,

    #[options(meta="SOURCE:DEST",
              help="Append a directory \
                   (skip if already exists and same contents)")]
    append: Vec<String>,

    #[options(meta="SOURCE:DEST",
              help="Append a directory \
                   (skip if already exists even if different contents)")]
    append_weak: Vec<String>,

    #[options(meta="SOURCE:DEST",
              help="Replace a directory \
                    (this should be allowed in server config)")]
    replace: Vec<String>,

    #[options(short="i", meta="FILENAME", help="
        Use the specified identity files (basically ssh-keys) to
        sign the upload. By default all supported keys in
        `$HOME/.ssh` and a key passed in environ variable `CIRUELA_KEY`
        are used. Note: multiple `-i` flags may be used.
    ")]
    identity: Vec<String>,
    #[options(short="k", meta="ENV_VAR", help="
        Use specified env variable to get identity (basically ssh-key).
        The environment variable contains actual key, not the file
        name. Multiple variables can be specified along with `-i`.
        If neither `-i` nor `-k` options present, default ssh keys
        and `CIRUELA_KEY` environment variable are used if present.
        Useful for CI systems.
    ")]
    key_from_env: Vec<String>,
}


pub fn convert_clusters(src: &Vec<String>, multi: bool)
    -> Result<Vec<Vec<Name>>, Error>
{
    if multi {
        let mut result = Vec::new();
        let mut cur = Vec::new();
        for name in src {
            if name == "--" {
                if cur.len() > 0 {
                    result.push(mem::replace(&mut cur, Vec::new()));
                }
            } else {
                let name = name.parse::<Name>()
                    .context(format!("bad name {:?}", name))?;
                cur.push(name);
            }
        }
        return Ok(result);
    } else {
        return Ok(vec![src.iter().map(|name| {
            name.parse::<Name>().context(format!("bad name {:?}", name))
        }).collect::<Result<_, _>>()?]);
    }
}


pub fn cli(gopt: GlobalOptions, args: Vec<String>) -> ! {
    let opts = match SyncOptions::parse_args_default(&args) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("ciruela sync: {}", e);
            exit(1);
        }
    };

    if opts.help {
        println!("Usage: ciruela sync [OPTIONS] [ENTRY_POINT...]");
        println!();
        println!("A tool for bulk-uploading a set of directories to a set ");
        println!("of clusters *each* having a single name on a command-line");
        println!("as an entry point (but see `-m`)");
        println!();
        println!("Executes a set of operations (uploads) to each mentioned");
        println!("cluster. Cluster dns name (ENTRY_POINT) should resolve");
        println!("to a multiple (e.g. at least three) ip addresses of");
        println!("servers for reliability.");
        println!();
        println!("All uploading is done in parallel. Command");
        println!("returns when all uploads are done or rejected.");
        println!();
        println!("{}", SyncOptions::usage());
        exit(0);
    } else {
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
}
