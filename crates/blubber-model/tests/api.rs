//! Independently written API contract witnesses.

#![forbid(unsafe_code)]

use blubber_model::{
    ApiEdit, ApiReturn, Edit, EntryMode, Error, IntWidth, Key, KeySchema, Model, Patch,
    PathSegment as Segment, Schema, Selector, Value,
};

fn text(value: &str) -> Value {
    Value::Str(value.into())
}
fn set(value: Value) -> Patch {
    Patch {
        path: vec![],
        edit: Edit::Set(value),
    }
}
fn words() -> Model {
    Model::new(
        Schema::List(Box::new(Schema::Str)),
        Value::List(vec![text("a"), text("b"), text("c")]),
    )
    .unwrap()
}
fn entries() -> Model {
    Model::new(
        Schema::Map(KeySchema::Unsigned(IntWidth::W1), Box::new(Schema::Str)),
        Value::Map([(Key::Unsigned(1), text("a"))].into()),
    )
    .unwrap()
}

#[test]
fn callback_error_keeps_the_failing_callbacks_edit() {
    let mut model = words();
    let outcome = model.apply_api(
        &[],
        &ApiEdit::Walk {
            cursor: false,
            patches: vec![set(text("long")), set(text("")), set(text("unused"))],
            fail_at: Some(1),
        },
        100,
        false,
    );
    assert_eq!(outcome.result, Err(Error::Callback));
    assert_eq!(outcome.callbacks.visit, 2);
    assert_eq!(
        model.value(),
        &Value::List(vec![text("long"), text(""), text("c")])
    );
    assert_eq!(
        model.encode().unwrap(),
        b"\x03\0\0\0\x04\0\0\0\x04\0\0\0longc"
    );
    assert!(!outcome.poisoned);
}

#[test]
fn callback_capacity_retains_only_completed_children() {
    let mut model = words();
    let outcome = model.apply_api(
        &[],
        &ApiEdit::Walk {
            cursor: false,
            patches: vec![
                set(text("aa")),
                set(text("will not fit")),
                set(text("unused")),
            ],
            fail_at: None,
        },
        16,
        false,
    );
    assert_eq!(outcome.result, Err(Error::Capacity));
    assert_eq!(outcome.callbacks.visit, 2);
    assert_eq!(
        model.value(),
        &Value::List(vec![text("aa"), text("b"), text("c")])
    );
    assert!(!outcome.poisoned);
}

#[test]
fn exact_count_and_exact_key_rejections_preserve_the_tree() {
    let mut model = entries();
    let before = model.clone();
    for edit in [
        ApiEdit::Rewrite {
            values: vec![],
            keys: None,
        },
        ApiEdit::Rewrite {
            values: vec![text("x")],
            keys: Some(vec![Key::Unsigned(2)]),
        },
    ] {
        assert_eq!(
            model.apply_api(&[], &edit, 100, false).result,
            Err(Error::InvalidBoundary)
        );
        assert_eq!(model, before);
    }
}

#[test]
fn export_preserves_the_short_buffer_and_spare_bytes() {
    let mut model = words();
    let before = model.clone();
    let short = model.apply_api(
        &[],
        &ApiEdit::RemoveInto {
            segment: Segment::Index(0),
            capacity: 0,
        },
        100,
        false,
    );
    assert_eq!(short.result, Err(Error::Capacity));
    assert_eq!(short.destination, Some(vec![]));
    assert_eq!(model, before);
    let copied = model.apply_api(
        &[],
        &ApiEdit::RemoveInto {
            segment: Segment::Index(0),
            capacity: 4,
        },
        100,
        false,
    );
    assert_eq!(copied.result, Ok(ApiReturn::Removed(Some(1))));
    assert_eq!(copied.destination, Some(vec![b'a', 0xa5, 0xa5, 0xa5]));
    assert_eq!(model.value(), &Value::List(vec![text("b"), text("c")]));
}

#[test]
fn absent_selection_skips_callbacks_and_required_selection_fails() {
    for route in [
        Selector::Borrowed,
        Selector::Consuming,
        Selector::With,
        Selector::At,
    ] {
        let mut model = words();
        let before = model.clone();
        let outcome = model.apply_api(
            &[],
            &ApiEdit::Select {
                route,
                segment: Segment::Index(3),
                patch: set(text("unused")),
                fail: true,
            },
            100,
            false,
        );
        assert_eq!(
            outcome.result,
            if route == Selector::At {
                Err(Error::Index)
            } else {
                Ok(ApiReturn::Selected(false))
            }
        );
        assert_eq!(outcome.callbacks.visit, 0);
        assert_eq!(model, before);
    }
}

#[test]
fn occupied_initializer_is_unused_and_returned_editor_is_reused() {
    let mut model = entries();
    let outcome = model.apply_api(
        &[],
        &ApiEdit::Entry {
            key: Key::Unsigned(1),
            mode: EntryMode::InsertWith,
            insert: Value::Bool(true),
            modify: set(text("unused")),
            after: set(text("grown and reused")),
            fail_modify: false,
            consuming: false,
        },
        100,
        false,
    );
    assert_eq!(outcome.result, Ok(ApiReturn::SelectedValue(text("a"))));
    assert_eq!(outcome.callbacks.make, 0);
    assert_eq!(
        model.value(),
        &Value::Map([(Key::Unsigned(1), text("grown and reused"))].into())
    );
}

#[test]
fn occupied_callback_error_skips_initializer_and_reuse() {
    let mut model = entries();
    let outcome = model.apply_api(
        &[],
        &ApiEdit::Entry {
            key: Key::Unsigned(1),
            mode: EntryMode::ModifyInsertWith,
            insert: text("unused"),
            modify: set(text("modified")),
            after: set(text("unused")),
            fail_modify: true,
            consuming: true,
        },
        100,
        false,
    );
    assert_eq!(outcome.result, Err(Error::Callback));
    assert_eq!((outcome.callbacks.modify, outcome.callbacks.make), (1, 0));
    assert_eq!(
        model.value(),
        &Value::Map([(Key::Unsigned(1), text("modified"))].into())
    );
}

#[test]
fn vacancy_calls_initializer_once_and_default_ignores_input() {
    for mode in [
        EntryMode::InsertWith,
        EntryMode::Default,
        EntryMode::ModifyInsertWith,
    ] {
        let mut model = entries();
        // Callback error after selection exposes the inserted value independently.
        let after = Patch {
            path: vec![Segment::Some],
            edit: Edit::Clear,
        };
        let outcome = model.apply_api(
            &[],
            &ApiEdit::Entry {
                key: Key::Unsigned(2),
                mode,
                insert: text("inserted"),
                modify: set(text("unused")),
                after,
                fail_modify: true,
                consuming: false,
            },
            100,
            false,
        );
        assert_eq!(outcome.result, Err(Error::InvalidPath));
        assert_eq!(outcome.callbacks.modify, 0);
        assert_eq!(
            outcome.callbacks.make,
            usize::from(mode != EntryMode::Default)
        );
        assert_eq!(
            model.value(),
            &Value::Map(
                [
                    (Key::Unsigned(1), text("a")),
                    (
                        Key::Unsigned(2),
                        text(if mode == EntryMode::Default {
                            ""
                        } else {
                            "inserted"
                        })
                    )
                ]
                .into()
            )
        );
    }
}

#[test]
fn sticky_poison_precedes_callbacks_and_caller_writes() {
    let mut model = words();
    let before = model.clone();
    let outcome = model.apply_api(
        &[],
        &ApiEdit::RemoveInto {
            segment: Segment::Index(0),
            capacity: 3,
        },
        100,
        true,
    );
    assert_eq!(outcome.result, Err(Error::Poisoned));
    assert_eq!(outcome.destination, Some(vec![0xa5; 3]));
    assert!(outcome.poisoned);
    assert_eq!(model, before);
}
