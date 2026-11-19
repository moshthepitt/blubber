#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        use adapter::{Blubber, sdk::{Address, Error, Owner}};
        #[wire::schema(account)] pub struct State { pub counter: u64, pub text: wire::Str }
        impl Owner for State { const OWNER: Address = Address::new_from_array([1; 32]); }

            use adapter::sdk::AnchorAccount;
            /// SDK handle references remain scoped to the wrapper.
            pub fn bad(state: &Blubber<State>) -> &'static adapter::sdk::AccountView { state.account() }
