//! Replacement preflight over corrupted envelopes and independently encoded trees.

#![forbid(unsafe_code)]

use blubber_core::raw::{plan_replace, resolve, InlinePath, Step};
use blubber_core::{
    Error, ErrorKind, FixedField, KeyDomain, KeyLayout, Layout, Leaf, Span, VariableField,
    MAX_DEPTH,
};

static STR: Layout = Layout::Str;
static U16: Layout = Layout::Leaf(Leaf::Unsigned(2));
static BOOL: Layout = Layout::Leaf(Leaf::Bool);
static STRS: Layout = Layout::List { element: &STR };
static WORDS: Layout = Layout::List { element: &U16 };
static SOME: Layout = Layout::Option { value: &STR };
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
    fixed_bytes: 3,
    fixed: &[
        FixedField {
            declaration: 1,
            offset: 0,
            layout: &U16,
        },
        FixedField {
            declaration: 3,
            offset: 2,
            layout: &BOOL,
        },
    ],
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
            declaration: 4,
            layout: &SOME,
        },
    ],
};

fn account(root: &[u8], spare: usize) -> Vec<u8> {
    let used = 12 + root.len();
    let mut bytes = b"blubber!".to_vec();
    bytes.extend_from_slice(&(used as u32).to_le_bytes());
    bytes.extend_from_slice(root);
    bytes.resize(used + spare, 0xa5);
    bytes
}

fn used(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[8..12].try_into().unwrap())
}

fn path(steps: &[Step]) -> InlinePath {
    let mut path = InlinePath::default();
    for step in steps {
        path.push(*step).unwrap();
    }
    path
}

fn strings(items: &[&[u8]]) -> Vec<u8> {
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

fn map(values: &[&[u8]]) -> Vec<u8> {
    let mut bytes = (values.len() as u32).to_le_bytes().to_vec();
    let mut end = 0u32;
    for (i, value) in values.iter().enumerate() {
        end += value.len() as u32;
        bytes.push(i as u8 + 1);
        bytes.extend_from_slice(&end.to_le_bytes());
    }
    for value in values {
        bytes.extend_from_slice(value);
    }
    bytes
}

fn fault(kind: ErrorKind) -> Error {
    kind.into()
}

#[test]
fn root_grow_equal_shrink_empty_and_spare() {
    for old in 0..8 {
        for len in 0..12 {
            let bytes = account(&vec![b'x'; old], 7);
            let before = bytes.clone();
            let p = InlinePath::new();
            assert_eq!(
                resolve(&bytes, &STR, &p).unwrap(),
                Span {
                    start: 12,
                    end: 12 + old as u32
                }
            );
            let plan = plan_replace(&bytes, used(&bytes), &STR, &p, len, 128).unwrap();
            assert_eq!(plan.new_used_len(), 12 + len);
            assert_eq!(plan.required_storage_len(), (12 + old as u32).max(12 + len));
            let mut relocated = bytes.clone();
            relocated.resize(
                plan.required_storage_len().max(relocated.len() as u32) as usize,
                0,
            );
            plan.check_ready(&relocated).unwrap();
            assert_eq!(bytes, before);
        }
    }
}

#[test]
fn declaration_ordinals_follow_packed_fixed_fields() {
    let list = strings(&[b"a", b"", b"xyz"]);
    let mut root = vec![0x34, 0x12, 2]; // Unrelated invalid bool is not visited semantically.
    root.extend_from_slice(&2u32.to_le_bytes());
    root.extend_from_slice(&(2 + list.len() as u32).to_le_bytes());
    root.extend_from_slice(b"hi");
    root.extend_from_slice(&list);
    root.extend_from_slice(&[1, b'!']);
    let bytes = account(&root, 0);
    let base = 12 + 3 + 8;
    for (field, span) in [
        (
            0,
            Span {
                start: base,
                end: base + 2,
            },
        ),
        (1, Span { start: 12, end: 14 }),
        (
            2,
            Span {
                start: base + 2,
                end: base + 2 + list.len() as u32,
            },
        ),
        (3, Span { start: 14, end: 15 }),
        (
            4,
            Span {
                start: base + 2 + list.len() as u32,
                end: used(&bytes),
            },
        ),
    ] {
        assert_eq!(
            resolve(&bytes, &MIXED, &path(&[Step::Field(field)])).unwrap(),
            span
        );
    }
    let selected = path(&[Step::Field(2), Step::Index(2)]);
    let span = resolve(&bytes, &MIXED, &selected).unwrap();
    assert_eq!(&bytes[span.start as usize..span.end as usize], b"xyz");
    let plan = plan_replace(&bytes, used(&bytes), &MIXED, &selected, 0, 512).unwrap();
    assert_eq!(plan.new_used_len(), used(&bytes) - 3);
    plan.check_ready(&bytes).unwrap();
}

#[test]
fn list_elision_and_every_small_child_length() {
    for n in 0..=4usize {
        for mask in 0..3usize.pow(n as u32) {
            let mut state = mask;
            let mut items = Vec::new();
            for _ in 0..n {
                items.push(vec![0xff; state % 3]);
                state /= 3;
            }
            let refs: Vec<_> = items.iter().map(Vec::as_slice).collect();
            let bytes = account(&strings(&refs), 5);
            let mut start = 12 + 4 + 4 * n.saturating_sub(1) as u32;
            for (i, item) in items.iter().enumerate() {
                let p = path(&[Step::Index(i as u32)]);
                let end = start + item.len() as u32;
                assert_eq!(resolve(&bytes, &STRS, &p).unwrap(), Span { start, end });
                for len in 0..5 {
                    let before = bytes.clone();
                    let plan = plan_replace(&bytes, used(&bytes), &STRS, &p, len, 1024).unwrap();
                    assert_eq!(plan.new_used_len(), used(&bytes) + len - item.len() as u32);
                    let mut ready = bytes.clone();
                    ready.resize(ready.len().max(plan.required_storage_len() as usize), 0);
                    plan.check_ready(&ready).unwrap();
                    assert_eq!(bytes, before);
                }
                start = end;
            }
            assert_eq!(
                resolve(&bytes, &STRS, &path(&[Step::Index(n as u32)])),
                Err(fault(ErrorKind::IndexOutOfBounds))
            );
        }
    }
}

#[test]
fn map_retains_terminal_and_empty_value_regions() {
    for n in 0..=4usize {
        for mask in 0..3usize.pow(n as u32) {
            let mut state = mask;
            let mut items = Vec::new();
            for _ in 0..n {
                items.push(vec![0xfe; state % 3]);
                state /= 3;
            }
            let refs: Vec<_> = items.iter().map(Vec::as_slice).collect();
            let bytes = account(&map(&refs), 10);
            let mut start = 12 + 4 + 5 * n as u32;
            for (i, item) in items.iter().enumerate() {
                let p = path(&[Step::Index(i as u32)]);
                let end = start + item.len() as u32;
                assert_eq!(resolve(&bytes, &MAP, &p).unwrap(), Span { start, end });
                for len in 0..5 {
                    let plan = plan_replace(&bytes, used(&bytes), &MAP, &p, len, 1024).unwrap();
                    assert_eq!(plan.new_used_len(), used(&bytes) + len - item.len() as u32);
                    plan.check_ready(&bytes).unwrap();
                }
                start = end;
            }
        }
    }
}

#[test]
fn fixed_targets_cannot_change_width_or_include_map_keys() {
    for (layout, root, expected) in [
        (
            &WORDS,
            vec![2, 0, 0, 0, 1, 0, 2, 0],
            Span { start: 18, end: 20 },
        ),
        (
            &FIXED_MAP,
            vec![2, 0, 0, 0, 1, 7, 0, 2, 8, 0],
            Span { start: 20, end: 22 },
        ),
    ] {
        let bytes = account(&root, 3);
        let p = path(&[Step::Index(1)]);
        assert_eq!(resolve(&bytes, layout, &p).unwrap(), expected);
        plan_replace(&bytes, used(&bytes), layout, &p, 2, 100)
            .unwrap()
            .check_ready(&bytes)
            .unwrap();
        for len in [0, 1, 3, u32::MAX] {
            assert_eq!(
                plan_replace(&bytes, used(&bytes), layout, &p, len, 100).unwrap_err(),
                fault(ErrorKind::InvalidBoundary)
            );
        }
    }
    let bytes = account(&[2], 0);
    // A bad old scalar may be repaired by a correctly sized replacement.
    plan_replace(&bytes, used(&bytes), &BOOL, &InlinePath::new(), 1, 100).unwrap();
}

#[test]
fn option_none_some_and_sticky_projection_overflow() {
    let p = path(&[Step::Some]);
    let none = account(&[0], 0);
    assert_eq!(
        resolve(&none, &SOME, &p),
        Err(fault(ErrorKind::MissingValue))
    );
    for value in [b"".as_slice(), b"hello"] {
        let mut root = vec![1];
        root.extend_from_slice(value);
        let bytes = account(&root, 0);
        assert_eq!(
            resolve(&bytes, &SOME, &p).unwrap(),
            Span {
                start: 13,
                end: used(&bytes)
            }
        );
        let plan = plan_replace(&bytes, used(&bytes), &SOME, &p, 0, 100).unwrap();
        assert_eq!(plan.new_used_len(), 13);
        plan.check_ready(&bytes).unwrap();
    }
    let mut too_deep = InlinePath::new();
    for _ in 1..MAX_DEPTH {
        too_deep.push(Step::Some).unwrap();
    }
    assert_eq!(
        too_deep.push(Step::Some),
        Err(fault(ErrorKind::DepthExceeded))
    );
    assert_eq!(
        too_deep.push(Step::Field(0)),
        Err(fault(ErrorKind::DepthExceeded))
    );
    assert_eq!(
        resolve(&none, &SOME, &too_deep),
        Err(fault(ErrorKind::DepthExceeded))
    );
}

#[test]
fn maximum_depth_nested_grow_shrink_and_no_writes() {
    // Every level has two strings, so every ancestor contributes an end run.
    let mut layout: &'static Layout = &STR;
    let mut root = b"end".to_vec();
    let mut steps = Vec::new();
    let mut start = 12u32;
    for _ in 1..MAX_DEPTH {
        layout = Box::leak(Box::new(Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: Box::leak(Box::new([
                VariableField {
                    declaration: 0,
                    layout,
                },
                VariableField {
                    declaration: 1,
                    layout: &STR,
                },
            ])),
        }));
        let mut parent = (root.len() as u32).to_le_bytes().to_vec();
        parent.extend_from_slice(&root);
        parent.push(b'!');
        root = parent;
        steps.push(Step::Field(0));
        start += 4;
    }
    let bytes = account(&root, 1);
    let before = bytes.clone();
    let p = path(&steps);
    assert_eq!(
        resolve(&bytes, layout, &p).unwrap(),
        Span {
            start,
            end: start + 3
        }
    );
    for len in [0, 1, 3, 4, 100] {
        let plan = plan_replace(&bytes, used(&bytes), layout, &p, len, 1024).unwrap();
        assert_eq!(plan.new_used_len(), used(&bytes) + len - 3);
        let mut relocated = bytes.clone();
        relocated.resize(
            plan.required_storage_len().max(bytes.len() as u32) as usize,
            0,
        );
        plan.check_ready(&relocated).unwrap();
        assert_eq!(bytes, before);
    }
}

#[test]
fn envelope_capacity_and_full_width_overflow_faults() {
    let p = InlinePath::new();
    for n in 0..12 {
        let bytes = vec![0; n];
        assert_eq!(
            resolve(&bytes, &STR, &p),
            Err(Error::new(ErrorKind::Truncated, 8))
        );
    }
    let bytes = account(b"x", 7);
    for bad in [0, 11, used(&bytes) + 1, u32::MAX] {
        assert_eq!(
            plan_replace(&bytes, bad, &STR, &p, 1, 100).unwrap_err(),
            Error::new(ErrorKind::UsedLength, 8)
        );
    }
    assert_eq!(
        plan_replace(&bytes, used(&bytes), &STR, &p, 1, bytes.len() as u32 - 1).unwrap_err(),
        fault(ErrorKind::Capacity)
    );
    assert_eq!(
        plan_replace(&bytes, used(&bytes), &STR, &p, 101, 100).unwrap_err(),
        fault(ErrorKind::Capacity)
    );
    for len in [u32::MAX, u32::MAX - 11] {
        assert_eq!(
            plan_replace(&bytes, used(&bytes), &STR, &p, len, u32::MAX).unwrap_err(),
            fault(ErrorKind::ArithmeticOverflow)
        );
    }
    let max = plan_replace(&bytes, used(&bytes), &STR, &p, u32::MAX - 12, u32::MAX).unwrap();
    assert_eq!(max.new_used_len(), u32::MAX);
    assert_eq!(max.check_ready(&bytes), Err(fault(ErrorKind::InvalidPlan)));
}

#[test]
fn readiness_rejects_changed_facts_before_writes_and_allows_relocation() {
    let bytes = account(&strings(&[b"a", b"bc", b"def"]), 10);
    let p = path(&[Step::Index(1)]);
    let plan = plan_replace(&bytes, used(&bytes), &STRS, &p, 4, 100).unwrap();
    plan.check_ready(&bytes).unwrap();
    for at in [0, 7, 8, 12, 16, 20] {
        let mut changed = bytes.clone();
        changed[at] ^= 1;
        let snapshot = changed.clone();
        assert_eq!(
            plan.check_ready(&changed),
            Err(fault(ErrorKind::InvalidPlan)),
            "byte {at}"
        );
        assert_eq!(changed, snapshot);
    }
    let mut relocated = bytes.clone();
    relocated.resize(100, 0);
    relocated[24] = 0xff; // Content changes need not change structural readiness.
    relocated[99] = 0xcc;
    plan.check_ready(&relocated).unwrap();
    relocated.push(0);
    assert_eq!(
        plan.check_ready(&relocated),
        Err(fault(ErrorKind::InvalidPlan))
    );
    assert_eq!(
        plan.check_ready(&bytes[..used(&bytes) as usize]),
        Err(fault(ErrorKind::InvalidPlan))
    );
}

#[test]
fn readiness_rechecks_all_boundaries_and_key_order() {
    let bytes = account(&map(&[b"a", b"bc", b"def"]), 10);
    let plan = plan_replace(&bytes, used(&bytes), &MAP, &path(&[Step::Index(0)]), 0, 100).unwrap();
    for at in [16, 17, 21, 22, 26, 27] {
        let mut changed = bytes.clone();
        changed[at] = if at == 26 { 1 } else { 0xff };
        let snapshot = changed.clone();
        assert_eq!(
            plan.check_ready(&changed),
            Err(fault(ErrorKind::InvalidPlan)),
            "byte {at}"
        );
        assert_eq!(changed, snapshot);
    }
    // Changes to an earlier sibling end remain compatible when the selected
    // exact interval and all representability checks remain the same.
    let bytes = account(&strings(&[b"a", b"b", b"c"]), 0);
    let plan = plan_replace(
        &bytes,
        used(&bytes),
        &STRS,
        &path(&[Step::Index(2)]),
        0,
        100,
    )
    .unwrap();
    let mut changed = bytes.clone();
    changed[16..20].copy_from_slice(&0u32.to_le_bytes());
    plan.check_ready(&changed).unwrap();
}

#[test]
fn corruption_is_rebased_once_and_unrelated_domains_are_lazy() {
    let bytes = account(&[3], 0);
    assert_eq!(
        resolve(&bytes, &SOME, &InlinePath::new()),
        Err(Error::new(ErrorKind::InvalidTag, 12))
    );
    let bytes = account(&strings(&[&[3]]), 0);
    let options = Layout::List { element: &SOME };
    assert_eq!(
        resolve(&bytes, &options, &path(&[Step::Index(0)])),
        Err(Error::new(ErrorKind::InvalidTag, 16))
    );
    let mut bytes = account(&map(&[b"a", b"b"]), 0);
    bytes[21] = 1;
    assert_eq!(
        resolve(&bytes, &MAP, &path(&[Step::Index(0)])),
        Err(Error::new(ErrorKind::DuplicateKey, 21))
    );
    bytes[16] = 2;
    assert_eq!(
        resolve(&bytes, &MAP, &path(&[Step::Index(0)])),
        Err(Error::new(ErrorKind::UnsortedKeys, 21))
    );
    let bytes = account(&strings(&[&[0xff], b"ok"]), 0);
    plan_replace(
        &bytes,
        used(&bytes),
        &STRS,
        &path(&[Step::Index(1)]),
        1,
        100,
    )
    .unwrap();
}

#[test]
fn invalid_selectors_metadata_and_corrupted_directories_preserve_all_bytes() {
    let bytes = account(&strings(&[b"a", b"b", b"c"]), 7);
    for p in [
        path(&[Step::Field(0)]),
        path(&[Step::Some]),
        path(&[Step::Index(u32::MAX)]),
    ] {
        let before = bytes.clone();
        assert!(resolve(&bytes, &STRS, &p).is_err());
        assert!(plan_replace(&bytes, used(&bytes), &STRS, &p, 1, 100).is_err());
        assert_eq!(bytes, before);
    }
    for at in [12, 16, 20] {
        for word in [0, 1, 4, u32::MAX] {
            let mut changed = bytes.clone();
            changed[at..at + 4].copy_from_slice(&word.to_le_bytes());
            let before = changed.clone();
            let _ = plan_replace(
                &changed,
                used(&changed),
                &STRS,
                &path(&[Step::Index(1)]),
                0,
                100,
            );
            assert_eq!(changed, before);
        }
    }
    let invalid = Layout::Leaf(Leaf::Unsigned(3));
    assert_eq!(
        resolve(&bytes, &invalid, &InlinePath::new()),
        Err(fault(ErrorKind::InvalidSchema))
    );
    assert!(std::mem::size_of::<blubber_core::raw::Plan>() <= 2048);
    assert!(std::mem::size_of::<InlinePath>() <= 16 * MAX_DEPTH);
}

#[test]
fn mixed_ancestors_at_depth_limit_include_unaligned_map_end_words() {
    let mut layout: &'static Layout = &STR;
    let mut root = b"abc".to_vec();
    let mut steps = Vec::new();
    let mut offset = 0u32;
    for level in 1..MAX_DEPTH {
        let (parent, prefix, selector) = match level % 4 {
            0 => {
                let mut parent = vec![1];
                parent.extend_from_slice(&root);
                layout = Box::leak(Box::new(Layout::Option { value: layout }));
                (parent, 1, Step::Some)
            }
            1 => {
                let mut parent = 1u32.to_le_bytes().to_vec();
                parent.extend_from_slice(&(1 + root.len() as u32).to_le_bytes());
                parent.push(b'l');
                parent.extend_from_slice(&root);
                parent.extend_from_slice(b"tail");
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
                (parent, 9, Step::Field(1))
            }
            2 => {
                let parent = strings(&[&root, &root, &root]);
                let prefix = 12 + root.len() as u32;
                layout = Box::leak(Box::new(Layout::List { element: layout }));
                (parent, prefix, Step::Index(1))
            }
            _ => {
                let mut parent = 2u32.to_le_bytes().to_vec();
                parent.extend_from_slice(&[0, 0, 1]);
                parent.extend_from_slice(&(root.len() as u32).to_le_bytes());
                parent.extend_from_slice(&[0, 0, 2]);
                parent.extend_from_slice(&(2 * root.len() as u32).to_le_bytes());
                parent.extend_from_slice(&root);
                parent.extend_from_slice(&root);
                let prefix = 18 + root.len() as u32;
                layout = Box::leak(Box::new(Layout::Map {
                    key: KeyLayout {
                        width: 3,
                        domain: KeyDomain::Raw,
                    },
                    value: layout,
                }));
                (parent, prefix, Step::Index(1))
            }
        };
        root = parent;
        steps.insert(0, selector);
        offset += prefix;
    }
    blubber_core::validate(&root, layout, MAX_DEPTH).unwrap();
    let bytes = account(&root, 10);
    let before = bytes.clone();
    let p = path(&steps);
    assert_eq!(
        resolve(&bytes, layout, &p).unwrap(),
        Span {
            start: 12 + offset,
            end: 15 + offset
        }
    );
    for len in [0, 3, 9] {
        let plan = plan_replace(&bytes, used(&bytes), layout, &p, len, bytes.len() as u32).unwrap();
        assert_eq!(plan.new_used_len(), used(&bytes) + len - 3);
        plan.check_ready(&bytes).unwrap();
        assert_eq!(bytes, before);
    }
}
