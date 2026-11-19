//! Indexed removal and bulk edits checked against owned element sequences.

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

fn check(items: &[Vec<u8>], operation: ListEdit, input: &[u8], levels: usize) {
    let mut expected = items.to_vec();
    let removed = match operation {
        ListEdit::Insert(i) => {
            expected.insert(i as usize, input.to_vec());
            None
        }
        ListEdit::Remove(i) => Some(expected.remove(i as usize)),
        ListEdit::Replace(i) => {
            expected[i as usize] = input.to_vec();
            None
        }
        ListEdit::Pop => expected.pop(),
        ListEdit::Truncate(n) => {
            expected.truncate(n as usize);
            None
        }
        ListEdit::Clear => {
            expected.clear();
            None
        }
    };
    let (layout, old, new, steps) = wrap(&LIST, indexed(items), indexed(&expected), levels);
    validate(&old, layout, MAX_DEPTH).unwrap();
    let original = account(&old, new.len().saturating_sub(old.len()) + 13);
    let mut guarded = vec![0x7d; 3];
    guarded.extend_from_slice(&original);
    guarded.extend_from_slice(&[0x7e; 5]);
    let bytes = &mut guarded[3..3 + original.len()];
    let old_used = used(bytes);
    let plan = plan_list_edit(
        bytes,
        old_used,
        layout,
        &path(&steps),
        operation,
        input.len() as u32,
        bytes.len() as u32,
    )
    .unwrap();
    assert_eq!(plan.new_used_len(), 12 + new.len() as u32);
    assert_eq!(
        plan.required_storage_len(),
        old_used.max(12 + new.len() as u32)
    );
    assert_eq!(
        plan.removed_span()
            .map(|s| bytes[s.start as usize..s.end as usize].to_vec()),
        removed
    );
    plan.check_ready(bytes).unwrap();
    assert_eq!(bytes, original);
    let mut apply = begin_apply(bytes, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(input);
    apply.finish();
    assert_eq!(&bytes[..12 + new.len()], account(&new, 0));
    if new.len() < old.len() {
        assert!(bytes[12 + new.len()..old_used as usize]
            .iter()
            .all(|b| *b == 0));
    }
    let spare = (12 + new.len()).max(old_used as usize);
    assert_eq!(&bytes[spare..], &original[spare..]);
    validate(&bytes[12..used(bytes) as usize], layout, MAX_DEPTH).unwrap();
    assert_eq!(&guarded[..3], &[0x7d; 3]);
    assert_eq!(&guarded[3 + original.len()..], &[0x7e; 5]);
}

#[test]
fn all_small_removals_truncations_and_empty_patterns() {
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
            for i in 0..count {
                check(&items, ListEdit::Remove(i as u32), &[], 0);
            }
            for n in 0..=count + 1 {
                check(&items, ListEdit::Truncate(n as u32), &[], 0);
            }
            check(&items, ListEdit::Pop, &[], 0);
            check(&items, ListEdit::Clear, &[], 0);
        }
    }
}

#[test]
fn replacement_resizes_each_element_without_changing_count() {
    for count in 1..=4 {
        let items: Vec<_> = (0..count).map(|i| vec![b'a' + i as u8; i * 7]).collect();
        for i in 0..count {
            for len in [0, 1, 7, 31, 1024] {
                check(&items, ListEdit::Replace(i as u32), &vec![b'x'; len], 0);
            }
        }
    }
}

#[test]
fn nested_edits_preserve_siblings_and_repair_all_ancestor_kinds() {
    for levels in [1, MAX_DEPTH - 2] {
        for count in 0..=4 {
            let items: Vec<_> = (0..count).map(|i| vec![b'a' + i as u8; i * 3]).collect();
            for i in 0..count {
                check(&items, ListEdit::Remove(i as u32), &[], levels);
                for len in [0, 1, 3, 6, 9, 19] {
                    check(
                        &items,
                        ListEdit::Replace(i as u32),
                        &vec![b'x'; len],
                        levels,
                    );
                }
            }
            for n in 0..=count + 1 {
                check(&items, ListEdit::Truncate(n as u32), &[], levels);
            }
            check(&items, ListEdit::Pop, &[], levels);
            check(&items, ListEdit::Clear, &[], levels);
        }
    }
}

#[test]
fn directory_is_compacted_before_overlapping_payload_prefix() {
    let items = vec![
        vec![b'a'; 257],
        vec![b'b'; 1024],
        vec![b'c'; 11],
        vec![b'd'; 17],
    ];
    for i in 0..items.len() {
        check(&items, ListEdit::Remove(i as u32), &[], 1);
    }
    for n in 0..=items.len() {
        check(&items, ListEdit::Truncate(n as u32), &[], 1);
    }
}

#[test]
fn zero_byte_elements_still_change_count_and_table() {
    for count in [0, 1, 2, 3, 65_537] {
        let items = vec![vec![]; count];
        check(&items, ListEdit::Pop, &[], 0);
        check(&items, ListEdit::Clear, &[], 0);
        for n in [0, 1, 2, count, count + 1] {
            check(&items, ListEdit::Truncate(n as u32), &[], 0);
        }
        if count != 0 {
            for i in [0, count / 2, count - 1] {
                check(&items, ListEdit::Remove(i as u32), &[], 0);
            }
        }
    }
}

#[test]
fn some_empty_return_is_distinct_from_empty_pop() {
    check(&[vec![]], ListEdit::Remove(0), &[], 0);
    check(&[vec![]], ListEdit::Pop, &[], 0);
    check(&[], ListEdit::Pop, &[], 0);
}

#[test]
fn no_ops_preserve_the_complete_buffer() {
    for items in [vec![], vec![vec![]], vec![b"abc".to_vec(), b"d".to_vec()]] {
        let operations = [
            ListEdit::Truncate(items.len() as u32),
            ListEdit::Truncate(u32::MAX),
        ];
        for operation in operations.into_iter().chain(if items.is_empty() {
            vec![ListEdit::Pop, ListEdit::Clear]
        } else {
            vec![]
        }) {
            let mut bytes = account(&indexed(&items), 17);
            let original = bytes.clone();
            let plan = plan_list_edit(
                &bytes,
                used(&bytes),
                &LIST,
                &InlinePath::new(),
                operation,
                0,
                bytes.len() as u32,
            )
            .unwrap();
            let mut apply = begin_apply(&mut bytes, plan).unwrap();
            apply.move_regions();
            assert!(apply.destination().is_empty());
            apply.finish();
            assert_eq!(bytes, original);
        }
    }
}

#[test]
fn invalid_indices_inputs_and_extent_are_read_only() {
    let bytes = account(&indexed(&[b"abc".to_vec(), vec![]]), 0);
    for (operation, len, expected) in [
        (ListEdit::Remove(2), 0, ErrorKind::IndexOutOfBounds),
        (
            ListEdit::Replace(u32::MAX),
            u32::MAX,
            ErrorKind::IndexOutOfBounds,
        ),
        (ListEdit::Remove(0), 1, ErrorKind::InvalidBoundary),
        (ListEdit::Pop, 1, ErrorKind::InvalidBoundary),
        (ListEdit::Truncate(u32::MAX), 1, ErrorKind::InvalidBoundary),
        (ListEdit::Clear, 1, ErrorKind::InvalidBoundary),
        (
            ListEdit::Replace(0),
            u32::MAX,
            ErrorKind::ArithmeticOverflow,
        ),
        (ListEdit::Replace(0), 100, ErrorKind::Capacity),
    ] {
        let original = bytes.clone();
        let limit = if expected == ErrorKind::Capacity {
            bytes.len() as u32
        } else {
            u32::MAX
        };
        assert_eq!(
            plan_list_edit(
                &bytes,
                used(&bytes),
                &LIST,
                &InlinePath::new(),
                operation,
                len,
                limit
            )
            .unwrap_err(),
            Error::new(expected, Error::NO_OFFSET)
        );
        assert_eq!(bytes, original);
    }
}

#[test]
fn malformed_directories_reject_before_even_no_op_edits() {
    for root in [
        vec![],
        vec![0; 3],
        [0u32.to_le_bytes().to_vec(), vec![1]].concat(),
        [3u32.to_le_bytes(), 2u32.to_le_bytes(), 1u32.to_le_bytes()].concat(),
        [2u32.to_le_bytes(), 1u32.to_le_bytes()].concat(),
        u32::MAX.to_le_bytes().to_vec(),
    ] {
        let bytes = account(&root, 0);
        for op in [
            ListEdit::Remove(0),
            ListEdit::Replace(0),
            ListEdit::Pop,
            ListEdit::Truncate(u32::MAX),
            ListEdit::Clear,
        ] {
            assert!(plan_list_edit(
                &bytes,
                used(&bytes),
                &LIST,
                &InlinePath::new(),
                op,
                0,
                u32::MAX
            )
            .is_err());
        }
    }
}

#[test]
fn readiness_captures_both_selected_ends_and_count() {
    for operation in [
        ListEdit::Remove(1),
        ListEdit::Replace(1),
        ListEdit::Truncate(1),
        ListEdit::Pop,
        ListEdit::Clear,
    ] {
        for fault in 0..4 {
            let mut bytes = account(
                &indexed(&[b"aa".to_vec(), b"bb".to_vec(), b"cc".to_vec()]),
                32,
            );
            let plan = plan_list_edit(
                &bytes,
                used(&bytes),
                &LIST,
                &InlinePath::new(),
                operation,
                0,
                128,
            )
            .unwrap();
            match fault {
                0 => bytes[0] ^= 1,
                1 => bytes[12..16].copy_from_slice(&1u32.to_le_bytes()),
                2 => bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes()),
                _ => bytes.truncate(used(&bytes) as usize - 1),
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
    for at in [16, 20] {
        let mut bytes = account(
            &indexed(&[b"aa".to_vec(), b"bb".to_vec(), b"cc".to_vec()]),
            16,
        );
        let plan = plan_list_edit(
            &bytes,
            used(&bytes),
            &LIST,
            &InlinePath::new(),
            ListEdit::Remove(1),
            0,
            128,
        )
        .unwrap();
        bytes[at..at + 4].copy_from_slice(&3u32.to_le_bytes());
        assert!(plan.check_ready(&bytes).is_err());
    }
}

#[test]
fn compatible_sibling_ends_relocation_and_spare_changes_are_accepted() {
    let mut bytes = account(
        &indexed(&[
            b"aa".to_vec(),
            b"bb".to_vec(),
            b"cc".to_vec(),
            b"dd".to_vec(),
        ]),
        16,
    );
    let plan = plan_list_edit(
        &bytes,
        used(&bytes),
        &LIST,
        &InlinePath::new(),
        ListEdit::Remove(0),
        0,
        128,
    )
    .unwrap();
    bytes[20..24].copy_from_slice(&5u32.to_le_bytes());
    let mut relocated = bytes.clone();
    relocated.resize(100, 0x7e);
    let mut apply = begin_apply(&mut relocated, plan).unwrap();
    apply.move_regions();
    apply.destination();
    apply.finish();
    assert_eq!(
        &relocated[12..used(&relocated) as usize],
        indexed(&[b"bbc".to_vec(), b"c".to_vec(), b"dd".to_vec()])
    );
}

#[test]
fn growing_replacement_requires_storage_before_apply() {
    let bytes = account(&indexed(&[b"old".to_vec(), vec![]]), 0);
    let plan = plan_list_edit(
        &bytes,
        used(&bytes),
        &LIST,
        &InlinePath::new(),
        ListEdit::Replace(0),
        100,
        128,
    )
    .unwrap();
    assert!(plan.check_ready(&bytes).is_err());
    let mut grown = bytes.clone();
    grown.resize(plan.required_storage_len() as usize, 0);
    let mut apply = begin_apply(&mut grown, plan).unwrap();
    apply.move_regions();
    apply.destination().fill(b'x');
    apply.finish();
    assert_eq!(grown, account(&indexed(&[vec![b'x'; 100], vec![]]), 0));
}

#[test]
fn count_and_used_remain_unpublished_until_finish() {
    let mut bytes = account(
        &indexed(&[b"a".to_vec(), b"bc".to_vec(), b"def".to_vec()]),
        16,
    );
    let old_used = used(&bytes);
    let plan = plan_list_edit(
        &bytes,
        old_used,
        &LIST,
        &InlinePath::new(),
        ListEdit::Remove(1),
        0,
        128,
    )
    .unwrap();
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    apply.move_regions();
    apply.destination();
    drop(apply);
    assert_eq!(used(&bytes), old_used);
    assert_eq!(&bytes[12..16], &3u32.to_le_bytes());
}

#[test]
fn structural_edits_leave_unrelated_leaf_domains_lazy() {
    let mut bytes = account(
        &indexed(&[vec![0xff], b"ok".to_vec(), b"gone".to_vec()]),
        16,
    );
    let plan = plan_list_edit(
        &bytes,
        used(&bytes),
        &LIST,
        &InlinePath::new(),
        ListEdit::Remove(2),
        0,
        128,
    )
    .unwrap();
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    apply.move_regions();
    apply.destination();
    apply.finish();
    assert_eq!(
        &bytes[12..used(&bytes) as usize],
        indexed(&[vec![0xff], b"ok".to_vec()])
    );
    assert!(validate(&bytes[12..used(&bytes) as usize], &LIST, MAX_DEPTH).is_err());
}

#[test]
fn option_elements_and_deterministic_mixed_sequences() {
    static OPTION: Layout = Layout::Option {
        value: &Layout::Leaf(Leaf::Bool),
    };
    let layout = Layout::List { element: &OPTION };
    for seed in [1u64, 0x1234_5678_9abc_def0, 0xdead_beef] {
        let mut state = seed;
        let mut items: Vec<Vec<u8>> = vec![];
        let mut bytes = account(&indexed(&items), 16);
        for _ in 0..500 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let index = state as usize % (items.len() + 1);
            let input = if state & 1 == 0 {
                vec![0]
            } else {
                vec![1, (state >> 1) as u8 & 1]
            };
            let operation = match state % 6 {
                0 => ListEdit::Truncate(index as u32),
                1 => ListEdit::Clear,
                2 => ListEdit::Pop,
                3 if index < items.len() => ListEdit::Remove(index as u32),
                4 if index < items.len() => ListEdit::Replace(index as u32),
                _ => ListEdit::Insert(index as u32),
            };
            let input = if matches!(operation, ListEdit::Insert(_) | ListEdit::Replace(_)) {
                input
            } else {
                vec![]
            };
            let plan = plan_list_edit(
                &bytes,
                used(&bytes),
                &layout,
                &InlinePath::new(),
                operation,
                input.len() as u32,
                65_536,
            )
            .unwrap();
            let removed = plan
                .removed_span()
                .map(|s| bytes[s.start as usize..s.end as usize].to_vec());
            let expected_removed = match operation {
                ListEdit::Insert(i) => {
                    items.insert(i as usize, input.clone());
                    None
                }
                ListEdit::Remove(i) => Some(items.remove(i as usize)),
                ListEdit::Replace(i) => {
                    items[i as usize] = input.clone();
                    None
                }
                ListEdit::Pop => items.pop(),
                ListEdit::Truncate(n) => {
                    items.truncate(n as usize);
                    None
                }
                ListEdit::Clear => {
                    items.clear();
                    None
                }
            };
            assert_eq!(removed, expected_removed);
            bytes.resize(bytes.len().max(plan.required_storage_len() as usize), 0);
            let mut apply = begin_apply(&mut bytes, plan).unwrap();
            apply.move_regions();
            apply.destination().copy_from_slice(&input);
            apply.finish();
            assert_eq!(
                &bytes[..used(&bytes) as usize],
                account(&indexed(&items), 0)
            );
            validate(&bytes[12..used(&bytes) as usize], &layout, MAX_DEPTH).unwrap();
        }
    }
}
