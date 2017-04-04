use std::fmt;

use serde::{Deserialize, Deserializer};
use serde::de::{Visitor, SeqVisitor, Error};

use proto::{dir_commands, index_commands, block_commands};
use proto::{NOTIFICATION, REQUEST, RESPONSE};




pub enum Message {
    Request(u64, Request),
    Response(u64, Response),
    Notification(Notification),
}

struct MessageVisitor;
struct TypeVisitor;
struct NotificationTypeVisitor;

pub enum Type {
    AppendDir,
    GetIndex,
    GetBlock,
}

pub enum NotificationType {
    PublishImage,
}

const TYPES: &'static [&'static str] = &[
    "AppendDir",
    "GetIndex",
    "GetBlock",
    ];

const NOTIFICATION_TYPES: &'static [&'static str] = &[
    "PublishImage",
    ];

pub enum Request {
    AppendDir(dir_commands::AppendDir),
    GetIndex(index_commands::GetIndex),
    GetBlock(block_commands::GetBlock),
}

pub enum Response {
    AppendDir(dir_commands::AppendDirAck),
    GetIndex(index_commands::GetIndexResponse),
    GetBlock(block_commands::GetBlockResponse),
}

pub enum Notification {
    PublishImage(index_commands::PublishImage),
}

impl Deserialize for Message {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_seq(MessageVisitor)
    }
}

impl Visitor for TypeVisitor {
    type Value = Type;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&TYPES.join(", "))
    }

    fn visit_str<E>(self, value: &str) -> Result<Type, E>
        where E: Error
    {
        match value {
            "AppendDir" => Ok(Type::AppendDir),
            "GetIndex" => Ok(Type::GetIndex),
            "GetBlock" => Ok(Type::GetBlock),
            _ => Err(Error::unknown_field(value, TYPES)),
        }
    }
}

impl Visitor for NotificationTypeVisitor {
    type Value = NotificationType;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&NOTIFICATION_TYPES.join(", "))
    }

    fn visit_str<E>(self, value: &str) -> Result<NotificationType, E>
        where E: Error
    {
        match value {
            "PublishImage" => Ok(NotificationType::PublishImage),
            _ => Err(Error::unknown_field(value, NOTIFICATION_TYPES)),
        }
    }
}

impl Deserialize for Type {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_str(TypeVisitor)
    }
}

impl Deserialize for NotificationType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_str(NotificationTypeVisitor)
    }
}

impl Visitor for MessageVisitor {
    type Value = Message;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a request, response or a notification")
    }

    #[inline]
    fn visit_seq<V>(self, mut visitor: V) -> Result<Message, V::Error>
        where V: SeqVisitor,
    {
        match visitor.visit()? {
            Some(NOTIFICATION) => {
                let typ = match visitor.visit()? {
                    Some(typ) => typ,
                    None => return Err(Error::invalid_length(1, &self)),
                };
                let data = match typ {
                    NotificationType::PublishImage => match visitor.visit()? {
                        Some(data) => Notification::PublishImage(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                };
                Ok(Message::Notification(data))
            }
            Some(REQUEST) => {
                let typ = match visitor.visit()? {
                    Some(typ) => typ,
                    None => return Err(Error::invalid_length(1, &self)),
                };
                let request_id = match visitor.visit()? {
                    Some(x) => x,
                    None => return Err(Error::invalid_length(2, &self)),
                };
                let data = match typ {
                    Type::AppendDir => match visitor.visit()? {
                        Some(data) => Request::AppendDir(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    Type::GetIndex => match visitor.visit()? {
                        Some(data) => Request::GetIndex(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    Type::GetBlock => match visitor.visit()? {
                        Some(data) => Request::GetBlock(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                };
                Ok(Message::Request(request_id, data))
            },
            Some(RESPONSE) => {
                let typ = match visitor.visit()? {
                    Some(typ) => typ,
                    None => return Err(Error::invalid_length(1, &self)),
                };
                let request_id = match visitor.visit()? {
                    Some(x) => x,
                    None => return Err(Error::invalid_length(2, &self)),
                };
                let data = match typ {
                    Type::AppendDir => match visitor.visit()? {
                        Some(data) => Response::AppendDir(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    Type::GetIndex => match visitor.visit()? {
                        Some(data) => Response::GetIndex(data),
                        None => return Err(Error::invalid_length(3, &self)),
                    },
                    Type::GetBlock => match visitor.visit()? {
                        Some(data) => Response::GetBlock(data),
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
