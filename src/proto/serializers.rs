use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Serializer, Deserializer, Deserialize};


pub fn write_timestamp<S>(tm: &SystemTime, ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer
{
    let ts = tm.duration_since(UNIX_EPOCH)
        .expect("timestamp is always after unix epoch");
    let ms = ts.as_secs() + (ts.subsec_nanos() / 1000000) as u64;
    ser.serialize_u64(ms)
}


pub fn read_timestamp<D>(des: D) -> Result<SystemTime, D::Error>
    where D: Deserializer
{
    let ms = u64::deserialize(des)?;
    // TODO(tailhook) this can overflow. How can we ensure that it doesn't?
    Ok(UNIX_EPOCH + Duration::new(ms / 1000, (ms % 1000) as u32 * 1000000))
}
