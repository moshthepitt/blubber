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

            let mut map = account.map_mut();
            let first = map.entry(1)?;
            let second = map.entry(2)?;
            first.or_insert(foreign::RecordInit { text: "x" })?.text_mut().set("y")?;
            let _ = second;

 ::core::result::Result::Ok(())
}

use foreign as _;
