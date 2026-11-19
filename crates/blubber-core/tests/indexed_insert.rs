//! Indexed insertion checked against independent complete encodings.

#![forbid(unsafe_code)]

use blubber_core::raw::{begin_apply, plan_list_edit, InlinePath, ListEdit, Step};
use blubber_core::{
    validate, Error, ErrorKind, KeyDomain, KeyLayout, Layout, Leaf, VariableField, MAX_DEPTH,
};

static STR: Layout = Layout::Str;
static LIST: Layout = Layout::List { element: &STR };

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

fn account(root: &[u8], spare: usize) -> Vec<u8> {
    let mut bytes = b"blubber!".to_vec();
    bytes.extend_from_slice(&(12 + root.len() as u32).to_le_bytes());
    bytes.extend_from_slice(root);
    bytes.resize(bytes.len() + spare, 0xa5);
    bytes
}

fn used(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[8..12].try_into().unwrap())
}

fn path(steps: &[Step]) -> InlinePath {
    let mut path = InlinePath::new();
    for step in steps {
        path.push(*step).unwrap();
    }
    path
}

fn check(old: &[u8], new: &[u8], layout: &Layout, steps: &[Step], ordinal: u32, input: &[u8]) {
    validate(old, layout, MAX_DEPTH).unwrap();
    let original = account(old, new.len() - old.len() + 11);
    let mut guarded = vec![0x7d; 3];
    guarded.extend_from_slice(&original);
    guarded.extend_from_slice(&[0x7e; 5]);
    let bytes = &mut guarded[3..3 + original.len()];
    let plan = plan_list_edit(
        bytes,
        used(bytes),
        layout,
        &path(steps),
        ListEdit::Insert(ordinal),
        input.len() as u32,
        bytes.len() as u32,
    )
    .unwrap();
    assert_eq!(plan.required_storage_len(), 12 + new.len() as u32);
    assert_eq!(plan.new_used_len(), 12 + new.len() as u32);
    assert_eq!(plan.removed_span(), None);
    plan.check_ready(bytes).unwrap();
    assert_eq!(bytes, original);
    let mut apply = begin_apply(bytes, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(input);
    apply.finish();
    assert_eq!(&bytes[..12 + new.len()], account(new, 0));
    assert_eq!(&bytes[12 + new.len()..], &original[12 + new.len()..]);
    validate(&bytes[12..12 + new.len()], layout, MAX_DEPTH).unwrap();
    assert_eq!(&guarded[..3], &[0x7d; 3]);
    assert_eq!(&guarded[3 + original.len()..], &[0x7e; 5]);
}

#[test]
fn every_small_ordinal_empty_pattern_and_input_length() {
    for count in 0..=6 {
        for pattern in 0..3usize.pow(count as u32) {
            let mut digits = pattern;
            let items: Vec<_> = (0..count)
                .map(|i| {
                    let value = vec![b'a' + i as u8; digits % 3];
                    digits /= 3;
                    value
                })
                .collect();
            let old = indexed(&items);
            for ordinal in 0..=count {
                for len in [0, 1, 2, 7, 33] {
                    let input = vec![b'z'; len];
                    let mut expected = items.clone();
                    expected.insert(ordinal, input.clone());
                    check(
                        &old,
                        &indexed(&expected),
                        &LIST,
                        &[],
                        ordinal as u32,
                        &input,
                    );
                }
            }
        }
    }
}

#[test]
fn append_synthesizes_old_implicit_terminal_after_payload_move() {
    for items in [
        vec![b"first".to_vec()],
        vec![vec![], b"last".to_vec()],
        vec![b"first".to_vec(), vec![]],
    ] {
        for input in [vec![], b"new".to_vec()] {
            let mut expected = items.clone();
            expected.push(input.clone());
            check(
                &indexed(&items),
                &indexed(&expected),
                &LIST,
                &[],
                items.len() as u32,
                &input,
            );
        }
    }
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
fn mixed_ancestors_preserve_outer_suffix_at_both_depth_limits() {
    for levels in [1, MAX_DEPTH - 2] {
        for count in 0..=3 {
            let items: Vec<_> = (0..count).map(|i| vec![b'a' + i as u8; i * 3]).collect();
            for ordinal in 0..=count {
                for input in [vec![], b"inserted payload".to_vec()] {
                    let mut expected = items.clone();
                    expected.insert(ordinal, input.clone());
                    let (layout, old, new, steps) =
                        wrap(&LIST, indexed(&items), indexed(&expected), levels);
                    check(&old, &new, layout, &steps, ordinal as u32, &input);
                }
            }
        }
    }
}

#[test]
fn payload_prefix_can_overlap_old_suffix_and_directory() {
    let items = vec![vec![b'a'; 1024], vec![b'b'; 1024], vec![b'c'; 1024]];
    for ordinal in 0..=3 {
        for len in [0, 1, 4096] {
            let input = vec![b'x'; len];
            let mut expected = items.clone();
            expected.insert(ordinal, input.clone());
            check(
                &indexed(&items),
                &indexed(&expected),
                &LIST,
                &[],
                ordinal as u32,
                &input,
            );
        }
    }
}

#[test]
fn option_children_are_complete_variable_encodings() {
    static BOOL_OPTION: Layout = Layout::Option {
        value: &Layout::Leaf(Leaf::Bool),
    };
    let list = Layout::List {
        element: &BOOL_OPTION,
    };
    let items = vec![vec![0], vec![1, 0], vec![1, 1]];
    for ordinal in 0..=3 {
        for input in [vec![0], vec![1, 1]] {
            let mut expected = items.clone();
            expected.insert(ordinal, input.clone());
            check(
                &indexed(&items),
                &indexed(&expected),
                &list,
                &[],
                ordinal as u32,
                &input,
            );
        }
    }
}

#[test]
fn counts_above_u16_and_empty_payloads() {
    let items = vec![vec![]; 65_537];
    for ordinal in [0, 65_535, 65_537] {
        let mut expected = items.clone();
        expected.insert(ordinal, vec![]);
        check(
            &indexed(&items),
            &indexed(&expected),
            &LIST,
            &[],
            ordinal as u32,
            &[],
        );
    }
}

fn reject(
    bytes: &[u8],
    layout: &Layout,
    operation: ListEdit,
    input: u32,
    cap: u32,
    kind: ErrorKind,
) {
    let original = bytes.to_vec();
    assert_eq!(
        plan_list_edit(
            bytes,
            used(bytes),
            layout,
            &InlinePath::new(),
            operation,
            input,
            cap
        )
        .unwrap_err(),
        Error::new(kind, Error::NO_OFFSET)
    );
    assert_eq!(bytes, original);
}

#[test]
fn invalid_indices_capacity_and_full_width_extent_overflow_are_read_only() {
    let bytes = account(&indexed(&[vec![], b"abc".to_vec()]), 0);
    reject(
        &bytes,
        &LIST,
        ListEdit::Insert(3),
        0,
        u32::MAX,
        ErrorKind::IndexOutOfBounds,
    );
    reject(
        &bytes,
        &LIST,
        ListEdit::Insert(u32::MAX),
        u32::MAX,
        u32::MAX,
        ErrorKind::IndexOutOfBounds,
    );
    reject(
        &bytes,
        &LIST,
        ListEdit::Insert(0),
        0,
        bytes.len() as u32,
        ErrorKind::Capacity,
    );
    reject(
        &bytes,
        &LIST,
        ListEdit::Insert(0),
        u32::MAX,
        u32::MAX,
        ErrorKind::ArithmeticOverflow,
    );
    reject(
        &bytes,
        &LIST,
        ListEdit::Insert(0),
        u32::MAX - 10,
        u32::MAX,
        ErrorKind::ArithmeticOverflow,
    );
    reject(
        &bytes,
        &STR,
        ListEdit::Insert(0),
        0,
        u32::MAX,
        ErrorKind::InvalidSchema,
    );
}

#[test]
fn corrupted_partitions_and_envelopes_preserve_every_byte() {
    for root in [
        vec![],
        vec![0; 3],
        [0u32.to_le_bytes().to_vec(), vec![1]].concat(),
        [3u32.to_le_bytes(), 2u32.to_le_bytes(), 1u32.to_le_bytes()].concat(),
        [2u32.to_le_bytes(), 1u32.to_le_bytes()].concat(),
        u32::MAX.to_le_bytes().to_vec(),
    ] {
        let bytes = account(&root, 0);
        let original = bytes.clone();
        assert!(plan_list_edit(
            &bytes,
            used(&bytes),
            &LIST,
            &InlinePath::new(),
            ListEdit::Insert(0),
            0,
            u32::MAX
        )
        .is_err());
        assert_eq!(bytes, original);
    }
    let mut bytes = account(&indexed(&[]), 0);
    bytes[8..12].copy_from_slice(&11u32.to_le_bytes());
    let original = bytes.clone();
    assert_eq!(
        plan_list_edit(
            &bytes,
            11,
            &LIST,
            &InlinePath::new(),
            ListEdit::Insert(0),
            0,
            u32::MAX
        )
        .unwrap_err(),
        Error::new(ErrorKind::UsedLength, 8)
    );
    assert_eq!(bytes, original);
}

#[test]
fn readiness_checks_boundary_but_accepts_compatible_sibling_changes() {
    let items = vec![b"aa".to_vec(), b"bb".to_vec(), b"cc".to_vec()];
    for compatible in [true, false] {
        let mut bytes = account(&indexed(&items), 16);
        let plan = plan_list_edit(
            &bytes,
            used(&bytes),
            &LIST,
            &InlinePath::new(),
            ListEdit::Insert(1),
            1,
            128,
        )
        .unwrap();
        // First boundary is the insertion point; second boundary may change.
        let at = if compatible { 20 } else { 16 };
        bytes[at..at + 4].copy_from_slice(&3u32.to_le_bytes());
        let original = bytes.clone();
        if compatible {
            let expected = indexed(&[
                b"aa".to_vec(),
                b"x".to_vec(),
                b"b".to_vec(),
                b"bcc".to_vec(),
            ]);
            let mut apply = begin_apply(&mut bytes, plan).unwrap();
            apply.move_regions();
            apply.destination().copy_from_slice(b"x");
            apply.finish();
            assert_eq!(&bytes[12..used(&bytes) as usize], expected);
        } else {
            assert_eq!(
                plan.check_ready(&bytes).unwrap_err(),
                Error::new(ErrorKind::InvalidPlan, Error::NO_OFFSET)
            );
            assert!(begin_apply(&mut bytes, plan).is_err());
            assert_eq!(bytes, original);
        }
    }
}

#[test]
fn relocation_and_exact_capacity() {
    for items in [vec![], vec![b"old".to_vec()]] {
        let bytes = account(&indexed(&items), 0);
        let mut expected = items.clone();
        expected.push(b"new".to_vec());
        let cap = (12 + indexed(&expected).len()) as u32;
        let plan = plan_list_edit(
            &bytes,
            used(&bytes),
            &LIST,
            &InlinePath::new(),
            ListEdit::Insert(items.len() as u32),
            3,
            cap,
        )
        .unwrap();
        assert!(plan.check_ready(&bytes).is_err());
        let mut relocated = bytes.clone();
        relocated.resize(cap as usize, 0xa5);
        assert_ne!(bytes.as_ptr(), relocated.as_ptr());
        let mut apply = begin_apply(&mut relocated, plan).unwrap();
        apply.move_regions();
        apply.destination().copy_from_slice(b"new");
        apply.finish();
        assert_eq!(relocated, account(&indexed(&expected), 0));
    }
}

#[test]
fn deterministic_insertion_sequences_match_owned_elements() {
    for seed in [1u64, 0x1234_5678_9abc_def0, 0xdead_beef] {
        let mut state = seed;
        let mut items: Vec<Vec<u8>> = vec![];
        let mut bytes = account(&indexed(&items), 0);
        for _ in 0..300 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let ordinal = state as usize % (items.len() + 1);
            let input = vec![b'a' + (state % 26) as u8; (state >> 32) as usize % 17];
            let plan = plan_list_edit(
                &bytes,
                used(&bytes),
                &LIST,
                &InlinePath::new(),
                ListEdit::Insert(ordinal as u32),
                input.len() as u32,
                65_536,
            )
            .unwrap();
            bytes.resize(plan.required_storage_len() as usize, 0);
            let mut apply = begin_apply(&mut bytes, plan).unwrap();
            apply.move_regions();
            apply.destination().copy_from_slice(&input);
            apply.finish();
            items.insert(ordinal, input);
            assert_eq!(bytes, account(&indexed(&items), 0));
        }
    }
}

#[test]
fn unfinished_apply_keeps_count_and_used_unpublished() {
    for items in [vec![], vec![b"a".to_vec(), b"bc".to_vec()]] {
        let mut bytes = account(&indexed(&items), 16);
        let old_used = used(&bytes);
        let plan = plan_list_edit(
            &bytes,
            old_used,
            &LIST,
            &InlinePath::new(),
            ListEdit::Insert(0),
            1,
            128,
        )
        .unwrap();
        let mut apply = begin_apply(&mut bytes, plan).unwrap();
        apply.move_regions();
        apply.destination().copy_from_slice(b"x");
        drop(apply);
        assert_eq!(used(&bytes), old_used);
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            items.len() as u32
        );
    }
}

#[test]
fn stale_identity_count_and_insufficient_storage_reject_before_writing() {
    for fault in 0..4 {
        let mut bytes = account(&indexed(&[b"aa".to_vec(), b"bb".to_vec()]), 16);
        let plan = plan_list_edit(
            &bytes,
            used(&bytes),
            &LIST,
            &InlinePath::new(),
            ListEdit::Insert(1),
            3,
            128,
        )
        .unwrap();
        match fault {
            0 => bytes[0] ^= 1,
            1 => bytes[12..16].copy_from_slice(&1u32.to_le_bytes()),
            2 => bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes()),
            _ => bytes.truncate(used(&bytes) as usize),
        }
        let original = bytes.clone();
        assert_eq!(
            plan.check_ready(&bytes).unwrap_err(),
            Error::new(ErrorKind::InvalidPlan, Error::NO_OFFSET)
        );
        assert!(begin_apply(&mut bytes, plan).is_err());
        assert_eq!(bytes, original);
    }
}

#[test]
fn old_leaf_domains_remain_lazy_during_structural_edit() {
    let old = indexed(&[vec![0xff], b"ok".to_vec()]);
    let new = indexed(&[vec![0xff], b"x".to_vec(), b"ok".to_vec()]);
    let mut bytes = account(&old, 16);
    let plan = plan_list_edit(
        &bytes,
        used(&bytes),
        &LIST,
        &InlinePath::new(),
        ListEdit::Insert(1),
        1,
        128,
    )
    .unwrap();
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(b"x");
    apply.finish();
    assert_eq!(&bytes[12..used(&bytes) as usize], new);
    assert!(matches!(
        validate(&new, &LIST, MAX_DEPTH).unwrap_err(),
        Error::Fault {
            kind: ErrorKind::InvalidUtf8,
            ..
        }
    ));
}
