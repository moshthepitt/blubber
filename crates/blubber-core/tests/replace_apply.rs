//! Replacement execution compared with independent complete wire encodings.

#![forbid(unsafe_code)]

use blubber_core::raw::{begin_apply, plan_replace, InlinePath, Step};
use blubber_core::{
    validate, ErrorKind, FixedField, KeyDomain, KeyLayout, Layout, Leaf, VariableField, MAX_DEPTH,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

static STR: Layout = Layout::Str;
static U16: Layout = Layout::Leaf(Leaf::Unsigned(2));
static STRS: Layout = Layout::List { element: &STR };
static MAP: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Unsigned,
    },
    value: &STR,
};
static FIXED_MAP: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Unsigned,
    },
    value: &U16,
};
static MIXED: Layout = Layout::Struct {
    fixed_bytes: 2,
    fixed: &[FixedField {
        declaration: 1,
        offset: 0,
        layout: &U16,
    }],
    variable: &[
        VariableField {
            declaration: 0,
            layout: &STR,
        },
        VariableField {
            declaration: 2,
            layout: &STRS,
        },
        VariableField {
            declaration: 3,
            layout: &STR,
        },
    ],
};

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

fn strings(items: &[Vec<u8>]) -> Vec<u8> {
    let mut out = (items.len() as u32).to_le_bytes().to_vec();
    let mut end = 0u32;
    for item in items.iter().take(items.len().saturating_sub(1)) {
        end += item.len() as u32;
        out.extend_from_slice(&end.to_le_bytes());
    }
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

fn map(items: &[Vec<u8>], width: usize) -> Vec<u8> {
    let mut out = (items.len() as u32).to_le_bytes().to_vec();
    let mut end = 0u32;
    for (i, item) in items.iter().enumerate() {
        out.extend(std::iter::repeat_n(0, width - 1));
        out.push(i as u8 + 1);
        end += item.len() as u32;
        out.extend_from_slice(&end.to_le_bytes());
    }
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

fn mixed(list: &[u8]) -> Vec<u8> {
    let mut out = vec![0x34, 0x12];
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(2 + list.len() as u32).to_le_bytes());
    out.extend_from_slice(b"hi");
    out.extend_from_slice(list);
    out.extend_from_slice(b"outer tail");
    out
}

fn replace(bytes: &mut [u8], layout: &Layout, steps: &[Step], input: &[u8]) {
    let plan = plan_replace(
        bytes,
        used(bytes),
        layout,
        &path(steps),
        input.len() as u32,
        bytes.len() as u32,
    )
    .unwrap();
    let mut apply = begin_apply(bytes, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(input);
    apply.finish();
}

fn check(old: &[u8], new: &[u8], layout: &Layout, steps: &[Step], input: &[u8]) {
    validate(old, layout, MAX_DEPTH).unwrap();
    validate(new, layout, MAX_DEPTH).unwrap();
    let mut bytes = account(old, new.len().saturating_sub(old.len()) + 13);
    let before = bytes.clone();
    // Unaligned root slice with guards on both sides also checks caller frames.
    let mut guarded = vec![0x7d; 3];
    guarded.extend_from_slice(&bytes);
    guarded.extend_from_slice(&[0x7e; 5]);
    let len = bytes.len();
    replace(&mut guarded[3..3 + len], layout, steps, input);
    bytes.copy_from_slice(&guarded[3..3 + len]);
    assert_eq!(&guarded[..3], &[0x7d; 3]);
    assert_eq!(&guarded[3 + len..], &[0x7e; 5]);
    let expected = account(new, 0);
    assert_eq!(&bytes[..expected.len()], expected);
    let old_used = 12 + old.len();
    let new_used = 12 + new.len();
    if new_used < old_used {
        assert!(bytes[new_used..old_used].iter().all(|b| *b == 0));
    }
    let unchanged_spare = old_used.max(new_used);
    assert_eq!(&bytes[unchanged_spare..], &before[unchanged_spare..]);
    validate(&bytes[12..used(&bytes) as usize], layout, MAX_DEPTH).unwrap();
}

#[test]
fn root_empty_grow_equal_shrink_and_untouched_spare() {
    for old in 0..9 {
        for new in 0..13 {
            check(
                &vec![b'x'; old],
                &vec![b'y'; new],
                &STR,
                &[],
                &vec![b'y'; new],
            );
        }
    }
}

#[test]
fn list_all_ordinals_elision_empty_children_and_overlap_directions() {
    for n in 1..=4usize {
        for mask in 0..3usize.pow(n as u32) {
            let mut state = mask;
            let items: Vec<_> = (0..n)
                .map(|i| {
                    let item = vec![b'a' + i as u8; state % 3];
                    state /= 3;
                    item
                })
                .collect();
            let old = strings(&items);
            for i in 0..n {
                for len in 0..7 {
                    let input = vec![b'z'; len];
                    let mut changed = items.clone();
                    changed[i] = input.clone();
                    check(
                        &old,
                        &strings(&changed),
                        &STRS,
                        &[Step::Index(i as u32)],
                        &input,
                    );
                }
            }
        }
    }
}

#[test]
fn indexed_maps_patch_terminal_ends_with_unaligned_key_widths() {
    for width in [1, 3, 8, 32] {
        let layout = Layout::Map {
            key: KeyLayout {
                width: width as u32,
                domain: KeyDomain::Raw,
            },
            value: &STR,
        };
        for n in 1..=4usize {
            for mask in 0..3usize.pow(n as u32) {
                let mut state = mask;
                let items: Vec<_> = (0..n)
                    .map(|i| {
                        let item = vec![b'a' + i as u8; state % 3];
                        state /= 3;
                        item
                    })
                    .collect();
                let old = map(&items, width);
                for i in 0..n {
                    for len in [0, 2, 7] {
                        let input = vec![b'z'; len];
                        let mut changed = items.clone();
                        changed[i] = input.clone();
                        check(
                            &old,
                            &map(&changed, width),
                            &layout,
                            &[Step::Index(i as u32)],
                            &input,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn nested_struct_list_updates_every_ancestor_and_keeps_outer_suffix() {
    let items = vec![b"one".to_vec(), b"".to_vec(), b"three".to_vec()];
    let old = mixed(&strings(&items));
    for i in 0..items.len() {
        for input in [b"".as_slice(), b"x", b"replacement longer than old suffix"] {
            let mut changed = items.clone();
            changed[i] = input.to_vec();
            check(
                &old,
                &mixed(&strings(&changed)),
                &MIXED,
                &[Step::Field(2), Step::Index(i as u32)],
                input,
            );
        }
    }
    let mut new = old.clone();
    new[..2].copy_from_slice(&[0xab, 0xcd]);
    check(&old, &new, &MIXED, &[Step::Field(1)], &[0xab, 0xcd]);
}

#[test]
fn fixed_map_value_replacement_keeps_keys_and_count() {
    let old = [3, 0, 0, 0, 1, 10, 0, 2, 20, 0, 3, 30, 0];
    for i in 0..3 {
        let mut new = old;
        new[5 + i * 3..7 + i * 3].copy_from_slice(&[0x34, 0x12]);
        check(
            &old,
            &new,
            &FIXED_MAP,
            &[Step::Index(i as u32)],
            &[0x34, 0x12],
        );
    }
}

#[test]
fn option_whole_replacement_and_present_child() {
    let option = Layout::Option { value: &STR };
    for (old, new) in [
        (b"\0".as_slice(), b"\x01hello".as_slice()),
        (b"\x01hello", b"\0"),
        (b"\x01hello", b"\x01"),
        (b"\x01", b"\x01hello"),
    ] {
        check(old, new, &option, &[], new);
    }
    check(b"\x01old", b"\x01longer", &option, &[Step::Some], b"longer");
}

#[test]
fn readiness_errors_preserve_entire_current_storage() {
    let original = account(
        &strings(&[b"a".to_vec(), b"bc".to_vec(), b"def".to_vec()]),
        10,
    );
    for at in [0, 7, 8, 12, 16, 20] {
        let plan = plan_replace(
            &original,
            used(&original),
            &STRS,
            &path(&[Step::Index(1)]),
            4,
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
    let plan = plan_replace(
        &original,
        used(&original),
        &STRS,
        &path(&[Step::Index(1)]),
        30,
        100,
    )
    .unwrap();
    let mut bytes = original.clone();
    assert_eq!(
        begin_apply(&mut bytes, plan).err(),
        Some(ErrorKind::InvalidPlan.into())
    );
    assert_eq!(bytes, original);
    let original = account(&map(&[b"a".to_vec(), b"b".to_vec()], 1), 10);
    let plan = plan_replace(
        &original,
        used(&original),
        &MAP,
        &path(&[Step::Index(0)]),
        0,
        100,
    )
    .unwrap();
    let mut bytes = original.clone();
    bytes[21] = 1;
    let before = bytes.clone();
    assert_eq!(
        begin_apply(&mut bytes, plan).err(),
        Some(ErrorKind::InvalidPlan.into())
    );
    assert_eq!(bytes, before);
}

#[test]
fn relocated_storage_and_compatible_sibling_changes_use_current_words() {
    let original = account(&strings(&[b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]), 0);
    let plan = plan_replace(
        &original,
        used(&original),
        &STRS,
        &path(&[Step::Index(2)]),
        8,
        100,
    )
    .unwrap();
    let mut bytes = vec![0xcc; 100];
    bytes[..original.len()].copy_from_slice(&original);
    // First sibling becomes empty; selected interval and changed run stay fixed.
    bytes[16..20].copy_from_slice(&0u32.to_le_bytes());
    bytes[24] = b'd';
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(b"expanded");
    apply.finish();
    let expected = account(&strings(&[vec![], b"db".to_vec(), b"expanded".to_vec()]), 0);
    assert_eq!(&bytes[..expected.len()], expected);
    assert!(bytes[expected.len()..].iter().all(|b| *b == 0xcc));
}

#[test]
fn invalid_phase_panics_before_its_next_write() {
    let mut bytes = account(b"old", 10);
    let plan = plan_replace(&bytes, used(&bytes), &STR, &InlinePath::new(), 3, 100).unwrap();
    let before = bytes.clone();
    let mut apply = begin_apply(&mut bytes, plan).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = apply.destination();
    }))
    .is_err());
    apply.move_regions();
    assert!(catch_unwind(AssertUnwindSafe(|| apply.move_regions())).is_err());
    apply.destination().copy_from_slice(b"new");
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = apply.destination();
    }))
    .is_err());
    apply.finish();
    assert_eq!(&bytes[12..15], b"new");
    assert_eq!(&bytes[..12], &before[..12]);
    assert_eq!(&bytes[15..], &before[15..]);
    for moved in [false, true] {
        let mut bytes = account(b"old", 10);
        let before = bytes.clone();
        let plan = plan_replace(&bytes, used(&bytes), &STR, &InlinePath::new(), 3, 100).unwrap();
        let mut apply = begin_apply(&mut bytes, plan).unwrap();
        if moved {
            apply.move_regions();
        }
        assert!(catch_unwind(AssertUnwindSafe(|| apply.finish())).is_err());
        assert_eq!(bytes, before);
    }
}

#[test]
fn maximum_depth_mixed_ancestors_preserve_relocated_sibling_subtrees() {
    for input in [b"".as_slice(), b"new", b"a replacement that grows"] {
        let mut layout: &'static Layout = &STR;
        let mut old = b"old".to_vec();
        let mut new = input.to_vec();
        let mut steps = Vec::new();
        for level in 1..MAX_DEPTH {
            let (old_parent, new_parent, selector) = match level % 4 {
                0 => {
                    layout = Box::leak(Box::new(Layout::Option { value: layout }));
                    (
                        [vec![1], old.clone()].concat(),
                        [vec![1], new].concat(),
                        Step::Some,
                    )
                }
                1 => {
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
                    let wrap = |selected: &[u8]| {
                        let mut out = 1u32.to_le_bytes().to_vec();
                        out.extend_from_slice(&(1 + selected.len() as u32).to_le_bytes());
                        out.extend_from_slice(b"l");
                        out.extend_from_slice(selected);
                        out.extend_from_slice(b"outer tail");
                        out
                    };
                    (wrap(&old), wrap(&new), Step::Field(1))
                }
                2 => {
                    layout = Box::leak(Box::new(Layout::List { element: layout }));
                    (
                        strings(&[old.clone(), old.clone(), old.clone()]),
                        strings(&[old.clone(), new, old.clone()]),
                        Step::Index(1),
                    )
                }
                _ => {
                    layout = Box::leak(Box::new(Layout::Map {
                        key: KeyLayout {
                            width: 3,
                            domain: KeyDomain::Raw,
                        },
                        value: layout,
                    }));
                    (
                        map(&[old.clone(), old.clone()], 3),
                        map(&[old.clone(), new], 3),
                        Step::Index(1),
                    )
                }
            };
            old = old_parent;
            new = new_parent;
            steps.insert(0, selector);
        }
        check(&old, &new, layout, &steps, input);
    }
}

#[test]
fn movement_and_encoding_leave_metadata_unpublished_until_finish() {
    let old = strings(&[b"a".to_vec(), b"bc".to_vec(), b"def".to_vec()]);
    for input in [b"".as_slice(), b"XY", b"longer than suffix"] {
        let mut bytes = account(&old, input.len() + 10);
        let before = bytes.clone();
        let plan = plan_replace(
            &bytes,
            used(&bytes),
            &STRS,
            &path(&[Step::Index(1)]),
            input.len() as u32,
            bytes.len() as u32,
        )
        .unwrap();
        {
            let mut apply = begin_apply(&mut bytes, plan).unwrap();
            apply.move_regions();
            apply.destination().copy_from_slice(input);
            // An unfinished apply leaves publication to the poison-owning caller.
        }
        assert_eq!(&bytes[..24], &before[..24]);
        assert_eq!(&bytes[24..25], b"a");
        assert_eq!(&bytes[25..25 + input.len()], input);
        assert_eq!(&bytes[25 + input.len()..28 + input.len()], b"def");
    }
}

#[test]
fn shrink_then_regrow_does_not_reveal_vacated_used_bytes() {
    let mut bytes = account(&strings(&[b"secret".to_vec(), b"tail".to_vec()]), 20);
    let old_used = used(&bytes) as usize;
    replace(&mut bytes, &STRS, &[Step::Index(0)], b"");
    let short_used = used(&bytes) as usize;
    assert!(bytes[short_used..old_used].iter().all(|b| *b == 0));
    replace(&mut bytes, &STRS, &[Step::Index(0)], b"public");
    let expected = account(&strings(&[b"public".to_vec(), b"tail".to_vec()]), 0);
    assert_eq!(&bytes[..expected.len()], expected);
    assert!(bytes[expected.len()..].iter().all(|b| *b == 0xa5));
}

#[test]
fn unrelated_invalid_leaf_content_remains_lazy_and_byte_identical() {
    let mut bytes = account(&strings(&[vec![0xff], b"ok".to_vec()]), 20);
    replace(&mut bytes, &STRS, &[Step::Index(1)], b"longer");
    let expected = account(&strings(&[vec![0xff], b"longer".to_vec()]), 0);
    assert_eq!(&bytes[..expected.len()], expected);
    assert!(validate(&bytes[12..used(&bytes) as usize], &STRS, MAX_DEPTH).is_err());
}
