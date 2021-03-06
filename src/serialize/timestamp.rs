use std::time::{SystemTime};

use serde::{Serializer, Deserializer, Deserialize};

use time_util::{to_ms, from_ms};


pub fn serialize<S>(tm: &SystemTime, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
{
    ser.serialize_u64(to_ms(*tm))
}


pub fn deserialize<'a, D>(des: D) -> Result<SystemTime, D::Error>
    where D: Deserializer<'a>
{
    let ms = u64::deserialize(des)?;
    // TODO(tailhook) this can overflow. How can we ensure that it doesn't?
    Ok(from_ms(ms))
}
