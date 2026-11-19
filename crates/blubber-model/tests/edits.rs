//! Logical edit outcomes, nested selection, and atomic model failures.

#![forbid(unsafe_code)]

mod support;

use blubber_model::{
    Edit, Error,
    IntWidth::*,
    Key, KeySchema, Model,
    PathSegment::{Field, Index, Key as MapKey, Some as Present},
    Schema, Value,
};
use support::*;

fn check(model: &Model, expected: &str) {
    let bytes = hex(expected);
    assert_eq!(model.encode(), Ok(bytes.clone()));
    assert_eq!(model.schema().decode(&bytes), Ok(model.value().clone()));
}

fn rejects(model: &mut Model, path: &[blubber_model::PathSegment], edit: Edit, error: Error) {
    let original = model.clone();
    let bytes = model.encode().unwrap();
    assert_eq!(model.apply(path, edit), Err(error));
    assert_eq!(*model, original);
    assert_eq!(model.encode(), Ok(bytes));
}

#[test]
fn variable_list_edits_cross_every_elision_transition() {
    let mut model = Model::new(list(Schema::Str), Value::List(vec![])).unwrap();
    check(&model, "00 00 00 00");
    assert_eq!(model.apply(&[], Edit::ListPop), Ok(None));
    model
        .apply(
            &[],
            Edit::ListInsert {
                index: 0,
                values: vec![string("a")],
            },
        )
        .unwrap();
    check(&model, "01 00 00 00 61");
    model
        .apply(
            &[],
            Edit::ListInsert {
                index: 0,
                values: vec![string("")],
            },
        )
        .unwrap();
    check(&model, "02 00 00 00 00 00 00 00 61");
    model
        .apply(
            &[],
            Edit::ListInsert {
                index: 1,
                values: vec![string("bc")],
            },
        )
        .unwrap();
    check(&model, "03 00 00 00 00 00 00 00 02 00 00 00 62 63 61");
    assert_eq!(
        model.apply(&[Index(2)], Edit::Set(string(""))),
        Ok(Some(string("a")))
    );
    check(&model, "03 00 00 00 00 00 00 00 02 00 00 00 62 63");
    assert_eq!(
        model.apply(&[], Edit::ListRemove(1)),
        Ok(Some(string("bc")))
    );
    check(&model, "02 00 00 00 00 00 00 00");
    assert_eq!(model.apply(&[], Edit::ListPop), Ok(Some(string(""))));
    check(&model, "01 00 00 00");
    assert_eq!(model.apply(&[], Edit::ListRemove(0)), Ok(Some(string(""))));
    check(&model, "00 00 00 00");
}

#[test]
fn fixed_list_bulk_insertion_replacement_truncation_and_clear() {
    let mut model = Model::new(
        list(Schema::Unsigned(W2)),
        Value::List(vec![Value::Unsigned(1)]),
    )
    .unwrap();
    assert_eq!(
        model.apply(
            &[],
            Edit::ListInsert {
                index: 1,
                values: vec![Value::Unsigned(256), Value::Unsigned(2)]
            }
        ),
        Ok(None)
    );
    check(&model, "03 00 00 00 01 00 00 01 02 00");
    assert_eq!(
        model.apply(&[Index(1)], Edit::Set(Value::Unsigned(3))),
        Ok(Some(Value::Unsigned(256)))
    );
    check(&model, "03 00 00 00 01 00 03 00 02 00");
    model.apply(&[], Edit::ListTruncate(usize::MAX)).unwrap();
    check(&model, "03 00 00 00 01 00 03 00 02 00");
    model.apply(&[], Edit::ListTruncate(2)).unwrap();
    check(&model, "02 00 00 00 01 00 03 00");
    model.apply(&[], Edit::Clear).unwrap();
    check(&model, "00 00 00 00");
    model
        .apply(
            &[],
            Edit::ListInsert {
                index: 0,
                values: vec![],
            },
        )
        .unwrap();
    check(&model, "00 00 00 00");
}

#[test]
fn variable_bulk_insertion_keeps_empty_and_nonempty_elements_in_order() {
    let mut model = Model::new(
        list(Schema::Str),
        Value::List(vec![string("a"), string("b")]),
    )
    .unwrap();
    model
        .apply(
            &[],
            Edit::ListInsert {
                index: 1,
                values: vec![string(""), string("xy")],
            },
        )
        .unwrap();
    check(
        &model,
        "04 00 00 00 01 00 00 00 01 00 00 00 03 00 00 00 61 78 79 62",
    );
    model.apply(&[], Edit::ListTruncate(1)).unwrap();
    check(&model, "01 00 00 00 61");
    model.apply(&[], Edit::ListTruncate(0)).unwrap();
    check(&model, "00 00 00 00");
}

#[test]
fn fixed_map_insertion_replaces_without_duplicate_keys_and_returns_old_values() {
    let schema = map(KeySchema::Signed(W1), Schema::Unsigned(W2));
    let mut model = Model::new(schema, Value::Map(Default::default())).unwrap();
    for (key, value) in [(1, 7), (-1, 256), (0, 2)] {
        assert_eq!(
            model.apply(
                &[],
                Edit::MapInsert {
                    key: Key::Signed(key),
                    value: Value::Unsigned(value)
                }
            ),
            Ok(None)
        );
    }
    check(&model, "03 00 00 00 7f 00 01 80 02 00 81 07 00");
    assert_eq!(
        model.apply(
            &[],
            Edit::MapInsert {
                key: Key::Signed(0),
                value: Value::Unsigned(8)
            }
        ),
        Ok(Some(Value::Unsigned(2)))
    );
    check(&model, "03 00 00 00 7f 00 01 80 08 00 81 07 00");
    assert_eq!(model.apply(&[], Edit::MapRemove(Key::Signed(5))), Ok(None));
    assert_eq!(
        model.apply(&[], Edit::MapRemove(Key::Signed(-1))),
        Ok(Some(Value::Unsigned(256)))
    );
    check(&model, "02 00 00 00 80 08 00 81 07 00");
    model.apply(&[], Edit::Clear).unwrap();
    check(&model, "00 00 00 00");
}

#[test]
fn variable_map_retains_its_last_end_across_empty_replacement_and_removal() {
    let mut model = Model::new(
        map(KeySchema::Unsigned(W1), Schema::Str),
        Value::Map(Default::default()),
    )
    .unwrap();
    model
        .apply(
            &[],
            Edit::MapInsert {
                key: Key::Unsigned(2),
                value: string(""),
            },
        )
        .unwrap();
    check(&model, "01 00 00 00 02 00 00 00 00");
    model
        .apply(
            &[],
            Edit::MapInsert {
                key: Key::Unsigned(1),
                value: string("x"),
            },
        )
        .unwrap();
    check(&model, "02 00 00 00 01 01 00 00 00 02 01 00 00 00 78");
    assert_eq!(
        model.apply(
            &[MapKey(Key::Unsigned(2))],
            Edit::StringPush("yz".to_owned())
        ),
        Ok(None)
    );
    check(&model, "02 00 00 00 01 01 00 00 00 02 03 00 00 00 78 79 7a");
    assert_eq!(
        model.apply(
            &[],
            Edit::MapInsert {
                key: Key::Unsigned(1),
                value: string("")
            }
        ),
        Ok(Some(string("x")))
    );
    check(&model, "02 00 00 00 01 00 00 00 00 02 02 00 00 00 79 7a");
    assert_eq!(
        model.apply(&[], Edit::MapRemove(Key::Unsigned(2))),
        Ok(Some(string("yz")))
    );
    check(&model, "01 00 00 00 01 00 00 00 00");
    model.apply(&[], Edit::Clear).unwrap();
    check(&model, "00 00 00 00");
}

#[test]
fn nested_demo_edits_rebuild_ancestors_and_keep_other_subtrees_byte_identical() {
    let mut model = Model::new(demo_schema(), demo_value()).unwrap();
    let original = model.encode().unwrap();
    // This adds four bytes inside Record.name; every following root payload
    // moves, but the tags and other complete logical subtrees keep their bytes.
    model
        .apply(
            &[Field(2), Index(0), Field(0)],
            Edit::StringPush("🦀".to_owned()),
        )
        .unwrap();
    let grown = model.encode().unwrap();
    assert_eq!(grown.len(), original.len() + 4);
    assert_eq!(&grown[76..], &original[72..]);
    assert_eq!(model.schema().decode(&grown), Ok(model.value().clone()));
    assert_eq!(
        model.apply(&[Field(2), Index(0), Field(0)], Edit::Set(string(""))),
        Ok(Some(string("A🦀")))
    );
    let shrunk = model.encode().unwrap();
    assert_eq!(shrunk.len(), original.len() - 1);
    assert_eq!(&shrunk[71..], &original[72..]);
    assert_eq!(
        model.apply(&[Field(8), Field(1)], Edit::Set(Value::Unsigned(65535))),
        Ok(Some(Value::Unsigned(2)))
    );
    assert_eq!(
        model.apply(&[Field(7), Present], Edit::StringPush("ore".to_owned())),
        Ok(None)
    );
    assert_eq!(model.apply(&[Field(7)], Edit::Clear), Ok(None));
    rejects(
        &mut model,
        &[Field(7), Present],
        Edit::Set(string("x")),
        Error::Missing,
    );
    model
        .apply(
            &[Field(7)],
            Edit::Set(Value::Option(Some(Box::new(string(""))))),
        )
        .unwrap();
    let bytes = model.encode().unwrap();
    assert_eq!(model.schema().decode(&bytes), Ok(model.value().clone()));
}

#[test]
fn logical_paths_work_at_depth_eight_and_explicit_sixteen() {
    for depth in [8, 16] {
        let mut schema = Schema::Str;
        let mut value = string("a");
        let mut path = Vec::new();
        for _ in 1..depth {
            schema = option(schema);
            value = Value::Option(Some(Box::new(value)));
            path.push(Present);
        }
        let mut model = Model::with_depth(schema, value, depth).unwrap();
        model
            .apply(&path, Edit::StringPush("b".to_owned()))
            .unwrap();
        let mut expected = vec![1; depth - 1];
        expected.extend_from_slice(b"ab");
        assert_eq!(model.encode(), Ok(expected.clone()));
        assert_eq!(
            model.schema().decode_with_depth(&expected, depth),
            Ok(model.value().clone())
        );
    }
}

#[test]
fn bad_paths_indices_inputs_and_operations_are_atomic() {
    let mut model = Model::new(demo_schema(), demo_value()).unwrap();
    for path in [
        vec![Field(99)],
        vec![Field(2), Index(99)],
        vec![Field(2), Field(0)],
        vec![Field(5), Index(0)],
        vec![Field(4), MapKey(Key::Unsigned(99))],
    ] {
        let error = match path.last().unwrap() {
            Field(99) | Index(99) => Error::Index,
            MapKey(_) => Error::Missing,
            _ => Error::InvalidPath,
        };
        rejects(&mut model, &path, Edit::Clear, error);
    }
    rejects(
        &mut model,
        &[Field(4), MapKey(Key::Signed(1))],
        Edit::Clear,
        Error::TypeMismatch,
    );
    rejects(
        &mut model,
        &[Field(2)],
        Edit::ListInsert {
            index: 2,
            values: vec![],
        },
        Error::Index,
    );
    rejects(&mut model, &[Field(2)], Edit::ListRemove(1), Error::Index);
    rejects(
        &mut model,
        &[Field(2)],
        Edit::ListInsert {
            index: 0,
            values: vec![record("ok", &[]), string("wrong")],
        },
        Error::TypeMismatch,
    );
    rejects(
        &mut model,
        &[Field(8), Field(1)],
        Edit::Set(Value::Unsigned(65536)),
        Error::OutOfRange,
    );
    rejects(
        &mut model,
        &[Field(4)],
        Edit::MapInsert {
            key: Key::Unsigned(65536),
            value: string("x"),
        },
        Error::OutOfRange,
    );
    rejects(
        &mut model,
        &[Field(4)],
        Edit::MapRemove(Key::Signed(1)),
        Error::TypeMismatch,
    );
    rejects(
        &mut model,
        &[Field(4)],
        Edit::MapInsert {
            key: Key::Unsigned(1),
            value: Value::Bool(true),
        },
        Error::TypeMismatch,
    );
    rejects(&mut model, &[Field(0)], Edit::Clear, Error::TypeMismatch);
    rejects(
        &mut model,
        &[],
        Edit::StringPush("x".to_owned()),
        Error::TypeMismatch,
    );
    rejects(
        &mut model,
        &[],
        Edit::Set(Value::Struct(vec![])),
        Error::TypeMismatch,
    );
}

#[test]
fn capacity_failure_preserves_the_entire_tree_for_each_growing_operation() {
    let mut model = Model::new(demo_schema(), demo_value()).unwrap();
    let limit = model.encode().unwrap().len();
    for (path, edit) in [
        (vec![Field(5)], Edit::StringPush("x".to_owned())),
        (vec![Field(5)], Edit::Set(string("longer"))),
        (
            vec![Field(2)],
            Edit::ListInsert {
                index: 1,
                values: vec![record("", &[])],
            },
        ),
        (
            vec![Field(4)],
            Edit::MapInsert {
                key: Key::Unsigned(2),
                value: string(""),
            },
        ),
        (
            vec![Field(4)],
            Edit::MapInsert {
                key: Key::Unsigned(1),
                value: string("longer"),
            },
        ),
    ] {
        let original = model.clone();
        assert_eq!(
            model.apply_with_limit(&path, edit, limit),
            Err(Error::Capacity)
        );
        assert_eq!(model, original);
    }
    model
        .apply_with_limit(&[Field(5)], Edit::StringPush("x".to_owned()), limit + 1)
        .unwrap();
    assert_eq!(model.encode().unwrap().len(), limit + 1);
    // Shrinking to the exact allowed size succeeds even if the initial size
    // exceeds the newly selected limit.
    model
        .apply_with_limit(&[Field(5)], Edit::Clear, limit - 2)
        .unwrap();
    assert_eq!(model.encode().unwrap().len(), limit - 2);
}

#[test]
fn root_replacement_and_clear_return_documented_owned_results() {
    let mut model = Model::new(Schema::Str, string("old")).unwrap();
    assert_eq!(
        model.apply(&[], Edit::Set(string("new"))),
        Ok(Some(string("old")))
    );
    assert_eq!(model.apply(&[], Edit::Clear), Ok(None));
    check(&model, "");
    assert!(Model::new(Schema::Bool, Value::Unsigned(0)).is_err());
}
