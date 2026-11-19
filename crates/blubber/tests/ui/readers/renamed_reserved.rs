#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Bad { #[blubber(accessor = "validate")] pub a: u8 }

use foreign as _;
