use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use futures::Future;

use ciruela::proto::{AppendDir, AppendDirAck};
use ciruela::proto::{ReplaceDir, ReplaceDirAck};
use ciruela::proto::{GetIndex, GetIndexResponse};
use ciruela::proto::{GetBlock, GetBlockResponse};
use ciruela::proto::{GetBaseDir, GetBaseDirResponse};
use tracking::{Tracking, Command, Downloading};
use websocket::Responder;
use base_dir;


impl Tracking {
    /// RPC Requests
    pub fn append_dir(&self, cmd: AppendDir, resp: Responder<AppendDirAck>)
    {
        let tracking = self.clone();
        let cfg = match self.0.config.dirs.get(cmd.path.key()) {
            Some(cfg) => cfg.clone(),
            None => {
                resp.respond_now(AppendDirAck {
                    accepted: false,
                });
                return;
            }
        };
        let path = cmd.path.clone();
        let image_id = cmd.image.clone();
        resp.respond_with_future(self.0.meta.append_dir(cmd)
            .map(move |result| {
            if result {
                tracking.send(Command::FetchDir(Downloading {
                    replacing: false,
                    virtual_path: path,
                    image_id: image_id,
                    config: cfg,
                    index_fetched: AtomicBool::new(false),
                    bytes_fetched: AtomicUsize::new(0),
                    bytes_total: AtomicUsize::new(0),
                    blocks_fetched: AtomicUsize::new(0),
                    blocks_total: AtomicUsize::new(0),
                }));
            }
            AppendDirAck {
                accepted: result,
            }
        }));
    }
    pub fn replace_dir(&self, cmd: ReplaceDir, resp: Responder<ReplaceDirAck>)
    {
        let tracking = self.clone();
        let cfg = match self.0.config.dirs.get(cmd.path.key()) {
            Some(cfg) => cfg.clone(),
            None => {
                resp.respond_now(ReplaceDirAck {
                    accepted: false,
                });
                return;
            }
        };
        let path = cmd.path.clone();
        let image_id = cmd.image.clone();
        resp.respond_with_future(self.0.meta.replace_dir(cmd)
            .map(move |result| {
            if result {
                tracking.send(Command::FetchDir(Downloading {
                    replacing: true,
                    virtual_path: path,
                    image_id: image_id,
                    config: cfg,
                    index_fetched: AtomicBool::new(false),
                    bytes_fetched: AtomicUsize::new(0),
                    bytes_total: AtomicUsize::new(0),
                    blocks_fetched: AtomicUsize::new(0),
                    blocks_total: AtomicUsize::new(0),
                }));
            }
            ReplaceDirAck {
                accepted: result,
            }
        }));
    }
    pub fn get_block(&self, cmd: GetBlock, resp: Responder<GetBlockResponse>)

    {
        unimplemented!();
    }
    pub fn get_index(&self, cmd: GetIndex, resp: Responder<GetIndexResponse>)
    {
        unimplemented!();
    }
    pub fn get_base_dir(&self, cmd: GetBaseDir,
        resp: Responder<GetBaseDirResponse>)
    {
        let cfg = match self.0.config.dirs.get(cmd.path.key()) {
            Some(cfg) => cfg.clone(),
            None => {
                resp.error_now("No such config");
                return;
            }
        };
        resp.respond_with_future(
            base_dir::scan(&cmd.path, &cfg, &self.0.meta, &self.0.disk)
            .map(|value| {
                GetBaseDirResponse {
                    config_hash:
                        value.config_hash,
                    keep_list_hash:
                        value.keep_list_hash,
                    dirs: value.dirs,
                }
            }));
    }
}
