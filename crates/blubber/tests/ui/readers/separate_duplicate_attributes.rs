#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema] pub struct Bad { #[blubber(accessor = "a")] #[blubber(accessor = "b")] pub field: u8 }
use foreign as _;
