#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
macro_rules! identity { ($ty:ty) => { $ty }; }
#[wire::schema] pub struct Captured<CapturedRef> { pub value: CapturedRef }

use foreign as _;
