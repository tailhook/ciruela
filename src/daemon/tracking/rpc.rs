use futures::Future;

use ciruela::proto::{AppendDir, AppendDirAck};
use ciruela::proto::{ReplaceDir, ReplaceDirAck};
use ciruela::proto::{GetIndex, GetIndexResponse};
use ciruela::proto::{GetBlock, GetBlockResponse};
use ciruela::proto::{GetBaseDir, GetBaseDirResponse};
use tracking::{Tracking, base_dir};
use remote::websocket::Responder;
use {metadata, disk};


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Meta(err: metadata::Error) {
            display("{}", err)
            description(err.description())
            cause(err)
        }
        Disk(err: disk::Error) {
            display("{}", err)
            description(err.description())
            cause(err)
        }
    }
}



impl Tracking {
    /// RPC Requests
    pub fn append_dir(&self, cmd: AppendDir, resp: Responder<AppendDirAck>)
    {
        if !self.0.config.dirs.contains_key(cmd.path.key()) {
            resp.respond_now(AppendDirAck {
                accepted: false,
            });
            return;
        };
        let tracking = self.clone();
        let path = cmd.path.clone();
        let image_id = cmd.image.clone();
        resp.respond_with_future(self.0.meta.append_dir(cmd)
            .map(move |result| {
            if result.new {
                tracking.fetch_dir(path, image_id, false);
            }
            AppendDirAck {
                accepted: result.accepted,
            }
        }));
    }
    pub fn replace_dir(&self, cmd: ReplaceDir, resp: Responder<ReplaceDirAck>)
    {
        if !self.0.config.dirs.contains_key(cmd.path.key()) {
            resp.respond_now(ReplaceDirAck {
                accepted: false,
            });
            return;
        };
        let tracking = self.clone();
        let path = cmd.path.clone();
        let image_id = cmd.image.clone();
        resp.respond_with_future(self.0.meta.replace_dir(cmd)
            .map(move |result| {
            if result.new {
                tracking.fetch_dir(path, image_id, true);
            }
            ReplaceDirAck {
                accepted: result.accepted,
            }
        }));
    }
    pub fn get_block(&self, cmd: GetBlock, resp: Responder<GetBlockResponse>)

    {
        match cmd.hint {
            Some((_, ref path, _)) if path.file_name().is_none() => {
                info!("invalid path {:?} in GetBlock", path);
                resp.error_now("Path must have at least one component");
            }
            Some((_, ref path, _)) if !path.is_absolute() => {
                resp.error_now("Path must be absolute");
            }
            Some((vpath, path, offset)) => {
                let disk = self.0.disk.clone();
                resp.respond_with_future(self.0.meta.is_writing(&vpath)
                    .map_err(Error::Meta)
                    .and_then(move |writing| {
                        disk.read_block(&vpath, &path, offset, writing)
                        .map_err(Error::Disk)
                    })
                    .map(|bytes| {
                        GetBlockResponse { data: bytes }
                    }));
            }
            None => {
                resp.error_now(
                    "Hint is required for fetching blocks from server");
            }
        }
    }
    pub fn get_index(&self, cmd: GetIndex, resp: Responder<GetIndexResponse>)
    {
        resp.respond_with_future(self.0.meta.read_index_bytes(&cmd.id)
            .map(|val| GetIndexResponse { data: val }));
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
