#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
macro_rules! identity { ($ty:ty) => { $ty }; } #[allow(missing_docs)] pub trait Fields { type Payload; } #[wire::schema] pub struct Owner { pub value: identity!(<Self as Fields>::Payload) } impl Fields for Owner { type Payload = u8; }
use foreign as _;
