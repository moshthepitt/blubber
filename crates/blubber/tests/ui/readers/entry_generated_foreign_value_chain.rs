#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema(account)] pub struct Collections {
            pub rows: wire::List<foreign::Record>,
            pub map: wire::Map<u16, foreign::Record>,
            pub option: ::core::option::Option<foreign::Record>,
        }

        /// Returned foreign editors retain the account lease through a temporary entry.
        pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut account = Collections::load_mut(store)?;
            let mut record = account.map_mut().into_entry(1)?.or_insert(foreign::RecordInit { text: "x" })?;
            record.text_mut().push_str(" growth")?;
            record.text_mut().set("short")?;
            account.map_mut().insert(2, foreign::RecordInit { text: "two" })?;
            account.map_mut().remove(&1)?;
            account.validate()
        }

use foreign as _;
