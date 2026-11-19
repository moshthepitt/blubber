#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

            use wire::AccountSchema;
            #[wire::schema(account, constraint_fields = false)] pub struct State { pub wide: [u8; 4096] }
            /// Opt-out retains ordinary checked field readers.
            pub fn check(bytes: &[u8]) -> wire::Result<[u8; 4096]> {
                let (): StatePrefix = State::read_prefix(wire::View::load::<State>(bytes, State::DISCRIMINATOR)?)?;
                State::load(bytes)?.try_wide()
            }
