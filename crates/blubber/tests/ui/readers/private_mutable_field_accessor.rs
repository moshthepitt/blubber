#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        /// Application module.
        pub mod app { #[wire::schema] pub struct Record { hidden: u8 } }
        /// Mutable projections preserve field visibility.
        pub fn bad(record: app::RecordMut<'_>) -> wire::Result<()> {
            record.hidden_mut().set(1)
        }

use foreign as _;
