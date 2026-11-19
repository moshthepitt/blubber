//! corrupt admission, physical frames and interrupted codec publication.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]
use blubber::{Error, ErrorKind, List, Map, ReadStorage, Resize, SliceStore, Str, View};

#[blubber::schema(account)]
struct Probe {
    map: Map<u8, bool>,
    words: List<Str>,
}

fn initial() -> [u8; 96] {
    let mut bytes = [0; 96];
    Probe::initialize(
        &mut SliceStore::new(&mut bytes),
        ProbeInit {
            map: &[(1, true), (2, true), (3, true)],
            words: &["a", "b", "c"],
        },
    )
    .unwrap();
    bytes[64..].fill(0xa5);
    bytes
}

#[test]
fn selected_bounds_reject_full_width_and_distant_end_without_panics() {
    for end in [4, u32::MAX] {
        let mut bytes = initial();
        let view = View::load::<Probe>(&bytes, Probe::DISCRIMINATOR).unwrap();
        let list = view.field(1).unwrap().base() as usize;
        bytes[list + 4..list + 8].copy_from_slice(&end.to_le_bytes());
        let view = View::new(
            &bytes[list..list + 15],
            <List<Str> as blubber::Schema>::LAYOUT,
            list as u32,
        )
        .unwrap();
        let list_ref = view.read::<List<Str>>().unwrap();
        assert_eq!(
            list_ref.try_get(0),
            Err(Error::new(ErrorKind::InvalidBoundary, list as u32 + 4)),
            "selected end bound"
        );
        assert_eq!(list_ref.try_get(u32::MAX as usize).unwrap(), None);
        assert_eq!(list_ref.try_get(usize::MAX).unwrap(), None);
    }
}

#[test]
fn lazy_history_cannot_mint_strict_directory_evidence() {
    let mut bytes = initial();
    let view = View::load::<Probe>(&bytes, Probe::DISCRIMINATOR).unwrap();
    let list = view.field(1).unwrap().base() as usize;
    bytes[list + 4..list + 8].copy_from_slice(&2u32.to_le_bytes());
    bytes[list + 8..list + 12].copy_from_slice(&1u32.to_le_bytes());
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    // The second end word precedes the first.
    let fault = Error::new(ErrorKind::InvalidBoundary, list as u32 + 8);
    let loaded = Probe::load_trusted_mut(&mut store);
    if cfg!(feature = "verify-on-load") {
        assert_eq!(loaded.err(), Some(fault));
        // Eager ingress rejects before any session exists: no poison.
        assert!(!store.is_poisoned(), "eager ingress poisoned");
    } else {
        let mut root = loaded.unwrap();
        assert_eq!(
            root.words_mut().rewrite_from(&["x", "y", "z"]),
            Err(fault),
            "false strict evidence"
        );
        // A detected source fault poisons even before any write.
        assert!(store.is_poisoned(), "lazy source fault poison");
        assert_eq!(
            Probe::load_trusted_mut(&mut store).err(),
            Some(ErrorKind::Poisoned.into())
        );
    }
    assert_eq!(store.bytes(), before);
}

#[test]
fn fixed_value_writes_preserve_every_key_and_spare_byte() {
    let mut bytes = initial();
    let mut expected = bytes;
    // Independent format: envelope, one root end, count, then key/value pairs.
    // Nonzero keys make any key-frame write visible.
    assert_eq!(bytes[16..26], [3, 0, 0, 0, 1, 1, 2, 1, 3, 1]);
    for value in [21, 23, 25] {
        expected[value] = 0;
    }
    let mut store = SliceStore::new(&mut bytes);
    Probe::load_trusted_mut(&mut store)
        .unwrap()
        .map_mut()
        .fill_values(false)
        .unwrap();
    assert_eq!(store.bytes(), expected, "key frame");
}

#[test]
fn interrupted_encoder_keeps_old_publication_and_poison_until_checked_recovery() {
    use blubber_core::raw::{InlinePath, RewritePlan};
    let mut bytes = initial();
    let old = bytes[8..12].to_vec();
    let view = View::load::<Probe>(&bytes, Probe::DISCRIMINATOR).unwrap();
    // Count and two end words precede the first, now half-written, slot.
    let slot = view.field(1).unwrap().base() + 12;
    let mut store = SliceStore::new(&mut bytes);
    let result = blubber::raw::rewrite_collection(
        &mut store,
        &blubber_core::Layout::Flat {
            checked: <Probe as blubber::RootSchema>::__layout(),
        },
        &InlinePath::root_field(1),
        (Probe::DISCRIMINATOR, <Probe as blubber::Schema>::MIN_SIZE),
        |view| Ok((RewritePlan::check(view.bytes(), &view.layout(), 3, 9)?, ())),
        |(), stream| {
            stream.prepare_slot(3)?;
            stream.destination().copy_from_slice(&[0xff; 3]);
            Err(ErrorKind::InvalidBoundary.into())
        },
    );
    assert_eq!(result, Err(ErrorKind::InvalidBoundary.into()));
    assert!(store.is_poisoned());
    assert_eq!(&store.bytes()[8..12], old, "early publication");
    assert!(
        matches!(Probe::load_trusted_mut(&mut store), Err(error) if error == ErrorKind::Poisoned.into())
    );
    assert_eq!(
        Probe::load_checked_mut(&mut store).err(),
        Some(Error::new(ErrorKind::InvalidUtf8, slot))
    );
    assert!(store.is_poisoned());
    // No drop/reopen shortcut: recover inside the same poisoned backend.
    blubber::raw::backend(&mut store)
        .bytes_mut()
        .copy_from_slice(&initial());
    Probe::load_checked_mut(&mut store)
        .unwrap()
        .verify()
        .unwrap();
    assert!(!store.is_poisoned());
}

#[test]
fn lazy_clear_poisons_on_a_malformed_count_without_writing() {
    for truncate in [false, true] {
        let mut bytes = initial();
        let view = View::load::<Probe>(&bytes, Probe::DISCRIMINATOR).unwrap();
        let list = view.field(1).unwrap().base() as usize;
        bytes[list..list + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let loaded = Probe::load_trusted_mut(&mut store);
        if cfg!(feature = "verify-on-load") {
            assert!(loaded.is_err(), "eager ingress");
            assert!(!store.is_poisoned());
        } else {
            let mut root = loaded.unwrap();
            let mut words = root.words_mut();
            let result = if truncate {
                words.truncate(0)
            } else {
                words.clear()
            };
            // A full-width count overflows the directory size before any write;
            // the fault names the count word.
            assert_eq!(
                result,
                Err(Error::new(ErrorKind::ArithmeticOverflow, list as u32)),
                "malformed count accepted"
            );
            assert!(store.is_poisoned(), "lazy clear poison");
            assert_eq!(
                Probe::load_trusted_mut(&mut store).err(),
                Some(ErrorKind::Poisoned.into())
            );
        }
        assert_eq!(store.bytes(), before, "clear wrote before admission");
    }
}

#[test]
fn lazy_clear_discards_distant_damage_and_repairs_the_root() {
    for truncate in [false, true] {
        let mut bytes = initial();
        let view = View::load::<Probe>(&bytes, Probe::DISCRIMINATOR).unwrap();
        let list = view.field(1).unwrap().base() as usize;
        // The second end word precedes the first; the count stays intact.
        bytes[list + 8..list + 12].copy_from_slice(&0u32.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let loaded = Probe::load_trusted_mut(&mut store);
        if cfg!(feature = "verify-on-load") {
            assert!(loaded.is_err(), "eager ingress");
            assert_eq!(store.bytes(), before);
        } else {
            let mut root = loaded.unwrap();
            let mut words = root.words_mut();
            if truncate {
                words.truncate(0).unwrap();
            } else {
                words.clear().unwrap();
            }
            assert!(!store.is_poisoned(), "discarded ends need no poison");
            let root = Probe::load_checked_mut(&mut store).unwrap();
            root.verify().unwrap();
            assert_eq!(root.words().len(), 0, "empty list");
            // One count word replaces three ends and payload; the old tail is zero.
            let used = u32::from_le_bytes(store.bytes()[8..12].try_into().unwrap()) as usize;
            assert_eq!(used, list + 4, "published used length");
            assert!(
                store.bytes()[used..64].iter().all(|byte| *byte == 0),
                "cleared tail"
            );
            assert!(
                store.bytes()[64..].iter().all(|byte| *byte == 0xa5),
                "spare"
            );
        }
    }
}

#[test]
fn lazy_list_splices_poison_on_a_malformed_selected_end_without_writing() {
    for route in 0..6 {
        let mut bytes = initial();
        let view = View::load::<Probe>(&bytes, Probe::DISCRIMINATOR).unwrap();
        let list = view.field(1).unwrap().base() as usize;
        // Damage the end word each route selects; the count stays intact.
        let end = list + if matches!(route, 3 | 4) { 8 } else { 4 };
        bytes[end..end + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let loaded = Probe::load_trusted_mut(&mut store);
        if cfg!(feature = "verify-on-load") {
            assert!(loaded.is_err(), "eager ingress");
            assert!(!store.is_poisoned());
        } else {
            let mut root = loaded.unwrap();
            let mut words = root.words_mut();
            let result = match route {
                0 => words.insert(1, "x"),
                1 => words.remove(0),
                2 => words.replace(0, "x"),
                3 => words.pop().map(|_| ()),
                4 => words.truncate(2),
                _ => words.insert_from_slice(1, &["x"]),
            };
            assert!(result.is_err(), "malformed selected end accepted");
            assert!(store.is_poisoned(), "lazy splice poison");
            assert_eq!(
                Probe::load_trusted_mut(&mut store).err(),
                Some(ErrorKind::Poisoned.into())
            );
        }
        assert_eq!(store.bytes(), before, "splice wrote before admission");
    }
}

#[blubber::schema(account)]
struct Ledger {
    entries: Map<u8, Str>,
    note: Str,
}

#[test]
fn lazy_map_edits_poison_on_a_malformed_selected_end_without_writing() {
    for route in 0..5 {
        let mut bytes = [0; 96];
        Ledger::initialize(
            &mut SliceStore::new(&mut bytes),
            LedgerInit {
                entries: &[(1, "one"), (3, "three")],
                note: "tail",
            },
        )
        .unwrap();
        let view = View::load::<Ledger>(&bytes, Ledger::DISCRIMINATOR).unwrap();
        // Records are [key:u8][end:u32]; damage the first end, which every
        // route selects. The count and both keys stay intact.
        let end = view.field(0).unwrap().base() as usize + 5;
        bytes[end..end + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let loaded = Ledger::load_trusted_mut(&mut store);
        if cfg!(feature = "verify-on-load") {
            assert!(loaded.is_err(), "eager ingress");
            assert!(!store.is_poisoned());
        } else {
            let mut root = loaded.unwrap();
            let mut entries = root.entries_mut();
            let result = match route {
                0 => entries.insert(1, "x").map(|_| ()),
                1 => entries.insert(2, "x").map(|_| ()),
                2 => entries.remove(&1).map(|_| ()),
                3 => entries.try_for_each_mut(|_, slot| slot.set("x")),
                _ => entries.cursor_mut().and_then(|mut cursor| {
                    cursor.next()?;
                    Ok(())
                }),
            };
            assert!(result.is_err(), "malformed selected end accepted");
            assert!(store.is_poisoned(), "lazy map poison");
            assert_eq!(
                Ledger::load_trusted_mut(&mut store).err(),
                Some(ErrorKind::Poisoned.into())
            );
        }
        assert_eq!(store.bytes(), before, "map edit wrote before admission");
    }
}

#[test]
fn lazy_nested_growth_poisons_on_a_later_ancestor_end() {
    for text in ["xyz", ""] {
        let mut bytes = initial();
        let view = View::load::<Probe>(&bytes, Probe::DISCRIMINATOR).unwrap();
        let list = view.field(1).unwrap().base() as usize;
        // Damage the second stored end. Editing item 0 selects only the
        // first end, so the fused ancestor pass meets the damage mid-pass.
        bytes[list + 8..list + 12].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let loaded = Probe::load_trusted_mut(&mut store);
        if cfg!(feature = "verify-on-load") {
            assert!(loaded.is_err(), "eager ingress");
            assert!(!store.is_poisoned());
            assert_eq!(store.bytes(), before, "eager ingress wrote");
            continue;
        }
        let mut root = loaded.unwrap();
        let result = root
            .words_mut()
            .at_mut(0)
            .and_then(|mut word| word.set(text));
        assert!(
            matches!(
                result,
                Err(Error::Fault {
                    kind: ErrorKind::InvalidBoundary,
                    ..
                })
            ),
            "later ancestor end accepted: {result:?}"
        );
        assert!(store.is_poisoned(), "nested ancestor poison");
        assert_eq!(
            Probe::load_trusted_mut(&mut store).err(),
            Some(ErrorKind::Poisoned.into())
        );
        assert_eq!(
            &store.bytes()[8..12],
            &before[8..12],
            "used length published"
        );
    }
}
