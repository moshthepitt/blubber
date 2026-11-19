#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema] pub struct Bad { #[blubber(accessor = "a", accessor = "b")] pub a: u8 }

use foreign as _;
