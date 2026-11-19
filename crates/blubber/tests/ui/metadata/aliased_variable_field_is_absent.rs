#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

            /// Variable alias.
            pub type Values = wire::List<u8>;
            #[wire::schema(account)] pub struct State { pub values: Values, pub counter: u64 }
            /// An alias cannot bypass variable-field exclusion.
            pub fn bad(prefix: StatePrefix) { let _ = prefix.values; }
