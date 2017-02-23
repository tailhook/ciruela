mod client;
mod dir_commands;
mod message;
mod request;
mod serializers;
mod signature;
mod server;

pub use self::client::Client;
pub use self::dir_commands::{AppendDir, AppendDirAck, ReplaceDir};
pub use self::message::{Message, Request, Response, Notification};
pub use self::request::Request as RequestTrait;
pub use self::signature::{Signature, SigData, sign};
pub use self::server::serialize_response;

// Protocol identifiers
const NOTIFICATION: u8 = 0;
const REQUEST: u8 = 1;
const RESPONSE: u8 = 2;
