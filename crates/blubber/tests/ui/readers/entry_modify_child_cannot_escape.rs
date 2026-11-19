#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema(account)] pub struct Collections {
            pub rows: wire::List<foreign::Record>,
            pub map: wire::Map<u16, foreign::Record>,
            pub option: ::core::option::Option<foreign::Record>,
        }

/// Retained entries exclude conflicting account access.
pub fn bad(store: &mut dyn wire::Resize) -> wire::Result<()> {
 let mut account = Collections::load_mut(store)?;

            let mut escaped = ::core::option::Option::None;
            account.map_mut().into_entry(1)?.and_modify(|record| {
                escaped = ::core::option::Option::Some(record.text_mut());
                ::core::result::Result::Ok(())
            })?;
            escaped.unwrap().set("outside callback")?;

 ::core::result::Result::Ok(())
}

use foreign as _;
