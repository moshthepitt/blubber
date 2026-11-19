#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
macro_rules! identity { ($ty:ty) => { $ty }; }
#[wire::schema] pub struct Captured<'CapturedMut> { pub value: u8 }

use foreign as _;
