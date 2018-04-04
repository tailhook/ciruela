//! Ciruela Client Library
//! ======================
//!
//! [Docs](https://docs.rs/ciruela/) |
//! [Server Docs](https://ciruela.readthedocs.io/) |
//! [Examples](https://github.com/tailhook/ciruela/tree/master/examples) |
//! [Github](https://github.com/tailhook/ciruela/) |
//! [Crate](https://crates.io/crates/ciruela)
//!
#![allow(dead_code)]  // temporarily
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
extern crate abstract_ns;
extern crate base64;
extern crate blake2;
extern crate crypto;
extern crate digest_writer;
extern crate dir_signature;
extern crate hex;
extern crate futures;
extern crate futures_cpupool;
extern crate tk_http;
extern crate rand;
extern crate serde;
extern crate serde_cbor;
extern crate serde_bytes;
extern crate ssh_keys;
extern crate tk_easyloop;
extern crate tk_bufstream;
extern crate tokio_core;
extern crate tokio_io;
extern crate valuable_futures;
extern crate void;

#[macro_use] extern crate log;
#[macro_use] extern crate mopa;
#[macro_use] extern crate matches;
#[macro_use] extern crate quick_error;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate failure;

mod failure_tracker;
mod id;
mod machine_id;
mod virtual_path;
mod proto;
mod database;
mod time_util;
mod serialize;
mod hexlify;
mod block_id;
pub mod blocks;
pub mod index;
pub mod cluster;
pub mod signature;

pub use virtual_path::VPath;
