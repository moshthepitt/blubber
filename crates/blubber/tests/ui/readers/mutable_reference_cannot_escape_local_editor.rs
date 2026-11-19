#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Account { pub text: wire::Str, pub nested: foreign::Record }

        /// Returning a read through a local mutable editor would outlive it.
        pub fn bad(store: &mut dyn wire::Resize) -> wire::Result<&str> {
            let mut account = Account::load_mut(store)?;
            let record = account.nested_mut();
            ::core::result::Result::Ok(record.text())
        }

use foreign as _;
