#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account, constraint_fields = false, constraint_fields = true)] pub struct State { pub counter: u64 }
