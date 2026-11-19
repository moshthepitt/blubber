#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        use adapter::{Blubber, sdk::{Address, Error, Owner}};
        #[wire::schema(account)] pub struct State { pub counter: u64, pub text: wire::Str }
        impl Owner for State { const OWNER: Address = Address::new_from_array([1; 32]); }

            /// An edit result cannot borrow the root.
            pub fn bad(state: &mut Blubber<State>) -> Result<&str, Error> { state.edit(|s| Ok(s.text())).map(|(s,_)| s) }
