#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema(account)] pub struct Collections {
            pub rows: wire::List<foreign::Record>,
            pub map: wire::Map<u16, foreign::Record>,
            pub option: ::core::option::Option<foreign::Record>,
        }

        /// Copy Ref views and shared Mut borrows yield ordinary reads.
        pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut account = Collections::load_mut(store)?;
            for row in account.rows() { let _: &str = row.text(); }
            for (key, row) in account.map() { let _: (u16, &str) = (key, row.text()); }
            let mut rows = account.rows_mut();
            for row in &rows { let _: &str = row.text(); }
            rows.clear()?;
            let mut map = account.map_mut();
            for (key, row) in &map { let _: (u16, &str) = (key, row.text()); }
            map.clear()
        }

use foreign as _;
