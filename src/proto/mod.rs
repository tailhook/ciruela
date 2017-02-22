mod client;
mod dir_commands;
mod message;
mod request;
mod serializers;
mod signature;

pub use self::client::Client;
pub use self::dir_commands::{AppendDir, ReplaceDir};
pub use self::message::{Message, Request, Response, Notification};
pub use self::request::Request as RequestTrait;
pub use self::signature::{Signature, SigData, sign};

// Protocol identifiers
const NOTIFICATION: u8 = 0;
const REQUEST: u8 = 1;
const RESPONSE: u8 = 2;
