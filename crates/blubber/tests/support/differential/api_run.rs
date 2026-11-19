//! API comparison, including partial progress, callback counts and caller buffers.

#![forbid(unsafe_code)]

use super::{api_oracle, bridge::Bridge, check, matches_error, oracle, Model, Stats};
use blubber::raw::{Edit, Resize};
use blubber::SliceStore;
use blubber_model::{ApiEdit, ApiReturn, Callbacks, PathSegment as Segment};

fn one<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    discriminator: [u8; 8],
    model: &mut Model,
    path: &[Segment],
    edit: &ApiEdit,
    stats: &mut Stats,
) {
    let before = store.bytes().to_vec();
    let old = model.clone();
    let expected = model.apply_api(path, edit, store.growth_limit() - 12, store.is_poisoned());
    let mut calls = Callbacks::default();
    let mut destination = match edit {
        ApiEdit::RemoveInto { capacity, .. } => Some(vec![0xa5; *capacity]),
        _ => None,
    };
    let actual: blubber::Result<ApiReturn> = Edit::load_trusted::<S>(store, discriminator)
        .and_then(|root| {
            S::api(
                blubber::raw::project::<S>(root),
                path,
                edit,
                &mut calls,
                &mut destination,
            )
        });
    match (&expected.result, actual) {
        (Ok(expected), Ok(actual)) => assert_eq!(&actual, expected, "{path:?} {edit:?}"),
        (Err(expected), Err(actual)) => assert!(
            matches_error(*expected, actual),
            "{path:?} {edit:?}: expected {expected:?}, got {actual:?}"
        ),
        (expected, actual) => panic!("{path:?} {edit:?}: expected {expected:?}, got {actual:?}"),
    }
    assert_eq!(
        calls, expected.callbacks,
        "callback counts: {path:?} {edit:?}"
    );
    assert_eq!(
        destination, expected.destination,
        "caller buffer: {path:?} {edit:?}"
    );
    assert_eq!(
        store.is_poisoned(),
        expected.poisoned,
        "poison: {path:?} {edit:?}"
    );
    if !expected.poisoned {
        check::<S>(store, discriminator, model);
        if model == &old {
            assert_eq!(
                &store.bytes()[..before.len()],
                before,
                "no logical progress must preserve bytes"
            );
        }
        let old_used = 12 + old.encode().unwrap().len();
        let new_used = 12 + model.encode().unwrap().len();
        if new_used < old_used {
            assert!(store.bytes()[new_used..old_used].iter().all(|b| *b == 0));
        }
    } else {
        assert_eq!(store.bytes(), before);
    }
    stats.api_edits += 1;
    stats.callback_errors += usize::from(expected.result == Err(blubber_model::Error::Callback));
}

pub(super) fn execute<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    discriminator: [u8; 8],
    model: &mut Model,
    path: &[Segment],
    edit: &ApiEdit,
    stats: &mut Stats,
) {
    let before = store.bytes().to_vec();
    let mut unlimited = model.clone();
    let result = unlimited.apply_api(path, edit, u32::MAX as usize, false);
    if result.result.is_ok() {
        let required = 12 + unlimited.encode().unwrap().len();
        let used = 12 + model.encode().unwrap().len();
        if required > used {
            for capacity in [required - 1, required] {
                let mut bytes = before.clone();
                bytes.resize(capacity, 0xa5);
                let mut expected = model.clone();
                one::<S>(
                    &mut SliceStore::new(&mut bytes),
                    discriminator,
                    &mut expected,
                    path,
                    edit,
                    stats,
                );
                stats.capacity_controls += 1;
            }
        }
    }
    one::<S>(store, discriminator, model, path, edit, stats);
}

pub(super) fn directed<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    discriminator: [u8; 8],
    model: &Model,
    stats: &mut Stats,
) {
    // Isolate each route so destructive operations cannot erase later witnesses.
    for path in api_oracle::paths(model.schema(), model.value()) {
        let (schema, value) = oracle::at(model.schema(), model.value(), &path);
        for kind in 0..18 {
            for ordinal in 0..4 {
                let command = oracle::Command {
                    selector: 0,
                    kind,
                    ordinal,
                    data: vec![(ordinal % 2) as u8],
                };
                let edit = api_oracle::at(schema, value, &command);
                let mut bytes = store.bytes().to_vec();
                // Directed API paths include growth beyond the initial physical allocation.
                bytes.resize(bytes.len() + 1024, 0xa5);
                let mut expected = model.clone();
                execute::<S>(
                    &mut SliceStore::new(&mut bytes),
                    discriminator,
                    &mut expected,
                    &path,
                    &edit,
                    stats,
                );
                // Every API route also sees a pre-existing sticky poison bit.
                let mut poisoned_bytes = store.bytes().to_vec();
                let mut poisoned = SliceStore::new(&mut poisoned_bytes);
                Resize::set_poisoned(&mut poisoned, true);
                one::<S>(
                    &mut poisoned,
                    discriminator,
                    &mut model.clone(),
                    &path,
                    &edit,
                    stats,
                );
                stats.poison_controls += 1;
            }
        }
    }
}

#[blubber::schema(account)]
struct ListSource {
    values: blubber::List<u8>,
}
#[blubber::schema(account)]
struct MapSource {
    values: blubber::Map<u8, u8>,
}

#[test]
fn api_source_admission_preserves_bytes_and_obeys_poison_policy() {
    use blubber::{Error, ErrorKind, ReadStorage as _, Resize as _};
    use blubber_model::{IntWidth, Key, KeySchema, Schema, Value};
    // One variable root field starts at byte 12. Fixed child geometry is exact:
    // count=2 cannot describe a single encoded element/entry. No production
    // resolver or encoder builds these inputs.
    for map in [false, true] {
        let child = Schema::Unsigned(IntWidth::W1);
        let (schema, value, id) = if map {
            (
                Schema::Map(KeySchema::Unsigned(IntWidth::W1), Box::new(child)),
                Value::Map([(Key::Unsigned(1), Value::Unsigned(7))].into()),
                MapSource::DISCRIMINATOR,
            )
        } else {
            (
                Schema::List(Box::new(child)),
                Value::List(vec![Value::Unsigned(7)]),
                ListSource::DISCRIMINATOR,
            )
        };
        let mut bytes = schema.encode_account(id, &value, 128).unwrap();
        bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
        assert!(schema.decode_account(id, &bytes).is_err());
        for operation in 0..10 {
            let mut bytes = bytes.clone();
            let before = bytes.clone();
            let mut destination = [0xa5; 8];
            let mut calls = 0;
            let mut store = SliceStore::new(&mut bytes);
            let result = if map {
                MapSource::load_trusted_mut(&mut store).and_then(|mut root| {
                    let mut values = root.values_mut();
                    match operation {
                        0 => values.fill_values(9),
                        1 => values.rewrite_values(&[9]),
                        2 => values.rewrite_values_from(&[(1, 9)]),
                        3 => values
                            .reborrow()
                            .cursor_mut()
                            .and_then(|mut c| c.next().map(|_| ())),
                        4 => values.try_for_each_mut(|_, _| {
                            calls += 1;
                            Ok(())
                        }),
                        5 => values.get_mut(&1).map(|_| ()),
                        6 => values.into_get_mut(&1).map(|_| ()),
                        7 => values.remove_into(&1, &mut destination).map(|_| ()),
                        8 => values
                            .entry(1)
                            .and_then(|e| {
                                e.or_insert_with(|| {
                                    calls += 1;
                                    9
                                })
                            })
                            .map(|_| ()),
                        _ => values
                            .into_entry(1)
                            .and_then(|e| {
                                e.and_modify(|_| {
                                    calls += 1;
                                    Ok(())
                                })
                            })
                            .map(|_| ()),
                    }
                })
            } else {
                ListSource::load_trusted_mut(&mut store).and_then(|mut root| {
                    let mut values = root.values_mut();
                    match operation {
                        0 => values.fill(9),
                        1 | 2 => values.rewrite_from(&[9]),
                        3 => values
                            .reborrow()
                            .cursor_mut()
                            .and_then(|mut c| c.next().map(|_| ())),
                        4 => values.try_for_each_mut(|_| {
                            calls += 1;
                            Ok(())
                        }),
                        5 => values.reborrow().get_mut(0).map(|_| ()),
                        6 => values.get_mut(0).map(|_| ()),
                        7 => values.remove_into(0, &mut destination).map(|_| ()),
                        8 => values
                            .with_mut(0, |_| {
                                calls += 1;
                                Ok(())
                            })
                            .map(|_| ()),
                        _ => values.at_mut(0).map(|_| ()),
                    }
                })
            };
            assert!(
                matches!(
                    result,
                    Err(Error::Fault {
                        kind: ErrorKind::InvalidBoundary | ErrorKind::Truncated,
                        ..
                    })
                ),
                "map={map} operation={operation}: {result:?}"
            );
            assert_eq!(store.bytes(), before);
            assert_eq!(destination, [0xa5; 8]);
            assert_eq!(calls, 0);
            assert_eq!(
                store.is_poisoned(),
                !cfg!(feature = "verify-on-load"),
                "map={map} operation={operation}"
            );
        }
    }
}
