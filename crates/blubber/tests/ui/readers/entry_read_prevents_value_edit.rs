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

            let mut value = account.map_mut().into_entry(1)?.or_insert(foreign::RecordInit { text: "x" })?;
            let old = value.text();
            value.text_mut().set("growth")?;
            let _ = old.len();

 ::core::result::Result::Ok(())
}

use foreign as _;
