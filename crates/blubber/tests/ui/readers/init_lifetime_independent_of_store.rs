#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Account { pub text: wire::Str, pub nested: foreign::Record }
/// Initial input can expire while the account session lives.
pub fn init(store: &mut dyn wire::Resize) -> wire::Result<AccountMut<'_>> {
let buffer = [b'x'];
let text = ::core::str::from_utf8(&buffer).unwrap();
Account::initialize(store, AccountInit { text, nested: foreign::RecordInit { text } })
}

use foreign as _;
