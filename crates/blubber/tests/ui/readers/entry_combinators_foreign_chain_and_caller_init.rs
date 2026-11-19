#![no_std]
#![forbid(unsafe_code)]
#![deny(warnings, missing_docs)]
//! Reader consumer.

        #[wire::schema(account)] pub struct Collections {
            pub rows: wire::List<foreign::Record>,
            pub map: wire::Map<u16, foreign::Record>,
            pub option: ::core::option::Option<foreign::Record>,
        }

        /// Callback borrows end before the retained entry is used again.
        pub fn allowed(store: &mut dyn wire::Resize) -> wire::Result<()> {
            let input = [b'x'];
            let text = ::core::str::from_utf8(&input).unwrap();
            struct Token;
            let captured = Token;
            let mut account = Collections::load_mut(store)?;
            {
                let mut map = account.map_mut();
                map.entry(2)?.or_insert(foreign::RecordInit { text })?.text_mut().set("two")?;
                map.get_mut(&2)?.ok_or(wire::ErrorKind::MissingKey)?.text_mut().push_str("!")?;
                map.entry(3)?.or_insert(foreign::RecordInit { text })?;
            }
            let mut record = account.map_mut().into_entry(1)?
                .and_modify(|record| {
                    record.text_mut().push_str(" growth")?;
                    ::core::mem::drop(captured);
                    ::core::result::Result::Ok(())
                })?
                .or_insert_with(|| foreign::RecordInit { text })?;
            record.text_mut().push_str(" later")?;
            account.map_mut().insert(2, foreign::RecordInit { text: "two" })?;
            account.validate()
        }
        /// The factory's input borrow ends at encoding, before the result escapes.
        pub fn returned(map: wire::MapMut<'_, u16, wire::Str>) -> wire::Result<wire::StrMut<'_>> {
            let input = [b'x'];
            let text = ::core::str::from_utf8(&input).unwrap();
            map.into_entry(1)?.or_insert_with(|| text)
        }
        /// Initializer defaults do not require a Default editor.
        pub fn defaults(map: wire::MapMut<'_, u16, wire::Str>) -> wire::Result<wire::StrMut<'_>> {
            map.into_entry(1)?.or_default()
        }

use foreign as _;
