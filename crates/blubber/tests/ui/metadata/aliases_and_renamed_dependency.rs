#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

            use wire::AccountSchema;
            /// A native alias is selected by schema kind.
            pub type Count = ::core::primitive::u64;
            /// A variable alias is omitted by schema kind.
            pub type Text = wire::Str;
            #[wire::schema(account)]
            pub struct State { pub text: Text, #[blubber(accessor = "renamed")] pub counter: Count, pub key: wire::Address }
            /// Prefixes and checked sizing need no SDK feature or allocation.
            pub fn check(bytes: &[::core::primitive::u8]) -> wire::Result<::core::primitive::usize> {
                let prefix: StatePrefix = State::read_prefix(wire::View::load::<State>(bytes, State::DISCRIMINATOR)?)?;
                let _: Count = prefix.counter;
                let _: wire::Address = prefix.key;
                State::space(17)
            }
