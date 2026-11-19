//! Corrupted bytes must produce checked faults, never unchecked references.

#![forbid(unsafe_code)]

mod support;

blubber::__root_schema!(__ConcreteRoot0, List<u16>);

use blubber::{Error, ErrorKind, List, Map, Schema, Str, View};
use blubber_core::{FixedField, Layout, Leaf, VariableField};
use support::{account, encoded, read, Record, DISCRIMINATOR};

#[track_caller]
fn fault<T>(result: blubber::Result<T>, kind: ErrorKind, offset: u32) {
    assert_eq!(result.err(), Some(Error::new(kind, offset)));
}

#[test]
fn envelope_faults_have_stable_precedence_and_offsets() {
    let valid = account::<Record>(&("ok", true, &[]));
    for len in 0..12 {
        fault(
            View::load::<Record>(&valid[..len], DISCRIMINATOR),
            ErrorKind::Truncated,
            0,
        );
    }
    let mut bytes = valid.clone();
    bytes[0] ^= 1;
    fault(
        View::load::<Record>(&bytes, DISCRIMINATOR),
        ErrorKind::Discriminator,
        0,
    );
    fault(
        View::load::<Record>(&bytes, [0; 8]),
        ErrorKind::InvalidSchema,
        Error::NO_OFFSET,
    );
    for used in [0, 11, 20, valid.len() as u32 + 1, u32::MAX] {
        let mut bytes = valid.clone();
        bytes[8..12].copy_from_slice(&used.to_le_bytes());
        fault(
            View::load::<Record>(&bytes, DISCRIMINATOR),
            ErrorKind::UsedLength,
            8,
        );
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn trusted_load_is_lazy_and_default_load_checks_the_root() {
    let mut bytes = account::<Record>(&("hello", true, &[7]));
    bytes[12] = 7;
    let root = View::load_trusted::<Record>(&bytes, DISCRIMINATOR).unwrap();
    assert_eq!(root.read::<Record>().unwrap().name(), "hello");
    fault(
        root.read::<Record>().unwrap().enabled(),
        ErrorKind::InvalidBool,
        12,
    );
    fault(root.validate(), ErrorKind::InvalidBool, 12);
    fault(
        View::load::<Record>(&bytes, DISCRIMINATOR),
        ErrorKind::InvalidBool,
        12,
    );
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn root_selection_cannot_read_spare_capacity_as_payload() {
    let mut bytes = account::<List<u16>>(&&[1, 2][..]);
    bytes[8..12].copy_from_slice(&18u32.to_le_bytes());
    let root = View::load_trusted::<__ConcreteRoot0>(&bytes, DISCRIMINATOR).unwrap();
    fault(root.read::<List<u16>>(), ErrorKind::Truncated, 16);
}

#[test]
fn scalar_and_nested_fixed_faults_rebase_exactly_once() {
    let view = View::new(&[2], bool::LAYOUT, 47).unwrap();
    fault(view.read::<bool>(), ErrorKind::InvalidBool, 47);
    fault(bool::read_removed(view), ErrorKind::InvalidBool, 47);
    fault(
        View::new(&[1], u32::LAYOUT, 23).unwrap().read::<u32>(),
        ErrorKind::Truncated,
        24,
    );
    fault(
        View::new(&[1, 0, 9], u16::LAYOUT, 23)
            .unwrap()
            .read::<u16>(),
        ErrorKind::InvalidBoundary,
        25,
    );
    let bytes = encoded::<List<Option<bool>>>(&&[Some(true), Some(false)][..]);
    let mut bytes = bytes;
    *bytes.last_mut().unwrap() = 2;
    let list = View::new(&bytes, <List<Option<bool>>>::LAYOUT, 100)
        .unwrap()
        .read::<List<Option<bool>>>()
        .unwrap();
    fault(list.try_get(1), ErrorKind::InvalidBool, 111);
    fault(list.validate(), ErrorKind::InvalidBool, 111);
}

#[test]
fn utf8_errors_never_turn_into_string_references() {
    let cases: &[&[u8]] = &[
        &[0x80],
        &[0xc0, 0x80],
        &[0xc2],
        &[0xe0, 0x80, 0x80],
        &[0xed, 0xa0, 0x80],
        &[0xf4, 0x90, 0x80, 0x80],
        &[0xf5, 0x80, 0x80, 0x80],
    ];
    for &invalid in cases {
        let mut bytes = b"valid".to_vec();
        bytes.extend_from_slice(invalid);
        let view = View::new(&bytes, Str::LAYOUT, 19).unwrap();
        fault(view.read::<Str>(), ErrorKind::InvalidUtf8, 24);
        fault(view.validate(), ErrorKind::InvalidUtf8, 24);
        fault(Str::read_removed(view), ErrorKind::InvalidUtf8, 24);
    }
    // Exercise every one-byte suffix and incomplete/malformed two-byte suffix
    // after a valid multibyte prefix against the standard library's location.
    for a in 0..=255u8 {
        for b in 0..=255u8 {
            let bytes = [0xc3, 0xa9, a, b];
            let result = View::new(&bytes, Str::LAYOUT, 200).unwrap().read::<Str>();
            match core::str::from_utf8(&bytes) {
                Ok(expected) => assert_eq!(result, Ok(expected)),
                Err(error) => fault(
                    result,
                    ErrorKind::InvalidUtf8,
                    200 + error.valid_up_to() as u32,
                ),
            }
        }
    }
}

#[test]
fn options_reject_invalid_tags_extents_and_children() {
    fault(read::<Option<Str>>(&[]), ErrorKind::Truncated, 0);
    fault(read::<Option<Str>>(&[2]), ErrorKind::InvalidTag, 0);
    fault(read::<Option<Str>>(&[0, 0]), ErrorKind::InvalidBoundary, 1);
    fault(read::<Option<u16>>(&[1, 0]), ErrorKind::Truncated, 1);
    fault(
        read::<Option<u16>>(&[1, 0, 0, 0]),
        ErrorKind::InvalidBoundary,
        3,
    );
    fault(read::<Option<bool>>(&[1, 2]), ErrorKind::InvalidBool, 1);
    fault(read::<Option<Str>>(&[1, 0xff]), ErrorKind::InvalidUtf8, 1);
}

#[test]
fn list_header_faults_precede_absence_and_check_overflow() {
    for len in 0..4 {
        fault(read::<List<u16>>(&[0; 4][..len]), ErrorKind::Truncated, 0);
    }
    fault(read::<List<u16>>(&[1, 0, 0, 0, 0]), ErrorKind::Truncated, 4);
    fault(
        read::<List<u16>>(&[0, 0, 0, 0, 0]),
        ErrorKind::InvalidBoundary,
        4,
    );
    fault(
        read::<List<Str>>(&[0, 0, 0, 0, 0]),
        ErrorKind::InvalidBoundary,
        4,
    );
    // Header size overflow names the count word.
    fault(
        read::<List<u128>>(&u32::MAX.to_le_bytes()),
        ErrorKind::ArithmeticOverflow,
        0,
    );
    fault(
        read::<List<Str>>(&u32::MAX.to_le_bytes()),
        ErrorKind::ArithmeticOverflow,
        0,
    );
    fault(
        read::<List<Str>>(&[3, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        ErrorKind::Truncated,
        8,
    );
}

#[test]
fn selected_list_ends_are_checked_and_other_ends_stay_lazy() {
    let mut bytes = encoded::<List<Str>>(&&["a", "b", "c", "d"][..]);
    bytes[4..8].copy_from_slice(&99u32.to_le_bytes());
    let list = read::<List<Str>>(&bytes).unwrap();
    assert_eq!(list.len(), 4);
    assert_eq!(list.try_get(3), Ok(Some("d")));
    assert_eq!(list.try_get(usize::MAX), Ok(None));
    fault(list.try_get(0), ErrorKind::InvalidBoundary, 4);
    fault(list.try_get(1), ErrorKind::InvalidBoundary, 4);
    fault(list.validate(), ErrorKind::InvalidBoundary, 4);
    let mut bytes = encoded::<List<Str>>(&&["a", "b", "c"][..]);
    bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
    fault(
        read::<List<Str>>(&bytes).unwrap().try_get(1),
        ErrorKind::InvalidBoundary,
        8,
    );
}

#[test]
fn struct_field_checks_reject_bad_extents_and_declaration_indices() {
    let mut bytes = encoded::<Record>(&("a", true, &[]));
    bytes[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
    let view = View::new(&bytes, Record::LAYOUT, 12).unwrap();
    assert_eq!(view.field(1).unwrap().read::<bool>(), Ok(true));
    fault(view.field(0), ErrorKind::InvalidBoundary, 13);
    fault(view.field(2), ErrorKind::InvalidBoundary, 13);
    fault(
        view.field(77),
        ErrorKind::IndexOutOfBounds,
        Error::NO_OFFSET,
    );
    fault(
        View::new(&[], Record::LAYOUT, 12).unwrap().field(1),
        ErrorKind::Truncated,
        12,
    );
    fault(
        View::new(&[0; 4], Record::LAYOUT, 12).unwrap().field(0),
        ErrorKind::Truncated,
        13,
    );
    fault(
        View::new(&[0], bool::LAYOUT, 0).unwrap().field(0),
        ErrorKind::InvalidSchema,
        Error::NO_OFFSET,
    );
}

#[test]
fn map_headers_reject_count_zero_payload_truncation_and_overflow() {
    for len in 0..4 {
        fault(
            read::<Map<u8, Str>>(&[0; 4][..len]),
            ErrorKind::Truncated,
            0,
        );
    }
    fault(
        read::<Map<u8, Str>>(&[0, 0, 0, 0, 1]),
        ErrorKind::InvalidBoundary,
        4,
    );
    fault(
        read::<Map<u8, u16>>(&[1, 0, 0, 0, 1]),
        ErrorKind::Truncated,
        4,
    );
    fault(
        read::<Map<u8, u16>>(&[0, 0, 0, 0, 1]),
        ErrorKind::InvalidBoundary,
        4,
    );
    fault(
        read::<Map<u8, Str>>(&[2, 0, 0, 0, 1, 0, 0, 0, 0, 2]),
        ErrorKind::Truncated,
        9,
    );
    fault(
        read::<Map<u128, u128>>(&u32::MAX.to_le_bytes()),
        ErrorKind::ArithmeticOverflow,
        0,
    );
    fault(
        read::<Map<u128, Str>>(&u32::MAX.to_le_bytes()),
        ErrorKind::ArithmeticOverflow,
        0,
    );
}

#[test]
fn map_lookup_checks_only_selected_value_boundaries() {
    let mut bytes = encoded::<Map<u8, Str>>(&&[(1, "a"), (2, "b"), (3, "c")][..]);
    bytes[5..9].copy_from_slice(&99u32.to_le_bytes());
    let map = read::<Map<u8, Str>>(&bytes).unwrap();
    assert_eq!(map.try_get(&3), Ok(Some("c")));
    assert_eq!(map.try_get(&4), Ok(None));
    fault(map.try_get(&1), ErrorKind::InvalidBoundary, 5);
    fault(map.try_get(&2), ErrorKind::InvalidBoundary, 5);
    fault(map.validate(), ErrorKind::InvalidBoundary, 5);
    let mut bytes = encoded::<Map<u8, Str>>(&&[(1, "a"), (2, "b")][..]);
    bytes[10..14].copy_from_slice(&1u32.to_le_bytes());
    let map = read::<Map<u8, Str>>(&bytes).unwrap();
    assert_eq!(map.get(&1), Some("a"));
    assert!(map.contains_key(&2));
    fault(map.try_get(&2), ErrorKind::InvalidBoundary, 10);
    fault(map.validate(), ErrorKind::InvalidBoundary, 10);
    bytes[10..14].copy_from_slice(&0u32.to_le_bytes());
    fault(
        read::<Map<u8, Str>>(&bytes).unwrap().try_get(&2),
        ErrorKind::InvalidBoundary,
        10,
    );
}

#[test]
fn probed_bool_keys_and_returned_values_check_their_domains() {
    let mut bytes = encoded::<Map<bool, bool>>(&&[(false, false), (true, true)][..]);
    bytes[6] = 2;
    let map = read::<Map<bool, bool>>(&bytes).unwrap();
    fault(map.try_get(&true), ErrorKind::InvalidBool, 6);
    fault(map.try_contains_key(&true), ErrorKind::InvalidBool, 6);
    fault(map.validate(), ErrorKind::InvalidBool, 6);
    bytes[6] = 1;
    bytes[7] = 2;
    let map = read::<Map<bool, bool>>(&bytes).unwrap();
    assert_eq!(map.try_contains_key(&true), Ok(true));
    fault(map.try_get(&true), ErrorKind::InvalidBool, 7);
}

#[test]
fn full_validation_certifies_key_uniqueness_and_order() {
    let mut bytes = encoded::<Map<u8, Str>>(&&[(1, "a"), (2, "b"), (3, "c")][..]);
    bytes[9] = 1;
    fault(
        read::<Map<u8, Str>>(&bytes).unwrap().validate(),
        ErrorKind::DuplicateKey,
        9,
    );
    bytes[9] = 0;
    let map = read::<Map<u8, Str>>(&bytes).unwrap();
    fault(map.validate(), ErrorKind::UnsortedKeys, 9);
    // Search still terminates on malformed order. Lazy iteration visits physical order.
    assert!(map.try_get(&255).unwrap().is_none());
    assert_eq!(
        map.iter().map(|(key, _)| key).collect::<Vec<_>>(),
        [1, 0, 3]
    );
}

#[test]
fn fallible_iterators_emit_one_error_then_stay_exhausted() {
    let bytes = [3, 0, 0, 0, 1, 2, 0];
    let list = read::<List<bool>>(&bytes).unwrap();
    let mut iter = list.try_iter();
    assert_eq!(iter.next(), Some(Ok(true)));
    let mut clone = iter.clone();
    for iter in [&mut iter, &mut clone] {
        assert_eq!(
            iter.next(),
            Some(Err(Error::new(ErrorKind::InvalidBool, 5)))
        );
        assert_eq!(iter.size_hint(), (0, Some(0)));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }
    let mut bytes = encoded::<Map<u8, Str>>(&&[(1, "ok"), (2, "x"), (3, "y")][..]);
    bytes[21] = 0xff;
    let map = read::<Map<u8, Str>>(&bytes).unwrap();
    let mut iter = map.try_iter();
    assert_eq!(iter.next(), Some(Ok((1, "ok"))));
    assert_eq!(
        iter.next(),
        Some(Err(Error::new(ErrorKind::InvalidUtf8, 21)))
    );
    assert_eq!(iter.next(), None);
    assert_eq!(iter.clone().next(), None);
    assert_eq!(iter.size_hint(), (0, Some(0)));
}

#[test]
fn infallible_reads_panic_on_corruption_instead_of_reporting_absence() {
    let bytes = [1, 0, 0, 0, 2];
    let list = read::<List<bool>>(&bytes).unwrap();
    assert!(std::panic::catch_unwind(|| list.get(0)).is_err());
    let mut iter = list.iter();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| iter.next())).is_err());
    assert_eq!(iter.next(), None);
    let bytes = [1, 0, 0, 0, 0, 2];
    let map = read::<Map<bool, bool>>(&bytes).unwrap();
    assert!(std::panic::catch_unwind(|| map.get(&false)).is_err());
    let mut iter = map.iter();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| iter.next())).is_err());
    assert_eq!(iter.next(), None);
}

#[test]
fn malformed_descriptors_and_unrepresentable_bases_are_rejected() {
    static BAD: Layout = Layout::Leaf(Leaf::Raw(0));
    fault(
        View::new(&[], BAD, 0),
        ErrorKind::InvalidSchema,
        Error::NO_OFFSET,
    );
    fault(
        View::new(&[0], bool::LAYOUT, u32::MAX),
        ErrorKind::ArithmeticOverflow,
        Error::NO_OFFSET,
    );
    static BAD_STRUCT: Layout = Layout::Struct {
        fixed_bytes: 1,
        fixed: &[FixedField {
            declaration: 0,
            offset: 1,
            layout: &Layout::Leaf(Leaf::Bool),
        }],
        variable: &[],
    };
    fault(
        View::new(&[0], BAD_STRUCT, 0),
        ErrorKind::InvalidSchema,
        Error::NO_OFFSET,
    );
    // Valid descriptors, but nonsensical corrupted directory words.
    static THREE: Layout = Layout::Struct {
        fixed_bytes: 0,
        fixed: &[],
        variable: &[
            VariableField {
                declaration: 0,
                layout: &Layout::Str,
            },
            VariableField {
                declaration: 1,
                layout: &Layout::Str,
            },
            VariableField {
                declaration: 2,
                layout: &Layout::Str,
            },
        ],
    };
    let bytes = [2, 0, 0, 0, 1, 0, 0, 0, b'a', b'b'];
    let view = View::new(&bytes, THREE, 50).unwrap();
    fault(view.field(1), ErrorKind::InvalidBoundary, 54);
    fault(view.validate(), ErrorKind::InvalidBoundary, 54);
}

#[test]
fn all_truncated_prefixes_and_single_byte_mutations_remain_checked() {
    // Exercise malformed counts, directories, child tags, keys and UTF-8 across
    // nested layouts. Fallible traversal must terminate without a panic.
    type Nested = List<Map<bool, Option<Str>>>;
    let bytes = encoded::<Nested>(
        &&[
            &[(false, Some("é")), (true, None)][..],
            &[(true, Some(""))][..],
        ][..],
    );
    let inspect = |bytes: &[u8]| {
        if let Ok(list) = read::<Nested>(bytes) {
            let _ = list.validate();
            let _ = list.try_get(usize::MAX);
            for map in list.try_iter().flatten() {
                let _ = map.validate();
                let _ = map.try_get(&false);
                let _ = map.try_get(&true);
                for item in map.try_iter() {
                    let _ = item;
                }
            }
        }
    };
    for len in 0..bytes.len() {
        inspect(&bytes[..len]);
    }
    for index in 0..bytes.len() {
        for replacement in [0, 1, 2, 127, 255] {
            let mut mutated = bytes.clone();
            mutated[index] = replacement;
            inspect(&mutated);
        }
    }
}
