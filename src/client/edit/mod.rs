mod network;
mod editor;

use std::process::exit;
use std::time::Duration;
use std::path::PathBuf;

use structopt::StructOpt;

use ciruela::blocks::ThreadedBlockReader;
use ciruela::index::InMemoryIndexes;
use ciruela::cluster::Config;

use keys::read_keys;
use global_options::GlobalOptions;
use sync::convert_clusters;

#[derive(StructOpt, Debug)]
#[structopt(name="ciruela edit", about="
    Downloads a file from a remote location, edits it and syncs
    an equivalent image with an updated file back to the cluster.
")]
pub struct EditOptions {
    #[structopt(name="ENTRY_POINT", help="\
        Domain names used as entry points to a cluster. Only the first one \
        is used to download the file. Others are used to upload file back to \
        the cluster. Upload works the same as in `ciruela sync`. \
    ")]
    clusters: Vec<String>,

    #[structopt(short="d", long="dir", help="\
        A virtual path to the directory to download file from. \
    ", parse(from_os_str))]
    dir: PathBuf,

    #[structopt(short="f", long="file", help="\
        Path to file to edit, must start with backslash `/`. \
    ", parse(from_os_str))]
    file: PathBuf,

    #[structopt(long="editor", help="\
        Override editor to run. By default we're using EDITOR environment \
        variable. \
    ", parse(from_os_str))]
    editor: Option<PathBuf>,

    #[structopt(short="m", long="multiple", help="\
        Multiple hosts per cluster mode. \
        See `ciruela sync --help` for more info on this mode. \
    ")]
    multiple: bool,

    #[structopt(short="i", long="identity", name="FILENAME",
                raw(number_of_values="1"),
                help="\
        Use the specified identity files (basically ssh-keys) to \
        sign the upload. By default all supported keys in \
        `$HOME/.ssh` and a key passed in environ variable `CIRUELA_KEY` \
        are used. Note: multiple `-i` flags may be used. \
    ")]
    identity: Vec<String>,

    #[structopt(short="k", long="key-from-env", name="ENV_VAR",
                raw(number_of_values="1"),
                help="\
        Use specified env variable to get identity (basically ssh-key). \
        The environment variable contains actual key, not the file \
        name. Multiple variables can be specified along with `-i`. \
        If neither `-i` nor `-k` options present, default ssh keys \
        and `CIRUELA_KEY` environment variable are used if present. \
        Useful for CI systems. \
    ")]
    key_from_env: Vec<String>,

    #[structopt(short="e", long="early-timeout", name="EARLY_TIMEO",
                parse(try_from_str="::humantime::parse_duration"),
                default_value="30s",
                help="\
        Report successful exit after this timeout even if not all hosts \
        received directories as long as most of them done \
    ")]
    early_timeout: Duration,

    #[structopt(long="early-fraction", name="EARLY_FACTION",
                default_value="0.75",
                help="\
        Report successful exit after early timeout if this fraction if known \
        hosts are done.\
    ")]
    early_fraction: f32,

    #[structopt(long="early-hosts", name="EARLY_HOSTS",
                default_value="3",
                help="\
        Report successful exit after early timeout after at least this number \
        of hosts are done, if this number is larger than \
        known-hosts*EARLY_FRACTION. If after early timeout number of known \
        hosts is less than this number the 100% of hosts are used as a \
        measure. \
    ")]
    early_hosts: u32,

    #[structopt(short="t", long="deadline", name="DEADLINE",
                parse(try_from_str="::humantime::parse_duration"),
                default_value="30min",
                help="\
        Maximum time ciruela sync is allowed to run. If not all hosts are \
        done and early exit conditions are not met utility will exit with \
        non-zero status. \
    ")]
    deadline: Duration,
}

pub fn cli(gopt: GlobalOptions, mut args: Vec<String>) -> ! {
    args.insert(0, String::from("ciruela edit"));  // temporarily
    let opts = EditOptions::from_iter(args);

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

    let config = Config::new()
        .port(gopt.destination_port)
        .early_upload(opts.early_hosts, opts.early_fraction,
                      opts.early_timeout)
        .maximum_timeout(opts.deadline)
        .done();

    match
        network::edit(config, clusters, keys, &indexes, &block_reader, opts)
    {
        Ok(()) => {}
        Err(e) => {
            error!("{}", e);
            exit(3);
        }
    }
    exit(0);
}
