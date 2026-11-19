#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema(account)]
        pub struct Leaves { pub scalar: u32, pub text: wire::Str, pub choice: ::core::option::Option<wire::Str>, pub bytes: wire::List<u8> }
        /// Leaf getters have copied/borrowed outputs; bulk inputs can expire after encoding.
        pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut account = Leaves::load_mut(store)?;
            let _: u32 = account.scalar_mut().get();
            let _: wire::Result<u32> = account.scalar_mut().try_get();
            let _: &str = account.text_mut().get();
            let _: wire::Result<&str> = account.text_mut().try_get();
            let _: ::core::option::Option<&str> = account.choice_mut().get();
            let _: wire::Result<::core::option::Option<&str>> = account.choice_mut().try_get();
            let mut list = account.bytes_mut();
            { let input = [1, 2]; list.extend_from_slice(&input)?; }
            list.insert_from_slice(0, &[3, 4])?;
            let _: u32 = (&list).into_iter().map(u32::from).sum();
            list.clear()?;
            account.validate()
        }

use foreign as _;
