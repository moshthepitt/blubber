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

            let mut cursor = account.rows_mut().cursor_mut()?;
            let mut child = cursor.next()?.unwrap();
            let _ = cursor.next()?;
            child.text_mut().set("still live")?;

 ::core::result::Result::Ok(())
}

use foreign as _;
