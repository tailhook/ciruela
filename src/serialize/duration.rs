use std::time::{Duration};

use serde::{Serializer, Deserializer, Deserialize};


pub fn serialize<S>(dur: &Duration, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
{
    let ms = dur.as_secs()*1000 + (dur.subsec_nanos() / 1000000) as u64;
    ser.serialize_u64(ms)
}

pub fn deserialize<'a, D>(des: D) -> Result<Duration, D::Error>
    where D: Deserializer<'a>
{
    let ms = u64::deserialize(des)?;
    // TODO(tailhook) this can overflow. How can we ensure that it doesn't?
    Ok(Duration::new(ms / 1000, (ms % 1000) as u32 * 1000000))
}
