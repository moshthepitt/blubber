#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema] pub struct FixedPair { pub enabled: bool, pub count: u16 }
        #[wire::schema(account)] pub struct FixedLoans { pub numbers: wire::List<u64>, pub pairs: wire::List<FixedPair> }


        /// Public editor types remain available to ordinary generic callers.
        pub fn generic<T: wire::Schema>(mut list: wire::ListMut<'_, T>, f: impl for<'s> FnMut(&mut T::Mut<'s>) -> wire::Result<()>) -> wire::Result<()> {
            list.try_for_each_mut(f)
        }
        /// Explicit scalar, list, cursor and generated child annotations compile.
        pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut root = FixedLoans::load_mut(store)?;
            let list: wire::ListMut<'_, u64> = root.numbers_mut();
            let mut cursor: wire::ListCursorMut<'_, u64> = list.cursor_mut()?;
            if let ::core::option::Option::Some(mut value) = cursor.next()? {
                let scalar: &mut wire::ScalarMut<'_, u64> = &mut value;
                scalar.set(scalar.get() + 1)?;
            }
            let mut pairs = root.pairs_mut().cursor_mut()?;
            if let ::core::option::Option::Some(mut pair) = pairs.next()? {
                let typed: &mut FixedPairMut<'_> = &mut pair;
                typed.count_mut().set(7)?;
            }
            generic(root.numbers_mut(), |value| value.set(9))?;
            root.pairs_mut().at_mut(0)?.count_mut().set(10)?;
            ::core::result::Result::Ok(())
        }

use foreign as _;
