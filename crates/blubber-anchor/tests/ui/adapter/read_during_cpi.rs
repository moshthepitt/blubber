#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        use adapter::{Blubber, sdk::{Address, Error, Owner}};
        #[wire::schema(account)] pub struct State { pub counter: u64, pub text: wire::Str }
        impl Owner for State { const OWNER: Address = Address::new_from_array([1; 32]); }

            pub fn bad(state: &mut Blubber<State>) -> Result<(), Error> {
                state.with_cpi_read(|_| state.read(|_| Ok(())))
            }
