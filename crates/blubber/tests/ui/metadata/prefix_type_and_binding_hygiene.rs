#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

            use wire::AccountSchema;
            #[allow(missing_docs, non_camel_case_types)]
            mod aliases {
                pub type Value = ::core::primitive::u64;
                pub type Tail = wire::Str;
                pub type __BlubberPrefixField0 = ::core::primitive::u16;
                pub type __BlubberPrefixTail0 = ::core::primitive::u32;
                #[wire::schema(account)]
                pub struct State {
                    pub counter: Value, pub text: Tail,
                    pub a: __BlubberPrefixField0, pub b: __BlubberPrefixTail0,
                    #[blubber(accessor = "tail_byte")] pub __blubber_prefix_tail: ::core::primitive::u8,
                }
            }
            /// Generated field helpers cannot capture authored aliases or field names.
            pub fn check(bytes: &[u8]) -> wire::Result<u64> {
                let p = aliases::State::read_prefix(wire::View::load::<aliases::State>(bytes, aliases::State::DISCRIMINATOR)?)?;
                let _: u16 = p.a; let _: u32 = p.b; let _: u8 = p.__blubber_prefix_tail;
                ::core::result::Result::Ok(p.counter)
            }
