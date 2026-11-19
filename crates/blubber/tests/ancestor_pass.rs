//! Forward ancestor publication agrees with strict read-only diagnostics.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]
use blubber::{RootSchema, Schema};
use blubber_core::raw::{InlinePath, Step};

#[blubber::schema(account)]
struct Ancestors {
    values: blubber::List<blubber::Str>,
    suffix: blubber::Str,
}

fn fixture() -> Vec<u8> {
    let mut bytes = vec![0; 160];
    let mut store = blubber::SliceStore::new(&mut bytes);
    Ancestors::initialize(
        &mut store,
        AncestorsInit {
            values: &["abcdefgh"; 6],
            suffix: "tail",
        },
    )
    .unwrap();
    bytes
}
fn path() -> InlinePath {
    let mut path = InlinePath::new();
    path.push(Step::Field(0)).unwrap();
    path.push(Step::Index(0)).unwrap();
    path
}

#[test]
fn later_ancestor_errors_match_read_only_preflight() {
    for input in [&b"replacement grows"[..], &b"abc"[..]] {
        for word in [3usize, 4] {
            for end in [u32::MAX, 0] {
                let mut old = fixture();
                let at = 20 + word * 4;
                old[at..at + 4].copy_from_slice(&end.to_le_bytes());
                let before = old.clone();
                let mut candidate = old.clone();
                let expected = blubber_core::raw::prepare_byte_replacement(
                    &mut old,
                    Ancestors::DISCRIMINATOR,
                    Ancestors::MIN_SIZE,
                    Ancestors::__layout(),
                    &path(),
                    input.len(),
                )
                .map(|_| ());
                assert_eq!(old, before);
                let mut poison = false;
                let actual = blubber_core::raw::prepare_checked_byte_replacement(
                    &mut candidate,
                    Ancestors::DISCRIMINATOR,
                    Ancestors::MIN_SIZE,
                    Ancestors::__layout(),
                    &path(),
                    input.len(),
                    &mut poison,
                )
                .and_then(|write| {
                    let mut write = write.move_regions(&mut poison).destination();
                    write.bytes().copy_from_slice(input);
                    write.finish(&mut poison)
                });
                assert_eq!(
                    actual,
                    expected,
                    "word={word} end={end} len={}",
                    input.len()
                );
                if actual.is_err() {
                    assert!(poison);
                    let first = u32::from_le_bytes(candidate[20..24].try_into().unwrap());
                    assert_eq!(first, input.len() as u32);
                }
            }
        }
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn typed_later_failure_poison_survives_reopen() {
    use blubber::{ReadStorage as _, Resize as _};
    for input in ["replacement grows", "abc"] {
        for word in [3usize, 4] {
            let mut bytes = fixture();
            let at = 20 + word * 4;
            bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            let before = bytes.clone();
            let mut store = blubber::SliceStore::new(&mut bytes);
            let result = Ancestors::load_trusted_mut(&mut store)
                .unwrap()
                .values_mut()
                .get_mut(0)
                .unwrap()
                .unwrap()
                .set(input);
            assert_eq!(
                result,
                Err(blubber::Error::new(
                    blubber::ErrorKind::InvalidBoundary,
                    at as u32
                ))
            );
            assert!(store.is_poisoned());
            assert_eq!(
                Ancestors::load_trusted_mut(&mut store).err().unwrap(),
                blubber::ErrorKind::Poisoned.into()
            );
            assert_eq!(&store.bytes()[..12], &before[..12]);
            let old_used = u32::from_le_bytes(before[8..12].try_into().unwrap()) as usize;
            let new_used = (old_used as i64 + input.len() as i64 - 8) as usize;
            assert_eq!(
                &store.bytes()[old_used.max(new_used)..],
                &before[old_used.max(new_used)..]
            );
        }
    }
}

#[test]
fn immediate_resolved_and_detached_routes_match_old_physical_result() {
    use blubber_core::raw::{begin_apply, plan_replace, resolve_account_edit};
    let layout = blubber_core::Layout::Flat {
        checked: Ancestors::__layout(),
    };
    // The last child leaves an empty inner run after a nonempty outer run.
    for index in [0, 5] {
        let mut selected = InlinePath::new();
        selected.push(Step::Field(0)).unwrap();
        selected.push(Step::Index(index)).unwrap();
        for input in [&b"replacement grows"[..], &b"new-text"[..], &b"abc"[..]] {
            for route in 0..3 {
                let mut bytes = fixture();
                let old_used = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
                if route == 2 {
                    bytes.truncate(old_used as usize);
                }
                let mut expected = bytes.clone();
                let plan = plan_replace(
                    &expected,
                    old_used,
                    &layout,
                    &selected,
                    input.len() as u32,
                    u32::MAX,
                )
                .unwrap();
                let required = plan.required_storage_len() as usize;
                if route == 2 {
                    expected.resize(required, 0);
                }
                let mut apply = begin_apply(&mut expected, plan).unwrap();
                apply.move_regions();
                apply.destination().copy_from_slice(input);
                apply.finish();
                let mut poison = false;
                match route {
                    0 => {
                        let write = blubber_core::raw::prepare_checked_byte_replacement(
                            &mut bytes,
                            Ancestors::DISCRIMINATOR,
                            Ancestors::MIN_SIZE,
                            Ancestors::__layout(),
                            &selected,
                            input.len(),
                            &mut poison,
                        )
                        .unwrap();
                        let mut write = write.move_regions(&mut poison).destination();
                        write.bytes().copy_from_slice(input);
                        write.finish(&mut poison).unwrap();
                    }
                    1 => {
                        let resolved = resolve_account_edit(
                            &mut bytes,
                            Ancestors::DISCRIMINATOR,
                            Ancestors::MIN_SIZE,
                            &layout,
                            &selected,
                        )
                        .unwrap();
                        let write = resolved
                            .prepare_subtree_checked(input.len() as u32, &mut poison)
                            .unwrap();
                        let mut write = write.move_regions(&mut poison).destination();
                        write.bytes().copy_from_slice(input);
                        write.finish(&mut poison).unwrap();
                    }
                    _ => {
                        let resolved = resolve_account_edit(
                            &mut bytes,
                            Ancestors::DISCRIMINATOR,
                            Ancestors::MIN_SIZE,
                            &layout,
                            &selected,
                        )
                        .unwrap();
                        let plan = resolved.defer_subtree(input.len() as u32).unwrap();
                        bytes.resize(required, 0);
                        let write = plan.begin(&mut bytes, &mut poison).unwrap();
                        let mut write = write.move_regions(&mut poison).destination();
                        write.bytes().copy_from_slice(input);
                        write.finish(&mut poison).unwrap();
                    }
                }
                assert!(!poison);
                assert_eq!(bytes, expected, "route={route} len={}", input.len());
            }
        }
    }
}

#[blubber::schema]
struct NumberGroup {
    values: blubber::List<u8>,
}
#[blubber::schema(account)]
struct NumberGroups {
    groups: blubber::List<NumberGroup>,
    suffix: blubber::Str,
}
#[blubber::schema]
struct MapGroup {
    values: blubber::Map<u8, u8>,
}
#[blubber::schema(account)]
struct MapGroups {
    groups: blubber::List<MapGroup>,
    suffix: blubber::Str,
}
#[blubber::schema]
struct StringGroup {
    values: blubber::List<blubber::Str>,
}
#[blubber::schema(account)]
struct StringGroups {
    groups: blubber::List<StringGroup>,
    suffix: blubber::Str,
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn nested_fixed_list_map_and_indexed_list_share_late_ancestor_rejection() {
    use blubber::{ReadStorage as _, Resize as _};
    use blubber_core::raw::{begin_apply, plan_list_edit, plan_map_edit, ListEdit, MapEdit};
    macro_rules! route {
        ($account:ident,$init:ident,$row:expr,$key:expr,$value:expr,$map:expr,$encoded:expr,$remove:expr) => {
            for grow in [true, false] {
                for bad in [None, Some(3usize), Some(4usize)] {
                    let rows = (0..6).map(|_| $row).collect::<Vec<_>>();
                    let mut bytes = vec![0; 256];
                    $account::initialize(
                        &mut blubber::SliceStore::new(&mut bytes),
                        $init {
                            groups: &rows,
                            suffix: "tail",
                        },
                    )
                    .unwrap();
                    if let Some(word) = bad {
                        let at = 20 + word * 4;
                        bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
                    }
                    let before = bytes.clone();
                    let mut expected = before.clone();
                    let mut selected = InlinePath::new();
                    selected.push(Step::Field(0)).unwrap();
                    selected.push(Step::Index(0)).unwrap();
                    selected.push(Step::Field(0)).unwrap();
                    let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
                    let layout = blubber_core::Layout::Flat {
                        checked: $account::__layout(),
                    };
                    let query = [if grow { 2u8 } else { 1 }];
                    let plan = if $map {
                        plan_map_edit(
                            &expected,
                            used,
                            &layout,
                            &selected,
                            if grow {
                                MapEdit::Insert
                            } else {
                                MapEdit::Remove
                            },
                            &query,
                            if grow { 1 } else { 0 },
                            u32::MAX,
                        )
                    } else {
                        plan_list_edit(
                            &expected,
                            used,
                            &layout,
                            &selected,
                            if grow {
                                ListEdit::Insert(0)
                            } else {
                                ListEdit::Remove(0)
                            },
                            if grow { 1 } else { 0 },
                            u32::MAX,
                        )
                    };
                    let strict = plan.map(|plan| {
                        let mut apply = begin_apply(&mut expected, plan).unwrap();
                        apply.move_regions();
                        if grow {
                            apply.destination().copy_from_slice(&[$encoded]);
                        } else {
                            assert!(apply.destination().is_empty());
                        }
                        apply.finish();
                    });
                    let mut store = blubber::SliceStore::new(&mut bytes);
                    let actual = {
                        let mut root = $account::load_trusted_mut(&mut store).unwrap();
                        let groups = root.groups_mut();
                        let mut group = groups.get_mut(0).unwrap().unwrap();
                        let mut selected = group.values_mut();
                        if grow {
                            selected.insert($key, $value).map(|_| ())
                        } else {
                            selected.remove($remove).map(|_| ())
                        }
                    };
                    assert_eq!(actual, strict);
                    if let Some(word) = bad {
                        assert!(store.is_poisoned());
                        assert_eq!(
                            actual,
                            Err(blubber::Error::new(
                                blubber::ErrorKind::InvalidBoundary,
                                (20 + word * 4) as u32
                            ))
                        );
                        assert_eq!(&store.bytes()[..12], &before[..12]);
                        assert_ne!(&store.bytes()[12..16], &before[12..16]);
                        assert_eq!(
                            $account::load_trusted_mut(&mut store).err().unwrap(),
                            blubber::ErrorKind::Poisoned.into()
                        );
                    } else {
                        assert!(!store.is_poisoned());
                        assert_eq!(store.bytes(), expected);
                    }
                }
            }
        };
    }
    route!(
        NumberGroups,
        NumberGroupsInit,
        NumberGroupInit { values: &[1, 2] },
        0,
        9,
        false,
        9u8,
        0
    );
    route!(
        MapGroups,
        MapGroupsInit,
        MapGroupInit {
            values: &[(1, 3), (3, 7)]
        },
        2,
        9,
        true,
        9u8,
        &1
    );
    route!(
        StringGroups,
        StringGroupsInit,
        StringGroupInit {
            values: &["a", "b"]
        },
        0,
        "z",
        false,
        b'z',
        0
    );
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn old_source_error_precedes_capacity_and_keeps_the_prewrite_frame() {
    use blubber::{ReadStorage as _, Resize as _};
    for bad in [None, Some(3usize), Some(4usize)] {
        let mut bytes = fixture();
        let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        bytes.truncate(used);
        if let Some(word) = bad {
            let at = 20 + word * 4;
            bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        }
        let before = bytes.clone();
        let mut store = blubber::SliceStore::new(&mut bytes);
        let actual = Ancestors::load_trusted_mut(&mut store)
            .unwrap()
            .values_mut()
            .get_mut(0)
            .unwrap()
            .unwrap()
            .set("replacement grows");
        let expected = match bad {
            Some(word) => {
                blubber::Error::new(blubber::ErrorKind::InvalidBoundary, (20 + word * 4) as u32)
            }
            None => blubber::ErrorKind::Capacity.into(),
        };
        assert_eq!(actual, Err(expected));
        assert_eq!(store.bytes(), before);
        assert_eq!(store.is_poisoned(), bad.is_some());
    }
}

#[test]
fn checked_admission_and_detached_rejections_keep_bytes() {
    use blubber_core::raw::resolve_account_edit;
    let layout = blubber_core::Layout::Flat {
        checked: Ancestors::__layout(),
    };
    // Selected source failure poisons even though admission has not written.
    let mut bytes = fixture();
    bytes[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
    let before = bytes.clone();
    let mut poison = false;
    let error = blubber_core::raw::prepare_checked_byte_replacement(
        &mut bytes,
        Ancestors::DISCRIMINATOR,
        Ancestors::MIN_SIZE,
        Ancestors::__layout(),
        &path(),
        3,
        &mut poison,
    )
    .err()
    .unwrap();
    assert_eq!(
        error,
        blubber::Error::new(
            blubber::ErrorKind::InvalidBoundary,
            blubber::Error::NO_OFFSET
        )
    );
    assert!(poison);
    assert_eq!(bytes, before);

    for fault in 0..4 {
        let mut bytes = fixture();
        let resolved = resolve_account_edit(
            &mut bytes,
            Ancestors::DISCRIMINATOR,
            Ancestors::MIN_SIZE,
            &layout,
            &path(),
        )
        .unwrap();
        let plan = resolved.defer_subtree(17).unwrap();
        match fault {
            0 => bytes.truncate(plan.required_storage_len() as usize - 1),
            1 => bytes[8..12].copy_from_slice(&0u32.to_le_bytes()),
            2 => bytes[20..24].copy_from_slice(&9u32.to_le_bytes()),
            _ => bytes[20..24].copy_from_slice(&u32::MAX.to_le_bytes()),
        }
        let before = bytes.clone();
        let mut poison = false;
        assert_eq!(
            plan.begin(&mut bytes, &mut poison).err().unwrap(),
            blubber::ErrorKind::InvalidPlan.into()
        );
        assert_eq!(poison, fault == 1 || fault == 3);
        assert_eq!(bytes, before);
    }
}

#[test]
fn raw_adjust_directory_retains_its_read_only_error_frame() {
    for stride in [4u32, 5] {
        for bad_word in [2u32, 3] {
            for grow in [true, false] {
                let mut bytes = [0xa7; 32];
                for i in 0..4 {
                    let at = (4 + stride * i) as usize;
                    bytes[at..at + 4].copy_from_slice(&(i + 1).to_le_bytes());
                }
                let at = (4 + stride * bad_word) as usize;
                let bad = if grow { u32::MAX } else { 0 };
                bytes[at..at + 4].copy_from_slice(&bad.to_le_bytes());
                let before = bytes;
                assert_eq!(
                    blubber_core::raw::adjust_directory(
                        &mut bytes,
                        4,
                        stride,
                        4,
                        if grow { 1 } else { -1 },
                        10,
                    ),
                    Err(blubber::Error::new(
                        if grow {
                            blubber::ErrorKind::InvalidBoundary
                        } else {
                            blubber::ErrorKind::ArithmeticOverflow
                        },
                        at as u32,
                    )),
                );
                assert_eq!(bytes, before);
            }
        }
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn typed_detached_growth_returns_success_after_publication() {
    use blubber::{ReadStorage as _, Resize as _};
    use blubber_core::raw::{begin_apply, plan_replace};
    let mut bytes = fixture();
    let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    bytes.truncate(used as usize);
    let input = b"abcdefgh-more";
    let mut expected = bytes.clone();
    let plan = plan_replace(
        &expected,
        used,
        &blubber_core::Layout::Flat {
            checked: Ancestors::__layout(),
        },
        &path(),
        input.len() as u32,
        u32::MAX,
    )
    .unwrap();
    expected.resize(plan.required_storage_len() as usize, 0);
    let mut apply = begin_apply(&mut expected, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(input);
    apply.finish();

    let mut store = blubber::VecStore::new(&mut bytes);
    {
        let mut root = Ancestors::load_trusted_mut(&mut store).unwrap();
        root.values_mut()
            .get_mut(0)
            .unwrap()
            .unwrap()
            .push_str("-more")
            .unwrap();
    }
    assert!(!store.is_poisoned());
    assert_eq!(store.bytes(), expected);
    assert!(Ancestors::load_trusted_mut(&mut store).is_ok());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn bulk_insert_and_extend_share_late_ancestor_rejection() {
    use blubber::{ReadStorage as _, Resize as _};
    use blubber_core::raw::{begin_apply, plan_replace};
    for extend in [false, true] {
        for bad in [None, Some(3usize), Some(4usize)] {
            let rows = (0..6)
                .map(|_| NumberGroupInit { values: &[1, 2] })
                .collect::<Vec<_>>();
            let mut bytes = vec![0; 256];
            NumberGroups::initialize(
                &mut blubber::SliceStore::new(&mut bytes),
                NumberGroupsInit {
                    groups: &rows,
                    suffix: "tail",
                },
            )
            .unwrap();
            if let Some(word) = bad {
                let at = 20 + word * 4;
                bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            }
            let before = bytes.clone();
            let used = u32::from_le_bytes(before[8..12].try_into().unwrap());
            let mut expected = before.clone();
            let mut selected = InlinePath::new();
            selected.push(Step::Field(0)).unwrap();
            selected.push(Step::Index(0)).unwrap();
            selected.push(Step::Field(0)).unwrap();
            let encoded = if extend {
                [4, 0, 0, 0, 1, 2, 9, 10]
            } else {
                [4, 0, 0, 0, 9, 10, 1, 2]
            };
            let strict = plan_replace(
                &expected,
                used,
                &blubber_core::Layout::Flat {
                    checked: NumberGroups::__layout(),
                },
                &selected,
                encoded.len() as u32,
                u32::MAX,
            )
            .map(|plan| {
                let mut apply = begin_apply(&mut expected, plan).unwrap();
                apply.move_regions();
                apply.destination().copy_from_slice(&encoded);
                apply.finish();
            });
            let mut store = blubber::SliceStore::new(&mut bytes);
            let actual = {
                let mut root = NumberGroups::load_trusted_mut(&mut store).unwrap();
                let groups = root.groups_mut();
                let mut group = groups.get_mut(0).unwrap().unwrap();
                let mut values = group.values_mut();
                if extend {
                    values.extend_from_slice(&[9, 10])
                } else {
                    values.insert_from_slice(0, &[9, 10])
                }
            };
            assert_eq!(actual, strict);
            if let Some(word) = bad {
                assert_eq!(
                    actual,
                    Err(blubber::Error::new(
                        blubber::ErrorKind::InvalidBoundary,
                        (20 + word * 4) as u32,
                    ))
                );
                assert!(store.is_poisoned());
                assert_eq!(&store.bytes()[..12], &before[..12]);
                assert_ne!(&store.bytes()[12..16], &before[12..16]);
                assert_eq!(
                    &store.bytes()[used as usize + 2..],
                    &before[used as usize + 2..]
                );
                assert_eq!(
                    NumberGroups::load_trusted_mut(&mut store).err().unwrap(),
                    blubber::ErrorKind::Poisoned.into()
                );
            } else {
                assert!(!store.is_poisoned());
                assert_eq!(store.bytes(), expected);
            }
        }
    }
}

#[cfg(all(feature = "alloc", not(feature = "verify-on-load")))]
#[test]
fn indexed_bulk_insert_and_extend_share_late_ancestor_rejection() {
    use blubber::{ReadStorage as _, Resize as _};
    use blubber_core::raw::{begin_apply, plan_replace};
    for extend in [false, true] {
        for detached in [false, true] {
            for bad in [None, Some(3usize), Some(4usize)] {
                let rows = (0..6)
                    .map(|_| StringGroupInit {
                        values: &["a", "bb"],
                    })
                    .collect::<Vec<_>>();
                let mut bytes = vec![0; 256];
                StringGroups::initialize(
                    &mut blubber::SliceStore::new(&mut bytes),
                    StringGroupsInit {
                        groups: &rows,
                        suffix: "tail",
                    },
                )
                .unwrap();
                let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
                if detached {
                    bytes.truncate(used as usize);
                }
                if let Some(word) = bad {
                    let at = 20 + word * 4;
                    bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
                }
                let before = bytes.clone();
                let mut expected = before.clone();
                let mut selected = InlinePath::new();
                selected.push(Step::Field(0)).unwrap();
                selected.push(Step::Index(0)).unwrap();
                selected.push(Step::Field(0)).unwrap();
                let mut encoded = vec![4, 0, 0, 0];
                for end in if extend { [1u32, 3, 6] } else { [3, 4, 5] } {
                    encoded.extend_from_slice(&end.to_le_bytes());
                }
                encoded.extend_from_slice(if extend { b"abbcccd" } else { b"cccdabb" });
                let strict = plan_replace(
                    &expected,
                    used,
                    &blubber_core::Layout::Flat {
                        checked: StringGroups::__layout(),
                    },
                    &selected,
                    encoded.len() as u32,
                    u32::MAX,
                )
                .map(|plan| {
                    if detached {
                        expected.resize(used as usize + 12, 0);
                    }
                    let mut apply = begin_apply(&mut expected, plan).unwrap();
                    apply.move_regions();
                    apply.destination().copy_from_slice(&encoded);
                    apply.finish();
                });
                let mut store = blubber::VecStore::new(&mut bytes);
                let actual = {
                    let mut root = StringGroups::load_trusted_mut(&mut store).unwrap();
                    let groups = root.groups_mut();
                    let mut group = groups.get_mut(0).unwrap().unwrap();
                    let mut values = group.values_mut();
                    if extend {
                        values.extend_from_slice(&["ccc", "d"])
                    } else {
                        values.insert_from_slice(0, &["ccc", "d"])
                    }
                };
                assert_eq!(actual, strict);
                if let Some(word) = bad {
                    assert_eq!(
                        actual,
                        Err(blubber::Error::new(
                            blubber::ErrorKind::InvalidBoundary,
                            (20 + word * 4) as u32
                        ))
                    );
                    assert!(store.is_poisoned());
                    assert_eq!(&store.bytes()[..12], &before[..12]);
                    assert_ne!(&store.bytes()[12..16], &before[12..16]);
                    if !detached {
                        assert_eq!(
                            &store.bytes()[used as usize + 12..],
                            &before[used as usize + 12..]
                        );
                    }
                    assert_eq!(
                        StringGroups::load_trusted_mut(&mut store).err().unwrap(),
                        blubber::ErrorKind::Poisoned.into()
                    );
                } else {
                    assert!(!store.is_poisoned());
                    assert_eq!(store.bytes(), expected);
                }
            }
        }
    }
}

#[test]
fn fixed_entry_readiness_rejections_preserve_physical_bytes_and_poison() {
    use blubber_core::raw::{resolve_account_edit, ListEdit, MapEdit};
    for map in [false, true] {
        for fault in 0..6 {
            let mut bytes = vec![0; 256];
            let (discriminator, minimum, layout) = if map {
                let rows = (0..6)
                    .map(|_| MapGroupInit {
                        values: &[(1, 7), (3, 9)],
                    })
                    .collect::<Vec<_>>();
                MapGroups::initialize(
                    &mut blubber::SliceStore::new(&mut bytes),
                    MapGroupsInit {
                        groups: &rows,
                        suffix: "tail",
                    },
                )
                .unwrap();
                (
                    MapGroups::DISCRIMINATOR,
                    MapGroups::MIN_SIZE,
                    MapGroups::__layout(),
                )
            } else {
                let rows = (0..6)
                    .map(|_| NumberGroupInit { values: &[7, 9] })
                    .collect::<Vec<_>>();
                NumberGroups::initialize(
                    &mut blubber::SliceStore::new(&mut bytes),
                    NumberGroupsInit {
                        groups: &rows,
                        suffix: "tail",
                    },
                )
                .unwrap();
                (
                    NumberGroups::DISCRIMINATOR,
                    NumberGroups::MIN_SIZE,
                    NumberGroups::__layout(),
                )
            };
            let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
            let mut path = InlinePath::new();
            path.push(Step::Field(0)).unwrap();
            path.push(Step::Index(0)).unwrap();
            path.push(Step::Field(0)).unwrap();
            if fault == 0 {
                bytes.truncate(used);
            }
            let original = bytes.clone();
            let resolved = resolve_account_edit(
                &mut bytes,
                discriminator,
                minimum,
                &blubber_core::Layout::Flat { checked: layout },
                &path,
            )
            .unwrap();
            let selected = resolved.selection().0;
            let prepared = if map {
                resolved.prepare_map_ancestors(MapEdit::Insert, &[2], 1, 260, None)
            } else {
                resolved.prepare_list_ancestors(ListEdit::Insert(0), 1, 260)
            }
            .unwrap();
            if fault == 0 {
                assert_eq!(
                    prepared.begin_prepared().err().unwrap(),
                    blubber::ErrorKind::InvalidPlan.into()
                );
                assert_eq!(bytes, original);
                continue;
            }
            let plan = prepared.detach();
            let mut expected_poison = false;
            match fault {
                1 => bytes.truncate(plan.required_storage_len() as usize - 1),
                2 => bytes.resize(261, 0),
                3 => {
                    bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
                    expected_poison = true;
                }
                4 => {
                    bytes[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
                    expected_poison = true;
                }
                _ if map => bytes[selected.start as usize + 4] = 2,
                _ => {
                    bytes[selected.start as usize..selected.start as usize + 4]
                        .copy_from_slice(&u32::MAX.to_le_bytes());
                    expected_poison = true;
                }
            }
            let before = bytes.clone();
            let mut poisoned = false;
            assert_eq!(
                plan.begin(&mut bytes, &mut poisoned).err().unwrap(),
                blubber::ErrorKind::InvalidPlan.into(),
                "map={map}, fault={fault}"
            );
            assert_eq!(poisoned, expected_poison, "map={map}, fault={fault}");
            assert_eq!(bytes, before, "map={map}, fault={fault}");
        }
    }
}

#[test]
fn ancestor_run_threshold_tracks_one_run_and_keeps_bytes() {
    let cases: &[(&[Step], u32)] = &[
        (&[], 0),
        (&[Step::Field(0)], 1),
        (&[Step::Field(0), Step::Index(0)], 5),
        (&[Step::Field(0), Step::Index(4)], 1),
        (&[Step::Field(1)], 0),
    ];
    for &(steps, largest) in cases {
        let mut bytes = fixture();
        let before = bytes.clone();
        let mut selected = InlinePath::new();
        for &step in steps {
            selected.push(step).unwrap();
        }
        {
            let resolved = blubber_core::raw::resolve_account_edit(
                &mut bytes,
                Ancestors::DISCRIMINATOR,
                Ancestors::MIN_SIZE,
                &blubber_core::Layout::Flat {
                    checked: Ancestors::__layout(),
                },
                &selected,
            )
            .unwrap();
            assert_eq!(resolved.has_ancestor_words(), largest != 0);
            for minimum in 1..=7 {
                assert_eq!(
                    resolved.has_ancestor_run(minimum),
                    largest >= minimum,
                    "largest={largest} minimum={minimum}"
                );
            }
        }
        assert_eq!(bytes, before);
    }
}
