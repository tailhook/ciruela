extern crate base64;
extern crate crypto;
extern crate hex;
extern crate futures;
extern crate futures_cpupool;
extern crate tk_http;
extern crate serde;
extern crate serde_cbor;
extern crate ssh_keys;
extern crate tk_easyloop;
extern crate tokio_core;

#[macro_use] extern crate log;
#[macro_use] extern crate mopa;
#[macro_use] extern crate quick_error;
#[macro_use] extern crate serde_derive;

pub mod proto;
pub mod database;
pub mod time;

pub type ImageId = Vec<u8>;  // TODO(tailhook) make a newtype
