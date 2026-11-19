#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema] pub struct Bad { pub reborrow: u8 }

use foreign as _;
