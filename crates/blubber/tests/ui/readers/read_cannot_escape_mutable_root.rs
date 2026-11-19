#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Account { pub text: wire::Str, pub nested: foreign::Record }
/// A mutable root owns the storage borrow.
pub fn bad(store: &mut dyn wire::Resize) -> wire::Result<&str> {
let account = Account::load_mut(store)?;
::core::result::Result::Ok(account.text())
}

use foreign as _;
