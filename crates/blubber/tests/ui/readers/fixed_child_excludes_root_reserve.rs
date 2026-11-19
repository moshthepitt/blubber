#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema] pub struct FixedPair { pub enabled: bool, pub count: u16 }
        #[wire::schema(account)] pub struct FixedLoans { pub numbers: wire::List<u64>, pub pairs: wire::List<FixedPair> }

/// Reject conflicting fixed-loan access.
pub fn bad(store: &mut dyn wire::Resize) -> wire::Result<()> { let mut root = FixedLoans::load_mut(store)?;
            let mut cursor = root.numbers_mut().cursor_mut()?;
            let mut first = cursor.next()?.unwrap();
            root.reserve(100)?;
            first.set(1)?;
         ::core::result::Result::Ok(()) }
use foreign as _;
