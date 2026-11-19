#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        type Text = wire::Str;
        /// Shadow a prelude name.
        pub struct Option;
        /// Shadow a crate name.
        pub mod core {}
        #[wire::schema(account)] pub struct Account {
            #[blubber(accessor = "checked")] pub validate: Text,
            pub nested: foreign::Record,
            pub optional: ::core::option::Option<foreign::Record>,
            pub r#type: u16,
        }
        impl<'a> AccountRef<'a> {
            /// An ordinary inherent helper, with no extension trait import.
            pub fn label(&self) -> &'a str { self.checked() }
        }
        impl AccountMut<'_> {
            /// An ordinary mutable-root read helper.
            pub fn label(&self) -> &str { self.nested().text() }
        }
        /// Return a nested read through a temporary parent.
        pub fn read(bytes: &[u8]) -> wire::Result<&str> {
            let root: AccountRef<'_> = Account::load(bytes)?;
            let _ = root.r#type();
            let _ = root.try_type()?;
            let _ = root.try_checked()?;
            let _ = root.optional();
            ::core::result::Result::Ok(root.nested().text())
        }

use foreign as _;
