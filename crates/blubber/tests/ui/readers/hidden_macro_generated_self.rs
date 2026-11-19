#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
macro_rules! hidden_type { () => { Self::Field0 }; } #[wire::schema] pub struct Capture { pub first: u8, pub second: hidden_type!() }
use foreign as _;
