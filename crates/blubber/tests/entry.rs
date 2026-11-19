//! Borrowed and consuming sorted-map entries with exclusive value projections.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::Resize as _;
use blubber::{Entry, ErrorKind, Map, ReadStorage, SliceStore, Str};
#[cfg(not(feature = "verify-on-load"))]
use blubber::{Error, View};
use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[blubber::schema]
struct Record {
    name: Str,
    tags: blubber::List<u16>,
}

#[blubber::schema(account)]
struct Account {
    fixed: Map<u16, u16>,
    texts: Map<u16, Str>,
    records: Map<u16, Record>,
    tail: Str,
}

fn init() -> AccountInit<'static> {
    AccountInit {
        fixed: &[(1, 10)],
        texts: &[(1, "one")],
        records: &[],
        tail: "suffix",
    }
}

#[test]
fn occupied_fixed_entry_copies_key_and_old_values() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    let mut map = root.fixed_mut();
    match map.entry(1).unwrap() {
        Entry::Occupied(entry) => {
            assert_eq!(entry.key(), 1);
            assert_eq!(entry.insert(20), Ok(10));
        }
        Entry::Vacant(_) => panic!("existing key"),
    }
    match map.entry(1).unwrap() {
        Entry::Occupied(entry) => entry.into_mut().set(30).unwrap(),
        Entry::Vacant(_) => panic!("existing key"),
    }
    match map.entry(1).unwrap() {
        Entry::Occupied(entry) => assert_eq!(entry.remove(), Ok(30)),
        Entry::Vacant(_) => panic!("existing key"),
    }
    assert!(map.is_empty());
    root.validate().unwrap();
}

#[test]
fn vacant_insert_and_or_insert_return_live_value_editors() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    let mut map = root.texts_mut();
    for key in [0, 2, u16::MAX] {
        match map.entry(key).unwrap() {
            Entry::Vacant(entry) => {
                assert_eq!(entry.key(), key);
                let mut text = entry.insert("").unwrap();
                text.push_str("é").unwrap();
                text.push_str(" longer").unwrap();
            }
            Entry::Occupied(_) => panic!("missing key"),
        }
    }
    map.entry(1)
        .unwrap()
        .or_insert("unused")
        .unwrap()
        .push_str("!")
        .unwrap();
    map.entry(3)
        .unwrap()
        .or_insert("new")
        .unwrap()
        .push_str(" value")
        .unwrap();
    assert_eq!(map.get(&1), Some("one!"));
    assert_eq!(map.get(&3), Some("new value"));
    assert_eq!(map.get(&0), Some("é longer"));
    assert_eq!(root.tail(), "suffix");
    root.validate().unwrap();
}

#[test]
fn record_entry_allows_chained_fields_and_retained_nested_list_edits() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    let name = String::from("borrowed");
    let mut record = root
        .records_mut()
        .into_entry(7)
        .unwrap()
        .or_insert(RecordInit {
            name: &name,
            tags: &[1],
        })
        .unwrap();
    record.name_mut().push_str(" suffix").unwrap();
    let mut tags = record.tags_mut();
    tags.push(2).unwrap();
    tags.insert(0, 0).unwrap();
    record.name_mut().set("short").unwrap();
    assert_eq!(record.tags().iter().collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(root.records().get(&7).unwrap().name(), "short");
    let entry = root.records_mut().into_entry(7).unwrap();
    match entry {
        Entry::Occupied(entry) => {
            let _: () = entry
                .insert(RecordInit {
                    name: "",
                    tags: &[],
                })
                .unwrap();
        }
        _ => panic!("occupied"),
    }
    match root.records_mut().into_entry(7).unwrap() {
        Entry::Occupied(entry) => {
            let _: () = entry.remove().unwrap();
        }
        _ => panic!("occupied"),
    }
    root.validate().unwrap();
}

#[blubber::schema(account)]
struct NestedInput {
    maps: Map<u8, Map<u8, Str>>,
}

#[test]
fn occupied_or_insert_does_not_measure_or_validate_unused_init() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let mut root = NestedInput::initialize(
        &mut store,
        NestedInputInit {
            maps: &[(1, &[(2, "old")])],
        },
    )
    .unwrap();
    let invalid = &[(3, "a"), (3, "duplicate")][..];
    let mut child = root
        .maps_mut()
        .into_entry(1)
        .unwrap()
        .or_insert(invalid)
        .unwrap();
    child.insert(4, "new").unwrap();
    assert_eq!(child.get(&2), Some("old"));
    assert!(
        matches!(root.maps_mut().into_entry(9).unwrap().or_insert(invalid), Err(e) if e == ErrorKind::DuplicateKey.into())
    );
    root.validate().unwrap();
}

#[test]
fn vacant_capacity_failure_and_occupied_growth_failure_are_atomic() {
    let used = Account::encoded_len_of(&init()).unwrap() as usize;
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room[..used]);
    Account::initialize(&mut store, init()).unwrap();
    let before = store.bytes().to_vec();
    {
        let mut root = Account::load_mut(&mut store).unwrap();
        assert!(
            matches!(root.texts_mut().into_entry(2).unwrap().or_insert(""), Err(e) if e == ErrorKind::Capacity.into())
        );
        match root.texts_mut().into_entry(1).unwrap() {
            Entry::Occupied(entry) => {
                assert_eq!(entry.insert("longer"), Err(ErrorKind::Capacity.into()))
            }
            _ => panic!("occupied"),
        }
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn corrupt_value_entry_selection_is_lazy_but_old_value_extraction_checks_it() {
    let mut valid = [0; 256];
    Account::initialize(&mut SliceStore::new(&mut valid), init()).unwrap();
    let view = View::load_trusted::<Account>(&valid, Account::DISCRIMINATOR).unwrap();
    let at = view.field(1).unwrap().base() + 10;
    for remove in [false, true] {
        let mut room = valid;
        room[at as usize] = 0xff;
        let before = room;
        let mut store = SliceStore::new(&mut room);
        {
            let mut root = Account::load_trusted_mut(&mut store).unwrap();
            match root.texts_mut().into_entry(1).unwrap() {
                Entry::Occupied(entry) => assert_eq!(
                    if remove {
                        entry.remove()
                    } else {
                        entry.insert("repair")
                    },
                    Err(Error::new(ErrorKind::InvalidUtf8, at))
                ),
                _ => panic!("occupied"),
            }
        }
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
        assert!(Account::load_trusted_mut(&mut store).is_err());
        blubber::raw::backend(&mut store)
            .bytes_mut()
            .copy_from_slice(&valid);
        let mut root = Account::load_checked_mut(&mut store).unwrap();
        root.texts_mut()
            .into_entry(1)
            .unwrap()
            .or_insert("unused")
            .unwrap()
            .set("repair")
            .unwrap();
        root.validate().unwrap();
    }
}

#[test]
fn fixed_vacant_entry_and_missing_selection_preserve_presence_semantics() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    assert!(root.fixed_mut().get_mut(&9).unwrap().is_none());
    assert!(matches!(root.fixed_mut().at_mut(&9), Err(e) if e == ErrorKind::MissingKey.into()));
    match root.fixed_mut().into_entry(9).unwrap() {
        Entry::Vacant(entry) => entry.insert(90).unwrap().set(91).unwrap(),
        Entry::Occupied(_) => panic!("absent key"),
    }
    root.fixed_mut()
        .into_entry(1)
        .unwrap()
        .or_insert(999)
        .unwrap()
        .set(11)
        .unwrap();
    assert_eq!(root.fixed().iter().collect::<Vec<_>>(), [(1, 11), (9, 91)]);
    root.validate().unwrap();
}

#[test]
fn combinators_call_only_the_selected_fnonce_branch_and_return_live_editors() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    for (key, expected) in [(1, 21), (2, 31)] {
        let modified = Cell::new(0);
        let made = Cell::new(0);
        let modify_capture = String::from("consumed");
        let make_capture = Box::new(30);
        let mut value = root
            .fixed_mut()
            .into_entry(key)
            .unwrap()
            .and_modify(|value| {
                drop(modify_capture);
                modified.set(modified.get() + 1);
                value.set(20)
            })
            .unwrap()
            .or_insert_with(|| {
                made.set(made.get() + 1);
                *make_capture
            })
            .unwrap();
        value.set(value.get() + 1).unwrap();
        assert_eq!(value.get(), expected);
        assert_eq!(modified.get(), usize::from(key == 1));
        assert_eq!(made.get(), usize::from(key == 2));
    }
    root.validate().unwrap();
}

#[test]
fn factory_inputs_borrow_callers_and_occupied_initializers_are_never_built() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = NestedInput::initialize(
        &mut store,
        NestedInputInit {
            maps: &[(1, &[(2, "old")])],
        },
    )
    .unwrap();
    let calls = Cell::new(0);
    let invalid = [(3, "a"), (3, "duplicate")];
    root.maps_mut()
        .into_entry(1)
        .unwrap()
        .or_insert_with(|| {
            calls.set(calls.get() + 1);
            &invalid[..]
        })
        .unwrap()
        .insert(4, "new")
        .unwrap();
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        root.maps_mut().into_entry(9).unwrap().or_insert_with(|| {
            calls.set(calls.get() + 1);
            &invalid[..]
        }),
        Err(e) if e == ErrorKind::DuplicateKey.into()
    ));
    assert_eq!(calls.get(), 1);
    let input = String::from("caller input");
    let consumed = Box::new(1);
    let pairs = [(5, input.as_str())];
    let mut child = root
        .maps_mut()
        .into_entry(9)
        .unwrap()
        .or_insert_with(|| {
            drop(consumed);
            &pairs[..]
        })
        .unwrap();
    child.insert(6, "later").unwrap();
    assert_eq!(child.get(&5), Some("caller input"));
    root.validate().unwrap();
}

thread_local! {
    static DEFAULT_CALLS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Copy, blubber::Fixed)]
#[repr(C)]
struct CountedDefault {
    value: u16,
}

impl Default for CountedDefault {
    fn default() -> Self {
        DEFAULT_CALLS.with(|calls| calls.set(calls.get() + 1));
        Self { value: 77 }
    }
}

impl Default for RecordInit<'_> {
    fn default() -> Self {
        DEFAULT_CALLS.with(|calls| calls.set(calls.get() + 1));
        Self {
            name: "",
            tags: &[],
        }
    }
}

#[blubber::schema(account)]
struct Defaults {
    fixed: Map<u16, CountedDefault>,
    texts: Map<u16, Str>,
    lists: Map<u16, blubber::List<u16>>,
    maps: Map<u16, Map<u16, Str>>,
    records: Map<u16, Record>,
}

#[test]
fn defaults_are_initializer_defaults_and_run_once_only_on_vacancy() {
    DEFAULT_CALLS.with(|calls| calls.set(0));
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Defaults::initialize(
        &mut store,
        DefaultsInit {
            fixed: &[(1, CountedDefault { value: 10 })],
            texts: &[(1, "existing")],
            lists: &[],
            maps: &[],
            records: &[(
                1,
                RecordInit {
                    name: "existing",
                    tags: &[],
                },
            )],
        },
    )
    .unwrap();
    assert_eq!(
        root.fixed_mut()
            .into_entry(1)
            .unwrap()
            .or_default()
            .unwrap()
            .get()
            .value,
        10
    );
    DEFAULT_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    assert_eq!(
        root.fixed_mut()
            .into_entry(2)
            .unwrap()
            .or_default()
            .unwrap()
            .get()
            .value,
        77
    );
    DEFAULT_CALLS.with(|calls| assert_eq!(calls.get(), 1));
    root.texts_mut()
        .into_entry(1)
        .unwrap()
        .or_default()
        .unwrap()
        .push_str("!")
        .unwrap();
    root.texts_mut()
        .into_entry(2)
        .unwrap()
        .or_default()
        .unwrap()
        .push_str("new")
        .unwrap();
    root.lists_mut()
        .into_entry(1)
        .unwrap()
        .or_default()
        .unwrap()
        .push(7)
        .unwrap();
    root.maps_mut()
        .into_entry(1)
        .unwrap()
        .or_default()
        .unwrap()
        .insert(2, "two")
        .unwrap();
    assert_eq!(root.texts().get(&1), Some("existing!"));
    assert_eq!(root.texts().get(&2), Some("new"));
    assert_eq!(root.lists().get(&1).unwrap().get(0), Some(7));
    assert_eq!(root.maps().get(&1).unwrap().get(&2), Some("two"));
    assert_eq!(
        root.records_mut()
            .into_entry(1)
            .unwrap()
            .or_default()
            .unwrap()
            .name(),
        "existing"
    );
    DEFAULT_CALLS.with(|calls| assert_eq!(calls.get(), 1));
    let mut record = root
        .records_mut()
        .into_entry(2)
        .unwrap()
        .or_default()
        .unwrap();
    assert_eq!(record.name(), "");
    record.tags_mut().push(8).unwrap();
    DEFAULT_CALLS.with(|calls| assert_eq!(calls.get(), 2));
    root.validate().unwrap();
}

#[test]
fn growing_callbacks_reacquire_entry_and_preserve_keys_suffix_and_spare() {
    let before = [
        (
            0,
            RecordInit {
                name: "first",
                tags: &[0],
            },
        ),
        (
            5,
            RecordInit {
                name: "middle",
                tags: &[5],
            },
        ),
        (
            9,
            RecordInit {
                name: "last",
                tags: &[9],
            },
        ),
    ];
    let mut room = [0xa5; 1024];
    room[..12].fill(0);
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(
        &mut store,
        AccountInit {
            records: &before,
            ..init()
        },
    )
    .unwrap();
    let mut record = root
        .records_mut()
        .into_entry(5)
        .unwrap()
        .and_modify(|record| {
            record.name_mut().push_str(" grows")?;
            record.tags_mut().push(6)
        })
        .unwrap()
        .and_modify(|record| {
            assert_eq!(record.name(), "middle grows");
            record.name_mut().push_str(" again")?;
            record.tags_mut().push(7)
        })
        .unwrap()
        .or_insert_with(|| panic!("occupied initializer called"))
        .unwrap();
    record.name_mut().push_str("!").unwrap();
    record.tags_mut().insert(0, 4).unwrap();
    assert_eq!(
        root.records().iter().map(|(k, _)| k).collect::<Vec<_>>(),
        [0, 5, 9]
    );
    assert_eq!(root.tail(), "suffix");
    root.validate().unwrap();
    assert!(!store.is_poisoned());
    let after = [
        before[0],
        (
            5,
            RecordInit {
                name: "middle grows again!",
                tags: &[4, 5, 6, 7],
            },
        ),
        before[2],
    ];
    let mut expected = [0xa5; 1024];
    expected[..12].fill(0);
    Account::initialize(
        &mut SliceStore::new(&mut expected),
        AccountInit {
            records: &after,
            ..init()
        },
    )
    .unwrap();
    assert_eq!(store.bytes(), expected);
}

#[test]
fn callback_error_and_unwind_keep_completed_edits_without_poison() {
    for unwind in [false, true] {
        let mut room = [0xa5; 256];
        room[..12].fill(0);
        let mut store = SliceStore::new(&mut room);
        let mut root = Account::initialize(&mut store, init()).unwrap();
        let calls = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            root.texts_mut()
                .into_entry(1)
                .unwrap()
                .and_modify(|text| {
                    calls.set(calls.get() + 1);
                    text.push_str(" completed")?;
                    if unwind {
                        panic!("callback unwind after completed edit");
                    }
                    Err(ErrorKind::InvalidPlan.into())
                })
                .map(|_| ())
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err(ErrorKind::InvalidPlan.into()));
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(root.texts().get(&1), Some("one completed"));
        root.texts_mut()
            .into_entry(1)
            .unwrap()
            .and_modify(|text| text.push_str("!"))
            .unwrap();
        assert_eq!(root.tail(), "suffix");
        root.validate().unwrap();
        assert!(!store.is_poisoned());
    }
}

#[test]
fn combinator_capacity_errors_keep_the_prewrite_frame_and_do_not_poison() {
    let used = Account::encoded_len_of(&init()).unwrap() as usize;
    let mut room = vec![0; used];
    let mut store = SliceStore::new(&mut room);
    Account::initialize(&mut store, init()).unwrap();
    let before = store.bytes().to_vec();
    let mut root = Account::load_mut(&mut store).unwrap();
    assert!(
        matches!(root.fixed_mut().into_entry(9).unwrap().or_default(), Err(e) if e == ErrorKind::Capacity.into())
    );
    let calls = Cell::new(0);
    assert!(
        matches!(root.texts_mut().into_entry(9).unwrap().or_insert_with(|| {
        calls.set(calls.get() + 1);
        "new"
    }), Err(e) if e == ErrorKind::Capacity.into())
    );
    assert_eq!(calls.get(), 1);
    assert!(
        matches!(root.texts_mut().into_entry(1).unwrap().and_modify(|text| text.push_str("growth")), Err(e) if e == ErrorKind::Capacity.into())
    );
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn callback_source_errors_poison_before_any_write() {
    let mut room = [0; 256];
    Account::initialize(&mut SliceStore::new(&mut room), init()).unwrap();
    let view = View::load_trusted::<Account>(&room, Account::DISCRIMINATOR).unwrap();
    let at = view.field(1).unwrap().base() + 10;
    room[at as usize] = 0xff;
    let before = room;
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::load_trusted_mut(&mut store).unwrap();
    let calls = Cell::new(0);
    let result = root.texts_mut().into_entry(1).unwrap().and_modify(|text| {
        calls.set(calls.get() + 1);
        text.push_str("growth")
    });
    assert!(matches!(result, Err(e) if e == Error::new(ErrorKind::InvalidUtf8, at)));
    assert_eq!(calls.get(), 1);
    assert_eq!(store.bytes(), before);
    assert!(store.is_poisoned());
}

#[cfg(feature = "alloc")]
#[test]
fn callback_growth_reacquires_the_value_on_a_resizable_backend() {
    let mut bytes = vec![0; 512];
    let mut store = blubber::VecStore::new(&mut bytes);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    assert!(root.shrink_to_fit().unwrap());
    #[cfg(miri)]
    let before = root.texts().get(&1).unwrap().as_ptr();
    let growth = "g".repeat(2048);
    let mut value = root
        .texts_mut()
        .into_entry(1)
        .unwrap()
        .and_modify(|text| {
            text.push_str(&growth)?;
            #[cfg(miri)]
            assert_ne!(text.get().as_ptr(), before);
            Ok(())
        })
        .unwrap()
        .or_insert_with(|| panic!("occupied initializer called"))
        .unwrap();
    value.push_str("!").unwrap();
    assert_eq!(root.texts().get(&1).unwrap(), format!("one{growth}!"));
    assert_eq!(root.tail(), "suffix");
    root.validate().unwrap();
}

#[test]
fn borrowed_map_selectors_reuse_after_growth_and_consuming_children() {
    let mut room = [0; 1024];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    {
        let mut map = root.texts_mut();
        map.get_mut(&1)
            .unwrap()
            .unwrap()
            .push_str(" grows")
            .unwrap();
        assert!(map.get_mut(&99).unwrap().is_none());
        map.entry(2)
            .unwrap()
            .or_insert("two")
            .unwrap()
            .push_str("!")
            .unwrap();
        map.entry(1)
            .unwrap()
            .and_modify(|value| value.push_str(" again"))
            .unwrap();
        assert_eq!(map.get_mut(&1).unwrap().unwrap().get(), "one grows again");
        assert_eq!(map.len(), 2);
    }
    {
        let mut child = root.texts_mut().into_get_mut(&2).unwrap().unwrap();
        child.push_str(" retained").unwrap();
    }
    {
        let mut child = root
            .texts_mut()
            .into_entry(3)
            .unwrap()
            .or_insert("three")
            .unwrap();
        child.push_str("!").unwrap();
    }
    assert_eq!(
        root.texts().iter().collect::<Vec<_>>(),
        [(1, "one grows again"), (2, "two! retained"), (3, "three!")]
    );
    assert_eq!(root.tail(), "suffix");
}

#[test]
fn entry_debug_uses_ordinal_without_decoding_values() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    let mut map = root.texts_mut();
    let entry = map.entry(1).unwrap();
    assert!(format!("{entry:?}").contains("OccupiedEntry"));
    entry.or_insert("").unwrap().set("changed").unwrap();
    let entry = map.entry(2).unwrap();
    assert!(format!("{entry:?}").contains("VacantEntry"));
    entry.or_insert("second").unwrap();
    assert_eq!(map.len(), 2);
}
