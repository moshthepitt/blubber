#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema]
pub struct Unsupported {
    pub value: f64,
}

use foreign as _;
