#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema] pub struct Child { pub tags: wire::List<u16> }
#[wire::schema(account)] pub struct Parent { pub child: Child }

        /// Bind the intermediate Mut before retaining a nested field.
        pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut acct = Parent::load_mut(store)?;
            let mut child = acct.child_mut();
            let mut tags = child.tags_mut();
            tags.reborrow().at_mut(0)?.set(7)?;
            tags.validate()
        }

use foreign as _;
