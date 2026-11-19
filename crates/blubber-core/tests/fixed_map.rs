//! Sorted fixed-value edits compared with an independent owned map.

#![forbid(unsafe_code)]

use blubber_core::raw::{begin_apply, plan_map_edit, InlinePath, MapEdit, Step};
use blubber_core::{
    validate, Error, ErrorKind, KeyDomain, KeyLayout, Layout, Leaf, VariableField, MAX_DEPTH,
};
use blubber_model::{FixedMap, FixedMapEdit};

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
fn layout(key_width: u32, value_width: u32) -> Layout {
    Layout::Map {
        key: KeyLayout {
            width: key_width,
            domain: KeyDomain::Raw,
        },
        value: Box::leak(Box::new(if value_width == 0 {
            Layout::Struct {
                fixed_bytes: 0,
                fixed: &[],
                variable: &[],
            }
        } else {
            Layout::Leaf(Leaf::Raw(value_width))
        })),
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

#[test]
fn all_ordinals_upserts_removals_replacements_and_clear() {
    for kw in [1, 3, 8, 32, 257] {
        for vw in [1, 3, 16] {
            for count in 0..=8 {
                let model = FixedMap::new(
                    kw,
                    vw,
                    (0..count).map(|i| (key(2 * i + 1, kw), vec![i as u8; vw])),
                )
                .unwrap();
                let layout = layout(kw as u32, vw as u32);
                for n in 0..=2 * count + 1 {
                    for operation in [MapEdit::Insert, MapEdit::Replace, MapEdit::Remove] {
                        let query = key(n, kw);
                        let input = if matches!(operation, MapEdit::Insert | MapEdit::Replace) {
                            vec![0xd7; vw]
                        } else {
                            vec![]
                        };
                        let mut next = model.clone();
                        if let Ok(removed) = next.apply(oracle(operation), &query, &input) {
                            check(
                                &model.encode(),
                                &next.encode(),
                                &layout,
                                &InlinePath::new(),
                                operation,
                                &query,
                                &input,
                                removed,
                            );
                        }
                    }
                }
                let mut next = model.clone();
                next.apply(FixedMapEdit::Clear, &[], &[]).unwrap();
                check(
                    &model.encode(),
                    &next.encode(),
                    &layout,
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
fn nested_all_ancestor_kinds_at_both_depth_limits() {
    let model = FixedMap::new(
        3,
        2,
        [(vec![2, 0, 0], vec![7, 8]), (vec![4, 0, 0], vec![9, 10])],
    )
    .unwrap();
    for operation in [
        MapEdit::Insert,
        MapEdit::Remove,
        MapEdit::Replace,
        MapEdit::Clear,
    ] {
        let query = if matches!(operation, MapEdit::Clear) {
            vec![]
        } else {
            vec![2, 0, 0]
        };
        let input = if matches!(operation, MapEdit::Insert | MapEdit::Replace) {
            vec![0, 1]
        } else {
            vec![]
        };
        let mut next = model.clone();
        let removed = next.apply(oracle(operation), &query, &input).unwrap();
        for rotation in 0..4 {
            for depth in 0..MAX_DEPTH - 1 {
                let mut old = model.encode();
                let mut new = next.encode();
                let mut desc = layout(3, 2);
                let mut steps = Vec::new();
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
                    &input,
                    removed.clone(),
                );
            }
        }
    }
    // A vacant insertion grows all ancestor boundaries, including stored map ends.
    let mut next = model.clone();
    next.apply(FixedMapEdit::Insert, &[3, 0, 0], &[11, 12])
        .unwrap();
    let (desc, old, step) = wrap(layout(3, 2), &model.encode(), 3);
    let (_, new, _) = wrap(layout(3, 2), &next.encode(), 3);
    let mut path = InlinePath::new();
    path.push(step).unwrap();
    check(
        &old,
        &new,
        &desc,
        &path,
        MapEdit::Insert,
        &[3, 0, 0],
        &[11, 12],
        None,
    );
}

#[test]
fn readiness_repeats_query_search_and_checks_geometry() {
    let desc = layout(1, 1);
    let model = FixedMap::new(1, 1, [(vec![2], vec![7]), (vec![6], vec![8])]).unwrap();
    let original = account(&model.encode(), 20);
    let query = [4];
    let plan = plan_map_edit(
        &original,
        used(&original),
        &desc,
        &InlinePath::new(),
        MapEdit::Insert,
        &query,
        1,
        original.len() as u32,
    )
    .unwrap();
    let mut changed = original.clone();
    changed[18] = 3; // second key crosses the query's insertion partition
    assert_eq!(
        plan.check_ready(&changed),
        Err(ErrorKind::InvalidPlan.into())
    );
    let snapshot = changed.clone();
    assert!(begin_apply(&mut changed, plan).is_err());
    assert_eq!(changed, snapshot);
    let plan = plan_map_edit(
        &original,
        used(&original),
        &desc,
        &InlinePath::new(),
        MapEdit::Insert,
        &query,
        1,
        original.len() as u32,
    )
    .unwrap();
    let mut relocated = original.clone();
    relocated[17] = 9;
    relocated[19] = 10;
    relocated[25] = 99;
    let mut apply = begin_apply(&mut relocated, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(&[11]);
    apply.finish();
    validate(&relocated[12..used(&relocated) as usize], &desc, MAX_DEPTH).unwrap();
}

#[allow(clippy::too_many_arguments)]
fn fault(
    root: &[u8],
    desc: &Layout,
    operation: MapEdit,
    query: &[u8],
    len: u32,
    limit: u32,
    kind: ErrorKind,
    offset: u32,
) {
    let bytes = account(root, 0);
    let before = bytes.clone();
    let error = plan_map_edit(
        &bytes,
        used(&bytes),
        desc,
        &InlinePath::new(),
        operation,
        query,
        len,
        limit,
    )
    .unwrap_err();
    assert_eq!(error, Error::new(kind, offset));
    assert_eq!(bytes, before);
}
#[test]
fn errors_have_stable_precedence_and_leave_all_bytes_unchanged() {
    let empty = 0u32.to_le_bytes();
    let desc = layout(1, 2);
    fault(
        &empty,
        &desc,
        MapEdit::Replace,
        &[1],
        1,
        100,
        ErrorKind::MissingKey,
        Error::NO_OFFSET,
    );
    fault(
        &empty,
        &desc,
        MapEdit::Insert,
        &[],
        2,
        100,
        ErrorKind::InvalidBoundary,
        Error::NO_OFFSET,
    );
    fault(
        &empty,
        &desc,
        MapEdit::Insert,
        &[1],
        1,
        100,
        ErrorKind::InvalidBoundary,
        Error::NO_OFFSET,
    );
    fault(
        &empty,
        &desc,
        MapEdit::Clear,
        &[1],
        0,
        100,
        ErrorKind::InvalidBoundary,
        Error::NO_OFFSET,
    );
    fault(
        &empty,
        &desc,
        MapEdit::Remove,
        &[1],
        1,
        100,
        ErrorKind::InvalidBoundary,
        Error::NO_OFFSET,
    );
    fault(
        &empty,
        &desc,
        MapEdit::Insert,
        &[1],
        2,
        16,
        ErrorKind::Capacity,
        Error::NO_OFFSET,
    );
    fault(
        &[1, 0, 0, 0],
        &desc,
        MapEdit::Insert,
        &[1],
        2,
        100,
        ErrorKind::Truncated,
        16,
    );
    let model = FixedMap::new(1, 2, [(vec![1], vec![0, 0]), (vec![2], vec![0, 0])]).unwrap();
    let mut bad = model.encode();
    bad[7] = 1;
    fault(
        &bad,
        &desc,
        MapEdit::Insert,
        &[1],
        2,
        100,
        ErrorKind::DuplicateKey,
        19,
    );
    bad[7] = 0;
    fault(
        &bad,
        &desc,
        MapEdit::Remove,
        &[1],
        0,
        100,
        ErrorKind::UnsortedKeys,
        19,
    );
}
#[test]
fn bool_queries_and_stored_keys_are_checked_but_values_remain_lazy() {
    let desc = Layout::Map {
        key: KeyLayout {
            width: 1,
            domain: KeyDomain::Bool,
        },
        value: &Layout::Leaf(Leaf::Bool),
    };
    fault(
        &[0, 0, 0, 0],
        &desc,
        MapEdit::Insert,
        &[2],
        1,
        100,
        ErrorKind::InvalidBool,
        Error::NO_OFFSET,
    );
    fault(
        &[1, 0, 0, 0, 2, 0],
        &desc,
        MapEdit::Clear,
        &[],
        0,
        100,
        ErrorKind::InvalidBool,
        16,
    );
    let mut bytes = account(&[1, 0, 0, 0, 1, 77], 10);
    let plan = plan_map_edit(
        &bytes,
        used(&bytes),
        &desc,
        &InlinePath::new(),
        MapEdit::Replace,
        &[1],
        1,
        bytes.len() as u32,
    )
    .unwrap();
    assert_eq!(
        plan.removed_span()
            .map(|s| bytes[s.start as usize..s.end as usize].to_vec()),
        Some(vec![77])
    );
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(&[0]);
    apply.finish();
    validate(&bytes[12..used(&bytes) as usize], &desc, MAX_DEPTH).unwrap();
}
#[test]
fn full_width_stride_overflow_and_empty_noops() {
    let desc = layout(u32::MAX, u32::MAX);
    let empty = 0u32.to_le_bytes();
    check(
        &empty,
        &empty,
        &desc,
        &InlinePath::new(),
        MapEdit::Clear,
        &[],
        &[],
        None,
    );
    // Valid small query with an enormous value detects extent overflow before capacity.
    let desc = layout(1, u32::MAX);
    fault(
        &empty,
        &desc,
        MapEdit::Insert,
        &[1],
        u32::MAX,
        u32::MAX,
        ErrorKind::ArithmeticOverflow,
        Error::NO_OFFSET,
    );
}
#[test]
fn more_than_u16_entries() {
    let model = FixedMap::new(4, 1, (0..65_537).map(|i| (key(i, 4), vec![7]))).unwrap();
    let mut next = model.clone();
    let query = key(65_535, 4);
    let removed = next.apply(FixedMapEdit::Remove, &query, &[]).unwrap();
    check(
        &model.encode(),
        &next.encode(),
        &layout(4, 1),
        &InlinePath::new(),
        MapEdit::Remove,
        &query,
        &[],
        removed,
    );
}
#[test]
fn seeded_mixed_operations_match_owned_map() {
    for seed in [0x17u64, 0x58a3, 0xdeadbeef] {
        let mut state = seed;
        let desc = layout(3, 3);
        let mut model = FixedMap::new(3, 3, []).unwrap();
        for _ in 0..700 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let operation = match state >> 61 {
                0 => MapEdit::Clear,
                1 | 2 => MapEdit::Remove,
                3 => MapEdit::Replace,
                _ => MapEdit::Insert,
            };
            let query = if matches!(operation, MapEdit::Clear) {
                vec![]
            } else {
                key((state >> 32) as u32 % 41, 3)
            };
            let input = if matches!(operation, MapEdit::Insert | MapEdit::Replace) {
                state.to_le_bytes()[..3].to_vec()
            } else {
                vec![]
            };
            let mut next = model.clone();
            if let Ok(removed) = next.apply(oracle(operation), &query, &input) {
                check(
                    &model.encode(),
                    &next.encode(),
                    &desc,
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
fn growth_after_relocation_and_old_value_extraction() {
    let desc = layout(3, 4);
    let model = FixedMap::new(3, 4, [(vec![2, 0, 0], vec![1, 2, 3, 4])]).unwrap();
    let mut bytes = account(&model.encode(), 0);
    let query = vec![1, 0, 0];
    let plan = plan_map_edit(
        &bytes,
        used(&bytes),
        &desc,
        &InlinePath::new(),
        MapEdit::Insert,
        &query,
        4,
        100,
    )
    .unwrap();
    assert_eq!(plan.check_ready(&bytes), Err(ErrorKind::InvalidPlan.into()));
    let mut relocated = vec![0xa5; plan.required_storage_len() as usize];
    relocated[..bytes.len()].copy_from_slice(&bytes);
    bytes.clear();
    let mut apply = begin_apply(&mut relocated, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(&[5, 6, 7, 8]);
    apply.finish();
    let mut next = model.clone();
    next.apply(FixedMapEdit::Insert, &query, &[5, 6, 7, 8])
        .unwrap();
    assert_eq!(&relocated[12..], next.encode());
}

#[test]
fn incompatible_identity_and_presence_fail_before_writes() {
    let desc = layout(1, 1);
    let original = account(&[2, 0, 0, 0, 2, 7, 6, 8], 20);
    for mutation in 0..4 {
        let plan = plan_map_edit(
            &original,
            used(&original),
            &desc,
            &InlinePath::new(),
            MapEdit::Insert,
            &[4],
            1,
            original.len() as u32,
        )
        .unwrap();
        let mut changed = original.clone();
        match mutation {
            0 => changed[0] ^= 1,
            1 => changed[18] = 4,
            2 => changed[12] = 1,
            _ => changed[8] = 255,
        };
        let before = changed.clone();
        assert!(begin_apply(&mut changed, plan).is_err());
        assert_eq!(changed, before);
    }
    let nonmap = Layout::List {
        element: &Layout::Leaf(Leaf::Unsigned(1)),
    };
    fault(
        &[0, 0, 0, 0],
        &nonmap,
        MapEdit::Clear,
        &[],
        0,
        100,
        ErrorKind::InvalidSchema,
        Error::NO_OFFSET,
    );
    let mut bytes = original.clone();
    let mut path = InlinePath::new();
    path.push(Step::Index(0)).unwrap();
    assert_eq!(
        plan_map_edit(
            &bytes,
            used(&bytes),
            &desc,
            &path,
            MapEdit::Clear,
            &[],
            0,
            100
        )
        .unwrap_err(),
        ErrorKind::InvalidSchema.into()
    );
    assert_eq!(bytes, original);
    // A replacement exposes only the value even when the encoder changes every byte.
    let plan = plan_map_edit(
        &bytes,
        used(&bytes),
        &desc,
        &InlinePath::new(),
        MapEdit::Replace,
        &[2],
        1,
        100,
    )
    .unwrap();
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    apply.move_regions();
    apply.destination().fill(255);
    apply.finish();
    assert_eq!(bytes[16], 2);
    assert_eq!(bytes[18], 6);
}

#[test]
fn map_phase_misuse_panics_before_its_next_write() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let desc = layout(1, 1);
    let original = account(&[1, 0, 0, 0, 2, 7], 20);
    for phase in 0..4 {
        let mut bytes = original.clone();
        let plan = plan_map_edit(
            &bytes,
            used(&bytes),
            &desc,
            &InlinePath::new(),
            MapEdit::Replace,
            &[2],
            1,
            100,
        )
        .unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut apply = begin_apply(&mut bytes, plan).unwrap();
            match phase {
                0 => {
                    apply.destination();
                }
                1 => {
                    apply.finish();
                }
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
