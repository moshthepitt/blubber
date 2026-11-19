#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Bad { pub value: u8, pub try_value: u16 }

use foreign as _;
