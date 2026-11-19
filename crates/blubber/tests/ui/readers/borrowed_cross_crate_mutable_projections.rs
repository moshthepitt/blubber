#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.
#[wire::schema(account)] pub struct Account { pub text: wire::Str, pub nested: foreign::Record }

        /// A helper accepts a generated foreign handle by value.
        pub fn child(mut record: foreign::RecordMut<'_>) -> wire::Result<()> {
            record.text_mut().set("helper")
        }
        /// Retained parent handles permit repeated child edits and short reads.
        pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let mut account = Account::load_mut(store)?;
            account.text_mut().set("root")?;
            let mut record = account.nested_mut();
            record.text_mut().set("first")?;
            record.text_mut().push_str(" second")?;
            let text = record.text();
            let _ = text.len();
            child(record.reborrow())?;
            record.text_mut().clear()?;
            child(account.nested_mut())?;
            account.validate()
        }

use foreign as _;
