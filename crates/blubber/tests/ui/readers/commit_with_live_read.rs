#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Account { pub text: wire::Str, pub nested: foreign::Record }
/// A read prevents relocation or completion while live.
pub fn bad(store: &mut dyn wire::Resize) -> wire::Result<()> {
#[allow(unused_mut)] let mut account = Account::load_mut(store)?;
let record = account.nested();
let _ = account.commit()?;
let _ = record.text();
::core::result::Result::Ok(())
}

use foreign as _;
