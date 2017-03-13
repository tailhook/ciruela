use serde::Serialize;


pub trait Request: Serialize + 'static {
    type Response: 'static;
    fn type_name(&self) -> &'static str;
}

pub trait Notification: Serialize + 'static {
    fn type_name(&self) -> &'static str;
}
