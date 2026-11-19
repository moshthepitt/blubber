#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema(account)] pub struct Collections {
            pub rows: wire::List<foreign::Record>,
            pub map: wire::Map<u16, foreign::Record>,
            pub option: ::core::option::Option<foreign::Record>,
        }

        /// Lending cursors and callbacks use ordinary generated field methods.
        pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut account = Collections::load_mut(store)?;
            let mut rows = account.rows_mut();
            let mut cursor: wire::ListCursorMut<'_, foreign::Record> = rows.reborrow().cursor_mut()?;
            while let ::core::option::Option::Some(mut record) = cursor.next()? {
                record.text_mut().push_str("cursor")?;
                record.text_mut().clear()?;
            }
            rows.try_for_each_mut(|record| record.text_mut().set("callback"))?;
            let mut cursor: wire::MapCursorMut<'_, u16, foreign::Record> = account.map_mut().cursor_mut()?;
            while let ::core::option::Option::Some((_, mut record)) = cursor.next()? {
                record.text_mut().set("map cursor")?;
            }
            account.map_mut().try_for_each_mut(|_, record| record.text_mut().set("map callback"))?;
            account.validate()
        }

use foreign as _;
