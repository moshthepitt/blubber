#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Bad { pub value_mut: u16, pub value: u8 }

use foreign as _;
