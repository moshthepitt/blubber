#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema] pub struct Child { pub tags: wire::List<u16> }
#[wire::schema(account)] pub struct Parent { pub child: Child }

        /// A retained field cannot borrow an intermediate temporary Mut.
        pub fn bad(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut acct = Parent::load_mut(store)?;
            let t = acct.child_mut().tags_mut();
            let _ = t.len();
            ::core::result::Result::Ok(())
        }

use foreign as _;
