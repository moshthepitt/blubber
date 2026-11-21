//! Concrete CTFE construction and exact checker rejection controls.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::{Address, List, Map, RootSchema, Schema, Str};

#[blubber::schema]
struct Packed {
    a: bool,
    b: u16,
}
#[blubber::schema]
struct Mixed {
    name: Str,
    id: u64,
    bytes: List<u8>,
    packed: Packed,
}
#[blubber::schema]
struct Root {
    records: List<Mixed>,
    map: Map<Address, Option<Str>>,
}

#[test]
fn concrete_tables_have_packed_ordinals_and_rebased_children() {
    let table = Root::__layout();
    assert_eq!(table.words().len(), Root::__LEN);
    assert_eq!(table.root(), Root::__LEN - 8);
    assert!(blubber_core::certify(table.words(), table.root(), blubber_core::MAX_DEPTH).is_some());
    let mixed = Mixed::__layout();
    let root = mixed.root();
    let fields = mixed.words()[root + 5] as usize;
    assert_eq!(
        &mixed.words()[root..root + 8],
        &[9, 11, 3, 2, 2, fields as u32, 0, 0]
    );
    assert_eq!(&mixed.words()[fields..fields + 3], &[10, 1, 0]);
    assert_eq!(&mixed.words()[fields + 8..fields + 11], &[10, 3, 8]);
    assert_eq!(&mixed.words()[fields + 16..fields + 19], &[10, 0, 0]);
    assert_eq!(&mixed.words()[fields + 24..fields + 27], &[10, 2, 0]);
    assert_eq!(
        (0..4)
            .map(|i| mixed.words()[fields + 8 * i + 4])
            .collect::<Vec<_>>(),
        [2, 0, 3, 1]
    );
    for field in 0..4 {
        assert!((mixed.words()[fields + field * 8 + 3] as usize) < fields);
    }
}

#[test]
fn hidden_concrete_context_supports_generic_collections_without_standalone_tables() {
    const TABLE: blubber_core::CheckedLayout<'static> =
        blubber::__layout_of!(List<Map<Address, Option<Str>>>);
    assert_eq!(TABLE.words()[TABLE.root()], 7);
    assert_eq!(TABLE.words()[TABLE.root() + 2], 4);
}

#[test]
fn malformed_records_indices_ordinals_offsets_and_depths_are_rejected() {
    let table = Mixed::__layout();
    for (index, value) in [
        (0, 0),
        (1, 1),
        (table.root() + 2, 0),
        (table.root() + 2, 8),
        (table.root() + 3, 65536),
        (table.root() + 5, 1),
        (table.root() + 6, 1),
    ] {
        let mut words = table.words().to_vec();
        words[index] = value;
        assert!(
            blubber_core::certify(&words, table.root(), 8).is_none(),
            "word {index}"
        );
    }
    let fields = table.words()[table.root() + 5] as usize;
    for (delta, value) in [
        (1, 0),
        (2, 1),
        (3, fields as u32),
        (4, 1),
        (8 + 1, 1),
        (16 + 2, 1),
    ] {
        let mut words = table.words().to_vec();
        words[fields + delta] = value;
        assert!(
            blubber_core::certify(&words, table.root(), 8).is_none(),
            "field delta {delta}"
        );
    }
    assert!(blubber_core::certify(table.words(), table.root() + 1, 8).is_none());
    assert!(blubber_core::certify(table.words(), table.root(), 0).is_none());
    assert!(blubber_core::certify(table.words(), table.root(), usize::MAX).is_none());
    assert!(blubber_core::certify(&[], 0, 8).is_none());
}

#[blubber::schema]
struct Generic<T> {
    value: T,
    label: Str,
}
#[blubber::schema]
struct GenericArray<const N: usize> {
    bytes: [u8; N],
}
#[blubber::schema(account)]
struct GenericRoot {
    rows: List<Generic<u64>>,
    nested: Generic<List<Str>>,
    array: GenericArray<5>,
}

#[test]
fn generic_structs_are_nested_in_concrete_tables_and_reuse_parent_views() {
    let rows = [GenericInit {
        value: 42,
        label: "row",
    }];
    let init = GenericRootInit {
        rows: &rows,
        nested: GenericInit {
            value: &["a", "bc"],
            label: "nested",
        },
        array: GenericArrayInit {
            bytes: [1, 2, 3, 4, 5],
        },
    };
    let mut bytes = [0; 512];
    let mut store = blubber::SliceStore::new(&mut bytes);
    let mut root = GenericRoot::initialize(&mut store, init).unwrap();
    assert_eq!(root.rows().get(0).unwrap().value(), 42);
    assert_eq!(root.nested().value().get(1), Some("bc"));
    root.rows_mut()
        .get_mut(0)
        .unwrap()
        .unwrap()
        .value_mut()
        .set(99)
        .unwrap();
    root.nested_mut().value_mut().push("growing child").unwrap();
    assert_eq!(root.rows().get(0).unwrap().value(), 99);
    assert_eq!(root.nested().value().get(2), Some("growing child"));
    assert_eq!(root.array().bytes(), [1, 2, 3, 4, 5]);
    root.validate().unwrap();
}

#[blubber::schema]
struct Many<T> {
    field0: T,
    field1: T,
    field2: T,
    field3: T,
    field4: T,
    field5: T,
    field6: T,
    field7: T,
    field8: T,
    field9: T,
    field10: T,
    field11: T,
    field12: T,
    field13: T,
    field14: T,
    field15: T,
    field16: T,
    field17: T,
    field18: T,
    field19: T,
    field20: T,
    field21: T,
    field22: T,
    field23: T,
    field24: T,
    field25: T,
    field26: T,
    field27: T,
    field28: T,
    field29: T,
    field30: T,
    field31: T,
    field32: T,
    field33: T,
    field34: T,
    field35: T,
    field36: T,
    field37: T,
    field38: T,
    field39: T,
    field40: T,
    field41: T,
    field42: T,
    field43: T,
    field44: T,
    field45: T,
    field46: T,
    field47: T,
    field48: T,
    field49: T,
    field50: T,
    field51: T,
    field52: T,
    field53: T,
    field54: T,
    field55: T,
    field56: T,
    field57: T,
    field58: T,
    field59: T,
    field60: T,
    field61: T,
    field62: T,
    field63: T,
    field64: T,
}
#[test]
fn materialization_has_no_scratch_child_or_page_size_cap() {
    const TABLE: blubber_core::CheckedLayout<'static> = blubber::__layout_of!(Many<u64>);
    assert!(TABLE.words().len() * 4 > 4096);
    assert_eq!(TABLE.words()[TABLE.root() + 3], 65);
    assert_eq!(TABLE.words()[TABLE.root() + 1], 65 * 8);
}

#[test]
fn public_child_selection_checks_ordinals_before_creating_a_witness() {
    let table = Mixed::__layout();
    for (index, variable) in [
        (2, false),
        (2, true),
        (usize::MAX, false),
        (usize::MAX, true),
    ] {
        assert!(std::panic::catch_unwind(|| table.field(index, variable)).is_err());
        assert!(std::panic::catch_unwind(|| table.child(index, variable)).is_err());
    }
}

#[blubber::schema]
struct BitPatterns {
    number: u64,
    key: Address,
    bytes: [u8; 7],
}
#[blubber::schema]
struct NestedBits {
    value: BitPatterns,
    integer: i16,
}

#[test]
fn unrestricted_fixed_certificates_compose_and_reject_bool_forgeries() {
    assert!(BitPatterns::__layout().is_unrestricted());
    assert!(NestedBits::__layout().is_unrestricted());
    assert!(!Packed::__layout().is_unrestricted());
    assert!(!Mixed::__layout().is_unrestricted());
    let mut forged = Packed::__layout().words().to_vec();
    forged[Packed::__layout().root() + 6] = 1;
    assert!(
        blubber_core::certify(&forged, Packed::__layout().root(), blubber_core::MAX_DEPTH)
            .is_none()
    );
    let mut forged = Mixed::__layout().words().to_vec();
    forged[Mixed::__layout().root() + 6] = 1;
    assert!(
        blubber_core::certify(&forged, Mixed::__layout().root(), blubber_core::MAX_DEPTH).is_none()
    );
    for seed in [0, 1, 127, 128, 255] {
        let layout = blubber_core::Layout::Flat {
            checked: blubber::__layout_of!(List<NestedBits>),
        };
        let mut bytes = 3u32.to_le_bytes().to_vec();
        bytes.resize(4 + 3 * 49, seed);
        assert!(blubber_core::validate(&bytes, &layout, blubber_core::MAX_DEPTH).is_ok());
        assert!(blubber_core::validate(&bytes, &layout, 2).is_err());
        bytes.pop();
        assert!(blubber_core::validate(&bytes, &layout, blubber_core::MAX_DEPTH).is_err());
    }
}

#[test]
fn string_payload_ascii_blocks_preserve_every_boundary_and_fault() {
    let layout = blubber_core::Layout::Flat {
        checked: blubber::__layout_of!(List<Str>),
    };
    let mut bytes = Vec::from(3u32.to_le_bytes());
    bytes.extend_from_slice(&31u32.to_le_bytes());
    bytes.extend_from_slice(&64u32.to_le_bytes());
    bytes.resize(12 + 96, b'a');
    blubber_core::validate(&bytes, &layout, blubber_core::MAX_DEPTH).unwrap();
    for offset in 0..96 {
        bytes[12 + offset] = 0xff;
        let error = blubber_core::validate(&bytes, &layout, blubber_core::MAX_DEPTH).unwrap_err();
        assert_eq!(
            error,
            blubber::Error::new(blubber::ErrorKind::InvalidUtf8, 12 + offset as u32)
        );
        bytes[12 + offset] = b'a';
    }
    // Each child must be valid independently, even when their concatenation is UTF-8.
    bytes[12 + 30] = 0xc3;
    bytes[12 + 31] = 0xa9;
    assert!(core::str::from_utf8(&bytes[12..]).is_ok());
    assert!(blubber_core::validate(&bytes, &layout, blubber_core::MAX_DEPTH).is_err());
    bytes[12 + 30] = b'a';
    bytes[12 + 31] = b'a';
    bytes[12 + 32] = 0xc3;
    bytes[12 + 33] = 0xa9;
    blubber_core::validate(&bytes, &layout, blubber_core::MAX_DEPTH).unwrap();
}

#[blubber::schema]
struct SoleTail {
    prefix: u16,
    text: Str,
    flag: bool,
}

#[test]
fn sole_tail_flat_selection_matches_facade_bytes_bases_and_faults() {
    use blubber::View;
    use blubber_core::Layout;
    let flat = Layout::Flat {
        checked: SoleTail::__layout(),
    };
    let bytes = [7, 0, 9, b'a', 0xff, b'z'];
    for len in 0..=bytes.len() {
        for base in [0, 99, u32::MAX - len as u32] {
            let a = View::new(&bytes[..len], flat, base).unwrap();
            let b = View::new(&bytes[..len], SoleTail::LAYOUT, base).unwrap();
            for field in 0..=3 {
                let a = a.field(field).map(|v| (v.bytes(), v.base()));
                let b = b.field(field).map(|v| (v.bytes(), v.base()));
                assert_eq!(a, b, "len={len} base={base} field={field}");
                let selected = View::new(&bytes[..len], flat, base)
                    .unwrap()
                    .field_tail::<SoleTail>(field)
                    .map(|v| (v.bytes(), v.base()));
                assert_eq!(selected, b);
            }
        }
    }
}

#[blubber::schema]
struct ForeignTail {
    prefix: u32,
    text: Str,
    flag: bool,
}

#[test]
fn sole_tail_identity_mismatch_uses_the_actual_descriptor() {
    use blubber::View;
    use blubber_core::Layout;
    let bytes = [7, 0, 0, 0, 0, b'a', b'b', b'c', b'd'];
    for layout in [
        *ForeignTail::LAYOUT,
        Layout::Flat {
            checked: ForeignTail::__layout(),
        },
    ] {
        let view = View::new(&bytes, layout, 80).unwrap();
        for field in 0..=3 {
            assert_eq!(
                view.field_tail::<SoleTail>(field)
                    .map(|v| (v.bytes(), v.base())),
                view.field(field).map(|v| (v.bytes(), v.base()))
            );
        }
        assert_eq!(view.read::<SoleTail>().unwrap().text(), "abcd");
    }
}

#[test]
fn sole_tail_budget_mismatch_retains_the_inherited_certificate() {
    use blubber::View;
    use blubber_core::Layout;
    let parent = SoleTail::__layout();
    let inherited = blubber_core::certify(parent.words(), parent.root(), 2).unwrap();
    let view = View::new(&[7, 0, 0, b'a'], Layout::Flat { checked: inherited }, 90).unwrap();
    let selected = view.field_tail::<SoleTail>(1).unwrap();
    let Layout::Flat { checked } = selected.layout() else {
        panic!("inherited flat descriptor")
    };
    assert_eq!(checked.budget(), 2);
    assert_eq!((selected.bytes(), selected.base()), (&b"a"[..], 93));
}
