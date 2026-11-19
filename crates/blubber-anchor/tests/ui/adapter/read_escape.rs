#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        use adapter::{Blubber, sdk::{Address, Error, Owner}};
        #[wire::schema(account)] pub struct State { pub counter: u64, pub text: wire::Str }
        impl Owner for State { const OWNER: Address = Address::new_from_array([1; 32]); }

            /// A borrowed variable field cannot leave a read callback.
            pub fn bad(state: &Blubber<State>) -> Result<&str, Error> { state.read(|s| Ok(s.text())) }
