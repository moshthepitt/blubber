#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

            mod model { #[wire::schema(account)] pub struct State { pub value: u8, secret: u64 } }
            /// Constraint copies preserve authored visibility.
            pub fn bad(prefix: model::StatePrefix) { let _ = prefix.secret; }
