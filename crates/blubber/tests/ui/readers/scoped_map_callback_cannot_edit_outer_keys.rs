#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema(account)] pub struct Collections {
            pub rows: wire::List<foreign::Record>,
            pub map: wire::Map<u16, foreign::Record>,
            pub option: ::core::option::Option<foreign::Record>,
        }

/// Reject overlapping lending or callback borrows.
pub fn bad(store: &mut dyn wire::Resize) -> wire::Result<()> {
 let mut account = Collections::load_mut(store)?;

            let mut map = account.map_mut();
            map.try_for_each_mut(|_, record| {
                map.clear()?;
                record.text_mut().set("child")
            })?;

 ::core::result::Result::Ok(())
}

use foreign as _;
