#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

            #[wire::schema(account)] pub struct State { pub text: wire::Str, pub counter: u64 }
            /// Variable fields require checked read closures.
            pub fn bad(prefix: StatePrefix) { let _ = prefix.text; }
