#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Bad { #[blubber(accessor = "same")] pub a: u8, #[blubber(accessor = "same")] pub b: u16 }

use foreign as _;
