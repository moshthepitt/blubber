//! Direct fixed and tail paths agree with checked generic operations.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]
use blubber_core::raw::{
    begin_apply, plan_list_edit, plan_map_edit, plan_replace, InlinePath, ListEdit, MapEdit, Step,
};
use blubber_core::{KeyDomain, KeyLayout, Layout, Leaf, VariableField};
static U16: Layout = Layout::Leaf(Leaf::Unsigned(2));
static TEXT: Layout = Layout::Str;
static NUMBERS: Layout = Layout::List { element: &U16 };
static FIXED_MAP: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Unsigned,
    },
    value: &U16,
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

static ROOT_NUMBERS: Layout = Layout::Struct {
    fixed_bytes: 0,
    fixed: &[],
    variable: &[VariableField {
        declaration: 0,
        layout: &NUMBERS,
    }],
};
static ROOT_MAP: Layout = Layout::Struct {
    fixed_bytes: 0,
    fixed: &[],
    variable: &[VariableField {
        declaration: 0,
        layout: &FIXED_MAP,
    }],
};
static ROOT_PAIR: Layout = Layout::Struct {
    fixed_bytes: 0,
    fixed: &[],
    variable: &[
        VariableField {
            declaration: 0,
            layout: &NUMBERS,
        },
        VariableField {
            declaration: 1,
            layout: &NUMBERS,
        },
    ],
};
static ROOT_TEXT: Layout = Layout::Struct {
    fixed_bytes: 0,
    fixed: &[],
    variable: &[VariableField {
        declaration: 0,
        layout: &TEXT,
    }],
};

#[test]
fn direct_tail_and_generic_plans_produce_identical_physical_buffers() {
    for map in [false, true] {
        for count in 0..=8u32 {
            let mut root = count.to_le_bytes().to_vec();
            for i in 0..count {
                if map {
                    root.push((2 * i + 1) as u8);
                }
                root.extend_from_slice(&(i as u16).to_le_bytes());
            }
            let layout = if map { &ROOT_MAP } else { &ROOT_NUMBERS };
            let input = 0x1234u16.to_le_bytes();
            let key = [255u8];
            let width = input.len() + usize::from(map);
            let mut path = InlinePath::new();
            path.push(Step::Field(0)).unwrap();
            for spare in [0, width - 1, width, width + 17] {
                let mut fast = account(&root, spare);
                let before = fast.clone();
                let mut generic = fast.clone();
                let admitted = match blubber_core::raw::prepare_tail_append(
                    &mut fast,
                    *b"prepared",
                    4,
                    layout,
                    &path,
                    2,
                    map.then_some(key.as_slice()),
                )
                .unwrap()
                {
                    Some(mut tail) => {
                        tail.destination().copy_from_slice(&input);
                        tail.finish();
                        true
                    }
                    None => false,
                };
                if !admitted {
                    assert_eq!(fast, before);
                    assert!(spare < width);
                    continue;
                }
                let plan = if map {
                    plan_map_edit(
                        &generic,
                        used(&generic),
                        layout,
                        &path,
                        MapEdit::Insert,
                        &key,
                        2,
                        u32::MAX,
                    )
                    .unwrap()
                } else {
                    plan_list_edit(
                        &generic,
                        used(&generic),
                        layout,
                        &path,
                        ListEdit::Insert(count),
                        2,
                        u32::MAX,
                    )
                    .unwrap()
                };
                let mut apply = begin_apply(&mut generic, plan).unwrap();
                apply.move_regions();
                apply.destination().copy_from_slice(&input);
                apply.finish();
                assert_eq!(fast, generic, "map={map} count={count} spare={spare}");
                blubber_core::validate(
                    &fast[12..used(&fast) as usize],
                    layout,
                    blubber_core::MAX_DEPTH,
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn direct_string_and_generic_plans_match_growth_shrink_and_same_size() {
    let mut path = InlinePath::new();
    path.push(Step::Field(0)).unwrap();
    for old in ["", "a", "é", "original", "0123456789abcdef"] {
        for new in ["", "a", "é", "abcdefgh", "0123456789abcdef0123456789abcdef"] {
            for spare in [0, 1, 7, 32] {
                let mut fast = account(old.as_bytes(), spare);
                let before = fast.clone();
                let mut generic = fast.clone();
                let admitted = match blubber_core::raw::prepare_string_write(
                    &mut fast,
                    *b"prepared",
                    0,
                    &ROOT_TEXT,
                    &path,
                    new.len(),
                )
                .unwrap()
                {
                    Some(mut write) => {
                        write.destination().copy_from_slice(new.as_bytes());
                        write.finish();
                        true
                    }
                    None => false,
                };
                if !admitted {
                    assert_eq!(fast, before);
                    assert!(old.len() + spare < new.len());
                    continue;
                }
                let plan = plan_replace(
                    &generic,
                    used(&generic),
                    &ROOT_TEXT,
                    &path,
                    new.len() as u32,
                    u32::MAX,
                )
                .unwrap();
                let mut apply = begin_apply(&mut generic, plan).unwrap();
                apply.move_regions();
                apply.destination().copy_from_slice(new.as_bytes());
                apply.finish();
                assert_eq!(fast, generic, "old={old:?} new={new:?} spare={spare}");
            }
        }
    }
}

#[test]
fn direct_tail_corrupted_headers_and_fallbacks_preserve_all_bytes() {
    let mut path = InlinePath::new();
    path.push(Step::Field(0)).unwrap();
    for logical in [0u32, 1, 11, 12, 13, 15, 16, 17, 32, u32::MAX] {
        for count in [0u32, 1, 2, u32::MAX] {
            for physical in [0, 1, 8, 12, 15, 16, 17, 32] {
                let mut bytes = account(&count.to_le_bytes(), 16);
                bytes.truncate(physical);
                if bytes.len() >= 12 {
                    bytes[8..12].copy_from_slice(&logical.to_le_bytes());
                }
                let before = bytes.clone();
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = blubber_core::raw::prepare_tail_append(
                        &mut bytes,
                        *b"prepared",
                        4,
                        &ROOT_NUMBERS,
                        &path,
                        2,
                        None,
                    );
                }));
                assert!(outcome.is_ok());
                assert_eq!(bytes, before);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = blubber_core::raw::prepare_string_write(
                        &mut bytes,
                        *b"prepared",
                        0,
                        &ROOT_TEXT,
                        &path,
                        8,
                    );
                }));
                assert!(outcome.is_ok());
                assert_eq!(bytes, before);
            }
        }
    }
}

use blubber::ReadStorage;
use blubber::Resize as _;
use blubber::{List, Map, SliceStore, Str};
#[blubber::schema(account)]
struct ScalarRoot {
    value: u64,
    a: Str,
    b: Str,
    c: Str,
}
#[blubber::schema(account)]
struct TextRoot {
    value: Str,
}
#[blubber::schema(account)]
struct ListRoot {
    value: List<u16>,
}
#[blubber::schema(account)]
struct MapRoot {
    value: Map<u8, u16>,
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn typed_fixed_prefix_and_tail_edits_preserve_corrupt_distant_bytes() {
    let mut bytes = vec![0; 96];
    {
        let mut store = SliceStore::new(&mut bytes);
        ScalarRoot::initialize(
            &mut store,
            ScalarRootInit {
                value: 1,
                a: "a",
                b: "b",
                c: "c",
            },
        )
        .unwrap();
    }
    bytes[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(ScalarRoot::load(&bytes).is_err());
    let before = bytes.clone();
    {
        let mut store = SliceStore::new(&mut bytes);
        ScalarRoot::load_trusted_mut(&mut store)
            .unwrap()
            .value_mut()
            .set(123)
            .unwrap();
        assert!(!store.is_poisoned());
    }
    let mut expected = before;
    expected[12..20].copy_from_slice(&123u64.to_le_bytes());
    assert_eq!(bytes, expected);
    assert!(ScalarRoot::load(&bytes).is_err());

    let mut bytes = vec![0; 96];
    {
        let mut store = SliceStore::new(&mut bytes);
        MapRoot::initialize(
            &mut store,
            MapRootInit {
                value: &[(1, 11), (2, 22), (3, 33), (4, 44)],
            },
        )
        .unwrap();
    }
    bytes[16] = 2; // Distant duplicate; endpoint 4 is still the checked predecessor.
    assert!(MapRoot::load(&bytes).is_err());
    {
        let mut store = SliceStore::new(&mut bytes);
        assert_eq!(
            MapRoot::load_trusted_mut(&mut store)
                .unwrap()
                .value_mut()
                .insert(255, 55)
                .unwrap(),
            None
        );
        assert!(!store.is_poisoned());
    }
    assert!(MapRoot::load(&bytes).is_err());
    assert_eq!(bytes[16], 2);
}

#[test]
fn typed_tail_loans_cover_repeated_edits_and_capacity_fallbacks() {
    for capacity in [20, 64] {
        let mut bytes = vec![0; capacity];
        let mut store = SliceStore::new(&mut bytes);
        let mut root =
            TextRoot::initialize(&mut store, TextRootInit { value: "original" }).unwrap();
        root.value_mut().set("abcdefgh").unwrap();
        let grow = root.value_mut().set("0123456789abcdef0123456789abcdef");
        if capacity == 20 {
            assert!(grow.is_err());
            assert_eq!(root.value(), "abcdefgh");
        } else {
            grow.unwrap();
        }
        root.value_mut().set("é").unwrap();
        root.value_mut().set("").unwrap();
        assert_eq!(root.value(), "");
        root.validate().unwrap();
    }
    let mut bytes = [0; 32];
    let mut store = SliceStore::new(&mut bytes);
    let mut root = ListRoot::initialize(&mut store, ListRootInit { value: &[1, 2] }).unwrap();
    root.value_mut().push(3).unwrap();
    root.value_mut().push(4).unwrap();
    assert_eq!(root.value().get(3), Some(4));
    root.validate().unwrap();
}

#[cfg(not(miri))]
proptest::proptest! {
    #[test]
    fn typed_map_histories_match_an_independent_byte_oracle(
        operations in proptest::collection::vec((proptest::num::u8::ANY, proptest::num::u16::ANY), 1..24)
    ) {
        let mut bytes = [0; 128];
        let mut store = SliceStore::new(&mut bytes);
        let mut expected = std::collections::BTreeMap::new();
        {
            let mut root = MapRoot::initialize(&mut store, MapRootInit { value: &[] }).unwrap();
            for (key, value) in operations {
                let previous = expected.insert(key, value);
                proptest::prop_assert_eq!(root.value_mut().insert(key, value).unwrap(), previous);
                root.validate().unwrap();
            }
        }
        let mut oracle = MapRoot::DISCRIMINATOR.to_vec();
        oracle.extend_from_slice(&(16 + expected.len() as u32 * 3).to_le_bytes());
        oracle.extend_from_slice(&(expected.len() as u32).to_le_bytes());
        for (key, value) in expected {
            oracle.push(key);
            oracle.extend_from_slice(&value.to_le_bytes());
        }
        oracle.resize(128, 0);
        proptest::prop_assert_eq!(store.bytes(), oracle.as_slice());
    }
}

#[blubber::schema(account)]
struct MixedPrefix {
    name: Str,
    a: u16,
    b: u64,
}

#[test]
fn generated_unaligned_prefix_setter_matches_the_generic_plan() {
    for new in [0, 1, u64::MAX, 0x1234_5678_90ab_cdef] {
        let mut fast = [0; 64];
        {
            let mut store = SliceStore::new(&mut fast);
            MixedPrefix::initialize(
                &mut store,
                MixedPrefixInit {
                    name: "é",
                    a: 0x1234,
                    b: 55,
                },
            )
            .unwrap();
        }
        let mut generic = fast;
        let layout = Layout::Flat {
            checked: <MixedPrefix as blubber::RootSchema>::__layout(),
        };
        let mut path = InlinePath::new();
        path.push(Step::Field(2)).unwrap();
        let plan = plan_replace(&generic, used(&generic), &layout, &path, 8, 64).unwrap();
        let mut apply = begin_apply(&mut generic, plan).unwrap();
        apply.move_regions();
        apply.destination().copy_from_slice(&new.to_le_bytes());
        apply.finish();
        {
            let mut store = SliceStore::new(&mut fast);
            MixedPrefix::load_mut(&mut store)
                .unwrap()
                .b_mut()
                .set(new)
                .unwrap();
        }
        assert_eq!(fast, generic);
    }
}

blubber::__root_schema!(__GapMap, Map<u8, u8>);

#[test]
fn missing_interior_key_removal_matches_generic_bytes_and_preserves_the_full_buffer() {
    for values in [[0, 0], [0, 255], [255, 0], [255, 255], [7, 9]] {
        let mut fast = [0xa5; 32];
        fast[..8].copy_from_slice(b"prepared");
        fast[8..12].copy_from_slice(&20u32.to_le_bytes());
        fast[12..16].copy_from_slice(&2u32.to_le_bytes());
        fast[16..20].copy_from_slice(&[2, values[0], 4, values[1]]);
        let before = fast;
        let mut generic = fast;
        let layout = Layout::Flat {
            checked: <__GapMap as blubber::RootSchema>::__layout(),
        };
        let plan = plan_map_edit(
            &generic,
            20,
            &layout,
            &InlinePath::new(),
            MapEdit::Remove,
            &[3],
            0,
            32,
        )
        .unwrap();
        let mut apply = begin_apply(&mut generic, plan).unwrap();
        apply.move_regions();
        assert!(apply.destination().is_empty());
        apply.finish();
        {
            let mut store = SliceStore::new(&mut fast);
            {
                let edit = blubber::raw::Edit::load::<__GapMap>(&mut store, *b"prepared").unwrap();
                let mut map = blubber::raw::project::<Map<u8, u8>>(edit);
                assert_eq!(map.remove(&3).unwrap(), None);
            }
            assert!(!store.is_poisoned());
        }
        assert_eq!(fast, generic);
        assert_eq!(fast, before);
    }
}

#[blubber::schema(account)]
struct OnlyNumber {
    value: u64,
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn fixed_root_setter_rejects_a_forged_extra_used_byte_without_writes() {
    let mut bytes = [0; 32];
    {
        let mut store = SliceStore::new(&mut bytes);
        OnlyNumber::initialize(&mut store, OnlyNumberInit { value: 7 }).unwrap();
    }
    bytes[8..12].copy_from_slice(&21u32.to_le_bytes());
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    assert!(matches!(
        OnlyNumber::load_trusted_mut(&mut store)
            .unwrap()
            .value_mut()
            .set(99),
        Err(blubber::Error::Fault {
            kind: blubber::ErrorKind::InvalidBoundary,
            ..
        })
    ));
    assert_eq!(store.bytes(), &before);
    assert!(!store.is_poisoned());
}

#[test]
fn manual_field_ordinals_select_fixed_and_later_variable_fields() {
    let mut bytes = [0; 96];
    let mut store = SliceStore::new(&mut bytes);
    ScalarRoot::initialize(
        &mut store,
        ScalarRootInit {
            value: 42,
            a: "aa",
            b: "bé",
            c: "ccc",
        },
    )
    .unwrap();
    let before = store.bytes().to_vec();
    {
        let mut edit =
            blubber::raw::Edit::load::<ScalarRoot>(&mut store, ScalarRoot::DISCRIMINATOR).unwrap();
        assert_eq!(
            edit.reborrow()
                .field(0)
                .view()
                .unwrap()
                .read_fixed::<u64>()
                .unwrap(),
            42
        );
        for (ordinal, expected) in [(1, "aa"), (2, "bé"), (3, "ccc")] {
            assert_eq!(
                edit.reborrow()
                    .field(ordinal)
                    .view()
                    .unwrap()
                    .read::<Str>()
                    .unwrap(),
                expected
            );
        }
        assert!(edit.reborrow().field(u16::MAX).view().is_err());
    }
    assert_eq!(store.bytes(), before.as_slice());
}

#[blubber::schema]
struct FixedLayer<T: blubber::Schema> {
    inner: T,
}

#[cfg(not(feature = "max-depth-16"))]
type AtDepthLimit =
    FixedLayer<FixedLayer<FixedLayer<FixedLayer<FixedLayer<FixedLayer<FixedLayer<u64>>>>>>>;
#[cfg(feature = "max-depth-16")]
type AtDepthLimit = FixedLayer<
    FixedLayer<
        FixedLayer<
            FixedLayer<
                FixedLayer<
                    FixedLayer<
                        FixedLayer<
                            FixedLayer<
                                FixedLayer<
                                    FixedLayer<
                                        FixedLayer<
                                            FixedLayer<FixedLayer<FixedLayer<FixedLayer<u64>>>>,
                                        >,
                                    >,
                                >,
                            >,
                        >,
                    >,
                >,
            >,
        >,
    >,
>;
blubber::__root_schema!(__FixedDepthRoot, AtDepthLimit);

#[test]
fn overflowing_a_fixed_field_path_rejects_before_lending_its_previous_selection() {
    let mut bytes = account(&7u64.to_le_bytes(), 12);
    let before = bytes.clone();
    let mut store = SliceStore::new(&mut bytes);
    {
        let mut edit =
            blubber::raw::Edit::load::<__FixedDepthRoot>(&mut store, *b"prepared").unwrap();
        for _ in 0..blubber_core::MAX_DEPTH {
            edit = edit.field(0);
        }
        assert!(matches!(
            edit.view(),
            Err(blubber::Error::Fault {
                kind: blubber::ErrorKind::DepthExceeded,
                ..
            })
        ));
    }
    assert_eq!(store.bytes(), before.as_slice());
    assert!(!store.is_poisoned());
}

#[path = "../../blubber-core/tests/directory.rs"]
mod directory;

#[test]
fn vacant_fixed_map_loans_match_generic_front_middle_and_end_insertions() {
    let mut path = InlinePath::new();
    path.push(Step::Field(0)).unwrap();
    for count in 0..=8u32 {
        let mut root = count.to_le_bytes().to_vec();
        for i in 0..count {
            root.push((2 * i + 1) as u8);
            root.extend_from_slice(&(100 + i as u16).to_le_bytes());
        }
        for index in 0..=count {
            let query = [(2 * index) as u8];
            for spare in [0, 2, 3, 20] {
                let mut fast = account(&root, spare);
                let before = fast.clone();
                let mut generic = fast.clone();
                let loan = blubber_core::raw::prepare_fixed_insert(
                    &mut fast,
                    *b"prepared",
                    0,
                    &ROOT_MAP,
                    &path,
                    2,
                    &query,
                )
                .unwrap();
                if let Some(mut loan) = loan {
                    assert_eq!(loan.required_len(), used(&before) + 3);
                    loan.destination().copy_from_slice(&1234u16.to_le_bytes());
                    loan.finish();
                    let plan = plan_map_edit(
                        &generic,
                        used(&generic),
                        &ROOT_MAP,
                        &path,
                        MapEdit::Insert,
                        &query,
                        2,
                        u32::MAX,
                    )
                    .unwrap();
                    let mut apply = begin_apply(&mut generic, plan).unwrap();
                    apply.move_regions();
                    apply.destination().copy_from_slice(&1234u16.to_le_bytes());
                    apply.finish();
                    assert_eq!(fast, generic, "count={count} index={index} spare={spare}");
                    blubber_core::validate(
                        &fast[12..used(&fast) as usize],
                        &ROOT_MAP,
                        blubber_core::MAX_DEPTH,
                    )
                    .unwrap();
                } else {
                    assert!(spare < 3);
                    assert_eq!(fast, before);
                }
            }
        }
        if count > 0 {
            let mut bytes = account(&root, 32);
            let before = bytes.clone();
            assert!(blubber_core::raw::prepare_fixed_insert(
                &mut bytes,
                *b"prepared",
                0,
                &ROOT_MAP,
                &path,
                2,
                &[1],
            )
            .unwrap()
            .is_none());
            assert_eq!(bytes, before);
        }
    }
}

#[test]
fn checked_directory_suffix_keeps_borrowed_inputs_and_prefix_frames() {
    use blubber_model::{FixedMapEdit, IndexedMap};
    for width in [1, 3, 32] {
        let desc = Layout::Map {
            key: KeyLayout {
                width,
                domain: KeyDomain::Raw,
            },
            value: &TEXT,
        };
        let key = |n| vec![n; width as usize];
        let model = IndexedMap::new(
            width as usize,
            [
                (key(1), b"first".to_vec()),
                (key(3), b"".to_vec()),
                (key(5), b"last".to_vec()),
            ],
        )
        .unwrap();
        for (operation, reference, selected) in [
            (MapEdit::Insert, FixedMapEdit::Insert, 4),
            (MapEdit::Replace, FixedMapEdit::Replace, 3),
            (MapEdit::Remove, FixedMapEdit::Remove, 3),
            (MapEdit::Clear, FixedMapEdit::Clear, 0),
        ] {
            let query = if selected == 0 { vec![] } else { key(selected) };
            let value = if matches!(operation, MapEdit::Insert | MapEdit::Replace) {
                b"replacement".as_slice()
            } else {
                b""
            };
            let mut expected = model.clone();
            expected.apply(reference, &query, value).unwrap();
            let before = account(&model.encode(), 100);
            // Inputs share the same allocation as the account through disjoint loans.
            let mut backing = before.clone();
            backing.extend_from_slice(&query);
            backing.extend_from_slice(value);
            let (bytes, inputs) = backing.split_at_mut(before.len());
            let (query, value) = inputs.split_at(query.len());
            let plan = plan_map_edit(
                bytes,
                used(bytes),
                &desc,
                &InlinePath::new(),
                operation,
                query,
                value.len() as u32,
                bytes.len() as u32,
            )
            .unwrap();
            let mut apply = begin_apply(bytes, plan).unwrap();
            apply.move_regions();
            apply.destination().copy_from_slice(value);
            apply.finish();
            assert_eq!(&bytes[12..used(bytes) as usize], expected.encode());
            let end = (used(&before).max(used(bytes))) as usize;
            assert_eq!(&bytes[end..], &before[end..]);
            assert_eq!(query, if selected == 0 { vec![] } else { key(selected) });
            assert_eq!(
                value,
                if matches!(operation, MapEdit::Insert | MapEdit::Replace) {
                    b"replacement".as_slice()
                } else {
                    b""
                }
            );
        }
    }
}

#[test]
fn tail_removals_match_generic_for_empty_singleton_and_all_positions() {
    for map in [false, true] {
        for count in 0..=8u32 {
            let mut root = count.to_le_bytes().to_vec();
            for i in 0..count {
                if map {
                    root.push((2 * i + 1) as u8);
                }
                root.extend_from_slice(&(i as u16 + 10).to_le_bytes());
            }
            let layout = if map { &ROOT_MAP } else { &ROOT_NUMBERS };
            let mut path = InlinePath::new();
            path.push(Step::Field(0)).unwrap();
            for index in 0..=count as usize + 1 {
                for spare in [0, 1, 31] {
                    let mut fast = account(&root, spare);
                    let before = fast.clone();
                    let mut generic = fast.clone();
                    let key = [(2 * index + 1) as u8];
                    let key = map.then_some(key.as_slice());
                    let plan = match key {
                        Some(key) => plan_map_edit(
                            &generic,
                            used(&generic),
                            layout,
                            &path,
                            MapEdit::Remove,
                            key,
                            0,
                            u32::MAX,
                        ),
                        None => plan_list_edit(
                            &generic,
                            used(&generic),
                            layout,
                            &path,
                            ListEdit::Remove(index as u32),
                            0,
                            u32::MAX,
                        ),
                    };
                    let loan = blubber_core::raw::prepare_tail_remove(
                        &mut fast,
                        *b"prepared",
                        4,
                        layout,
                        &path,
                        index,
                        key,
                    );
                    match plan {
                        Err(error) => {
                            assert_eq!(loan.err(), Some(error));
                            assert_eq!(fast, before);
                        }
                        Ok(plan) => {
                            let loan = loan.unwrap().expect("direct tail");
                            assert_eq!(loan.required_len(), used(&before));
                            assert_eq!(loan.removed_span(), plan.removed_span());
                            assert_eq!(loan.inspect(), before);
                            loan.finish();
                            let mut apply = begin_apply(&mut generic, plan).unwrap();
                            apply.move_regions();
                            assert!(apply.destination().is_empty());
                            apply.finish();
                            assert_eq!(fast, generic, "map={map} count={count} index={index}");
                            assert_eq!(
                                &fast[used(&fast) as usize..used(&before) as usize],
                                vec![0; used(&before) as usize - used(&fast) as usize]
                            );
                            assert_eq!(
                                &fast[used(&before) as usize..],
                                &before[used(&before) as usize..]
                            );
                        }
                    }
                }
            }
        }
    }
}

#[blubber::schema(account)]
struct TailBools {
    value: blubber::List<bool>,
}

#[blubber::schema(account)]
struct TailPair {
    left: List<u16>,
    right: List<u16>,
}

#[test]
fn tail_candidate_reborrows_match_generic_removal_bytes() {
    for map in [false, true] {
        let mut bytes = vec![0; 64];
        {
            let mut store = SliceStore::new(&mut bytes);
            if map {
                MapRoot::initialize(
                    &mut store,
                    MapRootInit {
                        value: &[(1, 7), (3, 9), (5, 11)],
                    },
                )
                .unwrap();
            } else {
                ListRoot::initialize(&mut store, ListRootInit { value: &[7, 9, 11] }).unwrap();
            }
        }
        let old_used = used(&bytes) as usize;
        bytes[old_used..].fill(0xa5);
        let mut expected = bytes.clone();
        let mut path = InlinePath::new();
        path.push(Step::Field(0)).unwrap();
        for position in [0, 1] {
            let key = [if position == 0 { 3 } else { 1 }];
            let plan = if map {
                plan_map_edit(
                    &expected,
                    used(&expected),
                    &ROOT_MAP,
                    &path,
                    MapEdit::Remove,
                    &key,
                    0,
                    u32::MAX,
                )
            } else {
                plan_list_edit(
                    &expected,
                    used(&expected),
                    &ROOT_NUMBERS,
                    &path,
                    ListEdit::Remove(position),
                    0,
                    u32::MAX,
                )
            }
            .unwrap();
            let mut apply = begin_apply(&mut expected, plan).unwrap();
            apply.move_regions();
            assert!(apply.destination().is_empty());
            apply.finish();
        }
        {
            let mut store = SliceStore::new(&mut bytes);
            if map {
                let mut root = MapRoot::load_trusted_mut(&mut store).unwrap();
                let mut values = root.value_mut();
                assert_eq!(values.reborrow().remove(&3).unwrap(), Some(9));
                assert_eq!(values.try_len().unwrap(), 2);
                assert_eq!(values.reborrow().remove(&1).unwrap(), Some(7));
                root.commit().unwrap();
            } else {
                let mut root = ListRoot::load_trusted_mut(&mut store).unwrap();
                let mut values = root.value_mut();
                assert_eq!(values.reborrow().remove(0).unwrap(), 7);
                assert_eq!(values.try_len().unwrap(), 2);
                assert_eq!(values.reborrow().remove(1).unwrap(), 11);
                root.commit().unwrap();
            }
        }
        assert_eq!(bytes, expected);
    }
    let mut bytes = vec![0; 64];
    {
        let mut store = SliceStore::new(&mut bytes);
        TailPair::initialize(
            &mut store,
            TailPairInit {
                left: &[7, 9],
                right: &[11],
            },
        )
        .unwrap();
    }
    let old_used = used(&bytes) as usize;
    bytes[old_used..].fill(0xa5);
    let mut expected = bytes.clone();
    let mut path = InlinePath::new();
    path.push(Step::Field(0)).unwrap();
    let plan = plan_list_edit(
        &expected,
        used(&expected),
        &ROOT_PAIR,
        &path,
        ListEdit::Remove(0),
        0,
        u32::MAX,
    )
    .unwrap();
    let mut apply = begin_apply(&mut expected, plan).unwrap();
    apply.move_regions();
    assert!(apply.destination().is_empty());
    apply.finish();
    {
        let mut store = SliceStore::new(&mut bytes);
        let mut root = TailPair::load_trusted_mut(&mut store).unwrap();
        assert_eq!(root.left_mut().reborrow().remove(0).unwrap(), 7);
        assert_eq!(root.right().try_get(0).unwrap(), Some(11));
        root.commit().unwrap();
    }
    assert_eq!(bytes, expected);
}
#[blubber::schema(account)]
struct TailBoolMap {
    value: blubber::Map<u8, bool>,
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn tail_removed_decode_failure_and_poison_preserve_every_byte() {
    use blubber::raw::Resize as _;
    use blubber::{ReadStorage as _, Resize as _};
    for map in [false, true] {
        let mut bytes = [0; 40];
        let mut store = blubber::SliceStore::new(&mut bytes);
        if map {
            TailBoolMap::initialize(
                &mut store,
                TailBoolMapInit {
                    value: &[(1, true)],
                },
            )
            .unwrap();
            store.bytes_mut()[17] = 2;
        } else {
            TailBools::initialize(&mut store, TailBoolsInit { value: &[true] }).unwrap();
            store.bytes_mut()[16] = 2;
        }
        let before = store.bytes().to_vec();
        let result = if map {
            TailBoolMap::load_trusted_mut(&mut store)
                .unwrap()
                .value_mut()
                .remove(&1)
                .map(|_| ())
        } else {
            TailBools::load_trusted_mut(&mut store)
                .unwrap()
                .value_mut()
                .remove(0)
                .map(|_| ())
        };
        assert_eq!(
            result.unwrap_err(),
            blubber::Error::new(blubber::ErrorKind::InvalidBool, if map { 17 } else { 16 })
        );
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
        let error = if map {
            TailBoolMap::load_trusted_mut(&mut store).err()
        } else {
            TailBools::load_trusted_mut(&mut store).err()
        };
        assert_eq!(error.unwrap(), blubber::ErrorKind::Poisoned.into());
        assert_eq!(store.bytes(), before);
    }
}

#[test]
fn tail_removal_faults_match_the_existing_local_planner() {
    use blubber_core::raw::{prepare_tail_remove, resolve_account_edit};
    for map in [false, true] {
        let root = if map {
            vec![2, 0, 0, 0, 1, 7, 0, 3, 9, 0]
        } else {
            vec![2, 0, 0, 0, 7, 0, 9, 0]
        };
        let valid = account(&root, 3);
        let mut cases = vec![valid.clone()];
        let mut identity = valid.clone();
        identity[0] = 0;
        cases.push(identity);
        for value in [0u32, 15, u32::MAX] {
            let mut bytes = valid.clone();
            bytes[8..12].copy_from_slice(&value.to_le_bytes());
            cases.push(bytes);
        }
        for value in [0u32, 1, 3, u32::MAX] {
            let mut bytes = valid.clone();
            bytes[12..16].copy_from_slice(&value.to_le_bytes());
            cases.push(bytes);
        }
        for len in [0, 7, 11, 12, 15, 16, 17, valid.len() - 4] {
            cases.push(valid[..len].to_vec());
        }
        if map {
            for value in [0, 1, 255] {
                let mut bytes = valid.clone();
                bytes[19] = value;
                cases.push(bytes);
            }
        }
        let layout = if map { &ROOT_MAP } else { &ROOT_NUMBERS };
        let mut path = InlinePath::new();
        path.push(Step::Field(0)).unwrap();
        for before in cases {
            for query in [
                &[][..],
                &[0][..],
                &[1][..],
                &[3][..],
                &[255][..],
                &[1, 2][..],
            ] {
                let key = map.then_some(query);
                let mut fast = before.clone();
                let mut generic = before.clone();
                let expected = resolve_account_edit(&mut generic, *b"prepared", 4, layout, &path)
                    .and_then(|resolved| match key {
                        Some(key) => resolved.prepare_map(MapEdit::Remove, key, 0, u32::MAX),
                        None => resolved.prepare_list(ListEdit::Remove(0), 0, u32::MAX),
                    });
                let expected = expected.map(|prepared| {
                    let removed = prepared.inspect_removed();
                    let mut apply = prepared.begin_prepared().unwrap();
                    apply.move_regions();
                    assert!(apply.destination().is_empty());
                    apply.finish();
                    removed
                });
                let actual = prepare_tail_remove(&mut fast, *b"prepared", 4, layout, &path, 0, key)
                    .map(|loan| {
                        let loan = loan.expect("supported fixed tail");
                        let removed = loan.removed_span();
                        loan.finish();
                        removed
                    });
                assert_eq!(
                    actual, expected,
                    "map={map} query={query:?} bytes={before:?}"
                );
                assert_eq!(fast, generic);
                if actual.is_err() {
                    assert_eq!(fast, before);
                }
            }
        }
    }
}

#[blubber::schema(account)]
struct FixedRemoval {
    values: blubber::Map<u64, u64>,
}

#[test]
fn fixed_u64_removal_crosses_buffer_boundaries_with_an_exact_spare_frame() {
    use blubber::{ReadStorage as _, Resize as _};
    let counts: &[u32] = if cfg!(miri) {
        &[2]
    } else {
        &[192, 193, 194, 384, 385, 1000]
    };
    let layout = Layout::Flat {
        checked: <FixedRemoval as blubber::RootSchema>::__layout(),
    };
    let mut path = InlinePath::new();
    path.push(Step::Field(0)).unwrap();
    for &count in counts {
        let mut payload = count.to_le_bytes().to_vec();
        for index in 0..count as u64 {
            payload.extend_from_slice(&(index * 2 + 2).to_be_bytes());
            payload.extend_from_slice(&(index * 73 + 19).to_le_bytes());
        }
        let mut before = account(&payload, 31);
        before[..8].copy_from_slice(&<FixedRemoval as blubber::AccountSchema>::DISCRIMINATOR);
        for index in [0, count / 2, count - 1] {
            let key = index as u64 * 2 + 2;
            let query = key.to_be_bytes();
            let mut expected = before.clone();
            let plan = plan_map_edit(
                &expected,
                used(&expected),
                &layout,
                &path,
                MapEdit::Remove,
                &query,
                0,
                before.len() as u32,
            )
            .unwrap();
            let mut apply = begin_apply(&mut expected, plan).unwrap();
            apply.move_regions();
            assert!(apply.destination().is_empty());
            apply.finish();
            let mut actual = before.clone();
            let mut store = blubber::SliceStore::new(&mut actual);
            {
                let mut root = FixedRemoval::load_trusted_mut(&mut store).unwrap();
                assert_eq!(
                    root.values_mut().remove(&key),
                    Ok(Some(index as u64 * 73 + 19))
                );
            }
            assert!(!store.is_poisoned());
            assert_eq!(store.bytes(), expected);
        }
    }
}

#[test]
fn fixed_u64_removal_matches_generic_at_every_position_and_preserves_strict_reads() {
    use blubber::{ReadStorage as _, Resize as _};
    let layout = Layout::Flat {
        checked: <FixedRemoval as blubber::RootSchema>::__layout(),
    };
    let mut path = InlinePath::new();
    path.push(Step::Field(0)).unwrap();
    for count in 0..=8 {
        let values: Vec<_> = (0..count).map(|i| (2 * i + 2, i + 10)).collect();
        let mut original = [0; 176];
        FixedRemoval::initialize(
            &mut blubber::SliceStore::new(&mut original),
            FixedRemovalInit { values: &values },
        )
        .unwrap();
        let live = used(&original) as usize;
        original[live..].fill(0xa5);
        for key in (0..=2 * count + 3).chain([u64::MAX]) {
            let mut expected = original;
            let query = key.to_be_bytes();
            let plan = plan_map_edit(
                &expected,
                used(&expected),
                &layout,
                &path,
                MapEdit::Remove,
                &query,
                0,
                176,
            )
            .unwrap();
            let removed = plan.removed_span().map(|span| {
                u64::from_le_bytes(
                    expected[span.start as usize..span.end as usize]
                        .try_into()
                        .unwrap(),
                )
            });
            let mut apply = begin_apply(&mut expected, plan).unwrap();
            apply.move_regions();
            assert!(apply.destination().is_empty());
            apply.finish();
            for checked in [false, true] {
                let mut actual = original;
                let mut store = blubber::SliceStore::new(&mut actual);
                {
                    let mut root = if checked {
                        FixedRemoval::load_checked_mut(&mut store)
                    } else {
                        FixedRemoval::load_trusted_mut(&mut store)
                    }
                    .unwrap();
                    assert_eq!(root.values_mut().remove(&key).unwrap(), removed);
                    root.validate().unwrap();
                }
                assert!(!store.is_poisoned());
                assert_eq!(store.bytes(), expected);
            }
        }
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn lazy_fixed_u64_removal_matches_generic_on_malformed_counts_and_key_order() {
    use blubber::{raw::Resize as _, ReadStorage as _, Resize as _};
    use blubber_core::raw::resolve_account_edit;
    let layout = Layout::Flat {
        checked: <FixedRemoval as blubber::RootSchema>::__layout(),
    };
    let mut path = InlinePath::new();
    path.push(Step::Field(0)).unwrap();
    let mut original = [0; 96];
    FixedRemoval::initialize(
        &mut blubber::SliceStore::new(&mut original),
        FixedRemovalInit {
            values: &[(2, 11), (4, 22), (6, 33)],
        },
    )
    .unwrap();
    let discriminator = original[..8].try_into().unwrap();
    let mut cases = Vec::new();
    for count in [0u32, 1, 2, 4, u32::MAX] {
        let mut bytes = original;
        bytes[12..16].copy_from_slice(&count.to_le_bytes());
        cases.push(bytes);
    }
    for keys in [[6u64, 4, 2], [2, 2, 6], [2, 6, 4]] {
        let mut bytes = original;
        for (index, key) in keys.iter().enumerate() {
            bytes[16 + index * 16..24 + index * 16].copy_from_slice(&key.to_be_bytes());
        }
        cases.push(bytes);
    }
    for before in cases {
        let mut checked = before;
        let mut store = blubber::SliceStore::new(&mut checked);
        assert!(FixedRemoval::load_checked_mut(&mut store).is_err());
        assert!(!store.is_poisoned());
        assert_eq!(store.bytes(), before);
        for key in [0u64, 1, 2, 3, 4, 5, 6, 7, u64::MAX] {
            let mut expected = before;
            let query = key.to_be_bytes();
            let result = resolve_account_edit(&mut expected, discriminator, 4, &layout, &path)
                .and_then(|resolved| resolved.prepare_map(MapEdit::Remove, &query, 0, 96))
                .map(|prepared| {
                    let removed = prepared.inspect_removed().map(|span| {
                        u64::from_le_bytes(
                            prepared.inspect_prepared()[span.start as usize..span.end as usize]
                                .try_into()
                                .unwrap(),
                        )
                    });
                    let mut apply = prepared.begin_prepared().unwrap();
                    apply.move_regions();
                    assert!(apply.destination().is_empty());
                    apply.finish();
                    removed
                });
            let mut actual = before;
            let mut store = blubber::SliceStore::new(&mut actual);
            {
                let mut root = FixedRemoval::load_trusted_mut(&mut store).unwrap();
                assert_eq!(root.values_mut().remove(&key), result);
            }
            assert_eq!(store.is_poisoned(), result.is_err());
            assert_eq!(store.bytes(), expected);
            store.set_poisoned(true);
            assert!(
                matches!(FixedRemoval::load_trusted_mut(&mut store), Err(error)
                if error == blubber::ErrorKind::Poisoned.into())
            );
            assert_eq!(store.bytes(), expected);
        }
    }
}

#[test]
fn local_word_probes_keep_long_prefixes_and_scalar_widths() {
    use blubber_core::{KeyDomain, KeyLayout, Layout, Leaf, Search};
    static VALUE: Layout = Layout::Leaf(Leaf::Unsigned(1));
    for (indexed, value) in [(false, &VALUE), (true, &TEXT)] {
        for width in [1, 31, 32, 33] {
            let layout = Layout::Map {
                key: KeyLayout {
                    width,
                    domain: KeyDomain::Raw,
                },
                value,
            };
            for at in 0..width as usize {
                let mut bytes = vec![3, 0, 0, 0];
                for byte in [1, 3, 5] {
                    let mut key = vec![0x97; width as usize];
                    key[at] = byte;
                    key[at + 1..].fill(255 - byte);
                    bytes.extend_from_slice(&key);
                    if indexed {
                        bytes.extend_from_slice(&0u32.to_le_bytes());
                    } else {
                        bytes.push(0xff);
                    }
                }
                for byte in 0..=6 {
                    let mut query = vec![0x97; width as usize];
                    query[at] = byte;
                    query[at + 1..].fill(255 - byte);
                    let index = [1, 3, 5]
                        .into_iter()
                        .take_while(|stored| *stored < byte)
                        .count();
                    assert_eq!(
                        blubber_core::search_map_local(&bytes, &layout, &query),
                        Ok(Search {
                            index: index as u32,
                            found: [1, 3, 5].contains(&byte),
                        })
                    );
                }
            }
        }
    }
}
