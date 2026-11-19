#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Account { pub text: wire::Str, pub nested: foreign::Record }
/// A finished read permits reserve and completion in the same scope.
pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<wire::CommitInfo> {
let mut account = Account::load_mut(store)?;
let text = account.nested().text();
let _ = text.len();
account.reserve(8)?;
account.commit()
}

use foreign as _;
