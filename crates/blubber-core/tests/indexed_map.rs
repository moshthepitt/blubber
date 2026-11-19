//! Indexed edits checked against an independent BTreeMap and whole-map encoder.
#![forbid(unsafe_code)]
use blubber_core::raw::{begin_apply, plan_map_edit, plan_replace, InlinePath, MapEdit, Step};
use blubber_core::{
    validate, Error, ErrorKind, KeyDomain, KeyLayout, Layout, VariableField, MAX_DEPTH,
};
use blubber_model::{FixedMapEdit, IndexedMap};
fn used(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[8..12].try_into().unwrap())
}
fn account(root: &[u8], spare: usize) -> Vec<u8> {
    let mut bytes = b"blubber!".to_vec();
    bytes.extend_from_slice(&(12 + root.len() as u32).to_le_bytes());
    bytes.extend_from_slice(root);
    bytes.resize(bytes.len() + spare, 0xa5);
    bytes
}
fn layout(key_width: u32) -> Layout {
    Layout::Map {
        key: KeyLayout {
            width: key_width,
            domain: KeyDomain::Raw,
        },
        value: &Layout::Str,
    }
}
fn key(n: u32, width: usize) -> Vec<u8> {
    let mut key = vec![0x51; width];
    let encoded = n.to_be_bytes();
    let len = width.min(4);
    key[..len].copy_from_slice(&encoded[4 - len..]);
    key
}
fn oracle(operation: MapEdit) -> FixedMapEdit {
    match operation {
        MapEdit::Insert => FixedMapEdit::Insert,
        MapEdit::Remove => FixedMapEdit::Remove,
        MapEdit::Replace => FixedMapEdit::Replace,
        MapEdit::Clear => FixedMapEdit::Clear,
    }
}
#[allow(clippy::too_many_arguments)]
fn check(
    old: &[u8],
    expected: &[u8],
    layout: &Layout,
    path: &InlinePath,
    operation: MapEdit,
    key: &[u8],
    input: &[u8],
    removed: Option<Vec<u8>>,
) {
    validate(old, layout, MAX_DEPTH).unwrap();
    let original = account(old, expected.len().saturating_sub(old.len()) + 19);
    let mut guarded = vec![0x7d; 3];
    guarded.extend_from_slice(&original);
    guarded.extend_from_slice(&[0x7e; 5]);
    let bytes = &mut guarded[3..3 + original.len()];
    let plan = plan_map_edit(
        bytes,
        used(bytes),
        layout,
        path,
        operation,
        key,
        input.len() as u32,
        bytes.len() as u32,
    )
    .unwrap();
    assert_eq!(plan.new_used_len(), 12 + expected.len() as u32);
    assert_eq!(
        plan.required_storage_len(),
        (12 + expected.len() as u32).max(used(bytes))
    );
    assert_eq!(
        plan.removed_span()
            .map(|span| bytes[span.start as usize..span.end as usize].to_vec()),
        removed
    );
    plan.check_ready(bytes).unwrap();
    assert_eq!(bytes, original);
    let mut apply = begin_apply(bytes, plan).unwrap();
    apply.move_regions();
    let destination = apply.destination();
    assert_eq!(destination.len(), input.len());
    destination.copy_from_slice(input);
    apply.finish();
    let mut want = original.clone();
    want[8..12].copy_from_slice(&(12 + expected.len() as u32).to_le_bytes());
    want[12..12 + expected.len()].copy_from_slice(expected);
    if 12 + expected.len() < used(&original) as usize {
        want[12 + expected.len()..used(&original) as usize].fill(0);
    }
    assert_eq!(bytes, want);
    validate(&bytes[12..used(bytes) as usize], layout, MAX_DEPTH).unwrap();
    assert_eq!(&guarded[..3], &[0x7d; 3]);
    assert_eq!(&guarded[3 + original.len()..], &[0x7e; 5]);
}

fn wrap(inner: Layout, bytes: &[u8], level: usize) -> (Layout, Vec<u8>, Step) {
    let inner = Box::leak(Box::new(inner));
    match level % 4 {
        0 => {
            let vars = Box::leak(Box::new([
                VariableField {
                    declaration: 0,
                    layout: inner,
                },
                VariableField {
                    declaration: 1,
                    layout: &Layout::Str,
                },
            ]));
            let mut out = (bytes.len() as u32).to_le_bytes().to_vec();
            out.extend_from_slice(bytes);
            out.extend_from_slice(b"outer suffix");
            (
                Layout::Struct {
                    fixed_bytes: 0,
                    fixed: &[],
                    variable: vars,
                },
                out,
                Step::Field(0),
            )
        }
        1 => {
            let mut out = vec![1];
            out.extend_from_slice(bytes);
            (Layout::Option { value: inner }, out, Step::Some)
        }
        2 => {
            let mut out = 1u32.to_le_bytes().to_vec();
            out.extend_from_slice(bytes);
            (Layout::List { element: inner }, out, Step::Index(0))
        }
        _ => {
            let mut out = 1u32.to_le_bytes().to_vec();
            out.extend_from_slice(&[1, 2, 3]);
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(bytes);
            (
                Layout::Map {
                    key: KeyLayout {
                        width: 3,
                        domain: KeyDomain::Raw,
                    },
                    value: inner,
                },
                out,
                Step::Index(0),
            )
        }
    }
}

#[test]
fn every_small_map_position_length_and_operation() {
    for kw in [1, 3, 8, 32, 257] {
        for count in 0..=5u32 {
            for pattern in 0..3usize.pow(count) {
                let mut lengths = pattern;
                let model = IndexedMap::new(
                    kw,
                    (0..count).map(|i| {
                        let len = lengths % 3;
                        lengths /= 3;
                        (key(2 * i + 1, kw), vec![b'a' + i as u8; len])
                    }),
                )
                .unwrap();
                for index in 0..=2 * count + 1 {
                    for operation in [MapEdit::Insert, MapEdit::Replace, MapEdit::Remove] {
                        for len in 0..=3 {
                            if operation == MapEdit::Remove && len != 0 {
                                continue;
                            }
                            let query = key(index, kw);
                            let input = vec![b'Z'; len];
                            let mut next = model.clone();
                            if let Ok(removed) = next.apply(oracle(operation), &query, &input) {
                                check(
                                    &model.encode(),
                                    &next.encode(),
                                    &layout(kw as u32),
                                    &InlinePath::new(),
                                    operation,
                                    &query,
                                    &input,
                                    removed,
                                );
                            }
                        }
                    }
                }
                check(
                    &model.encode(),
                    &[0; 4],
                    &layout(kw as u32),
                    &InlinePath::new(),
                    MapEdit::Clear,
                    &[],
                    &[],
                    None,
                );
            }
        }
    }
}

#[test]
fn nested_edits_grow_and_shrink_all_ancestor_kinds() {
    let model = IndexedMap::new(3, [(key(2, 3), b"abc".to_vec()), (key(4, 3), vec![])]).unwrap();
    for (operation, n, input) in [
        (MapEdit::Insert, 1, b"grown".as_slice()),
        (MapEdit::Insert, 5, b""),
        (MapEdit::Insert, 2, b""),
        (MapEdit::Insert, 2, b"XYZ"),
        (MapEdit::Replace, 2, b"XYZ"),
        (MapEdit::Replace, 2, b"long replacement"),
        (MapEdit::Remove, 2, b""),
        (MapEdit::Clear, 0, b""),
    ] {
        let query = if operation == MapEdit::Clear {
            vec![]
        } else {
            key(n, 3)
        };
        let mut next = model.clone();
        let removed = next.apply(oracle(operation), &query, input).unwrap();
        for rotation in 0..4 {
            for depth in 0..MAX_DEPTH - 1 {
                let mut old = model.encode();
                let mut new = next.encode();
                let mut desc = layout(3);
                let mut steps = vec![];
                for i in 0..depth {
                    let (_, new_bytes, _) = wrap(desc, &new, i + rotation);
                    let (outer, old_bytes, step) = wrap(desc, &old, i + rotation);
                    desc = outer;
                    old = old_bytes;
                    new = new_bytes;
                    steps.insert(0, step);
                }
                let mut path = InlinePath::new();
                for step in steps {
                    path.push(step).unwrap();
                }
                check(
                    &old,
                    &new,
                    &desc,
                    &path,
                    operation,
                    &query,
                    input,
                    removed.clone(),
                );
            }
        }
    }
}

#[test]
fn nested_value_replacement_preserves_keys_and_patches_stored_terminal_end() {
    let model = IndexedMap::new(
        1,
        [
            (vec![1], b"a".to_vec()),
            (vec![2], b"bc".to_vec()),
            (vec![3], vec![]),
        ],
    )
    .unwrap();
    for index in 0..3u32 {
        for input in [b"".as_slice(), b"long value", b"x"] {
            let mut next = model.clone();
            next.apply(FixedMapEdit::Replace, &[index as u8 + 1], input)
                .unwrap();
            let mut bytes = account(&model.encode(), 40);
            let original = bytes.clone();
            let mut path = InlinePath::new();
            path.push(Step::Index(index)).unwrap();
            let plan = plan_replace(
                &bytes,
                used(&bytes),
                &layout(1),
                &path,
                input.len() as u32,
                bytes.len() as u32,
            )
            .unwrap();
            let mut apply = begin_apply(&mut bytes, plan).unwrap();
            apply.move_regions();
            apply.destination().copy_from_slice(input);
            apply.finish();
            assert_eq!(&bytes[12..used(&bytes) as usize], next.encode());
            assert_eq!(
                &bytes[used(&original).max(used(&bytes)) as usize..],
                &original[used(&original).max(used(&bytes)) as usize..]
            );
            validate(&bytes[12..used(&bytes) as usize], &layout(1), MAX_DEPTH).unwrap();
        }
    }
}

#[test]
fn relocation_and_readiness_use_current_geometry() {
    let model = IndexedMap::new(1, [(vec![2], b"ab".to_vec()), (vec![6], b"cd".to_vec())]).unwrap();
    let old = account(&model.encode(), 0);
    let query = [4];
    let plan = plan_map_edit(
        &old,
        used(&old),
        &layout(1),
        &InlinePath::new(),
        MapEdit::Insert,
        &query,
        3,
        100,
    )
    .unwrap();
    assert_eq!(plan.check_ready(&old), Err(ErrorKind::InvalidPlan.into()));
    let mut grown = vec![0; plan.required_storage_len() as usize];
    grown[..old.len()].copy_from_slice(&old);
    let mut apply = begin_apply(&mut grown, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(b"xyz");
    apply.finish();
    let mut next = model.clone();
    next.apply(FixedMapEdit::Insert, &query, b"xyz").unwrap();
    assert_eq!(&grown[12..], next.encode());
    for mutation in 0..6 {
        let original = account(&model.encode(), 30);
        let plan = plan_map_edit(
            &original,
            used(&original),
            &layout(1),
            &InlinePath::new(),
            MapEdit::Insert,
            &query,
            1,
            100,
        )
        .unwrap();
        let mut changed = original.clone();
        match mutation {
            0 => changed[0] ^= 1,
            1 => changed[8] = 0,
            2 => changed[12] = 1,
            3 => changed[21] = 3,
            4 => changed[17] = 1,
            _ => changed[22] = 3,
        }
        let before = changed.clone();
        assert!(begin_apply(&mut changed, plan).is_err());
        assert_eq!(changed, before);
    }
    let mut bytes = account(&model.encode(), 30);
    let plan = plan_map_edit(
        &bytes,
        used(&bytes),
        &layout(1),
        &InlinePath::new(),
        MapEdit::Replace,
        &[2],
        1,
        100,
    )
    .unwrap();
    bytes[26] = b'Z'; // payload content changes without changing any coordinate
    plan.check_ready(&bytes).unwrap();
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(b"x");
    apply.finish();
    validate(&bytes[12..used(&bytes) as usize], &layout(1), MAX_DEPTH).unwrap();
}

#[test]
fn corrupted_metadata_queries_and_capacity_fail_before_writes() {
    let model = IndexedMap::new(1, [(vec![1], b"a".to_vec()), (vec![3], b"bc".to_vec())]).unwrap();
    for mutation in 0..5 {
        let mut root = model.encode();
        match mutation {
            0 => root[9] = 1,
            1 => root[9] = 0,
            2 => root[5] = 4,
            3 => root[10] = 2,
            _ => root[0] = 255,
        }
        let bytes = account(&root, 10);
        let before = bytes.clone();
        assert!(plan_map_edit(
            &bytes,
            used(&bytes),
            &layout(1),
            &InlinePath::new(),
            MapEdit::Clear,
            &[],
            0,
            100
        )
        .is_err());
        assert_eq!(bytes, before);
    }
    for (operation, query, len, cap, kind) in [
        (
            MapEdit::Replace,
            b"z".as_slice(),
            0,
            100,
            ErrorKind::MissingKey,
        ),
        (MapEdit::Insert, b"", 0, 100, ErrorKind::InvalidBoundary),
        (MapEdit::Clear, b"z", 0, 100, ErrorKind::InvalidBoundary),
        (MapEdit::Remove, b"z", 1, 100, ErrorKind::InvalidBoundary),
        (MapEdit::Insert, b"z", 50, 32, ErrorKind::Capacity),
        (
            MapEdit::Insert,
            b"z",
            u32::MAX,
            u32::MAX,
            ErrorKind::ArithmeticOverflow,
        ),
    ] {
        let bytes = account(&[0; 4], 0);
        let before = bytes.clone();
        assert_eq!(
            plan_map_edit(
                &bytes,
                used(&bytes),
                &layout(1),
                &InlinePath::new(),
                operation,
                query,
                len,
                cap
            )
            .unwrap_err(),
            Error::new(kind, Error::NO_OFFSET)
        );
        assert_eq!(bytes, before);
    }
    check(
        &[0; 4],
        &[0; 4],
        &layout(u32::MAX),
        &InlinePath::new(),
        MapEdit::Clear,
        &[],
        &[],
        None,
    );
}

#[test]
fn bool_domains_are_checked_and_unrelated_values_remain_lazy() {
    let desc = Layout::Map {
        key: KeyLayout {
            width: 1,
            domain: KeyDomain::Bool,
        },
        value: &Layout::Str,
    };
    let empty = account(&[0; 4], 10);
    assert_eq!(
        plan_map_edit(
            &empty,
            used(&empty),
            &desc,
            &InlinePath::new(),
            MapEdit::Insert,
            &[2],
            0,
            100
        )
        .unwrap_err(),
        ErrorKind::InvalidBool.into()
    );
    let model = IndexedMap::new(1, [(vec![0], vec![255]), (vec![1], b"valid".to_vec())]).unwrap();
    let mut bytes = account(&model.encode(), 10);
    let plan = plan_map_edit(
        &bytes,
        used(&bytes),
        &desc,
        &InlinePath::new(),
        MapEdit::Replace,
        &[1],
        0,
        100,
    )
    .unwrap();
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    apply.move_regions();
    apply.destination();
    apply.finish();
    assert_eq!(bytes[26], 255); // invalid sibling survives
    let mut bad = account(&[1, 0, 0, 0, 2, 0, 0, 0, 0], 0);
    let before = bad.clone();
    assert_eq!(
        plan_map_edit(
            &bad,
            used(&bad),
            &desc,
            &InlinePath::new(),
            MapEdit::Clear,
            &[],
            0,
            100
        )
        .unwrap_err(),
        Error::new(ErrorKind::InvalidBool, 16)
    );
    assert_eq!(bad, before);
    bad.clear();
}

#[test]
fn counts_above_u16_and_large_overlapping_payloads() {
    let model = IndexedMap::new(
        4,
        (0..65_537).map(|i| (key(i, 4), if i % 2 == 0 { vec![] } else { vec![b'a'; 3] })),
    )
    .unwrap();
    for operation in [MapEdit::Insert, MapEdit::Remove, MapEdit::Clear] {
        let query = if operation == MapEdit::Clear {
            vec![]
        } else {
            key(32_768, 4)
        };
        let input = if operation == MapEdit::Insert {
            b"replacement".as_slice()
        } else {
            b""
        };
        let mut next = model.clone();
        let removed = next.apply(oracle(operation), &query, input).unwrap();
        check(
            &model.encode(),
            &next.encode(),
            &layout(4),
            &InlinePath::new(),
            operation,
            &query,
            input,
            removed,
        );
    }
    let model = IndexedMap::new(
        32,
        [
            (key(1, 32), vec![b'a'; 4096]),
            (key(3, 32), vec![b'b'; 8192]),
        ],
    )
    .unwrap();
    for n in [0, 2, 4] {
        let query = key(n, 32);
        let mut next = model.clone();
        next.apply(FixedMapEdit::Insert, &query, b"x").unwrap();
        check(
            &model.encode(),
            &next.encode(),
            &layout(32),
            &InlinePath::new(),
            MapEdit::Insert,
            &query,
            b"x",
            None,
        );
    }
}

#[test]
fn seeded_mixed_edits_match_owned_map() {
    for seed in [0x18u64, 0x58a3, 0xdeadbeef] {
        let mut state = seed;
        let mut model = IndexedMap::new(3, []).unwrap();
        for _ in 0..700 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let operation = match state >> 61 {
                0 => MapEdit::Clear,
                1 | 2 => MapEdit::Remove,
                3 => MapEdit::Replace,
                _ => MapEdit::Insert,
            };
            let query = if operation == MapEdit::Clear {
                vec![]
            } else {
                key((state >> 32) as u32 % 41, 3)
            };
            let input = if matches!(operation, MapEdit::Insert | MapEdit::Replace) {
                vec![b'a' + (state % 26) as u8; ((state >> 16) % 25) as usize]
            } else {
                vec![]
            };
            let mut next = model.clone();
            if let Ok(removed) = next.apply(oracle(operation), &query, &input) {
                check(
                    &model.encode(),
                    &next.encode(),
                    &layout(3),
                    &InlinePath::new(),
                    operation,
                    &query,
                    &input,
                    removed,
                );
                model = next;
            }
        }
    }
}

#[test]
fn phase_misuse_panics_before_its_next_write() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let model = IndexedMap::new(1, [(vec![1], b"abc".to_vec())]).unwrap();
    let original = account(&model.encode(), 20);
    for phase in 0..4 {
        let mut bytes = original.clone();
        let plan = plan_map_edit(
            &bytes,
            used(&bytes),
            &layout(1),
            &InlinePath::new(),
            MapEdit::Replace,
            &[1],
            3,
            bytes.len() as u32,
        )
        .unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut apply = begin_apply(&mut bytes, plan).unwrap();
            match phase {
                0 => {
                    apply.destination();
                }
                1 => apply.finish(),
                2 => {
                    apply.move_regions();
                    apply.move_regions();
                }
                _ => {
                    apply.move_regions();
                    apply.destination();
                    apply.destination();
                }
            }
        }));
        assert!(result.is_err());
        assert_eq!(bytes, original);
    }
}
