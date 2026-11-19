//! Production derives, packed wire fixtures and checked native-value readers.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "support/fixed.rs"]
mod shapes;

use blubber::{Error, ErrorKind, Fixed, Kind, List, Map, Schema, View};
use blubber_core::{check_layout, validate, Layout, Leaf, MAX_DEPTH};
use shapes::{Limits, Padded};

#[derive(Clone, Copy, blubber::Fixed)]
#[repr(C, packed)]
struct PackedProjection {
    first: u8,
    next: u32,
}

#[test]
fn packed_native_fields_project_by_value_without_alignment_requirements() {
    let value = PackedProjection {
        first: 7,
        next: 0x0102_0304,
    };
    assert_eq!(value.encode(), [7, 4, 3, 2, 1]);
    let decoded = PackedProjection::decode(&[7, 4, 3, 2, 1]).unwrap();
    let (first, next) = (decoded.first, decoded.next);
    assert_eq!((first, next), (7, 0x0102_0304));
    let mut output = [0xA5; 7];
    <PackedProjection as Schema>::encode(&value, &mut output[1..6]);
    assert_eq!(output, [0xA5, 7, 4, 3, 2, 1, 0xA5]);
}

#[test]
fn limits_has_packed_wire_width_and_native_layout() {
    let value = Limits {
        enabled: true,
        limit: 0x1234,
    };
    assert_eq!(Limits::SIZE, 3);
    assert_eq!(core::mem::size_of::<Limits>(), 4);
    assert_eq!(value.encode(), [1, 0x34, 0x12]);
    assert_eq!(Limits::decode(&[1, 0x34, 0x12]), Ok(value));
    assert_eq!(Limits::KIND, Kind::Fixed);
    assert_eq!(Limits::FIXED_SIZE, Some(3));
    assert_eq!(Limits::MIN_SIZE, 3);
}

#[test]
fn padded_nested_values_preserve_repr_and_omit_every_padding_byte() {
    let value = Padded {
        flag: false,
        total: 0x0102_0304_0506_0708,
        limits: Limits {
            enabled: true,
            limit: 0x1234,
        },
        raw: [0, 0x80, 0xff],
    };
    assert_eq!(core::mem::align_of::<Padded>(), 16);
    assert_eq!(core::mem::size_of::<Padded>(), 32);
    assert_eq!(core::mem::offset_of!(Padded, total), 8);
    assert_eq!(core::mem::offset_of!(Padded, limits), 16);
    assert_eq!(Padded::SIZE, 15);
    let wire = [0, 8, 7, 6, 5, 4, 3, 2, 1, 1, 0x34, 0x12, 0, 0x80, 0xff];
    assert_eq!(value.encode(), wire);
    assert_eq!(Padded::decode(&wire), Ok(value));
    validate(&wire, Padded::LAYOUT, MAX_DEPTH).unwrap();
}

#[test]
fn metadata_has_exact_declaration_ordinals_offsets_and_nested_layouts() {
    check_layout(Padded::LAYOUT, MAX_DEPTH).unwrap();
    let Layout::Struct {
        fixed_bytes,
        fixed,
        variable,
    } = Padded::LAYOUT
    else {
        panic!("fixed struct descriptor");
    };
    assert_eq!(*fixed_bytes, 15);
    assert!(variable.is_empty());
    assert_eq!(
        fixed
            .iter()
            .map(|field| field.declaration)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(
        fixed.iter().map(|field| field.offset).collect::<Vec<_>>(),
        [0, 1, 9, 12]
    );
    let Layout::Struct {
        fixed_bytes,
        fixed,
        variable,
    } = fixed[2].layout
    else {
        panic!("nested fixed struct descriptor");
    };
    assert_eq!(*fixed_bytes, Limits::SIZE);
    assert!(variable.is_empty());
    assert_eq!(fixed.len(), 2);
    assert_eq!((fixed[0].declaration, fixed[0].offset), (0, 0));
    assert!(matches!(fixed[0].layout, Layout::Leaf(Leaf::Bool)));
    assert_eq!((fixed[1].declaration, fixed[1].offset), (1, 1));
    assert!(matches!(fixed[1].layout, Layout::Leaf(Leaf::Unsigned(2))));
}

#[test]
fn width_errors_precede_domains_and_nested_faults_rebase_once() {
    let mut bytes = [0xff; 18];
    for length in 0..=18 {
        let error = if length < 15 {
            Error::new(ErrorKind::Truncated, length as u32)
        } else if length > 15 {
            Error::new(ErrorKind::InvalidBoundary, 15)
        } else {
            Error::new(ErrorKind::InvalidBool, 0)
        };
        assert_eq!(Padded::decode(&bytes[..length]), Err(error));
    }
    bytes[0] = 0;
    assert_eq!(
        Padded::decode(&bytes[..15]),
        Err(Error::new(ErrorKind::InvalidBool, 9))
    );
    let view = View::new(&bytes[..15], Padded::LAYOUT, 100).unwrap();
    assert_eq!(
        view.read::<Padded>(),
        Err(Error::new(ErrorKind::InvalidBool, 109))
    );
    assert_eq!(
        Padded::read_removed(view),
        Err(Error::new(ErrorKind::InvalidBool, 109))
    );
    assert_eq!(
        view.validate(),
        Err(Error::new(ErrorKind::InvalidBool, 109))
    );
}

#[test]
fn every_bool_byte_matches_full_validation_and_never_decodes_invalid_tags() {
    for first in 0..=255u8 {
        for nested in 0..=255u8 {
            let mut bytes = [0; 15];
            bytes[0] = first;
            bytes[9] = nested;
            let result = Padded::decode(&bytes);
            assert_eq!(result.is_ok(), first <= 1 && nested <= 1);
            assert_eq!(
                result.map(|_| ()),
                validate(&bytes, Padded::LAYOUT, MAX_DEPTH)
            );
        }
    }
}

#[test]
fn schema_encodes_every_destination_byte_and_reads_native_removed_values() {
    let value = Limits {
        enabled: true,
        limit: u16::MAX,
    };
    assert_eq!(Limits::measure(&value), Ok(3));
    let mut room = [0xa5; 9];
    <Limits as Schema>::encode(&value, &mut room[3..6]);
    assert_eq!(room, [0xa5, 0xa5, 0xa5, 1, 0xff, 0xff, 0xa5, 0xa5, 0xa5]);
    let view = View::new(&room[3..6], Limits::LAYOUT, 3).unwrap();
    assert_eq!(view.read::<Limits>(), Ok(value));
    assert_eq!(Limits::read_removed(view), Ok(value));
    assert_eq!(view.field(0).unwrap().read::<bool>(), Ok(true));
    assert_eq!(view.field(1).unwrap().read::<u16>(), Ok(u16::MAX));
}

#[test]
fn derived_values_use_fixed_list_and_map_strides() {
    let values = [
        Limits {
            enabled: false,
            limit: 7,
        },
        Limits {
            enabled: true,
            limit: 9,
        },
    ];
    let mut list_bytes = [0; 10];
    assert_eq!(List::<Limits>::measure(&values.as_slice()), Ok(10));
    List::<Limits>::encode(&values.as_slice(), &mut list_bytes);
    assert_eq!(list_bytes, [2, 0, 0, 0, 0, 7, 0, 1, 9, 0]);
    let list = View::new(&list_bytes, List::<Limits>::LAYOUT, 0)
        .unwrap()
        .read::<List<Limits>>()
        .unwrap();
    assert_eq!(
        list.try_iter().collect::<Result<Vec<_>, _>>(),
        Ok(values.to_vec())
    );
    let entries = [(1u16, values[0]), (256, values[1])];
    let mut map_bytes = [0; 14];
    Map::<u16, Limits>::encode(&entries.as_slice(), &mut map_bytes);
    assert_eq!(map_bytes, [2, 0, 0, 0, 0, 1, 0, 7, 0, 1, 0, 1, 9, 0]);
    let map = View::new(&map_bytes, Map::<u16, Limits>::LAYOUT, 0)
        .unwrap()
        .read::<Map<u16, Limits>>()
        .unwrap();
    assert_eq!(map.try_get(&256), Ok(Some(values[1])));
    assert_eq!(map.try_get(&2), Ok(None));
}

#[test]
fn aliases_qualified_types_raw_identifiers_and_inherent_helpers_work() {
    type Word = u16;
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Fixed)]
    struct Alias {
        r#type: Word,
        value: shapes::Limits,
        raw: [u8; 33],
    }
    impl Alias {
        fn size(self) -> usize {
            self.raw.len()
        }
    }
    let value = Alias {
        r#type: 0x1234,
        value: Limits {
            enabled: true,
            limit: 7,
        },
        raw: [0x55; 33],
    };
    assert_eq!(value.size(), 33);
    assert_eq!(Alias::SIZE, 38);
    let wire = value.encode();
    assert_eq!(&wire[..5], &[0x34, 0x12, 1, 7, 0]);
    assert_eq!(Alias::decode(&wire), Ok(value));
    check_layout(Alias::LAYOUT, MAX_DEPTH).unwrap();
}

#[test]
fn generated_names_and_shadowed_prelude_types_do_not_capture_user_items() {
    #[allow(non_camel_case_types)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Fixed)]
    struct __BLUBBER_OFFSETS {
        destination: u16,
        bytes: bool,
    }
    struct Result;
    struct Option;
    let _ = (Result, Option);
    let value = __BLUBBER_OFFSETS {
        destination: 1,
        bytes: true,
    };
    assert_eq!(value.encode(), [1, 0, 1]);
    assert_eq!(__BLUBBER_OFFSETS::decode(&[1, 0, 1]), Ok(value));
}

#[test]
fn fixed_derived_chunks_match_checked_selection_and_rebase_nested_faults() {
    type Alias = Padded;
    for count in 0..=4 {
        let mut bytes = (count as u32).to_le_bytes().to_vec();
        bytes.resize(4 + count * Padded::SIZE as usize, 0);
        for bad in 0..=count {
            let mut bytes = bytes.clone();
            if bad < count {
                bytes[4 + bad * Padded::SIZE as usize + 9] = 9;
            }
            let list = View::new(&bytes, <List<Alias>>::LAYOUT, 100)
                .unwrap()
                .read::<List<Alias>>()
                .unwrap();
            let mut iter = list.try_iter();
            for i in 0..count {
                let expected = list.try_get(i).map(Option::unwrap);
                assert_eq!(iter.next(), Some(expected));
                if expected.is_err() {
                    assert_eq!(
                        expected,
                        Err(Error::new(
                            ErrorKind::InvalidBool,
                            113 + i as u32 * Padded::SIZE
                        ))
                    );
                    break;
                }
            }
            assert_eq!(iter.next(), None);
            assert_eq!(iter.clone().next(), None);
        }
    }
}
