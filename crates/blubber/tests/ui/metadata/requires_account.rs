#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(constraint_fields = false)] pub struct State { pub counter: u64 }
