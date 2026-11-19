#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Account { pub text: wire::Str, pub nested: foreign::Record }

/// Reject conflicting access while an old view or projection lives.
pub fn bad(store: &mut dyn wire::Resize) -> wire::Result<()> {
let mut account = Account::load_mut(store)?;

            let mut text = account.nested_mut();
            let _ = account.text();
            text.text_mut().clear()?;

::core::result::Result::Ok(())
}

use foreign as _;
