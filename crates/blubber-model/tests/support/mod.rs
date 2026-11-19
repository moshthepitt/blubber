//! Independent logical fixtures and literal hex parsing; no production imports.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use blubber_model::{IntWidth::*, Key, KeySchema, Schema, Value};

pub fn hex(input: &str) -> Vec<u8> {
    input
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

pub fn string(value: &str) -> Value {
    Value::Str(value.to_owned())
}

pub fn list(child: Schema) -> Schema {
    Schema::List(Box::new(child))
}

pub fn map(key: KeySchema, value: Schema) -> Schema {
    Schema::Map(key, Box::new(value))
}

pub fn option(child: Schema) -> Schema {
    Schema::Option(Box::new(child))
}

pub fn record_schema() -> Schema {
    Schema::Struct(vec![Schema::Str, list(Schema::Unsigned(W4))])
}

pub fn record(name: &str, tags: &[u32]) -> Value {
    Value::Struct(vec![
        string(name),
        Value::List(
            tags.iter()
                .map(|value| Value::Unsigned(u128::from(*value)))
                .collect(),
        ),
    ])
}

pub const DISCRIMINATOR: [u8; 8] = [47, 253, 130, 2, 67, 165, 139, 141];

pub fn demo_schema() -> Schema {
    Schema::Struct(vec![
        Schema::Pubkey,
        Schema::Unsigned(W8),
        list(record_schema()),
        map(KeySchema::Pubkey, Schema::Unsigned(W8)),
        map(KeySchema::Unsigned(W2), Schema::Str),
        Schema::Str,
        list(Schema::Unsigned(W1)),
        option(Schema::Str),
        Schema::Struct(vec![Schema::Bool, Schema::Unsigned(W2)]),
    ])
}

pub fn demo_value() -> Value {
    Value::Struct(vec![
        Value::Pubkey([0x11; 32]),
        Value::Unsigned(1),
        Value::List(vec![record("A", &[7, 9])]),
        Value::Map([(Key::Pubkey([0x22; 32]), Value::Unsigned(5))].into()),
        Value::Map(
            [
                (Key::Unsigned(1), string("x")),
                (Key::Unsigned(256), string("")),
            ]
            .into(),
        ),
        string("ok"),
        Value::List(vec![Value::Unsigned(0xaa), Value::Unsigned(0xbb)]),
        Value::Option(Some(Box::new(string("m")))),
        Value::Struct(vec![Value::Bool(true), Value::Unsigned(2)]),
    ])
}

pub fn demo_hex() -> Vec<u8> {
    hex(include_str!("../../../blubber/tests/fixtures/demo.hex"))
}

pub fn assert_wire(schema: &Schema, value: Value, literal: &str) {
    let expected = hex(literal);
    assert_eq!(schema.encode(&value), Ok(expected.clone()));
    assert_eq!(schema.decode(&expected), Ok(value));
}
