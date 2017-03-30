mod client;
pub mod message;
mod request;
mod serializers;
mod server;
mod signature;
mod stream_ext;

mod dir_commands;
mod index_commands;

pub use self::client::{Client, ImageInfo};
pub use self::request::{Request, Response, Notification};
pub use self::request::{RequestFuture, PacketStream};
pub use self::request::{RequestClient, RequestDispatcher, Registry, Sender};
pub use self::request::{WrapTrait}; // TODO(tailhook) hide it
pub use self::server::serialize_response;
pub use self::signature::{Signature, SigData, sign};
pub use self::stream_ext::StreamExt;

pub use self::dir_commands::{AppendDir, AppendDirAck, ReplaceDir};
pub use self::index_commands::{PublishIndex};
pub use self::index_commands::{GetIndex, GetIndexResponse};

// Protocol identifiers
const NOTIFICATION: u8 = 0;
const REQUEST: u8 = 1;
const RESPONSE: u8 = 2;
