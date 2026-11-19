#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

            #[wire::schema(account, constraint_fields = false)] pub struct State { pub counter: u64 }
            /// Opt-out removes all constraint fields.
            pub fn bad(prefix: StatePrefix) { let _ = prefix.counter; }
