#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema(account)] pub struct Collections {
            pub rows: wire::List<foreign::Record>,
            pub map: wire::Map<u16, foreign::Record>,
            pub option: ::core::option::Option<foreign::Record>,
        }

/// Shared iteration retains the handle borrow.
pub fn bad(store: &mut dyn wire::Resize) -> wire::Result<()> {
 let mut account = Collections::load_mut(store)?;

            let mut map = account.map_mut();
            let row = map.get(&1).unwrap();
            map.fill_values(foreign::RecordInit { text: row.text() })?;

 ::core::result::Result::Ok(())
}

use foreign as _;
