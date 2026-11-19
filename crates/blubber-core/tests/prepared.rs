//! Immediate/detached planning correspondence and exclusive inspection controls.
#![forbid(unsafe_code)]

use blubber_core::raw::{
    begin_apply, plan_list_edit, plan_map_edit, plan_replace, resolve_edit, InlinePath, ListEdit,
    MapEdit, Plan, Prepared, Step,
};
use blubber_core::{ErrorKind, KeyDomain, KeyLayout, Layout, Leaf, Search, VariableField};
use blubber_model::{Edit as ModelEdit, IntWidth, Key, KeySchema, Model, Schema, Value};

static U16: Layout = Layout::Leaf(Leaf::Unsigned(2));
static TEXT: Layout = Layout::Str;
static NUMBERS: Layout = Layout::List { element: &U16 };
static TEXTS: Layout = Layout::List { element: &TEXT };
static FIXED_MAP: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Unsigned,
    },
    value: &U16,
};
static INDEXED_MAP: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Unsigned,
    },
    value: &TEXT,
};
static NESTED: Layout = Layout::Struct {
    fixed_bytes: 0,
    fixed: &[],
    variable: &[
        VariableField {
            declaration: 0,
            layout: &TEXTS,
        },
        VariableField {
            declaration: 1,
            layout: &TEXT,
        },
    ],
};

fn account(root: &[u8], spare: usize) -> Vec<u8> {
    let mut bytes = b"prepared".to_vec();
    bytes.extend_from_slice(&(12 + root.len() as u32).to_le_bytes());
    bytes.extend_from_slice(root);
    bytes.resize(bytes.len() + spare, 0xa5);
    bytes
}

fn used(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[8..12].try_into().unwrap())
}

enum Operation {
    Replace(Vec<u8>),
    List(ListEdit, Vec<u8>),
    Map(MapEdit, [u8; 1], Vec<u8>, Option<Search>),
}

impl Operation {
    fn input(&self) -> &[u8] {
        match self {
            Self::Replace(v) | Self::List(_, v) | Self::Map(_, _, v, _) => v,
        }
    }

    fn plan(&self, bytes: &[u8], layout: &Layout, path: &InlinePath) -> Plan<'_> {
        let len = self.input().len() as u32;
        match self {
            Self::Replace(_) => {
                plan_replace(bytes, used(bytes), layout, path, len, u32::MAX).unwrap()
            }
            Self::List(op, _) => {
                plan_list_edit(bytes, used(bytes), layout, path, *op, len, u32::MAX).unwrap()
            }
            Self::Map(op, key, _, _) => plan_map_edit(
                bytes,
                used(bytes),
                layout,
                path,
                *op,
                if matches!(op, MapEdit::Clear) {
                    &[]
                } else {
                    key
                },
                len,
                u32::MAX,
            )
            .unwrap(),
        }
    }

    fn prepare<'a>(
        &self,
        bytes: &'a mut [u8],
        layout: &Layout,
        path: &InlinePath,
    ) -> Prepared<'a, '_> {
        let used = used(bytes);
        let resolved = resolve_edit(bytes, used, layout, path).unwrap();
        let len = self.input().len() as u32;
        match self {
            Self::Replace(_) => resolved.prepare_replace(len, u32::MAX).unwrap(),
            Self::List(op, _) => resolved.prepare_list(*op, len, u32::MAX).unwrap(),
            Self::Map(op, key, _, hint) => {
                let query = if matches!(op, MapEdit::Clear) {
                    &[][..]
                } else {
                    key
                };
                match hint {
                    Some(hint) => resolved
                        .prepare_entry(*op, query, len, u32::MAX, *hint)
                        .unwrap(),
                    None => resolved.prepare_map(*op, query, len, u32::MAX).unwrap(),
                }
            }
        }
    }
}

fn finish(mut apply: blubber_core::raw::Apply<'_, '_>, input: &[u8]) {
    apply.move_regions();
    apply.destination().copy_from_slice(input);
    apply.finish();
}

fn compare(
    initial: &[u8],
    layout: &Layout,
    path: &InlinePath,
    operation: Operation,
    expected: &[u8],
) {
    let mut strict = initial.to_vec();
    let mut immediate = initial.to_vec();
    let mut detached = initial.to_vec();
    let plan = operation.plan(&strict, layout, path);
    let expected_removed = plan
        .removed_span()
        .map(|s| strict[s.start as usize..s.end as usize].to_vec());
    finish(begin_apply(&mut strict, plan).unwrap(), operation.input());
    let prepared = operation.prepare(&mut immediate, layout, path);
    assert_eq!(prepared.inspect_prepared(), initial);
    let removed = prepared
        .inspect_removed()
        .map(|s| prepared.inspect_prepared()[s.start as usize..s.end as usize].to_vec());
    assert_eq!(removed, expected_removed);
    finish(prepared.begin_prepared().unwrap(), operation.input());
    let plan = operation.prepare(&mut detached, layout, path).detach();
    assert_eq!(detached, initial, "detaching preserves every byte");
    finish(begin_apply(&mut detached, plan).unwrap(), operation.input());
    assert_eq!(strict, immediate);
    assert_eq!(strict, detached);
    if matches!(operation, Operation::Replace(_)) {
        let mut compact = initial.to_vec();
        let old_used = used(&compact);
        let resolved = resolve_edit(&mut compact, old_used, layout, path).unwrap();
        assert!(resolved.subtree_fits(operation.input().len() as u32));
        let mut write = resolved
            .prepare_subtree(operation.input().len() as u32)
            .unwrap();
        write.destination().copy_from_slice(operation.input());
        write.finish();
        assert_eq!(
            strict, compact,
            "compact loan preserves the full physical frame"
        );
    }
    let end = used(&strict) as usize;
    assert_eq!(&strict[12..end], expected, "independent owned encoding");
    blubber_core::validate(&strict[12..end], layout, blubber_core::MAX_DEPTH).unwrap();
    assert_eq!(
        &strict[end.max(used(initial) as usize)..],
        &initial[end.max(used(initial) as usize)..]
    );
}

#[test]
fn fixed_and_indexed_list_plans_match_strict_detached_and_independent_oracle() {
    for (layout, element) in [
        (&NUMBERS, Schema::Unsigned(IntWidth::W2)),
        (&TEXTS, Schema::Str),
    ] {
        for count in 0..=3 {
            let values: Vec<_> = (0..count)
                .map(|i| {
                    if matches!(element, Schema::Str) {
                        Value::Str("a".repeat(i))
                    } else {
                        Value::Unsigned(i as u128)
                    }
                })
                .collect();
            let model =
                Model::new(Schema::List(Box::new(element.clone())), Value::List(values)).unwrap();
            let initial = account(&model.encode().unwrap(), 64);
            for index in 0..=count {
                let value = if matches!(element, Schema::Str) {
                    Value::Str("é".into())
                } else {
                    Value::Unsigned(99)
                };
                let input = element.encode(&value).unwrap();
                let mut expected = model.clone();
                expected
                    .apply(
                        &[],
                        ModelEdit::ListInsert {
                            index,
                            values: vec![value],
                        },
                    )
                    .unwrap();
                compare(
                    &initial,
                    layout,
                    &InlinePath::new(),
                    Operation::List(ListEdit::Insert(index as u32), input),
                    &expected.encode().unwrap(),
                );
                if index < count {
                    let mut expected = model.clone();
                    expected.apply(&[], ModelEdit::ListRemove(index)).unwrap();
                    compare(
                        &initial,
                        layout,
                        &InlinePath::new(),
                        Operation::List(ListEdit::Remove(index as u32), vec![]),
                        &expected.encode().unwrap(),
                    );
                }
            }
            for (op, edit) in [
                (ListEdit::Pop, ModelEdit::ListPop),
                (ListEdit::Clear, ModelEdit::Clear),
                (ListEdit::Truncate(1), ModelEdit::ListTruncate(1)),
                (
                    ListEdit::Truncate(u32::MAX),
                    ModelEdit::ListTruncate(u32::MAX as usize),
                ),
            ] {
                let mut expected = model.clone();
                expected.apply(&[], edit).unwrap();
                compare(
                    &initial,
                    layout,
                    &InlinePath::new(),
                    Operation::List(op, vec![]),
                    &expected.encode().unwrap(),
                );
            }
        }
    }
}

#[test]
fn map_layouts_and_retained_ordinals_match_strict_and_owned_oracle() {
    for (layout, value_schema) in [
        (&FIXED_MAP, Schema::Unsigned(IntWidth::W2)),
        (&INDEXED_MAP, Schema::Str),
    ] {
        let value = |i| {
            if matches!(value_schema, Schema::Str) {
                Value::Str("x".repeat(i))
            } else {
                Value::Unsigned(i as u128)
            }
        };
        let schema = Schema::Map(
            KeySchema::Unsigned(IntWidth::W1),
            Box::new(value_schema.clone()),
        );
        let model = Model::new(
            schema,
            Value::Map([(Key::Unsigned(2), value(0)), (Key::Unsigned(4), value(3))].into()),
        )
        .unwrap();
        let initial = account(&model.encode().unwrap(), 64);
        for key in 1..=5 {
            let index = match key {
                1 | 2 => 0,
                3 | 4 => 1,
                _ => 2,
            };
            let hint = Search {
                index,
                found: key == 2 || key == 4,
            };
            for hinted in [false, true] {
                let mut expected = model.clone();
                expected
                    .apply(
                        &[],
                        ModelEdit::MapInsert {
                            key: Key::Unsigned(key as u128),
                            value: value(2),
                        },
                    )
                    .unwrap();
                compare(
                    &initial,
                    layout,
                    &InlinePath::new(),
                    Operation::Map(
                        MapEdit::Insert,
                        [key],
                        value_schema.encode(&value(2)).unwrap(),
                        hinted.then_some(hint),
                    ),
                    &expected.encode().unwrap(),
                );
                let mut expected = model.clone();
                expected
                    .apply(&[], ModelEdit::MapRemove(Key::Unsigned(key as u128)))
                    .unwrap();
                compare(
                    &initial,
                    layout,
                    &InlinePath::new(),
                    Operation::Map(MapEdit::Remove, [key], vec![], hinted.then_some(hint)),
                    &expected.encode().unwrap(),
                );
            }
        }
        let mut expected = model.clone();
        expected.apply(&[], ModelEdit::Clear).unwrap();
        compare(
            &initial,
            layout,
            &InlinePath::new(),
            Operation::Map(MapEdit::Clear, [0], vec![], None),
            &expected.encode().unwrap(),
        );
    }
}

#[test]
fn nested_replace_reuses_exact_trail_and_preserves_siblings() {
    let schema = Schema::Struct(vec![Schema::List(Box::new(Schema::Str)), Schema::Str]);
    let mut model = Model::new(
        schema,
        Value::Struct(vec![
            Value::List(vec![Value::Str("".into()), Value::Str("old".into())]),
            Value::Str("tail".into()),
        ]),
    )
    .unwrap();
    let initial = account(&model.encode().unwrap(), 64);
    let mut path = InlinePath::new();
    path.push(Step::Field(0)).unwrap();
    path.push(Step::Index(1)).unwrap();
    model
        .apply(
            &[
                blubber_model::PathSegment::Field(0),
                blubber_model::PathSegment::Index(1),
            ],
            ModelEdit::Set(Value::Str("longer 🦀".into())),
        )
        .unwrap();
    compare(
        &initial,
        &NESTED,
        &path,
        Operation::Replace("longer 🦀".as_bytes().to_vec()),
        &model.encode().unwrap(),
    );
}

#[test]
fn insufficient_prepared_capacity_and_all_preflight_errors_preserve_full_buffer() {
    let mut bytes = account(b"old", 0);
    let before = bytes.clone();
    let prepared = resolve_edit(&mut bytes, 15, &TEXT, &InlinePath::new())
        .unwrap()
        .prepare_replace(20, u32::MAX)
        .unwrap();
    assert_eq!(prepared.prepared_len(), 32);
    assert_eq!(
        prepared.begin_prepared().err().unwrap().custom_code(),
        Some(0xB100 + ErrorKind::InvalidPlan as u32)
    );
    assert_eq!(bytes, before);
    let plan = resolve_edit(&mut bytes, 15, &TEXT, &InlinePath::new())
        .unwrap()
        .prepare_replace(20, u32::MAX)
        .unwrap()
        .detach();
    bytes.resize(32, 0);
    finish(begin_apply(&mut bytes, plan).unwrap(), &[b'x'; 20]);
    assert_eq!(&bytes[12..], &[b'x'; 20]);
    let mut bytes = account(&[0, 0, 0, 0], 16);
    let before = bytes.clone();
    let error = resolve_edit(&mut bytes, 16, &TEXTS, &InlinePath::new())
        .unwrap()
        .prepare_list(ListEdit::Remove(usize::MAX as u32), 0, u32::MAX)
        .err()
        .unwrap();
    assert_eq!(
        error.custom_code(),
        Some(0xB100 + ErrorKind::IndexOutOfBounds as u32)
    );
    assert_eq!(bytes, before);
}

#[test]
fn stale_detached_boundaries_reject_but_compatible_content_and_relocation_work() {
    let mut bytes = account(b"old", 10);
    let plan = resolve_edit(&mut bytes, 15, &TEXT, &InlinePath::new())
        .unwrap()
        .prepare_replace(3, u32::MAX)
        .unwrap()
        .detach();
    bytes[12..15].copy_from_slice(b"new");
    let mut relocated = bytes.clone();
    finish(begin_apply(&mut relocated, plan).unwrap(), b"yes");
    assert_eq!(&relocated[12..15], b"yes");
    let plan = resolve_edit(&mut bytes, 15, &TEXT, &InlinePath::new())
        .unwrap()
        .prepare_replace(3, u32::MAX)
        .unwrap()
        .detach();
    bytes[8..12].copy_from_slice(&14u32.to_le_bytes());
    let before = bytes.clone();
    assert_eq!(
        begin_apply(&mut bytes, plan).err().unwrap().custom_code(),
        Some(0xB100 + ErrorKind::InvalidPlan as u32)
    );
    assert_eq!(bytes, before);
}

#[test]
fn forged_entry_ordinals_and_corrupt_partitions_reject_without_writes() {
    let model = Model::new(
        Schema::Map(KeySchema::Unsigned(IntWidth::W1), Box::new(Schema::Str)),
        Value::Map([(Key::Unsigned(2), Value::Str("a".into()))].into()),
    )
    .unwrap();
    for hint in [
        Search {
            index: 0,
            found: true,
        },
        Search {
            index: 0,
            found: false,
        },
        Search {
            index: u32::MAX,
            found: false,
        },
    ] {
        let mut bytes = account(&model.encode().unwrap(), 32);
        let before = bytes.clone();
        let n = used(&bytes);
        let error = resolve_edit(&mut bytes, n, &INDEXED_MAP, &InlinePath::new())
            .unwrap()
            .prepare_entry(MapEdit::Insert, &[3], 1, u32::MAX, hint)
            .err()
            .unwrap();
        assert_eq!(
            error.custom_code(),
            Some(0xB100 + ErrorKind::InvalidPlan as u32)
        );
        assert_eq!(bytes, before);
    }
    let mut bytes = account(&[2, 0, 0, 0, 255, 255, 255, 255], 32);
    let before = bytes.clone();
    let n = used(&bytes);
    assert!(resolve_edit(&mut bytes, n, &TEXTS, &InlinePath::new())
        .unwrap()
        .prepare_list(ListEdit::Insert(1), 0, u32::MAX)
        .is_err());
    assert_eq!(bytes, before);
}

#[test]
fn typed_ingress_checks_identity_minimum_and_fault_precedence_without_writes() {
    let identity = *b"prepared";
    for (mut bytes, discriminator, minimum, kind, offset) in [
        (vec![], [0; 8], 0, ErrorKind::InvalidSchema, u32::MAX),
        (vec![0; 4], identity, 0, ErrorKind::Truncated, 0),
        (vec![0; 12], identity, 0, ErrorKind::Discriminator, 0),
        (account(b"old", 0), identity, 4, ErrorKind::UsedLength, 8),
        (
            account(b"old", 0),
            identity,
            u32::MAX,
            ErrorKind::ArithmeticOverflow,
            u32::MAX,
        ),
    ] {
        let before = bytes.clone();
        let error = blubber_core::raw::resolve_account_edit(
            &mut bytes,
            discriminator,
            minimum,
            &TEXT,
            &InlinePath::new(),
        )
        .err()
        .unwrap();
        assert_eq!(error, blubber_core::Error::new(kind, offset));
        assert_eq!(bytes, before);
    }
    let mut bytes = account(b"old", 8);
    let before = bytes.clone();
    let resolved =
        blubber_core::raw::resolve_account_edit(&mut bytes, identity, 3, &TEXT, &InlinePath::new())
            .unwrap();
    assert_eq!(resolved.inspect_resolved(), before);
    let (span, _) = resolved.selection();
    assert_eq!((span.start, span.end), (12, 15));
}

#[test]
fn zero_delta_ancestor_negative_geometries_are_reachable_on_valid_roots() {
    static RAW_MAP: Layout = Layout::Map {
        key: KeyLayout {
            width: 1,
            domain: KeyDomain::Raw,
        },
        value: &TEXT,
    };
    const ROOTS: [Layout; 3] = [
        Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[
                VariableField {
                    declaration: 0,
                    layout: &TEXT,
                },
                VariableField {
                    declaration: 1,
                    layout: &TEXT,
                },
            ],
        },
        Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[
                VariableField {
                    declaration: 0,
                    layout: &TEXTS,
                },
                VariableField {
                    declaration: 1,
                    layout: &TEXTS,
                },
            ],
        },
        Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[
                VariableField {
                    declaration: 0,
                    layout: &RAW_MAP,
                },
                VariableField {
                    declaration: 1,
                    layout: &RAW_MAP,
                },
            ],
        },
    ];
    for operation in 0..4 {
        let layout = &ROOTS[match operation {
            0 => 0,
            1 | 2 => 1,
            _ => 2,
        }];
        let mut bytes = [0; 64];
        bytes[..8].copy_from_slice(b"negative");
        bytes[8..12].copy_from_slice(&48u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&4u32.to_le_bytes());
        if operation != 0 {
            bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
            if operation == 3 {
                bytes[25..29].copy_from_slice(&19u32.to_le_bytes());
            }
        }
        bytes[48..].fill(0xa5);
        blubber_core::validate(&bytes[12..48], layout, blubber_core::MAX_DEPTH).unwrap();
        let before = bytes;
        let mut path = InlinePath::new();
        path.push(Step::Field(0)).unwrap();
        let resolved = resolve_edit(&mut bytes, 48, layout, &path).unwrap();
        let span = resolved.selection().0;
        assert_eq!((span.start, span.end), (16, 20));
        let prepared = match operation {
            0 => resolved.prepare_replace(4, 64),
            1 => resolved.prepare_list(ListEdit::Insert(0), 0, 64),
            2 => resolved.prepare_list(ListEdit::Pop, 0, 64),
            _ => resolved.prepare_map(MapEdit::Clear, &[], 0, 64),
        }
        .unwrap();
        assert_eq!(prepared.prepared_len(), 48);
        finish(
            prepared.begin_prepared().unwrap(),
            if operation == 0 { &[0; 4] } else { &[] },
        );
        let mut expected = before;
        if operation == 1 {
            expected[16..20].copy_from_slice(&1u32.to_le_bytes());
        }
        assert_eq!(bytes, expected, "zero-delta ancestor and full spare frame");
        blubber_core::validate(&bytes[12..48], layout, blubber_core::MAX_DEPTH).unwrap();
    }
}

#[test]
fn local_overwrite_ignores_distant_ends_but_strict_validation_rejects_them() {
    static FIELDS: Layout = Layout::Struct {
        fixed_bytes: 0,
        fixed: &[],
        variable: &[
            VariableField {
                declaration: 0,
                layout: &TEXT,
            },
            VariableField {
                declaration: 1,
                layout: &TEXT,
            },
            VariableField {
                declaration: 2,
                layout: &TEXT,
            },
            VariableField {
                declaration: 3,
                layout: &TEXT,
            },
        ],
    };
    let mut root = Vec::new();
    for end in [1u32, 2, u32::MAX] {
        root.extend_from_slice(&end.to_le_bytes());
    }
    root.extend_from_slice(b"abcd");
    let mut bytes = account(&root, 16);
    let before = bytes.clone();
    let mut path = InlinePath::new();
    path.push(Step::Field(0)).unwrap();
    assert!(plan_replace(&bytes, used(&bytes), &FIELDS, &path, 1, u32::MAX).is_err());
    assert!(blubber_core::validate(&root, &FIELDS, blubber_core::MAX_DEPTH).is_err());
    let len = used(&bytes);
    let resolved = resolve_edit(&mut bytes, len, &FIELDS, &path).unwrap();
    resolved.into_destination().copy_from_slice(b"z");
    let mut expected = before;
    expected[24] = b'z';
    assert_eq!(bytes, expected);
    let before = bytes.clone();
    let len = used(&bytes);
    let resolved = resolve_edit(&mut bytes, len, &FIELDS, &path).unwrap();
    assert!(resolved.prepare_replace(2, u32::MAX).is_err());
    assert_eq!(bytes, before);
}

#[test]
fn empty_local_maps_check_headers_and_query_domains_before_absence() {
    for value in [&U16, &TEXT] {
        let layout = Layout::Map {
            key: KeyLayout {
                width: 1,
                domain: KeyDomain::Bool,
            },
            value,
        };
        for key in [0, 1] {
            assert_eq!(
                blubber_core::search_map_local(&[0; 4], &layout, &[key]),
                Ok(Search {
                    index: 0,
                    found: false
                })
            );
        }
        for key in [&[][..], &[0, 1][..]] {
            assert_eq!(
                blubber_core::search_map_local(&[0; 4], &layout, key),
                Err(ErrorKind::InvalidBoundary.into())
            );
        }
        assert_eq!(
            blubber_core::search_map_local(&[0; 4], &layout, &[2]),
            Err(ErrorKind::InvalidBool.into())
        );
        assert!(blubber_core::search_map_local(&[0; 5], &layout, &[0]).is_err());
    }
}

#[test]
fn local_maps_check_neighbors_without_certifying_distant_order() {
    for layout in [&FIXED_MAP, &INDEXED_MAP] {
        let mut root = 4u32.to_le_bytes().to_vec();
        if core::ptr::eq(layout, &FIXED_MAP) {
            for key in [1u8, 1, 3, 4] {
                root.push(key);
                root.extend_from_slice(&7u16.to_le_bytes());
            }
        } else {
            for key in [1u8, 1, 3, 4] {
                root.push(key);
                root.extend_from_slice(&0u32.to_le_bytes());
            }
        }
        let mut bytes = account(&root, 32);
        let path = InlinePath::new();
        let input = if core::ptr::eq(layout, &FIXED_MAP) {
            vec![8, 0]
        } else {
            Vec::new()
        };
        assert!(plan_map_edit(
            &bytes,
            used(&bytes),
            layout,
            &path,
            MapEdit::Insert,
            &[5],
            input.len() as u32,
            u32::MAX
        )
        .is_err());
        let len = used(&bytes);
        let prepared = resolve_edit(&mut bytes, len, layout, &path)
            .unwrap()
            .prepare_map(MapEdit::Insert, &[5], input.len() as u32, u32::MAX)
            .unwrap();
        let mut apply = prepared.begin_prepared().unwrap();
        apply.move_regions();
        apply.destination().copy_from_slice(&input);
        apply.finish();
        assert!(blubber_core::validate(
            &bytes[12..used(&bytes) as usize],
            layout,
            blubber_core::MAX_DEPTH
        )
        .is_err());
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 5);
    }
}

#[test]
fn fixed_destination_preserves_envelope_spare_and_all_error_buffers() {
    let mut bytes = account(&[0; 12], 17);
    let before = bytes.clone();
    blubber_core::raw::fixed_destination(&mut bytes, *b"prepared", 12, Some(12), 4, 4)
        .unwrap()
        .copy_from_slice(&123u32.to_le_bytes());
    let mut expected = before;
    expected[16..20].copy_from_slice(&123u32.to_le_bytes());
    assert_eq!(bytes, expected);
    for (offset, width, exact) in [(u32::MAX, 4, Some(12)), (12, 1, Some(12)), (0, 4, Some(11))] {
        let before = bytes.clone();
        assert!(blubber_core::raw::fixed_destination(
            &mut bytes,
            *b"prepared",
            12,
            exact,
            offset,
            width
        )
        .is_err());
        assert_eq!(bytes, before);
    }
}
