//! Hand-calculated format-1 encodings and corrupted exact-region decoding.

#![forbid(unsafe_code)]

mod support;

use blubber_model::{Error, IntWidth::*, Key, KeySchema, Schema, Value};
use support::*;

#[test]
fn every_integer_width_has_exact_little_endian_scalars() {
    for width in [W1, W2, W4, W8, W16] {
        let bits = width.bytes() * 8;
        let unsigned_max = u128::MAX >> (128 - bits);
        let signed_max = i128::MAX >> (128 - bits);
        let signed_min = -signed_max - 1;
        for value in [0, 1, unsigned_max / 2, unsigned_max] {
            let schema = Schema::Unsigned(width);
            let bytes = value.to_le_bytes()[..width.bytes()].to_vec();
            assert_eq!(schema.encode(&Value::Unsigned(value)), Ok(bytes.clone()));
            assert_eq!(schema.decode(&bytes), Ok(Value::Unsigned(value)));
            assert_eq!(
                schema.decode(&bytes[..bytes.len() - 1]),
                Err(Error::InvalidLength)
            );
            let mut extra = bytes;
            extra.push(0);
            assert_eq!(schema.decode(&extra), Err(Error::InvalidLength));
        }
        for value in [signed_min, -1, 0, 1, signed_max] {
            let schema = Schema::Signed(width);
            let bytes = value.to_le_bytes()[..width.bytes()].to_vec();
            assert_eq!(schema.encode(&Value::Signed(value)), Ok(bytes.clone()));
            assert_eq!(schema.decode(&bytes), Ok(Value::Signed(value)));
        }
        if width != W16 {
            assert_eq!(
                Schema::Unsigned(width).encode(&Value::Unsigned(unsigned_max + 1)),
                Err(Error::OutOfRange)
            );
            for value in [signed_min - 1, signed_max + 1] {
                assert_eq!(
                    Schema::Signed(width).encode(&Value::Signed(value)),
                    Err(Error::OutOfRange)
                );
            }
        }
    }
    assert_wire(&Schema::Unsigned(W2), Value::Unsigned(256), "00 01");
    assert_wire(&Schema::Signed(W2), Value::Signed(-257), "ff fe");
}

#[test]
fn keys_have_native_order_and_independent_big_endian_encodings() {
    for width in [W1, W2, W4, W8, W16] {
        let bits = width.bytes() * 8;
        let max = i128::MAX >> (128 - bits);
        let schema = KeySchema::Signed(width);
        let values = [-max - 1, -1, 0, 1, max];
        let mut previous = None;
        for value in values {
            let key = Key::Signed(value);
            let bytes = schema.encode(&key).unwrap();
            assert_eq!(schema.decode(&bytes), Ok(key));
            if let Some(previous) = previous {
                assert!(previous < bytes);
            }
            previous = Some(bytes);
        }
        assert_eq!(
            schema.encode(&Key::Signed(-max - 1)),
            Ok(vec![0; width.bytes()])
        );
        assert_eq!(
            schema.encode(&Key::Signed(max)),
            Ok(vec![255; width.bytes()])
        );
        let schema = KeySchema::Unsigned(width);
        for value in [0, 1, u128::MAX >> (128 - bits)] {
            let key = Key::Unsigned(value);
            let bytes = value.to_be_bytes()[16 - width.bytes()..].to_vec();
            assert_eq!(schema.encode(&key), Ok(bytes.clone()));
            assert_eq!(schema.decode(&bytes), Ok(key));
        }
        if width != W16 {
            assert_eq!(
                schema.encode(&Key::Unsigned(1u128 << bits)),
                Err(Error::OutOfRange)
            );
            assert_eq!(
                KeySchema::Signed(width).encode(&Key::Signed(max + 1)),
                Err(Error::OutOfRange)
            );
            assert_eq!(
                KeySchema::Signed(width).encode(&Key::Signed(-max - 2)),
                Err(Error::OutOfRange)
            );
        }
    }
    let signed = KeySchema::Signed(W2);
    for (value, expected) in [
        (-32768, "00 00"),
        (-1, "7f ff"),
        (0, "80 00"),
        (32767, "ff ff"),
    ] {
        assert_eq!(signed.encode(&Key::Signed(value)), Ok(hex(expected)));
    }
    assert_eq!(
        KeySchema::Unsigned(W2).encode(&Key::Unsigned(256)),
        Ok(hex("01 00"))
    );
    assert_eq!(
        KeySchema::Unsigned(W2).encode(&Key::Signed(1)),
        Err(Error::TypeMismatch)
    );
}

#[test]
fn boolean_domains_are_exhaustive_for_scalars_and_keys() {
    for byte in 0..=255 {
        let scalar = Schema::Bool.decode(&[byte]);
        let key = KeySchema::Bool.decode(&[byte]);
        if byte <= 1 {
            assert_eq!(scalar, Ok(Value::Bool(byte == 1)));
            assert_eq!(key, Ok(Key::Bool(byte == 1)));
        } else {
            assert_eq!(scalar, Err(Error::InvalidBool));
            assert_eq!(key, Err(Error::InvalidBool));
        }
    }
    for bytes in [vec![], vec![0, 1]] {
        assert_eq!(Schema::Bool.decode(&bytes), Err(Error::InvalidLength));
        assert_eq!(KeySchema::Bool.decode(&bytes), Err(Error::InvalidLength));
    }
    assert_eq!(KeySchema::Bool.encode(&Key::Bool(false)), Ok(vec![0]));
    assert_eq!(KeySchema::Bool.encode(&Key::Bool(true)), Ok(vec![1]));
}

#[test]
fn raw_arrays_and_pubkeys_have_exact_positive_widths() {
    for width in [1, 2, 4, 16, 32, 33, 257] {
        let bytes: Vec<_> = (0..width).map(|index| index as u8).collect();
        let schema = Schema::Array(width);
        assert_eq!(
            schema.encode(&Value::Array(bytes.clone())),
            Ok(bytes.clone())
        );
        assert_eq!(schema.decode(&bytes), Ok(Value::Array(bytes.clone())));
        assert_eq!(
            schema.decode(&bytes[..bytes.len() - 1]),
            Err(Error::InvalidLength)
        );
        let key = KeySchema::Array(width);
        assert_eq!(key.encode(&Key::Array(bytes.clone())), Ok(bytes.clone()));
        assert_eq!(key.decode(&bytes), Ok(Key::Array(bytes)));
    }
    let address = [0x85; 32];
    assert_eq!(Schema::Pubkey.decode(&address), Ok(Value::Pubkey(address)));
    assert_eq!(
        Schema::Pubkey.encode(&Value::Pubkey(address)),
        Ok(address.to_vec())
    );
    assert_eq!(KeySchema::Pubkey.decode(&address), Ok(Key::Pubkey(address)));
    assert_eq!(
        KeySchema::Pubkey.encode(&Key::Pubkey(address)),
        Ok(address.to_vec())
    );
    assert_eq!(
        Schema::Array(2).encode(&Value::Array(vec![1])),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        KeySchema::Array(2).encode(&Key::Array(vec![1])),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        Schema::Array(0).encode(&Value::Array(vec![])),
        Err(Error::InvalidSchema)
    );
    assert_eq!(KeySchema::Array(0).width(), Err(Error::InvalidSchema));
}

#[test]
fn utf8_acceptance_matches_std_through_two_bytes_and_longer_scalars() {
    for first in 0..=255 {
        for bytes in std::iter::once(vec![first]).chain((0..=255).map(|second| vec![first, second]))
        {
            let expected = std::str::from_utf8(&bytes)
                .map(string)
                .map_err(|_| Error::InvalidUtf8);
            assert_eq!(Schema::Str.decode(&bytes), expected);
        }
    }
    for text in ["", "ascii", "é", "€", "\u{10000}", "\u{10ffff}", "a\0界🦀"] {
        assert_eq!(
            Schema::Str.encode(&string(text)),
            Ok(text.as_bytes().to_vec())
        );
        assert_eq!(Schema::Str.decode(text.as_bytes()), Ok(string(text)));
    }
    for bytes in [
        hex("ed a0 80"),
        hex("f4 90 80 80"),
        hex("e0 80 80"),
        hex("f0 80 80 80"),
        hex("f0 9f a6"),
    ] {
        assert_eq!(Schema::Str.decode(&bytes), Err(Error::InvalidUtf8));
    }
}

#[test]
fn option_tags_and_exact_inactive_payloads() {
    let schema = option(Schema::Str);
    assert_wire(&schema, Value::Option(None), "00");
    assert_wire(&schema, Value::Option(Some(Box::new(string("")))), "01");
    assert_wire(&schema, Value::Option(Some(Box::new(string("x")))), "01 78");
    assert_eq!(schema.fixed_width(), Ok(None));
    assert_eq!(schema.decode(&[]), Err(Error::InvalidLength));
    assert_eq!(schema.decode(&[0, 0]), Err(Error::InvalidLength));
    for tag in 2..=255 {
        assert_eq!(schema.decode(&[tag]), Err(Error::InvalidTag));
    }
    let schema = option(Schema::Unsigned(W2));
    assert_wire(
        &schema,
        Value::Option(Some(Box::new(Value::Unsigned(256)))),
        "01 00 01",
    );
    assert_eq!(schema.decode(&[1]), Err(Error::InvalidLength));
    assert_eq!(schema.decode(&[1, 0, 0, 0]), Err(Error::InvalidLength));
    assert_wire(
        &option(option(Schema::Bool)),
        Value::Option(Some(Box::new(Value::Option(None)))),
        "01 00",
    );
}

#[test]
fn packed_structs_keep_declaration_values_and_reorder_wire_fields() {
    let fixed = Schema::Struct(vec![Schema::Bool, Schema::Unsigned(W2)]);
    assert_eq!(fixed.fixed_width(), Ok(Some(3)));
    assert_wire(
        &fixed,
        Value::Struct(vec![Value::Bool(true), Value::Unsigned(2)]),
        "01 02 00",
    );
    assert_eq!(fixed.decode(&hex("02 02 00")), Err(Error::InvalidBool));
    assert!(fixed.decode(&hex("01 02 00 00")).is_err());
    let schema = Schema::Struct(vec![Schema::Str, Schema::Unsigned(W2), Schema::Str, fixed]);
    assert_wire(
        &schema,
        Value::Struct(vec![
            string("ab"),
            Value::Unsigned(256),
            string(""),
            Value::Struct(vec![Value::Bool(false), Value::Unsigned(3)]),
        ]),
        "00 01 00 03 00 02 00 00 00 61 62",
    );
    let single = Schema::Struct(vec![Schema::Str, Schema::Bool]);
    assert_wire(
        &single,
        Value::Struct(vec![string(""), Value::Bool(true)]),
        "01",
    );
    assert_wire(
        &single,
        Value::Struct(vec![string("x"), Value::Bool(true)]),
        "01 78",
    );
    assert_eq!(
        single.encode(&Value::Struct(vec![])),
        Err(Error::TypeMismatch)
    );
    assert_wire(
        &record_schema(),
        record("A", &[7, 9]),
        "01 00 00 00 41 02 00 00 00 07 00 00 00 09 00 00 00",
    );
}

#[test]
fn variable_struct_empty_fields_and_corrupted_boundaries() {
    let schema = Schema::Struct(vec![Schema::Str, Schema::Str, Schema::Str]);
    assert_wire(
        &schema,
        Value::Struct(vec![string(""), string(""), string("")]),
        "00 00 00 00 00 00 00 00",
    );
    assert_wire(
        &schema,
        Value::Struct(vec![string("ab"), string(""), string("xyz")]),
        "02 00 00 00 02 00 00 00 61 62 78 79 7a",
    );
    for bytes in [
        "01 00 00 00 00 00 00 00 78",
        "02 00 00 00 02 00 00 00 78",
        "ff ff ff ff 00 00 00 00",
        "00 00 00",
    ] {
        assert!(schema.decode(&hex(bytes)).is_err());
    }
}

#[test]
fn lists_have_hand_calculated_zero_one_two_and_three_transitions() {
    let schema = list(Schema::Unsigned(W2));
    assert_wire(&schema, Value::List(vec![]), "00 00 00 00");
    assert_wire(
        &schema,
        Value::List(vec![Value::Unsigned(256)]),
        "01 00 00 00 00 01",
    );
    assert_wire(
        &schema,
        Value::List(vec![Value::Unsigned(1), Value::Unsigned(256)]),
        "02 00 00 00 01 00 00 01",
    );
    let schema = list(Schema::Str);
    assert_wire(&schema, Value::List(vec![]), "00 00 00 00");
    assert_wire(&schema, Value::List(vec![string("")]), "01 00 00 00");
    assert_wire(
        &schema,
        Value::List(vec![string("ab")]),
        "01 00 00 00 61 62",
    );
    assert_wire(
        &schema,
        Value::List(vec![string(""), string("")]),
        "02 00 00 00 00 00 00 00",
    );
    assert_wire(
        &schema,
        Value::List(vec![string("a"), string("b")]),
        "02 00 00 00 01 00 00 00 61 62",
    );
    assert_wire(
        &schema,
        Value::List(vec![string("ab"), string(""), string("xyz")]),
        "03 00 00 00 02 00 00 00 02 00 00 00 61 62 78 79 7a",
    );
    assert_wire(
        &list(list(Schema::Unsigned(W1))),
        Value::List(vec![
            Value::List(vec![]),
            Value::List(vec![Value::Unsigned(7)]),
        ]),
        "02 00 00 00 04 00 00 00 00 00 00 00 01 00 00 00 07",
    );
    assert_wire(
        &list(Schema::Unsigned(W1)),
        Value::List(vec![Value::Unsigned(0xaa), Value::Unsigned(0xbb)]),
        "02 00 00 00 aa bb",
    );
    assert_wire(&Schema::Str, string("ab"), "61 62");
}

#[test]
fn lists_reject_nonexact_fixed_extents_and_invalid_elided_ends() {
    for bytes in [
        "00 00 00 00 00",
        "01 00 00 00",
        "01 00 00 00 00",
        "01 00 00 00 00 00 00",
        "ff ff ff ff",
    ] {
        assert!(list(Schema::Unsigned(W2)).decode(&hex(bytes)).is_err());
    }
    for bytes in [
        "00 00 00 00 78",
        "02 00 00 00",
        "02 00 00 00 02 00 00 00 78",
        "03 00 00 00 01 00 00 00 00 00 00 00 78",
        "ff ff ff ff",
    ] {
        assert!(list(Schema::Str).decode(&hex(bytes)).is_err());
    }
    assert_eq!(
        list(Schema::Bool).decode(&hex("01 00 00 00 02")),
        Err(Error::InvalidBool)
    );
}

#[test]
fn maps_encode_fixed_values_interleaved_and_variable_values_after_all_ends() {
    let fixed = map(KeySchema::Unsigned(W2), Schema::Unsigned(W2));
    assert_wire(&fixed, Value::Map(Default::default()), "00 00 00 00");
    assert_wire(
        &fixed,
        Value::Map(
            [
                (Key::Unsigned(1), Value::Unsigned(256)),
                (Key::Unsigned(256), Value::Unsigned(1)),
            ]
            .into(),
        ),
        "02 00 00 00 00 01 00 01 01 00 01 00",
    );
    let variable = map(KeySchema::Unsigned(W2), Schema::Str);
    assert_wire(&variable, Value::Map(Default::default()), "00 00 00 00");
    assert_wire(
        &variable,
        Value::Map([(Key::Unsigned(1), string(""))].into()),
        "01 00 00 00 00 01 00 00 00 00",
    );
    assert_wire(
        &variable,
        Value::Map(
            [
                (Key::Unsigned(1), string("x")),
                (Key::Unsigned(256), string("")),
            ]
            .into(),
        ),
        "02 00 00 00 00 01 01 00 00 00 01 00 01 00 00 00 78",
    );
    let signed = map(KeySchema::Signed(W1), Schema::Bool);
    assert_wire(
        &signed,
        Value::Map(
            [
                (Key::Signed(-128), Value::Bool(false)),
                (Key::Signed(127), Value::Bool(true)),
            ]
            .into(),
        ),
        "02 00 00 00 00 00 ff 01",
    );
}

#[test]
fn maps_reject_duplicates_descending_keys_and_missing_terminal_payload() {
    let fixed = map(KeySchema::Unsigned(W1), Schema::Bool);
    for bytes in [
        "02 00 00 00 01 00 01 01",
        "02 00 00 00 02 00 01 01",
        "01 00 00 00 01 02",
        "00 00 00 00 ff",
        "01 00 00 00 01",
        "ff ff ff ff",
    ] {
        assert!(fixed.decode(&hex(bytes)).is_err());
    }
    let variable = map(KeySchema::Unsigned(W1), Schema::Str);
    for bytes in [
        "00 00 00 00 78",
        "01 00 00 00 01 00 00 00 00 78",
        "01 00 00 00 01 02 00 00 00 78",
        "02 00 00 00 01 01 00 00 00 02 00 00 00 00 78",
        "02 00 00 00 01 00 00 00 00 01 00 00 00 00",
        "02 00 00 00 02 00 00 00 00 01 00 00 00 00",
        "ff ff ff ff",
    ] {
        assert!(variable.decode(&hex(bytes)).is_err());
    }
    assert_eq!(
        map(KeySchema::Bool, Schema::Str).decode(&hex("01 00 00 00 02 00 00 00 00")),
        Err(Error::InvalidBool)
    );
}

#[test]
fn depth_checks_include_inactive_children_and_allow_explicit_sixteen() {
    let mut schema = Schema::Bool;
    let mut value = Value::Bool(true);
    for depth in 1..=17 {
        for budget in [0, 1, 8, 16, 17, usize::MAX] {
            let valid = (1..=16).contains(&budget) && depth <= budget;
            assert_eq!(
                schema.check(budget).is_ok(),
                valid,
                "depth {depth}, budget {budget}"
            );
            assert_eq!(schema.encode_with_depth(&value, budget).is_ok(), valid);
        }
        if depth <= 16 {
            let bytes = schema.encode_with_depth(&value, 16).unwrap();
            assert_eq!(schema.decode_with_depth(&bytes, 16), Ok(value.clone()));
        }
        schema = option(schema);
        value = Value::Option(Some(Box::new(value)));
    }
    assert_eq!(
        schema.encode(&Value::Option(None)),
        Err(Error::InvalidSchema)
    );
    let too_deep = list(schema);
    assert_eq!(
        too_deep.encode(&Value::List(vec![])),
        Err(Error::InvalidSchema)
    );
    assert_eq!(too_deep.decode(&[0; 4]), Err(Error::InvalidSchema));
}

#[test]
fn invalid_metadata_and_full_width_extents_fail_without_allocating_payloads() {
    for schema in [
        Schema::Struct(vec![]),
        Schema::Struct(vec![Schema::Bool; 65536]),
        list(Schema::Array(0)),
        map(KeySchema::Array(0), Schema::Str),
    ] {
        assert_eq!(schema.check(8), Err(Error::InvalidSchema));
    }
    let max = Schema::Array(u32::MAX);
    assert_eq!(max.fixed_width(), Ok(Some(u32::MAX as usize)));
    assert_eq!(
        Schema::Struct(vec![max.clone(), Schema::Bool]).check(8),
        Err(Error::Overflow)
    );
    assert_eq!(
        list(max.clone()).decode(&hex("01 00 00 00")),
        Err(Error::InvalidLength)
    );
    assert_eq!(
        list(max.clone()).decode(&hex("02 00 00 00")),
        Err(Error::Overflow)
    );
    for schema in [
        map(KeySchema::Array(u32::MAX), Schema::Bool),
        map(KeySchema::Array(u32::MAX), Schema::Str),
        list(max),
    ] {
        assert_wire(
            &schema,
            if matches!(schema, Schema::Map(..)) {
                Value::Map(Default::default())
            } else {
                Value::List(vec![])
            },
            "00 00 00 00",
        );
        assert!(schema.decode(&hex("ff ff ff ff")).is_err());
    }
    assert_eq!(
        Schema::Bool.encode(&Value::Unsigned(1)),
        Err(Error::TypeMismatch)
    );
}

#[test]
fn every_accepted_corrupted_sample_reencodes_to_identical_bytes() {
    let schemas = [
        Schema::Bool,
        Schema::Str,
        option(Schema::Str),
        list(Schema::Str),
        list(Schema::Unsigned(W2)),
        map(KeySchema::Signed(W1), Schema::Str),
        record_schema(),
        demo_schema(),
    ];
    let mut state = 0x31c0_dec0_de12_3456u64;
    for length in 0..=96 {
        for _ in 0..32 {
            let bytes: Vec<_> = (0..length)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                })
                .collect();
            for schema in &schemas {
                if let Ok(value) = schema.decode(&bytes) {
                    assert_eq!(schema.encode(&value), Ok(bytes.clone()));
                }
            }
        }
    }
    let bytes = demo_hex();
    for index in 12..167 {
        for byte in [0, 1, 0x80, 0xff] {
            let mut mutated = bytes[12..167].to_vec();
            mutated[index - 12] = byte;
            if let Ok(value) = demo_schema().decode(&mutated) {
                assert_eq!(demo_schema().encode(&value), Ok(mutated));
            }
        }
    }
}

#[test]
fn zero_byte_schema_struct_values_stay_variable_kind_inside_collections() {
    let child = Schema::Struct(vec![Schema::Str]);
    let empty = Value::Struct(vec![string("")]);
    assert_eq!(child.fixed_width(), Ok(None));
    assert_eq!(child.minimum_size(8), Ok(0));
    assert_wire(
        &list(child.clone()),
        Value::List(vec![empty.clone(), empty.clone()]),
        "02 00 00 00 00 00 00 00",
    );
    assert_wire(
        &map(KeySchema::Bool, child),
        Value::Map([(Key::Bool(false), empty.clone()), (Key::Bool(true), empty)].into()),
        "02 00 00 00 00 00 00 00 00 01 00 00 00 00",
    );
}

#[test]
fn recursive_options_lists_maps_and_fixed_struct_values_compose() {
    let fixed = Schema::Struct(vec![Schema::Bool, Schema::Signed(W2)]);
    let fixed_value = Value::Struct(vec![Value::Bool(true), Value::Signed(-1)]);
    assert_wire(
        &list(fixed.clone()),
        Value::List(vec![fixed_value.clone()]),
        "01 00 00 00 01 ff ff",
    );
    assert_wire(
        &map(KeySchema::Bool, fixed),
        Value::Map([(Key::Bool(true), fixed_value)].into()),
        "01 00 00 00 01 01 ff ff",
    );
    // Outer list elides its only end. The map stores its sole final end (7);
    // its present option contains a fixed u16 list of length one.
    let schema = list(map(KeySchema::Array(2), option(list(Schema::Unsigned(W2)))));
    let value = Value::List(vec![Value::Map(
        [(
            Key::Array(vec![0, 255]),
            Value::Option(Some(Box::new(Value::List(vec![Value::Unsigned(256)])))),
        )]
        .into(),
    )]);
    assert_wire(
        &schema,
        value,
        "01 00 00 00 01 00 00 00 00 ff 07 00 00 00 01 01 00 00 00 00 01",
    );
    let schema = Schema::Struct(vec![
        option(Schema::Bool),
        Schema::Str,
        map(KeySchema::Pubkey, list(Schema::Str)),
    ]);
    let value = Value::Struct(vec![
        Value::Option(None),
        string(""),
        Value::Map(
            [(
                Key::Pubkey([0; 32]),
                Value::List(vec![string(""), string("abc")]),
            )]
            .into(),
        ),
    ]);
    let bytes = schema.encode(&value).unwrap();
    assert_eq!(schema.decode(&bytes), Ok(value));
}
