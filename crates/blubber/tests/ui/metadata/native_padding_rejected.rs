#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct State { pub flag: bool, pub code: u64, pub wide: [u8; 1009] }
