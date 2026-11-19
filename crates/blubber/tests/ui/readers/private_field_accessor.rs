#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        /// Application module.
        pub mod app {
            #[wire::schema] pub struct Record { hidden: u8 }
        }
        /// Cannot bypass declared field visibility.
        pub fn read(record: app::RecordRef<'_>) -> u8 { record.hidden() }

use foreign as _;
