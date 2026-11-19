#![forbid(unsafe_code)]

//! Full-schema validation against an independent recursive test decoder.

use blubber_core::{
    validate, Error, ErrorKind, FixedField, KeyDomain, KeyLayout, Layout, Leaf, VariableField,
    MAX_DEPTH,
};

static U8: Layout = Layout::Leaf(Leaf::Unsigned(1));
static U16: Layout = Layout::Leaf(Leaf::Unsigned(2));
static BOOL: Layout = Layout::Leaf(Leaf::Bool);
static STR: Layout = Layout::Str;
static FIXED: Layout = Layout::Struct {
    fixed_bytes: 3,
    fixed: &[
        FixedField {
            declaration: 0,
            offset: 0,
            layout: &U16,
        },
        FixedField {
            declaration: 1,
            offset: 2,
            layout: &BOOL,
        },
    ],
    variable: &[],
};
static OPT_BOOL: Layout = Layout::Option { value: &BOOL };
static OPT_STR: Layout = Layout::Option { value: &STR };
static OPT_FIXED: Layout = Layout::Option { value: &FIXED };
static BOOLS: Layout = Layout::List { element: &BOOL };
static STRS: Layout = Layout::List { element: &STR };
static FIXEDS: Layout = Layout::List { element: &FIXED };
static OPTS: Layout = Layout::List { element: &OPT_STR };
static NESTED_LIST: Layout = Layout::List { element: &BOOLS };
static MAP_BOOL: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Bool,
    },
    value: &BOOL,
};
static MAP_STR: Layout = Layout::Map {
    key: KeyLayout {
        width: 2,
        domain: KeyDomain::Unsigned,
    },
    value: &STR,
};
static MAP_OPT: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Raw,
    },
    value: &OPT_BOOL,
};
static MAP_FIXED: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Signed,
    },
    value: &FIXED,
};
static MIXED: Layout = Layout::Struct {
    fixed_bytes: 3,
    fixed: &[FixedField {
        declaration: 1,
        offset: 0,
        layout: &FIXED,
    }],
    variable: &[
        VariableField {
            declaration: 0,
            layout: &OPT_STR,
        },
        VariableField {
            declaration: 2,
            layout: &BOOLS,
        },
        VariableField {
            declaration: 3,
            layout: &MAP_STR,
        },
    ],
};
static SINGLE: Layout = Layout::Struct {
    fixed_bytes: 0,
    fixed: &[],
    variable: &[VariableField {
        declaration: 0,
        layout: &STR,
    }],
};
static CYCLE: Layout = Layout::Option { value: &CYCLE };

// Independent recursive decoder used only on the finite, valid test schemas.
// No production partition, width, word, UTF-8 or order helper is called.
fn width(layout: &Layout) -> Option<usize> {
    match layout {
        Layout::Flat { .. } => panic!("native oracle fixture expected"),
        Layout::Leaf(Leaf::Unsigned(w) | Leaf::Signed(w)) => Some(*w as usize),
        Layout::Leaf(Leaf::Bool) => Some(1),
        Layout::Leaf(Leaf::Raw(w)) => Some(*w as usize),
        Layout::Struct {
            fixed_bytes,
            variable: [],
            ..
        } => Some(*fixed_bytes as usize),
        _ => None,
    }
}

fn word(bytes: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(bytes.get(at..at.checked_add(4)?)?.try_into().ok()?) as usize)
}

fn reference(bytes: &[u8], layout: &Layout, budget: usize) -> bool {
    if budget == 0 || budget > MAX_DEPTH || bytes.len() > u32::MAX as usize {
        return false;
    }
    let child = |b: &[u8], l: &Layout| reference(b, l, budget - 1);
    match layout {
        Layout::Flat { .. } => panic!("native oracle fixture expected"),
        Layout::Leaf(leaf) => {
            width(layout) == Some(bytes.len()) && (!matches!(leaf, Leaf::Bool) || bytes[0] <= 1)
        }
        Layout::Str => core::str::from_utf8(bytes).is_ok(),
        Layout::Option { value } => match bytes.first() {
            Some(0) => bytes.len() == 1,
            Some(1) => child(&bytes[1..], value),
            _ => false,
        },
        Layout::Struct {
            fixed_bytes,
            fixed,
            variable,
        } => {
            let prefix = *fixed_bytes as usize;
            let payload = prefix + variable.len().saturating_sub(1) * 4;
            if payload > bytes.len() || (variable.is_empty() && bytes.len() != prefix) {
                return false;
            }
            let mut ends = vec![0];
            for i in 0..variable.len().saturating_sub(1) {
                let Some(end) = word(bytes, prefix + 4 * i) else {
                    return false;
                };
                if end < *ends.last().unwrap() || end > bytes.len() - payload {
                    return false;
                }
                ends.push(end);
            }
            ends.push(bytes.len() - payload);
            fixed.iter().all(|f| {
                child(
                    &bytes[f.offset as usize..f.offset as usize + width(f.layout).unwrap()],
                    f.layout,
                )
            }) && variable
                .iter()
                .enumerate()
                .all(|(i, f)| child(&bytes[payload + ends[i]..payload + ends[i + 1]], f.layout))
        }
        Layout::List { element } => {
            let Some(n) = word(bytes, 0) else {
                return false;
            };
            if let Some(w) = width(element) {
                if 4u64 + n as u64 * w as u64 != bytes.len() as u64 {
                    return false;
                }
                bytes[4..].chunks_exact(w).all(|b| child(b, element))
            } else {
                let payload = 4u64 + n.saturating_sub(1) as u64 * 4;
                if payload > bytes.len() as u64 || (n == 0 && bytes.len() != 4) {
                    return false;
                }
                let payload = payload as usize;
                let mut start = 0;
                for i in 0..n {
                    let end = if i + 1 == n {
                        bytes.len() - payload
                    } else {
                        word(bytes, 4 + i * 4).unwrap()
                    };
                    if end < start
                        || end > bytes.len() - payload
                        || !child(&bytes[payload + start..payload + end], element)
                    {
                        return false;
                    }
                    start = end;
                }
                true
            }
        }
        Layout::Map { key, value } => {
            let Some(n) = word(bytes, 0) else {
                return false;
            };
            let w = key.width as usize;
            let fixed = width(value);
            let stride = w as u64 + fixed.unwrap_or(4) as u64;
            let table_end = 4 + n as u64 * stride;
            if table_end > bytes.len() as u64
                || (fixed.is_some() && table_end != bytes.len() as u64)
                || (n == 0 && bytes.len() != 4)
            {
                return false;
            }
            let mut previous: Option<&[u8]> = None;
            let mut start = 0;
            for i in 0..n {
                let at = (4 + i as u64 * stride) as usize;
                let k = &bytes[at..at + w];
                if (matches!(key.domain, KeyDomain::Bool) && k[0] > 1)
                    || previous.is_some_and(|p| p >= k)
                {
                    return false;
                }
                previous = Some(k);
                if let Some(vw) = fixed {
                    if !child(&bytes[at + w..at + w + vw], value) {
                        return false;
                    }
                } else {
                    let end = word(bytes, at + w).unwrap();
                    let p = table_end as usize;
                    if end < start
                        || end > bytes.len() - p
                        || (i + 1 == n && end != bytes.len() - p)
                        || !child(&bytes[p + start..p + end], value)
                    {
                        return false;
                    }
                    start = end;
                }
            }
            true
        }
    }
}

fn agrees(bytes: &[u8], layout: &Layout, budget: usize) {
    assert_eq!(
        validate(bytes, layout, budget).is_ok(),
        reference(bytes, layout, budget),
        "bytes={bytes:02x?}, layout={layout:?}, budget={budget}"
    );
}

fn fault(bytes: &[u8], layout: &Layout, kind: ErrorKind, offset: u32) {
    assert_eq!(
        validate(bytes, layout, MAX_DEPTH),
        Err(Error::new(kind, offset))
    );
}

fn indexed(values: &[Vec<u8>]) -> Vec<u8> {
    let mut out = (values.len() as u32).to_le_bytes().to_vec();
    let mut end = 0u32;
    for value in values.iter().take(values.len().saturating_sub(1)) {
        end += value.len() as u32;
        out.extend_from_slice(&end.to_le_bytes());
    }
    for value in values {
        out.extend_from_slice(value);
    }
    out
}

fn mapped(keys: &[u8], values: &[Vec<u8>]) -> Vec<u8> {
    let mut out = (keys.len() as u32).to_le_bytes().to_vec();
    let mut end = 0u32;
    for (&key, value) in keys.iter().zip(values) {
        end += value.len() as u32;
        out.push(key);
        out.extend_from_slice(&end.to_le_bytes());
    }
    for value in values {
        out.extend_from_slice(value);
    }
    out
}

fn mixed() -> Vec<u8> {
    // Fixed prefix, two ends, Some("A"), [true,false], {1:"x",256:""}.
    vec![
        0x34, 0x12, 1, 2, 0, 0, 0, 8, 0, 0, 0, 1, b'A', 2, 0, 0, 0, 1, 0, 2, 0, 0, 0, 0, 1, 1, 0,
        0, 0, 1, 0, 1, 0, 0, 0, b'x',
    ]
}

#[test]
fn complete_mixed_schema_checks_every_wire_family_and_exact_region() {
    let bytes = mixed();
    assert_eq!(validate(&bytes, &MIXED, MAX_DEPTH), Ok(()));
    agrees(&bytes, &MIXED, MAX_DEPTH);
    for end in 0..bytes.len() {
        agrees(&bytes[..end], &MIXED, MAX_DEPTH);
    }
    for at in 0..bytes.len() {
        for value in [0, 1, 2, 0x7f, 0x80, 0xff] {
            let mut changed = bytes.clone();
            changed[at] = value;
            agrees(&changed, &MIXED, MAX_DEPTH);
        }
    }
    let mut unaligned = vec![0xff];
    unaligned.extend_from_slice(&bytes);
    unaligned.extend_from_slice(&[0xcc; 16]);
    assert_eq!(
        validate(&unaligned[1..1 + bytes.len()], &MIXED, MAX_DEPTH),
        Ok(())
    );
    assert_eq!(&unaligned[1..1 + bytes.len()], bytes);
}

#[test]
fn nested_faults_use_absolute_root_offsets_exactly_once() {
    let mut bytes = mixed();
    bytes[2] = 3;
    fault(&bytes, &MIXED, ErrorKind::InvalidBool, 2);
    bytes = mixed();
    bytes[11] = 2;
    fault(&bytes, &MIXED, ErrorKind::InvalidTag, 11);
    bytes = mixed();
    bytes[12] = 0xff;
    fault(&bytes, &MIXED, ErrorKind::InvalidUtf8, 12);
    bytes = mixed();
    bytes[18] = 2;
    fault(&bytes, &MIXED, ErrorKind::InvalidBool, 18);
    bytes = mixed();
    bytes[35] = 0xff;
    fault(&bytes, &MIXED, ErrorKind::InvalidUtf8, 35);
    let bytes = indexed(&[vec![0], vec![1, 3]]);
    let schema = Layout::List { element: &OPT_BOOL };
    fault(&bytes, &schema, ErrorKind::InvalidBool, 10);
    fault(&[1, 0, 0, 0, 0, 0, 3], &FIXEDS, ErrorKind::InvalidBool, 6);
    fault(
        &[1, 0, 0, 0, 5, 0, 0, 3],
        &MAP_FIXED,
        ErrorKind::InvalidBool,
        7,
    );
}

// Prefix lengths come from these fixture encodings, not production selectors.
fn wrap_fault_fixture(
    bytes: Vec<u8>,
    layout: &'static Layout,
    family: usize,
) -> (Vec<u8>, &'static Layout, u32) {
    let (outer, schema, prefix) = match family {
        0 => {
            let mut outer = vec![1];
            outer.extend_from_slice(&bytes);
            (outer, Layout::Option { value: layout }, 1)
        }
        1 => {
            let mut outer = 1u32.to_le_bytes().to_vec();
            outer.extend_from_slice(&bytes);
            (outer, Layout::List { element: layout }, 4)
        }
        2 => {
            let mut outer = vec![1, 0, 0, 0, 7];
            if width(layout).is_none() {
                outer.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            }
            let prefix = outer.len() as u32;
            outer.extend_from_slice(&bytes);
            (
                outer,
                Layout::Map {
                    key: KeyLayout {
                        width: 1,
                        domain: KeyDomain::Raw,
                    },
                    value: layout,
                },
                prefix,
            )
        }
        3 => {
            let mut outer = vec![0x34, 0x12];
            let (schema, prefix) = if let Some(size) = width(layout) {
                (
                    Layout::Struct {
                        fixed_bytes: 2 + size as u32,
                        fixed: Box::leak(Box::new([
                            FixedField {
                                declaration: 0,
                                offset: 0,
                                layout: &U16,
                            },
                            FixedField {
                                declaration: 1,
                                offset: 2,
                                layout,
                            },
                        ])),
                        variable: &[],
                    },
                    2,
                )
            } else {
                outer.extend_from_slice(&2u32.to_le_bytes());
                outer.extend_from_slice(b"ok");
                (
                    Layout::Struct {
                        fixed_bytes: 2,
                        fixed: Box::leak(Box::new([FixedField {
                            declaration: 0,
                            offset: 0,
                            layout: &U16,
                        }])),
                        variable: Box::leak(Box::new([
                            VariableField {
                                declaration: 1,
                                layout: &STR,
                            },
                            VariableField {
                                declaration: 2,
                                layout,
                            },
                        ])),
                    },
                    8,
                )
            };
            outer.extend_from_slice(&bytes);
            (outer, schema, prefix)
        }
        _ => unreachable!(),
    };
    (outer, Box::leak(Box::new(schema)), prefix)
}

#[test]
fn corrupt_diagnostics_at_depths_one_through_four() {
    // Each local fault is wrapped through all four parent families, in four
    // rotations. End offsets exercise truncation on an empty child as well.
    let cases: &[(&Layout, &[u8], ErrorKind, u32)] = &[
        (&BOOL, &[2], ErrorKind::InvalidBool, 0),
        (&STR, &[b'a', 0xff], ErrorKind::InvalidUtf8, 1),
        (&OPT_BOOL, &[7], ErrorKind::InvalidTag, 0),
        (&OPT_BOOL, &[1], ErrorKind::Truncated, 1),
        (&OPT_BOOL, &[0, 42], ErrorKind::InvalidBoundary, 1),
        (
            &STRS,
            &[2, 0, 0, 0, 9, 0, 0, 0],
            ErrorKind::InvalidBoundary,
            4,
        ),
        (&STRS, &[2, 0, 0, 0, 0], ErrorKind::Truncated, 4),
        (
            &MAP_STR,
            &[1, 0, 0, 0, 0, 1, 1, 0, 0, 0],
            ErrorKind::InvalidBoundary,
            6,
        ),
        (
            &MAP_BOOL,
            &[2, 0, 0, 0, 0, 2, 0, 0],
            ErrorKind::DuplicateKey,
            6,
        ),
        (
            &MAP_BOOL,
            &[2, 0, 0, 0, 1, 2, 0, 0],
            ErrorKind::UnsortedKeys,
            6,
        ),
    ];
    for &(leaf_layout, leaf_bytes, kind, leaf_offset) in cases {
        fault(leaf_bytes, leaf_layout, kind, leaf_offset);
        for rotation in 0..4 {
            let mut bytes = leaf_bytes.to_vec();
            let mut schema = leaf_layout;
            let mut offset = leaf_offset;
            for depth in 1..=4 {
                let (outer, parent, prefix) =
                    wrap_fault_fixture(bytes, schema, (rotation + depth - 1) % 4);
                bytes = outer;
                schema = parent;
                offset += prefix;
                assert_eq!(
                    validate(&bytes, schema, MAX_DEPTH),
                    Err(Error::new(kind, offset)),
                    "depth={depth}, rotation={rotation}, kind={kind:?}"
                );
                // The supplied slice is the coordinate origin, even unaligned.
                let mut storage = vec![0xcc];
                storage.extend_from_slice(&bytes);
                storage.push(0xdd);
                assert_eq!(
                    validate(&storage[1..1 + bytes.len()], schema, MAX_DEPTH),
                    Err(Error::new(kind, offset))
                );
            }
        }
    }
}

#[test]
fn partitions_and_all_keys_precede_child_domain_checks() {
    let mut bytes = indexed(&[vec![0xff], vec![]]);
    bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
    fault(&bytes, &STRS, ErrorKind::InvalidBoundary, 4);
    fault(
        &[2, 0, 0, 0, 0, 3, 0, 0],
        &MAP_BOOL,
        ErrorKind::DuplicateKey,
        6,
    );
    fault(
        &[2, 0, 0, 0, 1, 3, 0, 0],
        &MAP_BOOL,
        ErrorKind::UnsortedKeys,
        6,
    );
    fault(
        &[2, 0, 0, 0, 0, 3, 2, 0],
        &MAP_BOOL,
        ErrorKind::InvalidBool,
        6,
    );
    fault(&[1, 2, 3, 4], &OPT_BOOL, ErrorKind::InvalidBoundary, 2);
    let mut bytes = mixed();
    bytes[2] = 3;
    bytes[3..7].copy_from_slice(&u32::MAX.to_le_bytes());
    fault(&bytes, &MIXED, ErrorKind::InvalidBoundary, 3);
}

#[test]
fn empty_elided_children_still_receive_semantic_validation() {
    assert_eq!(validate(&[], &SINGLE, MAX_DEPTH), Ok(()));
    assert_eq!(validate(&[1], &OPT_STR, MAX_DEPTH), Ok(()));
    assert_eq!(
        validate(&indexed(&[vec![], vec![], vec![]]), &STRS, MAX_DEPTH),
        Ok(())
    );
    fault(&indexed(&[vec![]]), &OPTS, ErrorKind::Truncated, 4);
    let schema = Layout::List { element: &BOOLS };
    fault(&indexed(&[vec![]]), &schema, ErrorKind::Truncated, 4);
    assert_eq!(
        validate(&indexed(&[vec![0], vec![1]]), &OPTS, MAX_DEPTH),
        Ok(())
    );
    assert_eq!(
        validate(
            &mapped(&[0, 255], &[vec![0], vec![1, 1]]),
            &MAP_OPT,
            MAX_DEPTH
        ),
        Ok(())
    );
    fault(
        &mapped(&[0, 255], &[vec![0], vec![1, 2]]),
        &MAP_OPT,
        ErrorKind::InvalidBool,
        16,
    );
}

#[test]
fn budgets_count_active_nodes_and_skip_inactive_payloads() {
    assert_eq!(validate(&[], &STR, 1), Ok(()));
    assert_eq!(validate(&[0], &OPT_STR, 1), Ok(()));
    assert_eq!(validate(&[0; 4], &STRS, 1), Ok(()));
    assert_eq!(validate(&[0; 4], &MAP_STR, 1), Ok(()));
    for (bytes, layout) in [
        (vec![1], &OPT_STR),
        (indexed(&[vec![]]), &STRS),
        (vec![0, 0, 1], &FIXED),
    ] {
        assert_eq!(
            validate(&bytes, layout, 1),
            Err(ErrorKind::DepthExceeded.into())
        );
        agrees(&bytes, layout, 1);
        assert_eq!(validate(&bytes, layout, 2), Ok(()));
    }
    for budget in [0, MAX_DEPTH + 1, usize::MAX] {
        assert_eq!(
            validate(&[], &STR, budget),
            Err(ErrorKind::DepthExceeded.into())
        );
    }
    for budget in 1..=MAX_DEPTH {
        agrees(&mixed(), &MIXED, budget);
    }
}

#[test]
fn maximum_depth_and_sibling_continuations_do_not_lose_work() {
    let mut layout: &'static Layout = &BOOL;
    for _ in 1..MAX_DEPTH {
        layout = Box::leak(Box::new(Layout::Option { value: layout }));
    }
    let mut bytes = vec![1; MAX_DEPTH];
    assert_eq!(validate(&bytes, layout, MAX_DEPTH), Ok(()));
    assert_eq!(
        validate(&bytes, layout, MAX_DEPTH - 1),
        Err(ErrorKind::DepthExceeded.into())
    );
    bytes[MAX_DEPTH - 1] = 2;
    fault(
        &bytes,
        layout,
        ErrorKind::InvalidBool,
        (MAX_DEPTH - 1) as u32,
    );
    let rows = (0..100).map(|_| vec![2, 0, 0, 0, 0, 1]).collect::<Vec<_>>();
    let mut bytes = indexed(&rows);
    assert_eq!(validate(&bytes, &NESTED_LIST, MAX_DEPTH), Ok(()));
    *bytes.last_mut().unwrap() = 2;
    fault(
        &bytes,
        &NESTED_LIST,
        ErrorKind::InvalidBool,
        (bytes.len() - 1) as u32,
    );
}

#[test]
fn metadata_faults_precede_budget_and_data_faults_without_writes() {
    let invalid = Layout::Leaf(Leaf::Unsigned(3));
    for layout in [&invalid, &CYCLE] {
        for budget in [0, 1, MAX_DEPTH, usize::MAX] {
            assert_eq!(
                validate(&[0xff; 16], layout, budget),
                Err(ErrorKind::InvalidSchema.into())
            );
        }
    }
    let mut too_deep: &'static Layout = &U8;
    for _ in 0..MAX_DEPTH {
        too_deep = Box::leak(Box::new(Layout::Option { value: too_deep }));
    }
    assert_eq!(
        validate(&[0], too_deep, MAX_DEPTH),
        Err(ErrorKind::InvalidSchema.into())
    );
    let original = mixed();
    for at in 0..original.len() {
        let mut bytes = original.clone();
        bytes[at] = 0xff;
        let before = bytes.clone();
        let _ = validate(&bytes, &MIXED, MAX_DEPTH);
        assert_eq!(bytes, before);
    }
}

#[test]
fn exhaustive_short_inputs_match_the_independent_decoder() {
    let layouts = [
        &U8,
        &U16,
        &BOOL,
        &STR,
        &FIXED,
        &OPT_BOOL,
        &OPT_STR,
        &OPT_FIXED,
        &BOOLS,
        &STRS,
        &OPTS,
        &NESTED_LIST,
        &MAP_BOOL,
        &MAP_STR,
        &MAP_OPT,
        &MIXED,
        &SINGLE,
    ];
    for layout in layouts {
        agrees(&[], layout, MAX_DEPTH);
        for a in 0..=255 {
            agrees(&[a], layout, MAX_DEPTH);
            for b in 0..=255 {
                agrees(&[a, b], layout, MAX_DEPTH);
            }
        }
    }
}

#[test]
fn deterministic_corrupted_counts_boundaries_and_domains_match_reference() {
    let layouts = [
        &BOOLS,
        &STRS,
        &FIXEDS,
        &OPTS,
        &NESTED_LIST,
        &MAP_BOOL,
        &MAP_STR,
        &MAP_OPT,
        &MAP_FIXED,
        &MIXED,
    ];
    let mut state = 0x6513_29a7_fea2_740bu64;
    for n in 0..5000 {
        let len = n % 128;
        let mut bytes = vec![0; len];
        for byte in &mut bytes {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *byte = state as u8;
        }
        if len >= 4 && n % 2 == 0 {
            bytes[..4].copy_from_slice(&((n % 6) as u32).to_le_bytes());
        }
        for layout in layouts {
            agrees(&bytes, layout, MAX_DEPTH);
        }
    }
    for count in [0, 1, 2, 65536, u32::MAX] {
        for layout in layouts {
            agrees(&count.to_le_bytes(), layout, MAX_DEPTH);
        }
    }
}

#[test]
fn full_width_counts_and_zero_byte_elements_are_not_truncated_to_u16() {
    let count = 65537u32;
    let bytes = vec![0; count as usize];
    let mut bools = count.to_le_bytes().to_vec();
    bools.extend_from_slice(&bytes);
    assert_eq!(validate(&bools, &BOOLS, MAX_DEPTH), Ok(()));
    *bools.last_mut().unwrap() = 2;
    fault(&bools, &BOOLS, ErrorKind::InvalidBool, count + 3);
    let mut strings = count.to_le_bytes().to_vec();
    strings.resize(count as usize * 4, 0);
    assert_eq!(validate(&strings, &STRS, MAX_DEPTH), Ok(()));
    assert_eq!(
        validate(&strings, &STRS, 1),
        Err(ErrorKind::DepthExceeded.into())
    );
}

#[test]
fn every_container_family_composes_at_the_maximum_depth() {
    let mut layout: &'static Layout = &BOOL;
    let mut bytes = vec![1];
    for depth in 1..MAX_DEPTH {
        layout = Box::leak(Box::new(match depth % 4 {
            1 => {
                bytes.insert(0, 1);
                Layout::Option { value: layout }
            }
            2 => {
                bytes = indexed(&[bytes]);
                Layout::List { element: layout }
            }
            3 => {
                bytes = mapped(&[42], &[bytes]);
                Layout::Map {
                    key: KeyLayout {
                        width: 1,
                        domain: KeyDomain::Raw,
                    },
                    value: layout,
                }
            }
            _ => {
                bytes.insert(0, 0x42);
                Layout::Struct {
                    fixed_bytes: 1,
                    fixed: Box::leak(Box::new([FixedField {
                        declaration: 0,
                        offset: 0,
                        layout: &U8,
                    }])),
                    variable: Box::leak(Box::new([VariableField {
                        declaration: 1,
                        layout,
                    }])),
                }
            }
        }));
        assert_eq!(validate(&bytes, layout, MAX_DEPTH), Ok(()));
        for budget in 1..=MAX_DEPTH {
            agrees(&bytes, layout, budget);
        }
        let mut corrupt = bytes.clone();
        *corrupt.last_mut().unwrap() = 2;
        fault(
            &corrupt,
            layout,
            ErrorKind::InvalidBool,
            (corrupt.len() - 1) as u32,
        );
    }
}
