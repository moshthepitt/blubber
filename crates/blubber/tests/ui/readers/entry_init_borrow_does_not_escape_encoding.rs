#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        wire::__root_schema!(MapRoot, wire::Map<u16, foreign::Record>);
        /// Borrowed Init expires before the returned value editor.
        pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<foreign::RecordMut<'_>> {
            let input = [b'x'];
            let text = ::core::str::from_utf8(&input).unwrap();
            let edit = wire::raw::Edit::load::<MapRoot>(wire::raw::backend(store), *b"mapproof")?;
            let map = wire::raw::project::<wire::Map<u16, foreign::Record>>(edit);
            map.into_entry(1)?.or_insert(foreign::RecordInit { text })
        }

use foreign as _;
