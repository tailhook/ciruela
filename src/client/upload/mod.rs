use std::collections::HashMap;
use std::io::{stdout, stderr, Write};
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use std::time::SystemTime;

use abstract_ns::Resolver;
use futures::future::{Future, join_all};
use tk_easyloop;
use argparse::{ArgumentParser};
use dir_signature::{ScannerConfig, HashType, v1};

mod options;

use name;
use ciruela::time::to_ms;
use ciruela::proto::{SigData, sign_default};
use global_options::GlobalOptions;
use ciruela::proto::{Client, AppendDir};


fn do_upload(gopt: GlobalOptions, opt: options::UploadOptions)
    -> Result<bool, ()>
{
    let dir = opt.source_directory.clone().unwrap();
    let mut cfg = ScannerConfig::new();
    cfg.hash(HashType::Blake2b_256);
    cfg.add_dir(&dir, "/");
    cfg.print_progress();

    let mut indexbuf = Vec::new();
    v1::scan(&cfg, &mut indexbuf)
        .map_err(|e| error!("Error scanning {:?}: {}", dir, e))?;

    let image_id = b"unimplemented!()";
    let timestamp = SystemTime::now();
    let mut signatures = HashMap::new();
    for turl in &opt.target_urls {
        if signatures.contains_key(&turl.path[..]) {
            continue;
        }
        signatures.insert(&turl.path[..], sign_default(SigData {
            path: &turl.path,
            image: &image_id[..],
            timestamp: to_ms(timestamp),
        }));
    }
    let signatures = Arc::new(signatures);

    tk_easyloop::run(|| {
        let resolver = name::resolver();
        join_all(
            opt.target_urls.iter()
            .map(move |turl| {
                let host = Arc::new(
                    format!("{}:{}", turl.host, gopt.destination_port));
                let host2 = host.clone();
                let host3 = host.clone();
                let signatures = signatures.clone();
                resolver.resolve(&host)
                .map_err(move |e| {
                    error!("Error resolving host {}: {}", host2, e);
                })
                .and_then(move |addr| name::pick_hosts(&*host3, addr))
                .and_then(move |names| {
                    join_all(
                        names.iter()
                        .map(move |&addr| {
                            let turl = turl.clone();
                            let signatures = signatures.clone();
                            Client::spawn(addr, &host)
                            .and_then(move |cli| {
                                info!("Connected to {}", addr);
                                cli.request(AppendDir {
                                    image: image_id.to_vec(),
                                    timestamp: timestamp,
                                    signatures: signatures
                                        .get(&turl.path[..]).unwrap()
                                        .clone(),
                                    path: PathBuf::from(turl.path),
                                })
                                .map_err(|e| error!("Request error: {}", e))
                            })
                            .and_then(move |response| {
                                info!("Response from {}: {:?}",
                                    addr, response);
                                // TODO(tailhook) read notifications
                                Ok(true)
                            })
                            .then(move |res| match res {
                                Ok(x) => Ok(x),
                                Err(()) => {
                                    // Always succeed, for now, so join will
                                    // drive all the futures, even if one
                                    // failed
                                    error!("Address {} has failed.", addr);
                                    Ok(false)
                                }
                            })
                        })
                        .collect::<Vec<_>>() // to unrefer "names"
                    )
                    .map(|results: Vec<bool>| results.iter().any(|x| *x))
                })
            })
            .collect::<Vec<_>>()
        )
        .map(|results:Vec<bool>| results.iter().all(|x| *x))
    })
}


pub fn cli(mut gopt: GlobalOptions, mut args: Vec<String>) -> ! {
    let mut opt = options::UploadOptions::new();
    {
        let mut ap = ArgumentParser::new();
        opt.define(&mut ap);
        gopt.define(&mut ap);
        args.insert(0, String::from("ciruela upload"));
        match ap.parse(args, &mut stdout(), &mut stderr()) {
            Ok(()) => {}
            Err(code) => exit(code),
        }
    }
    if opt.source_directory.is_none() {
        writeln!(&mut stderr(),
            "Argument `-d` or `--directory` is required").ok();
        exit(1);
    };
    match do_upload(gopt, opt) {
        Ok(true) => exit(0),
        Ok(false) => exit(1),
        Err(()) => exit(2),
    }
}
