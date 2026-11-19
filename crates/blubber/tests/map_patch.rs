//! Lazy map patch frames and visited-only walk admission.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::{Error, ErrorKind, Map, ReadStorage, Resize, SliceStore, Str, View};

#[blubber::schema(account)]
struct PatchMaps {
    prefix: u16,
    values: Map<u8, Str>,
    suffix: Str,
}

fn initial() -> [u8; 256] {
    let mut bytes = [0; 256];
    PatchMaps::initialize(
        &mut SliceStore::new(&mut bytes),
        PatchMapsInit {
            prefix: 73,
            values: &[
                (2, "a"),
                (4, "b"),
                (6, "c"),
                (8, "d"),
                (10, "e"),
                (12, "f"),
                (14, "g"),
                (16, "h"),
            ],
            suffix: "retained suffix",
        },
    )
    .unwrap();
    bytes
}

fn map_base(bytes: &[u8]) -> usize {
    View::load::<PatchMaps>(bytes, PatchMaps::DISCRIMINATOR)
        .unwrap()
        .field(1)
        .unwrap()
        .base() as usize
}

#[test]
fn lazy_map_later_patch_fault_keeps_poison_and_old_used_length() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for route in 0..4 {
        for end in [0u32, u32::MAX] {
            let mut bytes = initial();
            let valid = bytes;
            let base = map_base(&bytes);
            bytes[base + 5 + 6 * 5..base + 9 + 6 * 5].copy_from_slice(&end.to_le_bytes());
            let before = bytes;
            let mut destination = [0xa5; 8];
            let mut store = SliceStore::new(&mut bytes);
            {
                let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
                let mut map = root.values_mut();
                let result = match route {
                    0 => map.insert(5, "long input"),
                    1 => map.insert(4, "replacement"),
                    2 => map.remove(&4),
                    _ => map
                        .remove_into(&4, &mut destination)
                        .map(|value| value.map(|_| ())),
                };
                assert!(matches!(
                    result,
                    Err(Error::Fault {
                        kind: ErrorKind::InvalidBoundary,
                        ..
                    })
                ));
                assert_eq!(map.try_len(), Err(ErrorKind::Poisoned.into()));
            }
            if route == 3 {
                assert_eq!(destination[0], b'b');
                assert_eq!(&destination[1..], &[0xa5; 7]);
            } else {
                assert_eq!(destination, [0xa5; 8]);
            }
            assert!(store.is_poisoned());
            assert_ne!(store.bytes(), before);
            assert_eq!(&store.bytes()[8..12], &before[8..12]);
            assert!(
                matches!(PatchMaps::load_trusted_mut(&mut store), Err(error) if error == ErrorKind::Poisoned.into())
            );
            assert!(PatchMaps::load_checked_mut(&mut store).is_err());
            assert!(store.is_poisoned());
            blubber::raw::backend(&mut store)
                .bytes_mut()
                .copy_from_slice(&valid);
            PatchMaps::load_checked_mut(&mut store)
                .unwrap()
                .verify()
                .unwrap();
            assert!(!store.is_poisoned());
        }
    }
}

#[test]
fn map_clear_discards_unvisited_ends_and_preserves_siblings() {
    let mut bytes = initial();
    let base = map_base(&bytes);
    bytes[base + 5 + 6 * 5..base + 9 + 6 * 5].copy_from_slice(&u32::MAX.to_le_bytes());
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    if cfg!(feature = "verify-on-load") {
        assert!(PatchMaps::load_trusted_mut(&mut store).is_err());
        assert_eq!(store.bytes(), before);
        return;
    }
    let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
    root.values_mut().clear().unwrap();
    assert_eq!(root.prefix(), 73);
    assert_eq!(root.suffix(), "retained suffix");
    assert_eq!(root.values().len(), 0);
    root.validate().unwrap();
}

#[test]
fn selected_map_fault_and_capacity_rejection_precede_writes() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for route in 0..3 {
        let mut bytes = initial();
        let base = map_base(&bytes);
        if route != 2 {
            bytes[base + 10..base + 14].copy_from_slice(&u32::MAX.to_le_bytes());
        }
        let before = bytes;
        let mut output = [0xa5; 8];
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
            let result = match route {
                0 => root.values_mut().insert(4, "replacement").map(|_| ()),
                1 => root.values_mut().remove_into(&4, &mut output).map(|_| ()),
                _ => root.values_mut().insert(5, &"x".repeat(256)).map(|_| ()),
            };
            let kind = if route == 2 {
                ErrorKind::Capacity
            } else {
                ErrorKind::InvalidBoundary
            };
            assert!(matches!(result, Err(Error::Fault { kind: actual, .. }) if actual == kind));
        }
        assert_eq!(store.bytes(), before);
        assert_eq!(output, [0xa5; 8]);
        assert_eq!(store.is_poisoned(), route != 2);
    }
}

#[test]
fn lazy_map_walk_checks_only_visited_keys_and_adjacent_ends() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for damage in 0..3 {
        let mut bytes = initial();
        let base = map_base(&bytes);
        let at = if damage == 2 {
            base + 5 + 6 * 5
        } else {
            base + 4 + 6 * 5
        };
        match damage {
            0 => bytes[at] = 3,
            1 => bytes[at] = 12,
            _ => bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes()),
        }
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
            let mut cursor = root.values_mut().cursor_mut().unwrap();
            for expected in [2, 4, 6, 8, 10, 12] {
                let (key, value) = cursor.next().unwrap().unwrap();
                assert_eq!(key, expected);
                assert_eq!(value.try_get().unwrap().len(), 1);
            }
            let kind = match damage {
                0 => ErrorKind::UnsortedKeys,
                1 => ErrorKind::DuplicateKey,
                _ => ErrorKind::InvalidBoundary,
            };
            assert!(
                matches!(cursor.next(), Err(Error::Fault { kind: actual, offset }) if actual == kind && offset == at as u32)
            );
        }
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
    }
}

#[test]
fn propagated_map_patch_error_restores_the_host_transaction_snapshot() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    let mut bytes = initial();
    let base = map_base(&bytes);
    bytes[base + 5 + 6 * 5..base + 9 + 6 * 5].copy_from_slice(&u32::MAX.to_le_bytes());
    let before = bytes;
    let result = {
        let mut store = SliceStore::new(&mut bytes);
        let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
        root.values_mut().insert(5, "replacement")
    };
    assert!(result.is_err());
    assert_ne!(bytes, before);
    if result.is_err() {
        bytes = before;
    }
    assert_eq!(bytes, before);
}

#[test]
fn lazy_map_edits_preserve_unvisited_distant_keys() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for route in 0..3 {
        for key in [11, 12] {
            let mut bytes = initial();
            let base = map_base(&bytes);
            bytes[base + 4 + 6 * 5] = key;
            let mut store = SliceStore::new(&mut bytes);
            let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
            match route {
                0 => {
                    root.values_mut().insert(5, "inserted").unwrap();
                }
                1 => {
                    root.values_mut().insert(4, "replacement").unwrap();
                }
                _ => {
                    root.values_mut().remove(&4).unwrap();
                }
            }
            assert_eq!(root.suffix(), "retained suffix");
            assert!(root.validate().is_err());
            assert!(!store.is_poisoned());
        }
    }
}

#[test]
fn map_removed_value_validation_precedes_export_capacity() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    let mut bytes = initial();
    let base = map_base(&bytes);
    let at = base + 4 + 8 * 5 + 1;
    bytes[at] = 0xff;
    let before = bytes;
    let mut output = [0xa5; 8];
    let mut store = SliceStore::new(&mut bytes);
    {
        let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
        assert_eq!(
            root.values_mut().remove_into(&4, &mut output[..0]),
            Err(Error::new(ErrorKind::InvalidUtf8, at as u32))
        );
    }
    assert_eq!(store.bytes(), before);
    assert_eq!(output, [0xa5; 8]);
    assert!(store.is_poisoned());
}

#[test]
fn valid_lazy_map_splices_match_checked_whole_bytes() {
    for route in 0..6 {
        let execute = |checked| {
            let mut bytes = initial();
            let mut store = SliceStore::new(&mut bytes);
            let mut root = if checked {
                PatchMaps::load_checked_mut(&mut store)
            } else {
                PatchMaps::load_trusted_mut(&mut store)
            }
            .unwrap();
            match route {
                0 => {
                    root.values_mut().insert(5, "inserted").unwrap();
                }
                1 => {
                    root.values_mut().insert(4, "replacement").unwrap();
                }
                2 => {
                    root.values_mut().remove(&4).unwrap();
                }
                3 => {
                    root.values_mut().clear().unwrap();
                }
                4 => match root.values_mut().into_entry(4).unwrap() {
                    blubber::Entry::Occupied(entry) => entry.insert("replacement").unwrap(),
                    _ => panic!("existing key"),
                },
                _ => {
                    let mut output = [0xa5; 8];
                    assert_eq!(root.values_mut().remove_into(&4, &mut output), Ok(Some(1)));
                    assert_eq!(output[0], b'b');
                    assert_eq!(&output[1..], &[0xa5; 7]);
                }
            }
            root.validate().unwrap();
            assert_eq!(root.suffix(), "retained suffix");
            bytes
        };
        assert_eq!(execute(false), execute(true));
    }
}

#[test]
fn walk_key_admission_handles_full_width_coordinates_without_panics() {
    let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
    for at in [0, 1, 7, 8, usize::MAX] {
        for width in [0, 1, 8, usize::MAX] {
            for stride in [0, 1, 5, usize::MAX] {
                for first in [false, true] {
                    let expected = at
                        .checked_add(width)
                        .filter(|&end| end <= bytes.len())
                        .is_some_and(|end| {
                            first
                                || (width <= stride
                                    && stride <= at
                                    && bytes[at - stride..at - stride + width] < bytes[at..end])
                        });
                    let result =
                        blubber_core::raw::indexed_map_key(&bytes, at, width, stride, first);
                    assert_eq!(
                        result.is_ok(),
                        expected,
                        "at={at} width={width} stride={stride} first={first}"
                    );
                }
            }
        }
    }
}

#[blubber::schema(account)]
struct WidePatch {
    values: Map<blubber::Pubkey, Str>,
}

#[test]
fn wide_keys_use_the_same_checked_transport_and_late_fault_frame() {
    let keys: [blubber::Pubkey; 8] = core::array::from_fn(|i| {
        let mut bytes = [0; 32];
        bytes[0] = 2 * (i as u8 + 1);
        blubber::Pubkey::from_bytes(bytes)
    });
    let entries = keys
        .iter()
        .copied()
        .map(|key| (key, "x"))
        .collect::<Vec<_>>();
    for fault in [false, true] {
        let mut bytes = [0; 1024];
        WidePatch::initialize(
            &mut SliceStore::new(&mut bytes),
            WidePatchInit { values: &entries },
        )
        .unwrap();
        let base = View::load::<WidePatch>(&bytes, WidePatch::DISCRIMINATOR)
            .unwrap()
            .field(0)
            .unwrap()
            .base() as usize;
        if fault {
            bytes[base + 4 + 6 * 36 + 32..base + 4 + 7 * 36]
                .copy_from_slice(&u32::MAX.to_le_bytes());
        }
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        if fault && cfg!(feature = "verify-on-load") {
            assert!(WidePatch::load_trusted_mut(&mut store).is_err());
            continue;
        }
        {
            let mut root = WidePatch::load_trusted_mut(&mut store).unwrap();
            let result = root.values_mut().insert(keys[1], "replacement");
            if fault {
                assert!(result.is_err());
            } else {
                result.unwrap();
                root.values_mut().remove(&keys[0]).unwrap();
                root.validate().unwrap();
            }
        }
        assert_eq!(store.is_poisoned(), fault);
        if fault {
            assert_eq!(&store.bytes()[8..12], &before[8..12]);
            assert_ne!(store.bytes(), before);
        }
    }
}

#[test]
fn raw_map_caller_key_errors_leave_bytes_and_poison_unchanged() {
    let layout = blubber_core::Layout::Map {
        key: blubber_core::KeyLayout {
            width: 1,
            domain: blubber_core::KeyDomain::Bool,
        },
        value: &blubber_core::Layout::Str,
    };
    for key in [&[][..], &[2u8][..]] {
        let mut bytes = [0; 64];
        bytes[..8].copy_from_slice(b"mappatch");
        bytes[8..12].copy_from_slice(&22u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
        bytes[17..21].copy_from_slice(&1u32.to_le_bytes());
        bytes[21] = b'a';
        let before = bytes;
        let mut poisoned = false;
        {
            let resolved = blubber_core::raw::resolve_edit(
                &mut bytes,
                22,
                &layout,
                &blubber_core::raw::InlinePath::new(),
            )
            .unwrap();
            let result = resolved.prepare_map_checked(
                blubber_core::raw::MapEdit::Insert,
                key,
                1,
                u32::MAX,
                None,
                &mut poisoned,
            );
            let expected = if key.is_empty() {
                ErrorKind::InvalidBoundary
            } else {
                ErrorKind::InvalidBool
            };
            assert!(
                matches!(result,Err(Error::Fault {kind,offset}) if kind==expected && offset==Error::NO_OFFSET)
            );
        }
        assert_eq!(bytes, before);
        assert!(!poisoned);
    }
}

#[test]
fn raw_map_full_width_growth_rejections_preserve_source() {
    let layout = blubber_core::Layout::Map {
        key: blubber_core::KeyLayout {
            width: 1,
            domain: blubber_core::KeyDomain::Raw,
        },
        value: &blubber_core::Layout::Str,
    };
    for input in [u32::MAX, u32::MAX - 1, u32::MAX - 40] {
        let mut bytes = [0; 64];
        bytes[..8].copy_from_slice(b"mappatch");
        bytes[8..12].copy_from_slice(&22u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
        bytes[16] = 2;
        bytes[17..21].copy_from_slice(&1u32.to_le_bytes());
        bytes[21] = b'a';
        let before = bytes;
        let mut poisoned = false;
        {
            let resolved = blubber_core::raw::resolve_edit(
                &mut bytes,
                22,
                &layout,
                &blubber_core::raw::InlinePath::new(),
            )
            .unwrap();
            let result = resolved.prepare_map_checked(
                blubber_core::raw::MapEdit::Insert,
                &[2],
                input,
                u32::MAX,
                None,
                &mut poisoned,
            );
            match result {
                Err(Error::Fault {
                    kind: ErrorKind::ArithmeticOverflow,
                    ..
                }) => (),
                Ok(prepared) => assert!(matches!(
                    prepared.begin_immediate(),
                    Err(Error::Fault {
                        kind: ErrorKind::Capacity,
                        ..
                    })
                )),
                _ => panic!("growth must fail before writes"),
            }
        }
        assert_eq!(bytes, before);
        assert!(!poisoned);
    }
}

#[cfg(feature = "alloc")]
#[test]
fn detached_map_growth_publishes_last_and_keeps_late_fault_poison() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for invalid in [false, true] {
        let original = initial();
        let old_used = blubber_core::read_u32(&original, 8).unwrap();
        let mut bytes = original[..old_used as usize].to_vec();
        let base = map_base(&bytes);
        if invalid {
            bytes[base + 5 + 6 * 5..base + 9 + 6 * 5].copy_from_slice(&u32::MAX.to_le_bytes());
        }
        let text = [b'x'; 400];
        let text = core::str::from_utf8(&text).unwrap();
        let mut store = blubber::VecStore::new(&mut bytes);
        let result = {
            let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
            root.values_mut().insert(5, text)
        };
        assert!(store.storage_len() > old_used as usize);
        if invalid {
            assert!(matches!(
                result,
                Err(Error::Fault {
                    kind: ErrorKind::InvalidBoundary,
                    ..
                })
            ));
            assert!(store.is_poisoned());
            assert_eq!(blubber_core::read_u32(store.bytes(), 8).unwrap(), old_used);
            assert!(PatchMaps::load_trusted_mut(&mut store).is_err());
        } else {
            result.unwrap();
            assert!(!store.is_poisoned());
            let root = PatchMaps::load(store.bytes()).unwrap();
            root.validate().unwrap();
            assert_eq!(root.prefix(), 73);
            assert_eq!(root.suffix(), "retained suffix");
            assert!(blubber_core::read_u32(store.bytes(), 8).unwrap() > old_used);
        }
    }
}

#[test]
fn map_entry_hints_use_the_same_later_word_failure_frame() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for route in 0..3 {
        let mut bytes = initial();
        let base = map_base(&bytes);
        let old_used = blubber_core::read_u32(&bytes, 8).unwrap();
        bytes[base + 5 + 6 * 5..base + 9 + 6 * 5].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
            let mut map = root.values_mut();
            let result = match (route, map.entry(if route == 0 { 5 } else { 4 }).unwrap()) {
                (0, blubber::Entry::Vacant(entry)) => entry.insert("new value").map(|_| ()),
                (1, blubber::Entry::Occupied(entry)) => entry.insert("replacement"),
                (2, blubber::Entry::Occupied(entry)) => entry.remove(),
                _ => panic!("matching presence"),
            };
            assert!(matches!(
                result,
                Err(Error::Fault {
                    kind: ErrorKind::InvalidBoundary,
                    ..
                })
            ));
        }
        assert!(store.is_poisoned());
        assert_eq!(blubber_core::read_u32(store.bytes(), 8).unwrap(), old_used);
        assert!(PatchMaps::load_trusted_mut(&mut store).is_err());
    }
}

#[cfg(feature = "alloc")]
#[test]
fn owned_map_edits_share_the_later_word_failure_and_poison() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for insert in [false, true] {
        let mut bytes = initial();
        let base = map_base(&bytes);
        let old_used = blubber_core::read_u32(&bytes, 8).unwrap();
        bytes[base + 5 + 6 * 5..base + 9 + 6 * 5].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
            let mut map = root.values_mut();
            let result = if insert {
                map.insert_owned(5, &"new value".to_owned()).map(|_| ())
            } else {
                map.remove_owned(&4).map(|_| ())
            };
            assert!(matches!(
                result,
                Err(Error::Fault {
                    kind: ErrorKind::InvalidBoundary,
                    ..
                })
            ));
        }
        assert!(store.is_poisoned());
        assert_eq!(blubber_core::read_u32(store.bytes(), 8).unwrap(), old_used);
        assert!(PatchMaps::load_trusted_mut(&mut store).is_err());
    }
}

#[test]
fn map_callback_walk_does_not_read_keys_after_a_propagated_callback_error() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    for stop_early in [false, true] {
        let mut bytes = initial();
        let base = map_base(&bytes);
        bytes[base + 4 + 6 * 5] = 3;
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let mut visited = 0;
        let result = {
            let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
            root.values_mut().try_for_each_mut(|key, value| {
                visited += 1;
                assert_eq!(key, 2 * visited);
                assert_eq!(value.try_get()?.len(), 1);
                if stop_early {
                    Err(ErrorKind::Capacity.into())
                } else {
                    Ok(())
                }
            })
        };
        assert_eq!(store.bytes(), before);
        if stop_early {
            assert_eq!(visited, 1);
            assert_eq!(result, Err(ErrorKind::Capacity.into()));
            assert!(!store.is_poisoned());
        } else {
            assert_eq!(visited, 6);
            assert!(matches!(
                result,
                Err(Error::Fault {
                    kind: ErrorKind::UnsortedKeys,
                    ..
                })
            ));
            assert!(store.is_poisoned());
        }
    }
}

#[test]
fn detached_raw_map_rechecks_distinguish_source_faults_from_stale_hints() {
    let layout = blubber_core::Layout::Map {
        key: blubber_core::KeyLayout {
            width: 1,
            domain: blubber_core::KeyDomain::Raw,
        },
        value: &blubber_core::Layout::Str,
    };
    for source_fault in [false, true] {
        let mut bytes = [0; 64];
        bytes[..8].copy_from_slice(b"mappatch");
        bytes[8..12].copy_from_slice(&34u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&3u32.to_le_bytes());
        for (index, key) in [2u8, 4, 6].into_iter().enumerate() {
            let at = 16 + 5 * index;
            bytes[at] = key;
            bytes[at + 1..at + 5].copy_from_slice(&(index as u32 + 1).to_le_bytes());
        }
        bytes[31..34].copy_from_slice(b"abc");
        let mut poisoned = false;
        let resolved = blubber_core::raw::resolve_account_edit(
            &mut bytes,
            *b"mappatch",
            4,
            &layout,
            &blubber_core::raw::InlinePath::new(),
        )
        .unwrap();
        let plan = resolved
            .prepare_map_checked(
                blubber_core::raw::MapEdit::Insert,
                &[1],
                1,
                64,
                None,
                &mut poisoned,
            )
            .unwrap()
            .detach();
        assert!(!poisoned);
        if source_fault {
            bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        } else {
            bytes[16] = 1;
        }
        let before = bytes;
        let error = plan.begin(&mut bytes, &mut poisoned).err().unwrap();
        if source_fault {
            assert!(matches!(
                error,
                Error::Fault {
                    kind: ErrorKind::ArithmeticOverflow | ErrorKind::InvalidBoundary,
                    ..
                }
            ));
        } else {
            assert_eq!(error, ErrorKind::InvalidPlan.into());
        }
        assert_eq!(bytes, before);
        assert_eq!(poisoned, source_fault);
    }
}

#[test]
fn lazy_map_walk_open_fault_sets_poison_without_writes() {
    if cfg!(feature = "verify-on-load") {
        return;
    }
    let mut bytes = initial();
    let base = map_base(&bytes);
    let count = blubber_core::read_u32(&bytes, base as u32).unwrap() as usize;
    let terminal = base + count * 5;
    bytes[terminal..terminal + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    {
        let mut root = PatchMaps::load_trusted_mut(&mut store).unwrap();
        assert!(matches!(
            root.values_mut().cursor_mut(),
            Err(Error::Fault {
                kind: ErrorKind::InvalidBoundary,
                ..
            })
        ));
    }
    assert_eq!(store.bytes(), before);
    assert!(store.is_poisoned());
}
