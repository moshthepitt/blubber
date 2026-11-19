#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        type Text = wire::Str;
        /// Shadow prelude and crate names without affecting generated paths.
        pub struct Option;
        /// Shadow a crate name.
        pub mod core {}
        #[wire::schema] pub struct Strange {
            #[blubber(accessor = "try_checked")] pub text: Text,
            pub try_field: Text,
        }
        #[wire::schema(account)] pub struct Raw {
            #[blubber(accessor = "r#type")] pub original: Text,
            #[blubber(accessor = "try_root")] pub renamed: Text,
            pub try_root_field: Text,
            pub strange: Strange,
            pub foreign: foreign::Record,
            pub optional: ::core::option::Option<foreign::Record>,
        }
        impl RawMut<'_> {
            /// Ordinary inherent helper on a generated local mutable type.
            pub fn append(&mut self) -> wire::Result<()> {
                self.type_mut().push_str("raw")?;
                self.try_root_mut().set("overridden")?;
                self.try_root_field_mut().set("try prefix")?;
                let mut strange = self.strange_mut();
                strange.try_checked_mut().set("nested override")?;
                strange.try_field_mut().set("nested try prefix")?;
                self.foreign_mut().text_mut().set("foreign")?;
                self.optional_mut().some_mut()?.text_mut().set("optional")
            }
        }

use foreign as _;
