//! Lazy empty-list publication and retained indexed-list admission frames.
#![forbid(unsafe_code)]

use blubber::{Error, ErrorKind, List, ReadStorage, Resize, SliceStore, Str, View};

#[blubber::schema(account)]
struct PatchLists {
    prefix: u16,
    values: List<Str>,
    suffix: Str,
}

#[blubber::schema(account)]
struct RemovedPatch {
    values: List<RemovedValue>,
}

#[blubber::schema]
struct RemovedValue {
    flag: bool,
    maybe: Option<Str>,
}

#[test]
fn lazy_removed_leaf_faults_preserve_bytes_and_poison() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    let routes = if cfg!(feature = "alloc") { 6 } else { 4 };
    for route in 0..routes {
        for (within, kind) in [
            (0, ErrorKind::InvalidBool),
            (1, ErrorKind::InvalidTag),
            (2, ErrorKind::InvalidUtf8),
        ] {
            let mut bytes = [0; 128];
            RemovedPatch::initialize(
                &mut SliceStore::new(&mut bytes),
                RemovedPatchInit {
                    values: &[
                        RemovedValueInit {
                            flag: true,
                            maybe: Some("a"),
                        },
                        RemovedValueInit {
                            flag: false,
                            maybe: Some("b"),
                        },
                        RemovedValueInit {
                            flag: true,
                            maybe: Some("c"),
                        },
                    ],
                },
            )
            .unwrap();
            let valid = bytes;
            let base = View::load::<RemovedPatch>(&bytes, RemovedPatch::DISCRIMINATOR)
                .unwrap()
                .field(0)
                .unwrap()
                .base() as usize;
            let index = if matches!(route, 2 | 5) { 2 } else { 1 };
            let at = base + 12 + 3 * index + within;
            bytes[at] = if within == 2 { 0xff } else { 2 };
            let before = bytes;
            let mut destination = [0xa5; 8];
            let mut store = SliceStore::new(&mut bytes);
            {
                let mut root = RemovedPatch::load_trusted_mut(&mut store).unwrap();
                let mut list = root.values_mut();
                let result = match route {
                    0 => list.remove(1),
                    1 => list.remove_into(1, &mut destination).map(|_| ()),
                    2 => list.pop().map(|_| ()),
                    // Source validation retains precedence over output capacity.
                    3 => list.remove_into(1, &mut destination[..0]).map(|_| ()),
                    #[cfg(feature = "alloc")]
                    4 => list.remove_owned(1).map(|_| ()),
                    #[cfg(feature = "alloc")]
                    5 => list.pop_owned().map(|_| ()),
                    _ => unreachable!("bounded removal route"),
                };
                assert_eq!(result, Err(Error::new(kind, at as u32)), "route={route}");
                assert_eq!(list.try_len(), Err(ErrorKind::Poisoned.into()));
            }
            assert_eq!(store.bytes(), before);
            assert_eq!(destination, [0xa5; 8]);
            assert!(store.is_poisoned());
            assert!(
                matches!(RemovedPatch::load_trusted_mut(&mut store), Err(error)
                if error == ErrorKind::Poisoned.into())
            );
            assert!(RemovedPatch::load_checked_mut(&mut store).is_err());
            assert!(store.is_poisoned());
            blubber::raw::backend(&mut store)
                .bytes_mut()
                .copy_from_slice(&valid);
            RemovedPatch::load_checked_mut(&mut store)
                .unwrap()
                .verify()
                .unwrap();
            assert!(!store.is_poisoned());
        }
    }
}

fn initial(values: &[&str]) -> [u8; 128] {
    let mut bytes = [0; 128];
    PatchLists::initialize(
        &mut SliceStore::new(&mut bytes),
        PatchListsInit {
            prefix: 73,
            values,
            suffix: "retained suffix",
        },
    )
    .unwrap();
    bytes
}

fn list_base(bytes: &[u8]) -> usize {
    View::load::<PatchLists>(bytes, PatchLists::DISCRIMINATOR)
        .unwrap()
        .field(1)
        .unwrap()
        .base() as usize
}

#[test]
fn lazy_clear_and_truncate_zero_discard_distant_ends_and_keep_siblings() {
    for truncate in [false, true] {
        let mut bytes = initial(&["a", "b", "c"]);
        let base = list_base(&bytes);
        bytes[base + 8..base + 12].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        {
            let result = PatchLists::load_trusted_mut(&mut store);
            if cfg!(feature = "verify-on-load") {
                assert!(result.is_err());
                continue;
            }
            let mut root = result.unwrap();
            if truncate {
                root.values_mut().truncate(0).unwrap();
            } else {
                root.values_mut().clear().unwrap();
            }
            assert_eq!(root.prefix(), 73);
            assert_eq!(root.suffix(), "retained suffix");
            assert_eq!(root.values().len(), 0);
            root.verify().unwrap();
        }
        assert!(!store.is_poisoned());
        assert_ne!(store.bytes(), before);
        PatchLists::load_checked_mut(&mut store)
            .unwrap()
            .verify()
            .unwrap();
    }
}

#[test]
fn lazy_clear_admission_failure_poison_is_sticky_until_checked_repair() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for parent in [false, true] {
        let valid = initial(&["a", "b", "c"]);
        let mut bytes = valid;
        let base = list_base(&bytes);
        let at = if parent { 14 } else { base };
        bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = PatchLists::load_trusted_mut(&mut store).unwrap();
            assert!(root.values_mut().clear().is_err());
            assert_eq!(root.values_mut().try_len(), Err(ErrorKind::Poisoned.into()));
        }
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
        assert!(
            matches!(PatchLists::load_trusted_mut(&mut store), Err(error) if error == ErrorKind::Poisoned.into())
        );
        assert!(PatchLists::load_checked_mut(&mut store).is_err());
        assert!(store.is_poisoned());
        blubber::raw::backend(&mut store)
            .bytes_mut()
            .copy_from_slice(&valid);
        PatchLists::load_checked_mut(&mut store)
            .unwrap()
            .verify()
            .unwrap();
        assert!(!store.is_poisoned());
    }
}

#[test]
fn checked_clear_and_retained_list_routes_keep_the_prewrite_frame() {
    for route in 0..9 {
        for distant in [false, true] {
            let mut bytes = initial(&["a", "b", "c"]);
            let base = list_base(&bytes);
            let list_end = base
                + View::load::<PatchLists>(&bytes, PatchLists::DISCRIMINATOR)
                    .unwrap()
                    .field(1)
                    .unwrap()
                    .bytes()
                    .len();
            let at = base + if distant { 8 } else { 4 };
            bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            let before = bytes;
            let mut path = blubber_core::raw::InlinePath::new();
            path.push(blubber_core::raw::Step::Field(1)).unwrap();
            let operation = match route {
                0 => blubber_core::raw::ListEdit::Insert(0),
                1 => blubber_core::raw::ListEdit::Replace(0),
                2 | 7 => blubber_core::raw::ListEdit::Remove(0),
                3 => blubber_core::raw::ListEdit::Pop,
                4 => blubber_core::raw::ListEdit::Truncate(1),
                8 => blubber_core::raw::ListEdit::Insert(u32::MAX),
                _ => blubber_core::raw::ListEdit::Clear,
            };
            if matches!(route, 5 | 6) {
                assert!(blubber_core::raw::BulkListSource::check(
                    &bytes[base..list_end],
                    <List<Str> as blubber::Schema>::LAYOUT
                )
                .is_err());
            } else {
                let resolved = blubber_core::raw::resolve_account_edit(
                    &mut bytes,
                    PatchLists::DISCRIMINATOR,
                    <PatchLists as blubber::Schema>::MIN_SIZE,
                    <PatchLists as blubber::Schema>::LAYOUT,
                    &path,
                )
                .unwrap();
                assert!(resolved
                    .prepare_list(operation, u32::from(matches!(route, 0 | 1 | 8)), u32::MAX,)
                    .is_err());
            }
            assert_eq!(bytes, before);
            let mut store = SliceStore::new(&mut bytes);
            assert!(PatchLists::load_checked_mut(&mut store).is_err());
            assert_eq!(store.bytes(), before);
            assert!(!store.is_poisoned());
        }
    }
}

#[test]
fn empty_list_loan_rejects_a_wrong_width_without_writes() {
    for len in [0, 1, 3, 4, 5, 8] {
        let mut bytes = [0x51; 8];
        let result = blubber_core::patch_pass::write_empty_list(&mut bytes[..len]);
        if len == 4 {
            assert_eq!(result, Ok(()));
            assert_eq!(&bytes[..4], &[0; 4]);
            assert_eq!(&bytes[4..], &[0x51; 4]);
        } else {
            assert_eq!(result, Err(ErrorKind::InvalidBoundary.into()));
            assert_eq!(bytes, [0x51; 8]);
        }
    }
}

#[test]
fn propagated_patch_error_rolls_back_in_the_host_transaction_model() {
    let original = [1, 0, 0, 0, 255, 255, 255, 255, 12, 0, 0, 0];
    let mut bytes = original;
    let mut poisoned = false;
    let operation = (|| -> blubber::Result<()> {
        blubber_core::patch_pass::patch_word(&mut bytes, 0, 0, 0, u32::MAX, 2, &mut poisoned)?;
        blubber_core::patch_pass::patch_word(&mut bytes, 4, 4, 0, u32::MAX, 1, &mut poisoned)?;
        Ok(())
    })();
    assert_eq!(operation, Err(Error::new(ErrorKind::ArithmeticOverflow, 4)));
    assert_ne!(bytes, original);
    assert!(poisoned);
    if operation.is_err() {
        bytes = original;
        poisoned = false;
    }
    assert_eq!(bytes, original);
    assert!(!poisoned);
}

#[test]
fn late_patch_failure_rejects_typed_reopen_and_keeps_old_used_length() {
    let valid = initial(&["a", "b", "c"]);
    let mut bytes = valid;
    let base = list_base(&bytes);
    bytes[base + 8..base + 12].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut store = SliceStore::new(&mut bytes);
    let mut poisoned = true;
    let result = {
        let backend = blubber::raw::backend(&mut store);
        backend.set_poisoned(true);
        let bytes = backend.bytes_mut();
        blubber_core::patch_pass::patch_word(
            bytes,
            (base + 4) as u32,
            (base + 4) as u32,
            0,
            u32::MAX,
            1,
            &mut poisoned,
        )
        .unwrap();
        blubber_core::patch_pass::patch_word(
            bytes,
            (base + 8) as u32,
            (base + 8) as u32,
            0,
            u32::MAX,
            1,
            &mut poisoned,
        )
    };
    assert_eq!(
        result,
        Err(Error::new(ErrorKind::ArithmeticOverflow, (base + 8) as u32))
    );
    assert_eq!(&store.bytes()[base + 4..base + 8], &2u32.to_le_bytes());
    assert_eq!(&store.bytes()[8..12], &valid[8..12]);
    let result = blubber_core::patch_pass::finish_publication(&mut poisoned, result);
    blubber::raw::backend(&mut store).set_poisoned(poisoned);
    assert!(result.is_err());
    assert!(store.is_poisoned());
    assert!(
        matches!(PatchLists::load_trusted_mut(&mut store), Err(error) if error == ErrorKind::Poisoned.into())
    );
    assert!(PatchLists::load_checked_mut(&mut store).is_err());
    assert!(store.is_poisoned());
    blubber::raw::backend(&mut store)
        .bytes_mut()
        .copy_from_slice(&valid);
    PatchLists::load_checked_mut(&mut store)
        .unwrap()
        .verify()
        .unwrap();
    assert!(!store.is_poisoned());
}

#[test]
fn lazy_list_later_words_fail_with_poison_before_used_publication() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for route in 0..5 {
        let mut bytes = initial(&["a", "bbb", "cc", "dddd", "e", "fffff"]);
        let base = list_base(&bytes);
        let boundary = if matches!(route, 0 | 3) { 2 } else { 4 };
        bytes[base + 4 * boundary..base + 4 * boundary + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = PatchLists::load_trusted_mut(&mut store).unwrap();
            let mut list = root.values_mut();
            let result = match route {
                0 => list.insert(0, "new"),
                1 => list.remove(0),
                2 => list.replace(0, "longer"),
                3 => list.insert_from_slice(0, &["new", "slots"]),
                _ => list.remove_into(0, &mut [0; 8]).map(|_| ()),
            };
            assert!(matches!(
                result,
                Err(Error::Fault {
                    kind: ErrorKind::InvalidBoundary,
                    ..
                })
            ));
            assert_eq!(list.try_len(), Err(ErrorKind::Poisoned.into()));
        }
        assert_ne!(store.bytes(), before);
        assert_eq!(&store.bytes()[8..12], &before[8..12]);
        assert!(store.is_poisoned());
        assert!(
            matches!(PatchLists::load_trusted_mut(&mut store), Err(error) if error == ErrorKind::Poisoned.into())
        );
    }
}

#[test]
fn lazy_selected_span_errors_precede_writes_and_poison() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for route in 0..7 {
        let mut bytes = initial(&["a", "b", "c"]);
        let base = list_base(&bytes);
        let boundary = if matches!(route, 3 | 4 | 6) { 2 } else { 1 };
        bytes[base + 4 * boundary..base + 4 * boundary + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = PatchLists::load_trusted_mut(&mut store).unwrap();
            let mut list = root.values_mut();
            let result = match route {
                0 => list.insert(1, "new"),
                1 => list.remove(0),
                2 => list.replace(0, "new"),
                3 => list.pop().map(|_| ()),
                4 => list.truncate(2),
                5 => list.insert_from_slice(1, &["new"]),
                _ => list.remove_into(2, &mut [0; 8]).map(|_| ()),
            };
            assert!(result.is_err());
        }
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
    }
}

#[test]
fn lazy_pop_truncate_and_extend_leave_unused_ends_unvisited() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for route in 0..4 {
        let mut bytes = initial(&["a", "b", "c", "d", "e", "f"]);
        let base = list_base(&bytes);
        bytes[base + 4..base + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = PatchLists::load_trusted_mut(&mut store).unwrap();
            let mut list = root.values_mut();
            match route {
                0 => list.pop().map(|_| ()).unwrap(),
                1 => list.truncate(3).unwrap(),
                2 => list.extend_from_slice(&["new"]).unwrap(),
                _ => list.insert(6, "new").unwrap(),
            }
        }
        assert!(!store.is_poisoned());
        assert!(PatchLists::load_checked_mut(&mut store).is_err());
    }
}

#[cfg(feature = "alloc")]
#[test]
fn detached_append_leaves_unused_ends_unvisited() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for route in 0..3 {
        let initial = initial(&["a", "b", "c", "d", "e", "f"]);
        let used = blubber_core::read_u32(&initial, 8).unwrap() as usize;
        let mut bytes = initial[..used].to_vec();
        let base = list_base(&bytes);
        bytes[base + 4..base + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut store = blubber::VecStore::new(&mut bytes);
        let input = "x".repeat(256);
        {
            let mut root = PatchLists::load_trusted_mut(&mut store).unwrap();
            let mut list = root.values_mut();
            match route {
                0 => list.extend_from_slice(&[input.as_str()]),
                1 => list.insert_from_slice(6, &[input.as_str()]),
                _ => list.push(input.as_str()),
            }
            .unwrap();
            assert_eq!(list.len(), 7);
            assert_eq!(list.get(6), Some(input.as_str()));
            assert_eq!(root.prefix(), 73);
            assert_eq!(root.suffix(), "retained suffix");
        }
        assert!(store.bytes().len() > used);
        assert_eq!(
            blubber_core::read_u32(store.bytes(), 8).unwrap() as usize,
            store.bytes().len()
        );
        assert_eq!(&store.bytes()[base + 4..base + 8], &u32::MAX.to_le_bytes());
        assert!(!store.is_poisoned());
        assert!(PatchLists::load_checked_mut(&mut store).is_err());
        assert!(!store.is_poisoned());
    }
}

#[cfg(feature = "alloc")]
#[test]
fn detached_list_growth_publishes_only_after_complete_success() {
    for bulk in [false, true] {
        for invalid in [false, true] {
            if invalid && cfg!(feature = "verify-on-load") {
                continue;
            }
            let valid = initial(&["a", "b", "c", "d", "e", "f"]);
            let used = blubber_core::read_u32(&valid, 8).unwrap() as usize;
            let mut bytes = valid[..used].to_vec();
            let base = list_base(&bytes);
            if invalid {
                bytes[base + 8..base + 12].copy_from_slice(&u32::MAX.to_le_bytes());
            }
            let before = bytes.clone();
            let mut store = blubber::VecStore::new(&mut bytes);
            let result = {
                let mut root = PatchLists::load_trusted_mut(&mut store).unwrap();
                if bulk {
                    root.values_mut().insert_from_slice(0, &["new", "slots"])
                } else {
                    root.values_mut().insert(0, "new")
                }
            };
            assert_eq!(result.is_err(), invalid);
            assert_eq!(store.is_poisoned(), invalid);
            assert!(store.bytes().len() > before.len());
            if invalid {
                assert_eq!(&store.bytes()[8..12], &before[8..12]);
                assert!(
                    matches!(PatchLists::load_trusted_mut(&mut store), Err(error) if error == ErrorKind::Poisoned.into())
                );
                // A propagated transaction error restores account data and session state.
                bytes = before;
                let store = blubber::VecStore::new(&mut bytes);
                assert!(!store.is_poisoned());
                assert_eq!(
                    blubber_core::read_u32(store.bytes(), 8).unwrap() as usize,
                    used
                );
            } else {
                PatchLists::load_checked_mut(&mut store)
                    .unwrap()
                    .verify()
                    .unwrap();
            }
        }
    }
}

#[blubber::schema(account)]
struct TailPatch {
    prefix: u16,
    values: List<Str>,
}

#[test]
fn small_tail_lists_skip_stationary_movement_and_publish_correctly() {
    for values in [&[][..], &["a"][..], &["a", "bbb"][..]] {
        for route in 0..4 {
            let mut expected = values.to_vec();
            let mut bytes = [0; 128];
            let mut store = SliceStore::new(&mut bytes);
            TailPatch::initialize(&mut store, TailPatchInit { prefix: 73, values }).unwrap();
            {
                let mut root = TailPatch::load_trusted_mut(&mut store).unwrap();
                let mut list = root.values_mut();
                match route {
                    0 if !values.is_empty() => {
                        list.replace(0, "longer replacement").unwrap();
                        expected[0] = "longer replacement";
                    }
                    0 => {
                        list.insert(0, "new").unwrap();
                        expected.push("new");
                    }
                    1 => {
                        assert_eq!(list.pop().unwrap().is_some(), expected.pop().is_some());
                    }
                    2 => {
                        list.truncate(1).unwrap();
                        expected.truncate(1);
                    }
                    _ if !values.is_empty() => {
                        list.remove(0).unwrap();
                        expected.remove(0);
                    }
                    _ => {
                        assert_eq!(list.pop(), Ok(None));
                    }
                }
                root.verify().unwrap();
                assert_eq!(root.prefix(), 73);
                assert_eq!(root.values().iter().collect::<Vec<_>>(), expected);
            }
            assert!(!store.is_poisoned());
            TailPatch::load_checked_mut(&mut store)
                .unwrap()
                .verify()
                .unwrap();
        }
    }
}
