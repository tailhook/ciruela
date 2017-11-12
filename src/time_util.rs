use std::time::{Duration, SystemTime, UNIX_EPOCH};


pub fn to_ms(tm: SystemTime) -> u64 {
    let ts = tm.duration_since(UNIX_EPOCH)
        .expect("timestamp is always after unix epoch");
    return ts.as_secs()*1000 + (ts.subsec_nanos() / 1000000) as u64;
}

pub fn time_ms() -> u64 {
    to_ms(SystemTime::now())
}

pub fn from_ms(ms: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms)
}

