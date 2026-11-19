#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Bad { pub r#type: u8, pub try_type: u16 }

use foreign as _;
