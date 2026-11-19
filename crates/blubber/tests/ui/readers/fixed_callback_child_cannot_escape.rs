#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema] pub struct FixedPair { pub enabled: bool, pub count: u16 }
        #[wire::schema(account)] pub struct FixedLoans { pub numbers: wire::List<u64>, pub pairs: wire::List<FixedPair> }

/// Reject conflicting fixed-loan access.
pub fn bad(store: &mut dyn wire::Resize) -> wire::Result<()> { let mut root = FixedLoans::load_mut(store)?;
            let mut saved = ::core::option::Option::None;
            root.numbers_mut().try_for_each_mut(|value| {
                saved = ::core::option::Option::Some(value.reborrow());
                ::core::result::Result::Ok(())
            })?;
            let _ = saved;
         ::core::result::Result::Ok(()) }
use foreign as _;
