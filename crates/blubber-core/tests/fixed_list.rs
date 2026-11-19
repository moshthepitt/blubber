//! Fixed-list splices checked against an independent owned sequence model.

#![forbid(unsafe_code)]

use blubber_core::raw::{begin_apply, plan_list_edit, InlinePath, ListEdit, Step};
use blubber_core::{
    validate, Error, ErrorKind, FixedField, KeyDomain, KeyLayout, Layout, Leaf, VariableField,
    MAX_DEPTH,
};
use blubber_model::{FixedList, FixedListEdit};
use std::panic::{catch_unwind, AssertUnwindSafe};

static UNIT: Layout = Layout::Struct {
    fixed_bytes: 0,
    fixed: &[],
    variable: &[],
};
static STR: Layout = Layout::Str;
static BOOL: Layout = Layout::Leaf(Leaf::Bool);
static RECORD: Layout = Layout::Struct {
    fixed_bytes: 3,
    fixed: &[
        FixedField {
            declaration: 0,
            offset: 0,
            layout: &BOOL,
        },
        FixedField {
            declaration: 1,
            offset: 1,
            layout: &Layout::Leaf(Leaf::Unsigned(2)),
        },
    ],
    variable: &[],
};

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

fn path(steps: &[Step]) -> InlinePath {
    let mut path = InlinePath::new();
    for step in steps {
        path.push(*step).unwrap();
    }
    path
}

fn oracle_operation(operation: ListEdit) -> FixedListEdit {
    match operation {
        ListEdit::Insert(i) => FixedListEdit::Insert(i as usize),
        ListEdit::Remove(i) => FixedListEdit::Remove(i as usize),
        ListEdit::Replace(i) => FixedListEdit::Replace(i as usize),
        ListEdit::Pop => FixedListEdit::Pop,
        ListEdit::Truncate(n) => FixedListEdit::Truncate(n as usize),
        ListEdit::Clear => FixedListEdit::Clear,
    }
}

fn check(
    old: &[u8],
    new: &[u8],
    layout: &Layout,
    steps: &[Step],
    operation: ListEdit,
    input: &[u8],
    removed: Option<Vec<u8>>,
) {
    validate(old, layout, MAX_DEPTH).unwrap();
    let original = account(old, new.len().saturating_sub(old.len()) + 13);
    let mut guarded = vec![0x7d; 3];
    guarded.extend_from_slice(&original);
    guarded.extend_from_slice(&[0x7e; 5]);
    let bytes = &mut guarded[3..3 + original.len()];
    let plan = plan_list_edit(
        bytes,
        used(bytes),
        layout,
        &path(steps),
        operation,
        input.len() as u32,
        bytes.len() as u32,
    )
    .unwrap();
    assert_eq!(plan.new_used_len(), 12 + new.len() as u32);
    assert_eq!(
        plan.required_storage_len(),
        12 + old.len().max(new.len()) as u32
    );
    plan.check_ready(bytes).unwrap();
    assert_eq!(bytes, original);
    let returned = plan
        .removed_span()
        .map(|span| bytes[span.start as usize..span.end as usize].to_vec());
    assert_eq!(returned, removed);
    let mut apply = begin_apply(bytes, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(input);
    apply.finish();
    let expected = account(new, 0);
    assert_eq!(&bytes[..expected.len()], expected);
    let new_used = expected.len();
    let old_used = 12 + old.len();
    if new_used < old_used {
        assert!(bytes[new_used..old_used].iter().all(|byte| *byte == 0));
    }
    assert_eq!(
        &bytes[old_used.max(new_used)..],
        &original[old_used.max(new_used)..]
    );
    validate(&bytes[12..new_used], layout, MAX_DEPTH).unwrap();
    assert_eq!(&guarded[..3], &[0x7d; 3]);
    assert_eq!(&guarded[3 + original.len()..], &[0x7e; 5]);
}

fn operations(count: u32) -> Vec<ListEdit> {
    let mut operations = vec![ListEdit::Pop, ListEdit::Clear];
    operations.extend((0..=count).map(ListEdit::Insert));
    operations.extend((0..count).map(ListEdit::Remove));
    operations.extend((0..count).map(ListEdit::Replace));
    operations.extend((0..=count + 2).map(ListEdit::Truncate));
    operations.push(ListEdit::Truncate(u32::MAX));
    operations
}

#[test]
fn all_operations_every_ordinal_and_fixed_width_against_owned_model() {
    for width in [1, 2, 3, 16, 257] {
        let element = if width == 0 {
            &UNIT
        } else {
            Box::leak(Box::new(Layout::Leaf(Leaf::Raw(width as u32))))
        };
        let layout = Layout::List { element };
        for count in 0..=8 {
            let items: Vec<_> = (0..count).map(|i| vec![i as u8 + 1; width]).collect();
            let model = FixedList::new(width, items).unwrap();
            let old = model.encode();
            for operation in operations(count) {
                let input = match operation {
                    ListEdit::Insert(_) | ListEdit::Replace(_) => vec![0x7c; width],
                    _ => vec![],
                };
                let mut expected = model.clone();
                let removed = expected.apply(oracle_operation(operation), &input).unwrap();
                check(
                    &old,
                    &expected.encode(),
                    &layout,
                    &[],
                    operation,
                    &input,
                    removed,
                );
            }
        }
    }
}

fn indexed(items: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = (items.len() as u32).to_le_bytes().to_vec();
    let mut end = 0u32;
    for item in items.iter().take(items.len().saturating_sub(1)) {
        end += item.len() as u32;
        bytes.extend_from_slice(&end.to_le_bytes());
    }
    for item in items {
        bytes.extend_from_slice(item);
    }
    bytes
}

fn wrap(
    mut layout: &'static Layout,
    mut old: Vec<u8>,
    mut new: Vec<u8>,
    levels: usize,
) -> (&'static Layout, Vec<u8>, Vec<u8>, Vec<Step>) {
    let mut steps = Vec::new();
    for level in 0..levels {
        let selector;
        match level % 4 {
            0 => {
                layout = Box::leak(Box::new(Layout::Struct {
                    fixed_bytes: 0,
                    fixed: &[],
                    variable: Box::leak(Box::new([
                        VariableField {
                            declaration: 0,
                            layout: &STR,
                        },
                        VariableField {
                            declaration: 1,
                            layout,
                        },
                        VariableField {
                            declaration: 2,
                            layout: &STR,
                        },
                    ])),
                }));
                let encode = |child: Vec<u8>| {
                    [
                        3u32.to_le_bytes().to_vec(),
                        (3 + child.len() as u32).to_le_bytes().to_vec(),
                        b"pre".to_vec(),
                        child,
                        b"outer tail".to_vec(),
                    ]
                    .concat()
                };
                old = encode(old);
                new = encode(new);
                selector = Step::Field(1);
            }
            1 => {
                layout = Box::leak(Box::new(Layout::List { element: layout }));
                new = indexed(&[old.clone(), new, old.clone()]);
                old = indexed(&[old.clone(), old.clone(), old]);
                selector = Step::Index(1);
            }
            2 => {
                layout = Box::leak(Box::new(Layout::Map {
                    key: KeyLayout {
                        width: 3,
                        domain: KeyDomain::Raw,
                    },
                    value: layout,
                }));
                let encode = |values: &[Vec<u8>]| {
                    let mut bytes = (values.len() as u32).to_le_bytes().to_vec();
                    let mut end = 0u32;
                    for (i, value) in values.iter().enumerate() {
                        bytes.extend_from_slice(&[0, 0, i as u8 + 1]);
                        end += value.len() as u32;
                        bytes.extend_from_slice(&end.to_le_bytes());
                    }
                    for value in values {
                        bytes.extend_from_slice(value);
                    }
                    bytes
                };
                new = encode(&[old.clone(), new, old.clone()]);
                old = encode(&[old.clone(), old.clone(), old]);
                selector = Step::Index(1);
            }
            _ => {
                layout = Box::leak(Box::new(Layout::Option { value: layout }));
                old.insert(0, 1);
                new.insert(0, 1);
                selector = Step::Some;
            }
        }
        steps.insert(0, selector);
    }
    (layout, old, new, steps)
}

#[test]
fn nested_outer_suffix_and_every_boundary_run_at_maximum_depth() {
    static LIST: Layout = Layout::List {
        element: &Layout::Leaf(Leaf::Raw(3)),
    };
    let model = FixedList::new(3, vec![vec![1; 3], vec![2; 3], vec![3; 3]]).unwrap();
    for levels in [1, MAX_DEPTH - 2] {
        for operation in operations(3) {
            let input = match operation {
                ListEdit::Insert(_) | ListEdit::Replace(_) => vec![9; 3],
                _ => vec![],
            };
            let mut changed = model.clone();
            let removed = changed.apply(oracle_operation(operation), &input).unwrap();
            let (layout, old, new, steps) = wrap(&LIST, model.encode(), changed.encode(), levels);
            check(&old, &new, layout, &steps, operation, &input, removed);
        }
    }
}

#[test]
fn packed_fixed_records_validate_bool_domains_after_edits() {
    let layout = Layout::List { element: &RECORD };
    let model = FixedList::new(3, vec![vec![0, 12, 0], vec![1, 34, 0]]).unwrap();
    for operation in operations(2) {
        let input = match operation {
            ListEdit::Insert(_) | ListEdit::Replace(_) => vec![1, 0x34, 0x12],
            _ => vec![],
        };
        let mut changed = model.clone();
        let removed = changed.apply(oracle_operation(operation), &input).unwrap();
        check(
            &model.encode(),
            &changed.encode(),
            &layout,
            &[],
            operation,
            &input,
            removed,
        );
    }
}

fn reject(bytes: &[u8], layout: &Layout, operation: ListEdit, input_len: u32, kind: ErrorKind) {
    let before = bytes.to_vec();
    let error = plan_list_edit(
        bytes,
        used(bytes),
        layout,
        &InlinePath::new(),
        operation,
        input_len,
        bytes.len() as u32,
    )
    .unwrap_err();
    assert!(matches!(error, Error::Fault { kind: found, .. } if found == kind));
    assert_eq!(bytes, before);
}

#[test]
fn bounds_width_and_kind_errors_preserve_every_byte() {
    let layout = Layout::List {
        element: &Layout::Leaf(Leaf::Raw(3)),
    };
    let bytes = account(&FixedList::new(3, vec![vec![1; 3]]).unwrap().encode(), 20);
    for operation in [
        ListEdit::Insert(2),
        ListEdit::Remove(1),
        ListEdit::Replace(1),
        ListEdit::Remove(u32::MAX),
    ] {
        reject(&bytes, &layout, operation, 3, ErrorKind::IndexOutOfBounds);
    }
    for operation in operations(1) {
        let wrong = match operation {
            ListEdit::Insert(_) | ListEdit::Replace(_) => 2,
            _ => 1,
        };
        reject(
            &bytes,
            &layout,
            operation,
            wrong,
            ErrorKind::InvalidBoundary,
        );
    }
    let variable = Layout::List { element: &STR };
    reject(
        &account(&0u32.to_le_bytes(), 4),
        &variable,
        ListEdit::Clear,
        1,
        ErrorKind::InvalidBoundary,
    );
    reject(
        &account(b"text", 4),
        &STR,
        ListEdit::Clear,
        0,
        ErrorKind::InvalidSchema,
    );
    let invalid = Layout::List {
        element: &Layout::Leaf(Leaf::Unsigned(3)),
    };
    reject(
        &bytes,
        &invalid,
        ListEdit::Clear,
        0,
        ErrorKind::InvalidSchema,
    );
}

#[test]
fn corrupted_envelopes_partitions_and_selector_errors_are_read_only() {
    let layout = Layout::List { element: &BOOL };
    let original = account(&[2, 0, 0, 0, 0, 1], 10);
    for prefix in 0..original.len() {
        let bytes = &original[..prefix];
        let before = bytes.to_vec();
        let _ = plan_list_edit(
            bytes,
            used(&original),
            &layout,
            &InlinePath::new(),
            ListEdit::Clear,
            0,
            original.len() as u32,
        );
        assert_eq!(bytes, before);
    }
    for at in [8, 12] {
        let mut bytes = original.clone();
        bytes[at] = 255;
        let before = bytes.clone();
        assert!(plan_list_edit(
            &bytes,
            used(&bytes),
            &layout,
            &InlinePath::new(),
            ListEdit::Clear,
            0,
            100
        )
        .is_err());
        assert_eq!(bytes, before);
    }
    assert!(plan_list_edit(
        &original,
        used(&original),
        &layout,
        &path(&[Step::Index(0)]),
        ListEdit::Clear,
        0,
        100
    )
    .is_err());
}

#[test]
fn capacity_growth_relocation_and_stale_plan_failures() {
    let layout = Layout::List {
        element: &Layout::Leaf(Leaf::Raw(2)),
    };
    let model = FixedList::new(2, vec![vec![1; 2], vec![2; 2]]).unwrap();
    let original = account(&model.encode(), 0);
    reject(
        &original,
        &layout,
        ListEdit::Insert(1),
        2,
        ErrorKind::Capacity,
    );
    let plan = plan_list_edit(
        &original,
        used(&original),
        &layout,
        &InlinePath::new(),
        ListEdit::Insert(1),
        2,
        100,
    )
    .unwrap();
    let mut bytes = original.clone();
    assert_eq!(
        begin_apply(&mut bytes, plan).err(),
        Some(ErrorKind::InvalidPlan.into())
    );
    assert_eq!(bytes, original);
    let plan = plan_list_edit(
        &original,
        used(&original),
        &layout,
        &InlinePath::new(),
        ListEdit::Insert(1),
        2,
        100,
    )
    .unwrap();
    let mut relocated = original.clone();
    relocated.resize(100, 0xcc);
    relocated[16] = 7; // Compatible content changes remain current.
    let mut apply = begin_apply(&mut relocated, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(&[9, 9]);
    apply.finish();
    assert_eq!(&relocated[12..22], &[3, 0, 0, 0, 7, 1, 9, 9, 2, 2]);
    assert!(relocated[22..].iter().all(|byte| *byte == 0xcc));
    for at in [0, 8, 12] {
        let plan = plan_list_edit(
            &original,
            used(&original),
            &layout,
            &InlinePath::new(),
            ListEdit::Remove(0),
            0,
            100,
        )
        .unwrap();
        let mut bytes = original.clone();
        bytes[at] ^= 1;
        let before = bytes.clone();
        assert_eq!(
            begin_apply(&mut bytes, plan).err(),
            Some(ErrorKind::InvalidPlan.into())
        );
        assert_eq!(bytes, before);
    }
}

#[test]
fn wide_counts_extent_overflow_and_zero_width_rejection() {
    let layout = Layout::List { element: &BOOL };
    for count in [u32::MAX, u32::MAX - 3] {
        reject(
            &account(&count.to_le_bytes(), 0),
            &layout,
            ListEdit::Clear,
            0,
            ErrorKind::ArithmeticOverflow,
        );
    }
    let model = FixedList::new(1, vec![vec![1]; 65_537]).unwrap();
    for operation in [
        ListEdit::Remove(65_536),
        ListEdit::Pop,
        ListEdit::Truncate(65_536),
        ListEdit::Clear,
    ] {
        let mut changed = model.clone();
        let removed = changed.apply(oracle_operation(operation), &[]).unwrap();
        check(
            &model.encode(),
            &changed.encode(),
            &layout,
            &[],
            operation,
            &[],
            removed,
        );
    }
    let huge = Layout::List {
        element: &Layout::Leaf(Leaf::Raw(u32::MAX)),
    };
    reject(
        &account(&0u32.to_le_bytes(), 0),
        &huge,
        ListEdit::Insert(0),
        u32::MAX,
        ErrorKind::ArithmeticOverflow,
    );
    let zero = Layout::List { element: &UNIT };
    reject(
        &account(&0u32.to_le_bytes(), 0),
        &zero,
        ListEdit::Pop,
        0,
        ErrorKind::InvalidSchema,
    );
}

#[test]
fn pop_empty_and_truncate_above_count_leave_complete_storage_unchanged() {
    let layout = Layout::List { element: &BOOL };
    for (root, operation) in [
        (vec![0, 0, 0, 0], ListEdit::Pop),
        (vec![0, 0, 0, 0], ListEdit::Clear),
        (vec![2, 0, 0, 0, 0, 1], ListEdit::Truncate(u32::MAX)),
    ] {
        let mut bytes = account(&root, 15);
        let before = bytes.clone();
        let plan = plan_list_edit(
            &bytes,
            used(&bytes),
            &layout,
            &InlinePath::new(),
            operation,
            0,
            bytes.len() as u32,
        )
        .unwrap();
        assert_eq!(plan.removed_span(), None);
        let mut apply = begin_apply(&mut bytes, plan).unwrap();
        apply.move_regions();
        assert!(apply.destination().is_empty());
        apply.finish();
        assert_eq!(bytes, before);
    }
}

#[test]
fn count_and_used_length_publish_only_at_finish() {
    let layout = Layout::List {
        element: &Layout::Leaf(Leaf::Raw(2)),
    };
    let root = FixedList::new(2, vec![vec![1; 2], vec![2; 2]])
        .unwrap()
        .encode();
    for operation in [ListEdit::Insert(0), ListEdit::Remove(0), ListEdit::Clear] {
        let mut bytes = account(&root, 10);
        let before = bytes.clone();
        let input_len = if matches!(operation, ListEdit::Insert(_)) {
            2
        } else {
            0
        };
        let plan = plan_list_edit(
            &bytes,
            used(&bytes),
            &layout,
            &InlinePath::new(),
            operation,
            input_len,
            bytes.len() as u32,
        )
        .unwrap();
        {
            let mut apply = begin_apply(&mut bytes, plan).unwrap();
            apply.move_regions();
            apply.destination().fill(9);
        }
        assert_eq!(&bytes[..16], &before[..16]);
    }
}

#[test]
fn invalid_phase_calls_panic_before_modifying_list_metadata() {
    let layout = Layout::List { element: &BOOL };
    let mut bytes = account(&[1, 0, 0, 0, 1], 10);
    let plan = plan_list_edit(
        &bytes,
        used(&bytes),
        &layout,
        &InlinePath::new(),
        ListEdit::Insert(0),
        1,
        bytes.len() as u32,
    )
    .unwrap();
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = apply.destination();
    }))
    .is_err());
    apply.move_regions();
    assert!(catch_unwind(AssertUnwindSafe(|| apply.move_regions())).is_err());
    apply.destination().copy_from_slice(&[0]);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = apply.destination();
    }))
    .is_err());
    apply.finish();
    assert_eq!(&bytes[12..18], &[2, 0, 0, 0, 0, 1]);
}

#[test]
fn existing_unrelated_invalid_bool_remains_lazy() {
    let layout = Layout::List { element: &BOOL };
    let mut bytes = account(&[2, 0, 0, 0, 255, 1], 10);
    let plan = plan_list_edit(
        &bytes,
        used(&bytes),
        &layout,
        &InlinePath::new(),
        ListEdit::Insert(1),
        1,
        bytes.len() as u32,
    )
    .unwrap();
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(&[0]);
    apply.finish();
    assert_eq!(&bytes[12..19], &[3, 0, 0, 0, 255, 0, 1]);
    assert_eq!(
        validate(&bytes[12..19], &layout, MAX_DEPTH),
        Err(Error::new(ErrorKind::InvalidBool, 4))
    );
}

#[test]
fn recorded_operation_streams_preserve_model_and_shrink_regrow_frames() {
    let layout = Layout::List {
        element: &Layout::Leaf(Leaf::Raw(3)),
    };
    for seed in [0x1b62_93e7u64, 0x4d88_f2a1, 0xc0ff_ee14] {
        let mut random = seed;
        let mut model = FixedList::new(3, vec![]).unwrap();
        let mut bytes = account(&model.encode(), 1024);
        for _ in 0..1_000 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let count = model.elements().len() as u32;
            let index = (random >> 32) as u32;
            let operation = match random % 7 {
                0 => ListEdit::Insert(index % (count + 1)),
                1 if count > 0 => ListEdit::Remove(index % count),
                2 if count > 0 => ListEdit::Replace(index % count),
                3 => ListEdit::Pop,
                4 => ListEdit::Clear,
                _ => ListEdit::Truncate(index % (count + 3)),
            };
            let input = match operation {
                ListEdit::Insert(_) | ListEdit::Replace(_) => random.to_le_bytes()[..3].to_vec(),
                _ => vec![],
            };
            let before = bytes.clone();
            let old_used = used(&bytes) as usize;
            let plan = plan_list_edit(
                &bytes,
                used(&bytes),
                &layout,
                &InlinePath::new(),
                operation,
                input.len() as u32,
                bytes.len() as u32,
            )
            .unwrap();
            let returned = plan
                .removed_span()
                .map(|span| bytes[span.start as usize..span.end as usize].to_vec());
            assert_eq!(
                returned,
                model.apply(oracle_operation(operation), &input).unwrap()
            );
            let mut apply = begin_apply(&mut bytes, plan).unwrap();
            apply.move_regions();
            apply.destination().copy_from_slice(&input);
            apply.finish();
            let expected = account(&model.encode(), 0);
            assert_eq!(&bytes[..expected.len()], expected);
            let new_used = used(&bytes) as usize;
            if new_used < old_used {
                assert!(bytes[new_used..old_used].iter().all(|byte| *byte == 0));
            }
            assert_eq!(
                &bytes[new_used.max(old_used)..],
                &before[new_used.max(old_used)..]
            );
            validate(&bytes[12..new_used], &layout, MAX_DEPTH).unwrap();
        }
    }
}
