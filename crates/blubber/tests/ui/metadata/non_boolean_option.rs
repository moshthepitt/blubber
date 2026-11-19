#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account, constraint_fields = 1)] pub struct State { pub counter: u64 }
