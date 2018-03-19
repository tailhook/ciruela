use std::fmt;

use serde::{Deserialize, Deserializer};
use serde::de::{Visitor, SeqAccess, Error};

use proto::{dir_commands, index_commands, block_commands, p2p_commands};
use proto::{NOTIFICATION, REQUEST, RESPONSE};




pub enum Message {
    Request(u64, Request),
    Response(u64, Response),
    Notification(Notification),
}

struct MessageVisitor;
struct RequestTypeVisitor;
struct ResponseTypeVisitor;
struct NotificationTypeVisitor;

pub enum RequestType {
    AppendDir,
    ReplaceDir,
    GetIndex,
    GetIndexAt,
    GetBlock,
    GetBaseDir,
}

pub enum ResponseType {
    AppendDir,
    ReplaceDir,
    GetIndex,
    GetIndexAt,
    GetBlock,
    GetBaseDir,
    RequestError,
}

pub enum NotificationType {
    PublishImage,
    ReceivedImage,
    AbortedImage,
}

const REQUEST_TYPES: &'static [&'static str] = &[
    "AppendDir",
    "ReplaceDir",
    "GetIndex",
    "GetIndexAt",
    "GetBlock",
    "GetBaseDir",
    ];

const RESPONSE_TYPES: &'static [&'static str] = &[
    "AppendDir",
    "ReplaceDir",
    "GetIndex",
    "GetIndexAt",
    "GetBlock",
    "GetBaseDir",
    ];

const NOTIFICATION_TYPES: &'static [&'static str] = &[
    "PublishImage",
    "ReceivedImage",
    "AbortedImage",
    ];

pub enum Request {
    AppendDir(dir_commands::AppendDir),
    ReplaceDir(dir_commands::ReplaceDir),
    GetIndex(index_commands::GetIndex),
    GetIndexAt(index_commands::GetIndexAt),
    GetBlock(block_commands::GetBlock),
    GetBaseDir(p2p_commands::GetBaseDir),
}

pub enum Response {
    AppendDir(dir_commands::AppendDirAck),
    ReplaceDir(dir_commands::ReplaceDirAck),
    GetIndex(index_commands::GetIndexResponse),
    GetIndexAt(index_commands::GetIndexAtResponse),
    GetBlock(block_commands::GetBlockResponse),
    GetBaseDir(p2p_commands::GetBaseDirResponse),
    Error(String),
}

#[derive(Debug)]
pub enum Notification {
    PublishImage(index_commands::PublishImage),
    ReceivedImage(index_commands::ReceivedImage),
    AbortedImage(index_commands::AbortedImage),
}

impl<'a> Deserialize<'a> for Message {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'a>
    {
        deserializer.deserialize_seq(MessageVisitor)
    }
}

impl<'a> Visitor<'a> for RequestTypeVisitor {
    type Value = RequestType;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&REQUEST_TYPES.join(", "))
    }

    fn visit_str<E>(self, value: &str) -> Result<RequestType, E>
        where E: Error
    {
        match value {
            "AppendDir" => Ok(RequestType::AppendDir),
            "ReplaceDir" => Ok(RequestType::ReplaceDir),
            "GetIndex" => Ok(RequestType::GetIndex),
            "GetIndexAt" => Ok(RequestType::GetIndexAt),
            "GetBlock" => Ok(RequestType::GetBlock),
            "GetBaseDir" => Ok(RequestType::GetBaseDir),
            _ => Err(Error::unknown_variant(value, REQUEST_TYPES)),
        }
    }
}

impl<'a> Visitor<'a> for ResponseTypeVisitor {
    type Value = ResponseType;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&RESPONSE_TYPES.join(", "))
    }

    fn visit_str<E>(self, value: &str) -> Result<ResponseType, E>
        where E: Error
    {
        match value {
            "AppendDir" => Ok(ResponseType::AppendDir),
            "ReplaceDir" => Ok(ResponseType::ReplaceDir),
            "GetIndex" => Ok(ResponseType::GetIndex),
            "GetIndexAt" => Ok(ResponseType::GetIndexAt),
            "GetBlock" => Ok(ResponseType::GetBlock),
            "GetBaseDir" => Ok(ResponseType::GetBaseDir),
            "Error" => Ok(ResponseType::RequestError),
            _ => Err(Error::unknown_variant(value, RESPONSE_TYPES)),
        }
    }
}

impl<'a> Visitor<'a> for NotificationTypeVisitor {
    type Value = NotificationType;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&NOTIFICATION_TYPES.join(", "))
    }

    fn visit_str<E>(self, value: &str) -> Result<NotificationType, E>
        where E: Error
    {
        match value {
            "PublishImage" => Ok(NotificationType::PublishImage),
            "ReceivedImage" => Ok(NotificationType::ReceivedImage),
            "AbortedImage" => Ok(NotificationType::AbortedImage),
            _ => Err(Error::unknown_field(value, NOTIFICATION_TYPES)),
        }
    }
}

impl<'a> Deserialize<'a> for RequestType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'a>
    {
        deserializer.deserialize_str(RequestTypeVisitor)
    }
}

impl<'a> Deserialize<'a> for ResponseType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'a>
    {
        deserializer.deserialize_str(ResponseTypeVisitor)
    }
}

impl<'a> Deserialize<'a> for NotificationType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'a>
    {
        deserializer.deserialize_str(NotificationTypeVisitor)
    }
}

impl<'a> Visitor<'a> for MessageVisitor {
    type Value = Message;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a request, response or a notification")
    }

    #[inline]
    fn visit_seq<V>(self, mut visitor: V) -> Result<Message, V::Error>
        where V: SeqAccess<'a>,
    {
        match visitor.next_element()? {
            Some(NOTIFICATION) => {
                let typ = match visitor.next_element()? {
                    Some(typ) => typ,
                    None => return Err(Error::invalid_length(1, &self)),
                };
                let data = match typ {
                    NotificationType::PublishImage => match visitor.next_element()? {
                        Some(data) => Notification::PublishImage(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    NotificationType::ReceivedImage => match visitor.next_element()? {
                        Some(data) => Notification::ReceivedImage(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    NotificationType::AbortedImage => match visitor.next_element()? {
                        Some(data) => Notification::AbortedImage(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                };
                Ok(Message::Notification(data))
            }
            Some(REQUEST) => {
                use self::RequestType::*;
                let typ = match visitor.next_element()? {
                    Some(typ) => typ,
                    None => return Err(Error::invalid_length(1, &self)),
                };
                let request_id = match visitor.next_element()? {
                    Some(x) => x,
                    None => return Err(Error::invalid_length(2, &self)),
                };
                let data = match typ {
                    AppendDir => match visitor.next_element()? {
                        Some(data) => Request::AppendDir(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    ReplaceDir => match visitor.next_element()? {
                        Some(data) => Request::ReplaceDir(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    GetIndex => match visitor.next_element()? {
                        Some(data) => Request::GetIndex(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    GetIndexAt => match visitor.next_element()? {
                        Some(data) => Request::GetIndexAt(data),
                        None => return Err(Error::invalid_length(2, &self)),
                    },
                    GetBlock => match visitor.next_element()? {
                        Some(data) => Request::GetBlock(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    GetBaseDir => match visitor.next_element()? {
                        Some(data) => Request::GetBaseDir(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                };
                Ok(Message::Request(request_id, data))
            },
            Some(RESPONSE) => {
                use self::ResponseType::*;
                let typ = match visitor.next_element()? {
                    Some(typ) => typ,
                    None => return Err(Error::invalid_length(1, &self)),
                };
                let request_id = match visitor.next_element()? {
                    Some(x) => x,
                    None => return Err(Error::invalid_length(2, &self)),
                };
                let data = match typ {
                    AppendDir => match visitor.next_element()? {
                        Some(data) => Response::AppendDir(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    ReplaceDir => match visitor.next_element()? {
                        Some(data) => Response::ReplaceDir(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    GetIndex => match visitor.next_element()? {
                        Some(data) => Response::GetIndex(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    GetIndexAt => match visitor.next_element()? {
                        Some(data) => Response::GetIndexAt(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    GetBlock => match visitor.next_element()? {
                        Some(data) => Response::GetBlock(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    GetBaseDir => match visitor.next_element()? {
                        Some(data) => Response::GetBaseDir(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    RequestError => match visitor.next_element()? {
                        Some(data) => Response::Error(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                };
                Ok(Message::Response(request_id, data))
            }
            Some(_) => Err(Error::custom("invalid message kind")),
            None => Err(Error::invalid_length(0, &self)),
        }
    }
}
