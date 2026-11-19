#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema(account)] pub struct Collections {
            pub rows: wire::List<foreign::Record>,
            pub map: wire::Map<u16, foreign::Record>,
            pub option: ::core::option::Option<foreign::Record>,
        }

        /// A local map lends only a local child.
        pub fn bad<'a>(account: &'a mut CollectionsMut<'_>) -> wire::Result<foreign::RecordMut<'a>> {
            let mut map = account.map_mut();
            map.get_mut(&1)?.ok_or(wire::ErrorKind::MissingKey.into())
        }

use foreign as _;
