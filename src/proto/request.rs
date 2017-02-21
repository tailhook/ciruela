use serde::Serialize;


pub trait Request: Serialize {
    type Response;
    fn type_name(&self) -> &'static str;
}
