//! API command expansion from native trees. No production imports.

#![forbid(unsafe_code)]

use super::oracle::{self, Command, Entropy, Path};
use blubber_model::{
    ApiEdit, Edit, EntryMode, Patch, PathSegment as Segment, Schema, Selector, Value,
};

pub(crate) fn paths(schema: &Schema, value: &Value) -> Vec<Path> {
    oracle::nodes(schema, value)
        .into_iter()
        .filter(|path| {
            matches!(
                oracle::at(schema, value, path).0,
                Schema::List(_) | Schema::Map(_, _) | Schema::Option(_)
            )
        })
        .collect()
}

fn patch(schema: &Schema, value: &Value, salt: u64) -> Patch {
    let nodes = oracle::nodes(schema, value);
    let path = nodes
        .iter()
        .find(|p| {
            matches!(
                oracle::at(schema, value, p).0,
                Schema::Str
                    | Schema::Unsigned(_)
                    | Schema::Signed(_)
                    | Schema::Bool
                    | Schema::Pubkey
                    | Schema::Array(_)
            )
        })
        .cloned()
        .unwrap_or_default();
    let (selected, old) = oracle::at(schema, value, &path);
    let mut entropy = Entropy(salt);
    let edit = match (selected, old) {
        (Schema::List(child), Value::List(values)) => Edit::ListInsert {
            index: values.len(),
            values: vec![oracle::sample(child, &mut entropy, false)],
        },
        (Schema::Map(key, child), _) => Edit::MapInsert {
            key: oracle::key(key, &mut entropy),
            value: oracle::sample(child, &mut entropy, false),
        },
        (Schema::Str, Value::Str(old)) => Edit::Set(Value::Str(format!("{old}é🦀{salt:x}"))),
        _ => Edit::Set(oracle::sample(selected, &mut entropy, false)),
    };
    Patch { path, edit }
}

pub(crate) fn action(schema: &Schema, value: &Value, command: &Command) -> (Path, ApiEdit) {
    let paths = paths(schema, value);
    let path = paths[command.selector as usize % paths.len()].clone();
    let (schema, value) = oracle::at(schema, value, &path);
    (path, at(schema, value, command))
}

pub(crate) fn at(schema: &Schema, value: &Value, command: &Command) -> ApiEdit {
    let mut entropy = Entropy(command.data.iter().fold(command.ordinal as u64, |a, b| {
        a.wrapping_mul(257).wrapping_add(*b as u64)
    }));
    let kind = command.kind % 18;
    let ordinal = command.ordinal as usize;
    match (schema, value) {
        (Schema::Option(child), Value::Option(old)) => {
            let sample = oracle::sample(child, &mut entropy, true);
            ApiEdit::Select {
                route: [Selector::Borrowed, Selector::Consuming, Selector::At][kind as usize % 3],
                segment: Segment::Some,
                patch: patch(child, old.as_deref().unwrap_or(&sample), ordinal as u64),
                fail: false,
            }
        }
        (Schema::List(child), Value::List(values)) => {
            let sample = oracle::sample(child, &mut entropy, true);
            match kind {
                0 => ApiEdit::Fill(sample),
                1..=3 => ApiEdit::Rewrite {
                    values: (0..values.len() + usize::from(kind == 3))
                        .map(|_| oracle::sample(child, &mut entropy, false))
                        .collect(),
                    keys: None,
                },
                4..=7 => ApiEdit::Walk {
                    cursor: kind & 1 == 0,
                    patches: values
                        .iter()
                        .enumerate()
                        .map(|(i, v)| patch(child, v, ordinal as u64 + i as u64))
                        .collect(),
                    fail_at: (kind >= 6 && !values.is_empty()).then(|| ordinal % values.len()),
                },
                8..=13 => {
                    let index = ordinal % (values.len() + 1);
                    ApiEdit::Select {
                        route: [
                            Selector::Borrowed,
                            Selector::Consuming,
                            Selector::At,
                            Selector::With,
                        ][(kind - 8) as usize % 4],
                        segment: Segment::Index(index),
                        patch: patch(child, values.get(index).unwrap_or(&sample), ordinal as u64),
                        fail: command.data.first().is_some_and(|b| b & 1 != 0),
                    }
                }
                _ => {
                    let index = ordinal % (values.len() + 1);
                    let len = values
                        .get(index)
                        .map(|v| child.encode(v).unwrap().len())
                        .unwrap_or(0);
                    ApiEdit::RemoveInto {
                        segment: Segment::Index(index),
                        capacity: match kind % 3 {
                            0 => len.saturating_sub(1),
                            1 => len,
                            _ => len + 7,
                        },
                    }
                }
            }
        }
        (Schema::Map(key_schema, child), Value::Map(values)) => {
            let sample = oracle::sample(child, &mut entropy, true);
            let key = if command.ordinal & 1 == 0 && !values.is_empty() {
                values.keys().nth(ordinal % values.len()).unwrap().clone()
            } else {
                oracle::key(key_schema, &mut entropy)
            };
            match kind {
                0 => ApiEdit::Fill(sample),
                1..=3 => {
                    let keys = if kind == 2 {
                        Some(values.keys().cloned().collect())
                    } else if kind == 3 {
                        let mut keys: Vec<_> = values.keys().cloned().collect();
                        if keys.len() > 1 {
                            keys.swap(0, 1);
                        } else if let Some(k) = keys.first_mut() {
                            let mut replacement = oracle::key(key_schema, &mut entropy);
                            while replacement == *k {
                                replacement = oracle::key(key_schema, &mut entropy);
                            }
                            *k = replacement;
                        }
                        Some(keys)
                    } else {
                        None
                    };
                    ApiEdit::Rewrite {
                        values: (0..values.len())
                            .map(|_| oracle::sample(child, &mut entropy, false))
                            .collect(),
                        keys,
                    }
                }
                4..=7 => ApiEdit::Walk {
                    cursor: kind & 1 == 0,
                    patches: values
                        .values()
                        .enumerate()
                        .map(|(i, v)| patch(child, v, ordinal as u64 + i as u64))
                        .collect(),
                    fail_at: (kind >= 6 && !values.is_empty()).then(|| ordinal % values.len()),
                },
                8..=10 => ApiEdit::Select {
                    route: [Selector::Borrowed, Selector::Consuming, Selector::At]
                        [(kind - 8) as usize],
                    segment: Segment::Key(key.clone()),
                    patch: patch(child, values.get(&key).unwrap_or(&sample), ordinal as u64),
                    fail: false,
                },
                11 => {
                    let len = values
                        .get(&key)
                        .map(|v| child.encode(v).unwrap().len())
                        .unwrap_or(0);
                    ApiEdit::RemoveInto {
                        segment: Segment::Key(key),
                        capacity: if ordinal.is_multiple_of(3) {
                            len.saturating_sub(1)
                        } else {
                            len + ordinal % 3
                        },
                    }
                }
                _ => {
                    let mut mode = [
                        EntryMode::Insert,
                        EntryMode::InsertWith,
                        EntryMode::Default,
                        EntryMode::ModifyInsertWith,
                        EntryMode::MatchInsert,
                        EntryMode::MatchRemove,
                    ][(kind - 12) as usize];
                    // Derived borrowed initializers have no Default implementation.
                    if mode == EntryMode::Default && matches!(child.as_ref(), Schema::Struct(_)) {
                        mode = EntryMode::InsertWith;
                    }
                    let old = values.get(&key).unwrap_or(&sample);
                    ApiEdit::Entry {
                        key,
                        mode,
                        insert: sample.clone(),
                        modify: patch(child, old, ordinal as u64),
                        after: patch(child, old, ordinal as u64 + 1),
                        fail_modify: ordinal.is_multiple_of(3),
                        consuming: command.data.first().is_some_and(|b| b & 1 != 0),
                    }
                }
            }
        }
        _ => panic!("API command requires a container"),
    }
}
