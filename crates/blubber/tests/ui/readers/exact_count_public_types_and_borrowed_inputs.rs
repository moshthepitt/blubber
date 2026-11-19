#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema(account)] pub struct Collections {
            pub rows: wire::List<foreign::Record>,
            pub map: wire::Map<u16, foreign::Record>,
            pub option: ::core::option::Option<foreign::Record>,
        }

        /// Rewrites retain the existing concrete editor and borrowed Init types.
        pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut account = Collections::load_mut(store)?;
            let mut rows: wire::ListMut<'_, foreign::Record> = account.rows_mut();
            { let text = [b'a']; let text = ::core::str::from_utf8(&text).unwrap();
              rows.fill(foreign::RecordInit { text })?;
              rows.rewrite_from(&[foreign::RecordInit { text }])?; }
            let mut map: wire::MapMut<'_, u16, foreign::Record> = account.map_mut();
            map.fill_values(foreign::RecordInit { text: "same" })?;
            map.rewrite_values_from(&[(1,foreign::RecordInit { text: "new" })])?;
            account.validate()
        }
        /// Generic callers keep the same GAT input slice and editor types.
        pub fn generic<T: wire::Schema>(mut values: wire::ListMut<'_,T>, inputs: &[T::Init<'_>]) -> wire::Result<()> {
            values.rewrite_from(inputs)
        }

use foreign as _;
