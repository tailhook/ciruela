mod client;
pub mod message;
mod request;
mod server;
mod signature;
mod stream_ext;
mod hash;

mod dir_commands;
mod index_commands;
mod block_commands;
mod p2p_commands;

pub use self::client::{Client, ClientFuture, Listener};
pub use self::hash::{Hash, Builder as HashBuilder};
pub use self::request::{RequestClient, RequestDispatcher, Registry, Sender};
pub use self::request::{RequestFuture, PacketStream};
pub use self::request::{Request, Response, Notification};
pub use self::request::{WrapTrait, Error}; // TODO(tailhook) hide it
pub use self::server::serialize_response;
pub use self::signature::{Signature, SigData, sign, verify};
pub use self::stream_ext::StreamExt;

pub use self::dir_commands::{AppendDir, AppendDirAck};
pub use self::dir_commands::{ReplaceDir, ReplaceDirAck};
pub use self::index_commands::{PublishImage, ReceivedImage, AbortedImage};
pub use self::index_commands::{GetIndex, GetIndexResponse};
pub use self::index_commands::{GetIndexAt, GetIndexAtResponse};
pub use self::block_commands::{GetBlock, GetBlockResponse};
pub use self::p2p_commands::{GetBaseDir, GetBaseDirResponse, BaseDirState};

// Protocol identifiers
const NOTIFICATION: u8 = 0;
const REQUEST: u8 = 1;
const RESPONSE: u8 = 2;
