#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        use adapter::{Blubber, sdk::{Address, Error, Owner}};
        #[wire::schema(account)] pub struct State { pub counter: u64, pub text: wire::Str }
        impl Owner for State { const OWNER: Address = Address::new_from_array([1; 32]); }

            /// Copies can leave callbacks and survive edits.
            pub fn run(state: &mut Blubber<State>) -> Result<u64, Error> {
                let old = state.read(|s| Ok(s.counter()))?;
                state.edit(|s| s.counter_mut().set(old + 1))?;
                Ok(state.counter)
            }
