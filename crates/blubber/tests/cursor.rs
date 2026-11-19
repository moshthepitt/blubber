//! Lending traversal across resizes, corrupt encodings and scoped failures.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "support/record.rs"]
mod records;

#[path = "support/scoped_point.rs"]
mod scoped_point;

blubber::__root_schema!(__ConcreteRoot0, List<Str>);
blubber::__root_schema!(__ConcreteRoot1, Map<CheckedKey, u16>);
blubber::__root_schema!(__ConcreteRoot2, List<List<Interrupt>>);
blubber::__root_schema!(__ConcreteRoot3, Map<u16, List<Interrupt>>);

use blubber::raw::{Edit, Resize};
use blubber::Resize as _;
use blubber::{Error, ErrorKind, List, Map, ReadStorage, Schema, SliceStore, Str, View};
use records::{Record, RecordInit};

#[blubber::schema(account)]
struct Catalog {
    numbers: List<u16>,
    flags: List<bool>,
    records: List<Record>,
    fixed: Map<i16, u16>,
    mapped: Map<u16, Record>,
    tail: Str,
}

fn init() -> CatalogInit<'static> {
    CatalogInit {
        numbers: &[10, 20, 30],
        flags: &[true, false, true],
        records: &[
            RecordInit {
                name: "a",
                tags: &[1],
            },
            RecordInit {
                name: "",
                tags: &[],
            },
            RecordInit {
                name: "last",
                tags: &[3],
            },
        ],
        fixed: &[(i16::MIN, 10), (0, 20), (i16::MAX, 30)],
        mapped: &[
            (
                1,
                RecordInit {
                    name: "first",
                    tags: &[1],
                },
            ),
            (
                256,
                RecordInit {
                    name: "last",
                    tags: &[],
                },
            ),
        ],
        tail: "untouched suffix",
    }
}

#[test]
fn fixed_cursors_lend_copied_scalars_and_keys_in_wire_order() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Catalog::initialize(&mut store, init()).unwrap();
    let mut list = root.numbers_mut();
    let mut cursor = list.reborrow().cursor_mut().unwrap();
    for expected in [10, 20, 30] {
        let mut value = cursor.next().unwrap().unwrap();
        assert_eq!(value.try_get(), Ok(expected));
        value.set(expected + 1).unwrap();
    }
    assert!(cursor.next().unwrap().is_none());
    assert!(cursor.next().unwrap().is_none());
    assert_eq!(list.iter().collect::<Vec<_>>(), [11, 21, 31]);
    let mut cursor = root.fixed_mut().cursor_mut().unwrap();
    for (expected_key, expected_value) in [(i16::MIN, 10), (0, 20), (i16::MAX, 30)] {
        let (key, mut value) = cursor.next().unwrap().unwrap();
        assert_eq!(key, expected_key);
        assert_eq!(value.try_get(), Ok(expected_value));
        value.set(expected_value + 2).unwrap();
    }
    assert!(cursor.next().unwrap().is_none());
    assert!(cursor.next().unwrap().is_none());
    assert_eq!(
        root.fixed().iter().collect::<Vec<_>>(),
        [(i16::MIN, 12), (0, 22), (i16::MAX, 32)]
    );
    root.validate().unwrap();
}

#[test]
fn variable_cursors_resolve_each_child_after_growth_and_compaction() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Catalog::initialize(&mut store, init()).unwrap();
    let mut cursor = root.records_mut().cursor_mut().unwrap();
    for expected in ["a", "", "last"] {
        let mut record = cursor.next().unwrap().unwrap();
        assert_eq!(record.name(), expected);
        record.name_mut().push_str(" 🦀 longer").unwrap();
        record.tags_mut().insert(0, 42).unwrap();
        record.name_mut().set("x").unwrap();
    }
    assert!(cursor.next().unwrap().is_none());
    let mut cursor = root.mapped_mut().cursor_mut().unwrap();
    for (expected_key, expected_name) in [(1, "first"), (256, "last")] {
        let (key, mut record) = cursor.next().unwrap().unwrap();
        assert_eq!((key, record.name()), (expected_key, expected_name));
        record.name_mut().push_str(" growing mapped value").unwrap();
        record.tags_mut().push(u32::from(key)).unwrap();
        record.name_mut().clear().unwrap();
    }
    assert!(cursor.next().unwrap().is_none());
    assert_eq!(root.records().len(), 3);
    assert_eq!(root.mapped().len(), 2);
    assert_eq!(root.records().get(2).unwrap().tags().get(0), Some(42));
    assert_eq!(root.mapped().get(&256).unwrap().tags().get(0), Some(256));
    assert_eq!(root.tail(), "untouched suffix");
    root.validate().unwrap();
}

#[test]
fn scoped_callbacks_edit_generated_records_without_mut_parameter_bindings() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Catalog::initialize(&mut store, init()).unwrap();
    let mut records = root.records_mut();
    records
        .try_for_each_mut(|record| {
            record.name_mut().set("visited")?;
            record.tags_mut().push(9)
        })
        .unwrap();
    records
        .try_for_each_mut(|record| record.name_mut().push_str(" twice"))
        .unwrap();
    assert_eq!(records.get(2).unwrap().name(), "visited twice");
    let mut keys = Vec::new();
    root.mapped_mut()
        .try_for_each_mut(|key, record| {
            keys.push(key);
            record.name_mut().push_str(" visited")?;
            record.tags_mut().push(u32::from(key))
        })
        .unwrap();
    root.numbers_mut()
        .try_for_each_mut(|value| value.set(value.try_get()? + 1))
        .unwrap();
    root.fixed_mut()
        .try_for_each_mut(|_, value| value.set(value.try_get()? + 2))
        .unwrap();
    assert_eq!(keys, [1, 256]);
    assert_eq!(root.mapped().get(&256).unwrap().name(), "last visited");
    assert_eq!(root.numbers().iter().collect::<Vec<_>>(), [11, 21, 31]);
    assert_eq!(root.fixed().get(&0), Some(22));
    root.validate().unwrap();
}

type Deep = Option<Option<Option<List<Map<i16, List<Str>>>>>>;

#[blubber::schema(account)]
struct Nested {
    deep: Deep,
    tail: Str,
}

#[test]
fn nested_callbacks_and_cursors_patch_ancestors_at_the_default_depth_limit() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Nested::initialize(
        &mut store,
        NestedInit {
            deep: Some(Some(Some(&[&[(-1, &["", "last"])], &[(1, &["next"])]]))),
            tail: "suffix",
        },
    )
    .unwrap();
    let mut lists = root
        .deep_mut()
        .some_mut()
        .unwrap()
        .some_mut()
        .unwrap()
        .some_mut()
        .unwrap();
    lists
        .try_for_each_mut(|map| {
            map.try_for_each_mut(|_, strings| {
                strings.try_for_each_mut(|text| text.push_str(" growth"))
            })
        })
        .unwrap();
    let mut outer = lists.cursor_mut().unwrap();
    let map = outer.next().unwrap().unwrap();
    let mut values = map.cursor_mut().unwrap();
    let (_, mut strings) = values.next().unwrap().unwrap();
    let mut text_cursor = strings.reborrow().cursor_mut().unwrap();
    text_cursor.next().unwrap().unwrap().clear().unwrap();
    assert_eq!(
        text_cursor.next().unwrap().unwrap().try_get(),
        Ok("last growth")
    );
    strings.push("inserted inside a lent value").unwrap();
    let mut next_map = outer.next().unwrap().unwrap();
    assert_eq!(next_map.get(&1).unwrap().get(0), Some("next growth"));
    next_map
        .try_for_each_mut(|_, strings| strings.push("tail"))
        .unwrap();
    assert!(outer.next().unwrap().is_none());
    assert_eq!(root.tail(), "suffix");
    root.validate().unwrap();
}

#[test]
fn empty_and_singleton_traversals_are_exhausted_without_extra_callbacks() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Catalog::initialize(&mut store, init()).unwrap();
    root.numbers_mut().clear().unwrap();
    root.fixed_mut().clear().unwrap();
    root.records_mut().truncate(1).unwrap();
    root.mapped_mut().remove(&256).unwrap();
    root.numbers_mut()
        .try_for_each_mut(|_| panic!("empty list"))
        .unwrap();
    root.fixed_mut()
        .try_for_each_mut(|_, _| panic!("empty map"))
        .unwrap();
    let mut empty = root.numbers_mut().cursor_mut().unwrap();
    assert!(empty.next().unwrap().is_none());
    assert!(empty.next().unwrap().is_none());
    let mut empty = root.fixed_mut().cursor_mut().unwrap();
    assert!(empty.next().unwrap().is_none());
    assert!(empty.next().unwrap().is_none());
    let mut visited = 0;
    root.records_mut()
        .try_for_each_mut(|record| {
            visited += 1;
            record.name_mut().set("one")
        })
        .unwrap();
    root.mapped_mut()
        .try_for_each_mut(|_, record| {
            visited += 1;
            record.name_mut().clear()
        })
        .unwrap();
    assert_eq!(visited, 2);
    root.records_mut().clear().unwrap();
    root.mapped_mut().clear().unwrap();
    root.validate().unwrap();
    root.commit().unwrap();

    let before = room;
    let mut store = SliceStore::new(&mut room);
    let mut root = Catalog::load_mut(&mut store).unwrap();
    {
        let mut empty = root.records_mut().cursor_mut().unwrap();
        for _ in 0..3 {
            assert!(empty.next().unwrap().is_none());
        }
    }
    {
        let mut empty = root.mapped_mut().cursor_mut().unwrap();
        for _ in 0..3 {
            assert!(empty.next().unwrap().is_none());
        }
    }
    root.records_mut()
        .try_for_each_mut(|_| panic!("empty indexed list"))
        .unwrap();
    root.mapped_mut()
        .try_for_each_mut(|_, _| panic!("empty indexed map"))
        .unwrap();
    assert_eq!(root.tail(), "untouched suffix");
    root.validate().unwrap();
    root.commit().unwrap();
    assert_eq!(room, before);
}

#[test]
fn callback_errors_keep_prior_and_current_edits_and_skip_remaining_children() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Catalog::initialize(&mut store, init()).unwrap();
    let failure = Error::Program { code: 777 };
    assert_eq!(
        root.numbers_mut().try_for_each_mut(|value| {
            let old = value.try_get()?;
            value.set(old + 1)?;
            if old == 20 {
                return Err(failure);
            }
            Ok(())
        }),
        Err(failure)
    );
    assert_eq!(root.numbers().iter().collect::<Vec<_>>(), [11, 21, 30]);
    let mut visits = 0;
    assert_eq!(
        root.records_mut().try_for_each_mut(|record| {
            visits += 1;
            record.name_mut().set("completed")?;
            if visits == 2 {
                return Err(failure);
            }
            Ok(())
        }),
        Err(failure)
    );
    assert_eq!(visits, 2);
    assert_eq!(root.records().get(0).unwrap().name(), "completed");
    assert_eq!(root.records().get(1).unwrap().name(), "completed");
    assert_eq!(root.records().get(2).unwrap().name(), "last");
    assert_eq!(
        root.fixed_mut().try_for_each_mut(|key, value| {
            value.set(99)?;
            if key == 0 {
                return Err(failure);
            }
            Ok(())
        }),
        Err(failure)
    );
    assert_eq!(
        root.fixed().iter().collect::<Vec<_>>(),
        [(i16::MIN, 99), (0, 99), (i16::MAX, 30)]
    );
    root.validate().unwrap();
}

#[test]
fn failed_child_growth_is_atomic_and_manual_cursor_can_continue() {
    let used = Catalog::encoded_len_of(&init()).unwrap() as usize;
    let mut room = vec![0; used];
    let mut store = SliceStore::new(&mut room);
    Catalog::initialize(&mut store, init()).unwrap();
    let before = store.bytes().to_vec();
    {
        let mut root = Catalog::load_mut(&mut store).unwrap();
        let mut cursor = root.records_mut().cursor_mut().unwrap();
        let mut first = cursor.next().unwrap().unwrap();
        assert_eq!(
            first.name_mut().push_str("growth"),
            Err(ErrorKind::Capacity.into())
        );
        assert_eq!(first.name(), "a");
        assert_eq!(cursor.next().unwrap().unwrap().name(), "");
        assert_eq!(cursor.next().unwrap().unwrap().name(), "last");
        assert!(cursor.next().unwrap().is_none());
        let mut calls = 0;
        assert_eq!(
            root.mapped_mut().try_for_each_mut(|_, record| {
                calls += 1;
                record.name_mut().push_str("growth")
            }),
            Err(ErrorKind::Capacity.into())
        );
        assert_eq!(calls, 1);
        root.validate().unwrap();
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn malformed_selected_list_boundary_is_reported_once_without_writes() {
    let mut room = [0; 64];
    let mut store = SliceStore::new(&mut room);
    const ID: [u8; 8] = *b"listlend";
    Edit::initialize::<__ConcreteRoot0>(&mut store, ID, &["a", "b", "c"]).unwrap();
    store.bytes_mut()[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    let before = store.bytes().to_vec();
    {
        let edit = Edit::load_trusted::<__ConcreteRoot0>(&mut store, ID).unwrap();
        let mut list = blubber::raw::project::<List<Str>>(edit);
        let mut cursor = list.reborrow().cursor_mut().unwrap();
        assert!(matches!(cursor.next(), Err(e) if e == Error::new(ErrorKind::InvalidBoundary, 16)));
        assert!(cursor.next().unwrap().is_none());
        assert!(cursor.next().unwrap().is_none());
        assert_eq!(
            list.try_for_each_mut(|_| panic!("invalid selection")),
            Err(Error::new(ErrorKind::InvalidBoundary, 16))
        );
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn cursor_selection_leaves_corrupt_leaf_domains_available_for_repair() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    Catalog::initialize(&mut store, init()).unwrap();
    let view = View::load_trusted::<Catalog>(store.bytes(), Catalog::DISCRIMINATOR).unwrap();
    let flag = view.field(1).unwrap().base() as usize + 4;
    let mapped_name = view
        .field(4)
        .unwrap()
        .read::<Map<u16, Record>>()
        .unwrap()
        .get(&1)
        .unwrap()
        .name()
        .as_ptr();
    let name = (0..store.storage_len())
        .find(|&at| core::ptr::eq(store.bytes()[at..].as_ptr(), mapped_name))
        .unwrap();
    store.bytes_mut()[flag] = 2;
    store.bytes_mut()[name] = 0xff;
    let mut root = Catalog::load_trusted_mut(&mut store).unwrap();
    let mut cursor = root.flags_mut().cursor_mut().unwrap();
    let mut value = cursor.next().unwrap().unwrap();
    assert_eq!(
        value.try_get(),
        Err(Error::new(ErrorKind::InvalidBool, flag as u32))
    );
    value.set(false).unwrap();
    assert_eq!(cursor.next().unwrap().unwrap().try_get(), Ok(false));
    let mut cursor = root.mapped_mut().cursor_mut().unwrap();
    let (_, mut record) = cursor.next().unwrap().unwrap();
    assert_eq!(
        record.try_name(),
        Err(Error::new(ErrorKind::InvalidUtf8, name as u32))
    );
    record.name_mut().set("repaired").unwrap();
    assert_eq!(cursor.next().unwrap().unwrap().1.name(), "last");
    root.validate().unwrap();
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn callback_read_errors_stop_after_prior_successful_edits() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    Catalog::initialize(&mut store, init()).unwrap();
    let flag = View::load_trusted::<Catalog>(store.bytes(), Catalog::DISCRIMINATOR)
        .unwrap()
        .field(1)
        .unwrap()
        .base() as usize
        + 5;
    store.bytes_mut()[flag] = 2;
    let mut root = Catalog::load_trusted_mut(&mut store).unwrap();
    let mut calls = 0;
    assert_eq!(
        root.flags_mut().try_for_each_mut(|value| {
            calls += 1;
            value.set(!value.try_get()?)
        }),
        Err(Error::new(ErrorKind::InvalidBool, flag as u32))
    );
    assert_eq!(calls, 2);
    assert_eq!(root.flags().get(0), Some(false));
    assert_eq!(root.flags().get(2), Some(true));
    root.flags_mut().at_mut(1).unwrap().set(false).unwrap();
    root.validate().unwrap();
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn malformed_headers_fail_before_callbacks() {
    for (field, relative, replacement) in [(0, 0, u32::MAX), (4, 6, u32::MAX)] {
        let mut room = [0; 512];
        let mut store = SliceStore::new(&mut room);
        Catalog::initialize(&mut store, init()).unwrap();
        let base = View::load_trusted::<Catalog>(store.bytes(), Catalog::DISCRIMINATOR)
            .unwrap()
            .field(field)
            .unwrap()
            .base() as usize;
        store.bytes_mut()[base + relative..base + relative + 4]
            .copy_from_slice(&replacement.to_le_bytes());
        let before = store.bytes().to_vec();
        {
            let mut root = Catalog::load_trusted_mut(&mut store).unwrap();
            let result = match field {
                0 => root
                    .numbers_mut()
                    .try_for_each_mut(|_| panic!("bad header")),
                _ => root.mapped_mut().try_for_each_mut(|_, _| panic!("bad end")),
            };
            assert!(result.is_err());
        }
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
    }
}

#[test]
fn closed_sessions_reject_cursor_construction_and_callbacks() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Catalog::initialize(&mut store, init()).unwrap();
    root.reborrow().commit().unwrap();
    assert!(matches!(root.numbers_mut().cursor_mut(), Err(e) if e == ErrorKind::Closed.into()));
    assert!(matches!(root.fixed_mut().cursor_mut(), Err(e) if e == ErrorKind::Closed.into()));
    assert_eq!(
        root.numbers_mut().try_for_each_mut(|_| panic!("closed")),
        Err(ErrorKind::Closed.into())
    );
    assert_eq!(
        root.fixed_mut().try_for_each_mut(|_, _| panic!("closed")),
        Err(ErrorKind::Closed.into())
    );
}

#[cfg(feature = "alloc")]
#[test]
fn vec_growth_preserves_lending_paths_and_scoped_mutation() {
    let mut bytes = Vec::new();
    let mut store = blubber::VecStore::new(&mut bytes);
    let mut root = Catalog::initialize(&mut store, init()).unwrap();
    root.records_mut()
        .try_for_each_mut(|record| {
            record
                .name_mut()
                .push_str("a longer name requiring growth")?;
            record.tags_mut().push(u32::MAX)
        })
        .unwrap();
    let mut cursor = root.mapped_mut().cursor_mut().unwrap();
    while let Some((key, mut record)) = cursor.next().unwrap() {
        record
            .name_mut()
            .push_str("a longer map value requiring growth")
            .unwrap();
        record.tags_mut().push(u32::from(key)).unwrap();
    }
    root.validate().unwrap();
    let receipt = root.commit().unwrap();
    assert_eq!(receipt.used_len, receipt.storage_len);
    Catalog::load(&bytes).unwrap();
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CheckedKey(u16);

impl blubber::Fixed for CheckedKey {
    const SIZE: u32 = 2;
    type Encoded = [u8; 2];
    fn encode(self) -> Self::Encoded {
        self.0.to_le_bytes()
    }
    fn decode(bytes: &[u8]) -> blubber::Result<Self> {
        u16::decode(bytes).map(Self)
    }
}

impl blubber::MapKey for CheckedKey {
    const KEY_LAYOUT: blubber_core::KeyLayout = <u16 as blubber::MapKey>::KEY_LAYOUT;
    type KeyBytes = [u8; 2];
    fn encode_key(self) -> Self::KeyBytes {
        self.0.to_be_bytes()
    }
    fn decode_key(bytes: &[u8]) -> blubber::Result<Self> {
        let value = <u16 as blubber::MapKey>::decode_key(bytes)?;
        if value == 2 {
            return Err(Error::new(ErrorKind::InvalidBoundary, 0));
        }
        assert_ne!(value, 4, "injected key codec unwind");
        Ok(Self(value))
    }
}

#[test]
fn checked_key_decode_error_is_rebased_once_and_exhausts_the_map_cursor() {
    let mut room = [0; 64];
    let mut store = SliceStore::new(&mut room);
    let edit = Edit::initialize::<__ConcreteRoot1>(
        &mut store,
        *b"keyproof",
        &[
            (CheckedKey(1), 10),
            (CheckedKey(2), 20),
            (CheckedKey(3), 30),
        ],
    )
    .unwrap();
    let map = blubber::raw::project::<Map<CheckedKey, u16>>(edit);
    let mut cursor = map.cursor_mut().unwrap();
    assert_eq!(cursor.next().unwrap().unwrap().1.try_get(), Ok(10));
    assert!(matches!(cursor.next(), Err(e) if e == Error::new(ErrorKind::InvalidBoundary, 20)));
    assert!(cursor.next().unwrap().is_none());
    assert!(cursor.next().unwrap().is_none());
    assert!(!store.is_poisoned());
}

struct Interrupt;

impl Schema for Interrupt {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:cursor:Interrupt",
                epoch: 0,
            },
            wire: <u8 as blubber::Schema>::WIRE_DESCRIPTOR,
        };
    const KIND: blubber::Kind = blubber::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <u8 as Schema>::LAYOUT;
    const __LEN: usize = <u8 as blubber::Schema>::__LEN;
    const __HEADER: &'static [u32] = <u8 as blubber::Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = u8;
    type Mut<'a> = blubber::ScalarMut<'a, u8>;
    type Init<'a> = u8;
    type Removed = u8;
    #[cfg(feature = "alloc")]
    type Owned = u8;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &u8) -> blubber::Result<u32> {
        Self::measure(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &u8, destination: &mut [u8]) {
        Self::encode(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> blubber::Result<u8> {
        Self::read(view)
    }

    fn read<'a>(view: View<'a>) -> blubber::Result<u8> {
        view.read::<u8>()
    }
    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::scalar(blubber::raw::into_edit(edit))
    }
    fn read_removed(view: View<'_>) -> blubber::Result<u8> {
        Self::read(view)
    }
    fn measure(_: &u8) -> blubber::Result<u32> {
        Ok(1)
    }
    fn encode(value: &u8, destination: &mut [u8]) {
        destination[0] = *value;
        assert_ne!(*value, 99, "injected post-move encoder unwind");
    }
}

#[test]
fn encoder_unwind_poison_is_reported_once_by_live_cursors() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let edit =
        Edit::initialize::<__ConcreteRoot2>(&mut store, *b"listboom", &[&[1], &[2]]).unwrap();
    let mut lists = blubber::raw::project::<List<List<Interrupt>>>(edit);
    let mut cursor = lists.reborrow().cursor_mut().unwrap();
    let mut child = cursor.next().unwrap().unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| child.push(99))).is_err());
    assert!(matches!(cursor.next(), Err(e) if e == ErrorKind::Poisoned.into()));
    assert!(cursor.next().unwrap().is_none());
    assert!(cursor.next().unwrap().is_none());
    assert!(matches!(lists.reborrow().cursor_mut(), Err(e) if e == ErrorKind::Poisoned.into()));
    assert_eq!(
        lists.try_for_each_mut(|_| panic!("poisoned")),
        Err(ErrorKind::Poisoned.into())
    );
    assert!(store.is_poisoned());

    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let edit =
        Edit::initialize::<__ConcreteRoot3>(&mut store, *b"mapboom!", &[(1, &[1]), (2, &[2])])
            .unwrap();
    let mut map = blubber::raw::project::<Map<u16, List<Interrupt>>>(edit);
    let mut cursor = map.reborrow().cursor_mut().unwrap();
    let (_, mut child) = cursor.next().unwrap().unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| child.push(99))).is_err());
    assert!(matches!(cursor.next(), Err(e) if e == ErrorKind::Poisoned.into()));
    assert!(cursor.next().unwrap().is_none());
    assert!(cursor.next().unwrap().is_none());
    assert!(matches!(map.reborrow().cursor_mut(), Err(e) if e == ErrorKind::Poisoned.into()));
    assert_eq!(
        map.try_for_each_mut(|_, _| panic!("poisoned")),
        Err(ErrorKind::Poisoned.into())
    );
    assert!(store.is_poisoned());
}

#[blubber::schema]
struct FixedChild {
    flag: bool,
    count: u16,
}

#[blubber::schema]
struct TailInner {
    prefix: u32,
    values: List<FixedChild>,
}

#[blubber::schema(account)]
struct TailOuter {
    prefix: u16,
    inner: TailInner,
}

#[test]
fn fixed_loans_preserve_explicit_types_nested_offsets_and_full_spare_frames() {
    // Every sole-variable projection has an implicit terminal end. The same
    // byte loan must survive generic calls, reborrows and temporary chains.
    fn update(mut list: blubber::ListMut<'_, FixedChild>) -> blubber::Result<()> {
        let mut cursor: blubber::ListCursorMut<'_, FixedChild> = list.reborrow().cursor_mut()?;
        let mut first: FixedChildMut<'_> = cursor.next()?.unwrap();
        let mut field: blubber::ScalarMut<'_, u16> = first.count_mut();
        field.set(0x1234)?;
        assert_eq!(field.try_get()?, 0x1234);
        let mut second: FixedChildMut<'_> = cursor.next()?.unwrap();
        assert!(!second.flag());
        second.flag_mut().set(true)?;
        assert!(cursor.next()?.is_none());
        list.try_for_each_mut(|child| {
            let count = child.count();
            child.count_mut().set(count + 1)
        })
    }
    let mut bytes = [0xa5; 128];
    bytes[..12].fill(0);
    let before;
    {
        let mut store = SliceStore::new(&mut bytes);
        TailOuter::initialize(
            &mut store,
            TailOuterInit {
                prefix: 7,
                inner: TailInnerInit {
                    prefix: 9,
                    values: &[
                        FixedChildInit {
                            flag: true,
                            count: 11,
                        },
                        FixedChildInit {
                            flag: false,
                            count: 22,
                        },
                    ],
                },
            },
        )
        .unwrap();
        // Install nonzero spare after initialization to catch writes outside
        // the selected slots, including any accidental envelope publication.
        let used = u32::from_le_bytes(store.bytes()[8..12].try_into().unwrap()) as usize;
        store.bytes_mut()[used..].fill(0xa5);
        before = store.bytes().to_vec();
        let mut root = TailOuter::load_mut(&mut store).unwrap();
        assert_eq!(root.inner_mut().values_mut().try_len(), Ok(2));
        update(root.inner_mut().values_mut()).unwrap();
        assert_eq!((root.prefix(), root.inner().prefix()), (7, 9));
        assert_eq!(root.inner().values().get(0).unwrap().count(), 0x1235);
        assert_eq!(root.inner().values().get(1).unwrap().count(), 23);
        root.validate().unwrap();
    }
    let mut expected = before;
    // Envelope 12 + outer fixed 2 + inner fixed 4 + list count 4.
    expected[23..25].copy_from_slice(&0x1235u16.to_le_bytes());
    expected[25] = 1;
    expected[26..28].copy_from_slice(&23u16.to_le_bytes());
    assert_eq!(bytes.as_slice(), expected);
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn fixed_domain_errors_rebase_each_slot_once_and_remain_repairable() {
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    TailOuter::initialize(
        &mut store,
        TailOuterInit {
            prefix: 7,
            inner: TailInnerInit {
                prefix: 9,
                values: &[
                    FixedChildInit {
                        flag: true,
                        count: 11,
                    },
                    FixedChildInit {
                        flag: false,
                        count: 22,
                    },
                ],
            },
        },
    )
    .unwrap();
    store.bytes_mut()[25] = 2;
    let mut root = TailOuter::load_trusted_mut(&mut store).unwrap();
    let mut inner = root.inner_mut();
    assert_eq!(
        inner.values_mut().try_get(1).unwrap().unwrap().try_flag(),
        Err(Error::new(ErrorKind::InvalidBool, 25)),
    );
    let mut cursor = inner.values_mut().cursor_mut().unwrap();
    // The lending protocol must also work when the child is forgotten.
    #[allow(clippy::forget_non_drop)]
    std::mem::forget(cursor.next().unwrap().unwrap());
    let mut second: FixedChildMut<'_> = cursor.next().unwrap().unwrap();
    assert_eq!(
        second.try_flag(),
        Err(Error::new(ErrorKind::InvalidBool, 25))
    );
    assert_eq!(second.count(), 22);
    second.flag_mut().set(true).unwrap();
    assert!(cursor.next().unwrap().is_none());
    root.validate().unwrap();
}

#[test]
fn swapping_fixed_children_keeps_each_store_and_account_offset_attached() {
    let mut a = [0; 128];
    let mut b = [0; 128];
    let mut left_store = SliceStore::new(&mut a);
    let mut right_store = SliceStore::new(&mut b);
    let input = TailOuterInit {
        prefix: 7,
        inner: TailInnerInit {
            prefix: 9,
            values: &[FixedChildInit {
                flag: true,
                count: 11,
            }],
        },
    };
    let mut left = TailOuter::initialize(&mut left_store, input).unwrap();
    let mut right = TailOuter::initialize(&mut right_store, input).unwrap();
    let mut left_inner = left.inner_mut();
    let mut right_inner = right.inner_mut();
    let mut lc = left_inner.values_mut().cursor_mut().unwrap();
    let mut rc = right_inner.values_mut().cursor_mut().unwrap();
    let mut x = lc.next().unwrap().unwrap();
    let mut y = rc.next().unwrap().unwrap();
    std::mem::swap(&mut x, &mut y);
    x.count_mut().set(41).unwrap();
    y.count_mut().set(42).unwrap();
    assert!(lc.next().unwrap().is_none());
    assert!(rc.next().unwrap().is_none());
    assert_eq!(left.inner().values().get(0).unwrap().count(), 42);
    assert_eq!(right.inner().values().get(0).unwrap().count(), 41);
    left.validate().unwrap();
    right.validate().unwrap();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WalkCodec {
    value: u8,
    mode: u8,
}
struct WalkEncoding {
    bytes: [u8; 2],
    mode: u8,
    calls: core::cell::Cell<u8>,
}
impl Drop for WalkEncoding {
    fn drop(&mut self) {
        assert_ne!(self.mode, 5, "injected encoded-value destructor unwind");
    }
}
impl AsRef<[u8]> for WalkEncoding {
    fn as_ref(&self) -> &[u8] {
        let call = self.calls.get();
        self.calls.set(call + 1);
        if self.mode == 4 && call > 0 {
            panic!("injected fixed-copy encoder unwind");
        }
        let len = match self.mode {
            1 => 0,
            2 => 2,
            _ => 1,
        };
        &self.bytes[..len]
    }
}
impl blubber::Fixed for WalkCodec {
    const SIZE: u32 = 1;
    type Encoded = WalkEncoding;
    fn decode(bytes: &[u8]) -> blubber::Result<Self> {
        Ok(Self {
            value: <u8 as blubber::Fixed>::decode(bytes)?,
            mode: 0,
        })
    }
    fn encode(self) -> Self::Encoded {
        assert_ne!(self.mode, 3, "injected staged encoder unwind");
        WalkEncoding {
            bytes: [self.value, 0],
            mode: self.mode,
            calls: core::cell::Cell::new(0),
        }
    }
}
impl Schema for WalkCodec {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:cursor:WalkCodec",
                epoch: 0,
            },
            wire: <u8 as blubber::Schema>::WIRE_DESCRIPTOR,
        };
    const KIND: blubber::Kind = blubber::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <u8 as Schema>::LAYOUT;
    const __LEN: usize = <u8 as Schema>::__LEN;
    const __HEADER: &'static [u32] = <u8 as Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = Self;
    type Mut<'a> = blubber::ScalarMut<'a, Self>;
    type Init<'a> = Self;
    type Removed = Self;
    fn read<'a>(view: View<'a>) -> blubber::Result<Self> {
        view.read_fixed::<Self>()
    }
    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::scalar(blubber::raw::into_edit(edit))
    }
    fn read_removed(view: View<'_>) -> blubber::Result<Self> {
        Self::read(view)
    }
    fn measure(_: &Self) -> blubber::Result<u32> {
        Ok(1)
    }
    fn encode(value: &Self, destination: &mut [u8]) {
        destination[0] = value.value;
    }
    #[cfg(feature = "alloc")]
    type Owned = Self;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self) -> blubber::Result<u32> {
        Self::measure(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self, destination: &mut [u8]) {
        Self::encode(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> blubber::Result<Self> {
        Self::read(view)
    }
}
blubber::__root_schema!(__WalkCodecRoot, List<WalkCodec>);

#[test]
fn fixed_setter_rejects_short_long_and_staged_unwind_before_writes() {
    for mode in 1..=3 {
        let mut bytes = [0; 64];
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::initialize::<__WalkCodecRoot>(
            &mut store,
            *b"walkcode",
            &[
                WalkCodec { value: 1, mode: 0 },
                WalkCodec { value: 2, mode: 0 },
            ],
        )
        .unwrap();
        let mut cursor = blubber::raw::project::<List<WalkCodec>>(edit)
            .cursor_mut()
            .unwrap();
        let mut first = cursor.next().unwrap().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            first.set(WalkCodec { value: 9, mode })
        }));
        if mode == 3 {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), Err(ErrorKind::InvalidBoundary.into()));
        }
        assert_eq!(first.get().value, 1);
        cursor
            .next()
            .unwrap()
            .unwrap()
            .set(WalkCodec { value: 3, mode: 0 })
            .unwrap();
        assert!(cursor.next().unwrap().is_none());
        assert!(!store.is_poisoned());
        assert_eq!(&store.bytes()[16..18], &[1, 3]);
    }
}

#[test]
fn fixed_copy_unwind_stays_poisoned_and_terminal_commit_closes_storage() {
    for mode in [4, 5] {
        let mut bytes = [0; 64];
        let mut store = SliceStore::new(&mut bytes);
        let mut root = Edit::initialize::<__WalkCodecRoot>(
            &mut store,
            *b"walkcode",
            &[
                WalkCodec { value: 1, mode: 0 },
                WalkCodec { value: 2, mode: 0 },
            ],
        )
        .unwrap();
        {
            let mut cursor = blubber::raw::project::<List<WalkCodec>>(root.reborrow())
                .cursor_mut()
                .unwrap();
            let mut first = cursor.next().unwrap().unwrap();
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || first.set(WalkCodec { value: 9, mode })
            ))
            .is_err());
            assert_eq!(first.try_get(), Err(ErrorKind::Poisoned.into()));
            assert!(matches!(cursor.next(), Err(error) if error == ErrorKind::Poisoned.into()));
            assert!(cursor.next().unwrap().is_none());
        }
        assert!(
            matches!(blubber::raw::project::<List<WalkCodec>>(root.reborrow()).cursor_mut(), Err(error) if error == ErrorKind::Poisoned.into())
        );
        assert_eq!(root.commit(), Err(ErrorKind::Poisoned.into()));
        assert!(store.is_closed());
        assert!(store.is_poisoned());
    }
}

blubber::__root_schema!(__WalkFlags, List<bool>);
blubber::__root_schema!(__WalkNestedFlags, List<List<bool>>);

#[test]
fn fixed_callback_histories_match_an_independent_nested_byte_oracle() {
    // Variable outer paths exercise the checked fallback before each fixed loan.
    // The oracle writes count/end words itself and compares all physical spare.
    for count in 0..=4usize {
        for stop in 0..=count {
            let mut bytes = [0; 96];
            let mut store = SliceStore::new(&mut bytes);
            let values = [true, false, true, false];
            let children = [&values[..count], &values[..count]];
            let mut root =
                Edit::initialize::<__WalkNestedFlags>(&mut store, *b"walknest", &children).unwrap();
            let mut outer = blubber::raw::project::<List<List<bool>>>(root.reborrow());
            let mut seen = 0;
            let failure = Error::new(ErrorKind::InvalidBoundary, Error::NO_OFFSET);
            let result = outer.try_for_each_mut(|list| {
                let mut ordinal = 0;
                list.try_for_each_mut(|value| {
                    value.set(!value.get())?;
                    seen += 1;
                    let at = ordinal;
                    ordinal += 1;
                    if at == stop {
                        Err(failure)
                    } else {
                        Ok(())
                    }
                })
            });
            let failed = stop < count;
            assert_eq!(result, if failed { Err(failure) } else { Ok(()) });
            assert_eq!(seen, if failed { stop + 1 } else { 2 * count });
            root.validate().unwrap();
            let mut expected = [0u8; 96];
            expected[..8].copy_from_slice(b"walknest");
            let child_len = 4 + count;
            let used = 20 + 2 * child_len;
            expected[8..12].copy_from_slice(&(used as u32).to_le_bytes());
            expected[12..16].copy_from_slice(&2u32.to_le_bytes());
            expected[16..20].copy_from_slice(&(child_len as u32).to_le_bytes());
            for child in 0..2 {
                let at = 20 + child * child_len;
                expected[at..at + 4].copy_from_slice(&(count as u32).to_le_bytes());
                for i in 0..count {
                    let edited = !failed || (child == 0 && i <= stop);
                    expected[at + 4 + i] = u8::from(values[i] ^ edited);
                }
            }
            assert_eq!(store.bytes(), expected);
        }
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn fixed_scalar_slot_errors_use_current_root_offset() {
    let mut bytes = [0; 64];
    let mut store = SliceStore::new(&mut bytes);
    Edit::initialize::<__WalkFlags>(&mut store, *b"walkbool", &[true, false]).unwrap();
    store.bytes_mut()[17] = 2;
    let edit = Edit::load_trusted::<__WalkFlags>(&mut store, *b"walkbool").unwrap();
    let mut cursor = blubber::raw::project::<List<bool>>(edit)
        .cursor_mut()
        .unwrap();
    assert!(cursor.next().unwrap().unwrap().get());
    assert_eq!(
        cursor.next().unwrap().unwrap().try_get(),
        Err(Error::new(ErrorKind::InvalidBool, 17))
    );
    assert!(cursor.next().unwrap().is_none());
}

#[path = "fixtures/walk_contract.rs"]
mod source_contract;

#[test]
fn identical_fixed_walk_source_compiles_and_runs_on_host_and_sbf() {
    use source_contract::{WalkRowInit, WalkRows, WalkRowsInit};
    for active in [false, true] {
        let mut bytes = [0; 96];
        let before;
        {
            let mut store = SliceStore::new(&mut bytes);
            WalkRows::initialize(
                &mut store,
                WalkRowsInit {
                    value: &[WalkRowInit { value: 7 }, WalkRowInit { value: 9 }],
                },
            )
            .unwrap();
            before = store.bytes().to_vec();
            assert_eq!(
                source_contract::exercise(&mut store, WalkRows::DISCRIMINATOR, active, |_| {})
                    .unwrap(),
                if active { 16 } else { 0 }
            );
            assert!(store.is_closed());
        }
        assert_eq!(bytes.as_slice(), before);
    }
}

#[test]
fn fixed_root_views_use_the_account_payload_origin() {
    let mut bytes = [0; 40];
    let mut store = SliceStore::new(&mut bytes);
    let mut root = Edit::initialize::<u64>(&mut store, *b"walkroot", 7).unwrap();
    assert_eq!(root.view().unwrap().base(), 12);
    assert_eq!(root.view().unwrap().read_fixed::<u64>().unwrap(), 7);
    let mut value = blubber::raw::project::<u64>(root.reborrow());
    value.set(9).unwrap();
    assert_eq!(root.view().unwrap().read_fixed::<u64>().unwrap(), 9);
}

#[blubber::schema]
struct VariableWalkValue {
    pub prefix: u64,
    pub suffix: Str,
}
blubber::__root_schema!(__VariableWalkList, List<VariableWalkValue>);

#[test]
fn fixed_walks_reject_wrong_kinds_and_element_widths_before_callbacks() {
    fn reject(mut root: Edit<'_>) {
        assert!(
            matches!(blubber::raw::project::<List<u64>>(root.reborrow()).get_mut(0),
            Err(error) if error == ErrorKind::InvalidSchema.into())
        );
        assert!(
            matches!(blubber::raw::project::<List<u64>>(root.reborrow()).cursor_mut(), Err(error) if error == ErrorKind::InvalidSchema.into())
        );
        let mut calls = 0;
        let result = blubber::raw::project::<List<u64>>(root.reborrow()).try_for_each_mut(|_| {
            calls += 1;
            Ok(())
        });
        assert_eq!(result, Err(ErrorKind::InvalidSchema.into()));
        assert_eq!(calls, 0);
        root.validate().unwrap();
    }
    let mut bytes = [0; 80];
    let mut store = SliceStore::new(&mut bytes);
    reject(Edit::initialize::<Str>(&mut store, *b"walktext", "abc").unwrap());
    let mut bytes = [0; 80];
    let mut store = SliceStore::new(&mut bytes);
    reject(Edit::initialize::<__WalkFlags>(&mut store, *b"walkflag", &[true]).unwrap());
    let mut bytes = [0; 80];
    let mut store = SliceStore::new(&mut bytes);
    reject(
        Edit::initialize::<__VariableWalkList>(
            &mut store,
            *b"walkvary",
            &[VariableWalkValueInit {
                prefix: 7,
                suffix: "",
            }],
        )
        .unwrap(),
    );
}

// A handwritten schema may expose Edit directly as its lending projection.
// This exercises account metadata through the same public hook as codecs do.
struct MetadataByte;
impl Schema for MetadataByte {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:cursor:MetadataByte",
                epoch: 0,
            },
            wire: <u8 as blubber::Schema>::WIRE_DESCRIPTOR,
        };
    const KIND: blubber::Kind = blubber::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <u8 as Schema>::LAYOUT;
    const __LEN: usize = <u8 as Schema>::__LEN;
    const __HEADER: &'static [u32] = <u8 as Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = u8;
    type Mut<'a> = Edit<'a>;
    type Init<'a> = u8;
    type Removed = u8;
    fn read(view: View<'_>) -> blubber::Result<u8> {
        <u8 as Schema>::read(view)
    }
    fn project(edit: blubber::__private::Projection<'_, Self>) -> Edit<'_> {
        blubber::raw::into_edit(edit)
    }
    fn read_removed(view: View<'_>) -> blubber::Result<u8> {
        Self::read(view)
    }
    fn measure(value: &u8) -> blubber::Result<u32> {
        <u8 as Schema>::measure(value)
    }
    fn encode(value: &u8, destination: &mut [u8]) {
        <u8 as Schema>::encode(value, destination);
    }
    #[cfg(feature = "alloc")]
    type Owned = u8;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &u8) -> blubber::Result<u32> {
        Self::measure(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &u8, destination: &mut [u8]) {
        Self::encode(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> blubber::Result<u8> {
        Self::read(view)
    }
}
blubber::__root_schema!(__MetadataWalk, List<List<MetadataByte>>);

#[test]
fn nested_fixed_loans_report_root_used_and_physical_lengths() {
    let mut bytes = [0; 64];
    let mut store = SliceStore::new(&mut bytes);
    let mut root =
        Edit::initialize::<__MetadataWalk>(&mut store, *b"walkmeta", &[&[1, 2]]).unwrap();
    assert_eq!(root.used_len().unwrap(), 22);
    let mut outer = blubber::raw::project::<List<List<MetadataByte>>>(root.reborrow())
        .cursor_mut()
        .unwrap();
    let mut inner = outer.next().unwrap().unwrap().cursor_mut().unwrap();
    for (value, base) in [(1, 20), (2, 21)] {
        let slot: Edit<'_> = inner.next().unwrap().unwrap();
        assert_eq!(slot.used_len().unwrap(), 22);
        assert_eq!(slot.storage_len().unwrap(), 64);
        let view = slot.view().unwrap();
        assert_eq!(view.base(), base);
        assert_eq!(view.read_fixed::<u8>().unwrap(), value);
    }
    assert!(inner.next().unwrap().is_none());
    let mut nested = blubber::raw::project::<List<List<MetadataByte>>>(root.reborrow())
        .at_mut(0)
        .unwrap();
    let slot: Edit<'_> = nested.reborrow().at_mut(1).unwrap();
    assert_eq!(slot.used_len().unwrap(), 22);
    assert_eq!(slot.storage_len().unwrap(), 64);
    assert_eq!(slot.view().unwrap().base(), 21);
    assert_eq!(slot.view().unwrap().read_fixed::<u8>(), Ok(2));
}

#[blubber::schema]
struct DropCodecRecord {
    pub value: WalkCodec,
}
#[blubber::schema(account)]
struct DropCodecAccount {
    pub value: WalkCodec,
    pub records: List<DropCodecRecord>,
}
#[test]
fn fixed_encoding_drop_unwind_stays_poisoned_for_account_and_region_selections() {
    for region in 0..3 {
        let mut bytes = [0; 96];
        let mut store = SliceStore::new(&mut bytes);
        let mut root = DropCodecAccount::initialize(
            &mut store,
            DropCodecAccountInit {
                value: WalkCodec { value: 1, mode: 0 },
                records: &[
                    DropCodecRecordInit {
                        value: WalkCodec { value: 1, mode: 0 },
                    },
                    DropCodecRecordInit {
                        value: WalkCodec { value: 2, mode: 0 },
                    },
                ],
            },
        )
        .unwrap();
        if region == 1 {
            let mut cursor = root.records_mut().cursor_mut().unwrap();
            let mut first = cursor.next().unwrap().unwrap();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                first.value_mut().set(WalkCodec { value: 9, mode: 5 })
            }));
            assert!(panic.is_err());
            assert_eq!(first.try_value(), Err(ErrorKind::Poisoned.into()));
            assert!(matches!(cursor.next(), Err(error) if error == ErrorKind::Poisoned.into()));
        } else if region == 2 {
            let mut record = root.records_mut().at_mut(0).unwrap();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                record.value_mut().set(WalkCodec { value: 9, mode: 5 })
            }));
            assert!(panic.is_err());
            assert_eq!(record.try_value(), Err(ErrorKind::Poisoned.into()));
        } else {
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                root.value_mut().set(WalkCodec { value: 9, mode: 5 })
            }));
            assert!(panic.is_err());
        }
        assert_eq!(root.try_value(), Err(ErrorKind::Poisoned.into()));
        assert_eq!(root.commit(), Err(ErrorKind::Poisoned.into()));
        assert!(store.is_poisoned());
        assert!(store.is_closed());
    }
}

#[test]
fn manual_fixed_loans_preserve_disjoint_store_frames() {
    let mut left = [0; 512];
    let mut right = [0; 512];
    let mut left_store = SliceStore::new(&mut left);
    let mut right_store = SliceStore::new(&mut right);
    Catalog::initialize(&mut left_store, init()).unwrap();
    Catalog::initialize(&mut right_store, init()).unwrap();
    let used = u32::from_le_bytes(left_store.bytes()[8..12].try_into().unwrap()) as usize;
    left_store.bytes_mut()[used..].fill(0xa5);
    right_store.bytes_mut()[used..].fill(0x5a);
    let mut left_expected = left_store.bytes().to_vec();
    let mut right_expected = right_store.bytes().to_vec();
    // Six variable fields use five ends followed by the payload at byte 32.
    let map_start = 32 + u32::from_le_bytes(right_expected[20..24].try_into().unwrap()) as usize;
    {
        let mut left_root = Catalog::load_mut(&mut left_store).unwrap();
        let mut right_root = Catalog::load_mut(&mut right_store).unwrap();
        let mut list_value = left_root.numbers_mut().at_mut(1).unwrap();
        let mut map_value = right_root.fixed_mut().at_mut(&0).unwrap();
        core::mem::swap(&mut list_value, &mut map_value);
        list_value.set(77).unwrap();
        map_value.set(99).unwrap();
        assert_eq!(list_value.try_get(), Ok(77));
        assert_eq!(map_value.try_get(), Ok(99));
        assert!(left_root.numbers_mut().get_mut(3).unwrap().is_none());
        assert!(right_root.fixed_mut().get_mut(&1).unwrap().is_none());
        assert_eq!(left_root.used_len() as usize, used);
        assert_eq!(right_root.storage_len(), 512);
        left_root.validate().unwrap();
        right_root.validate().unwrap();
    }
    left_expected[38..40].copy_from_slice(&99u16.to_le_bytes());
    right_expected[map_start + 10..map_start + 12].copy_from_slice(&77u16.to_le_bytes());
    assert_eq!(left_store.bytes(), left_expected);
    assert_eq!(right_store.bytes(), right_expected);
}

#[blubber::schema]
struct GenericProjection<T> {
    pub payload: T,
    pub count: u16,
}
#[blubber::schema(account)]
struct GenericProjections {
    pub rows: List<GenericProjection<Str>>,
    pub suffix: Str,
}

#[blubber::schema(account)]
struct FixedProjections {
    rows: List<GenericProjection<FixedChild>>,
    suffix: Str,
}

#[test]
fn fixed_generated_nested_histories_preserve_partial_progress_and_all_frames() {
    for count in 0..=4 {
        for stop in 0..=count {
            let inputs: Vec<_> = (0..count)
                .map(|i| GenericProjectionInit {
                    payload: FixedChildInit {
                        flag: i % 2 == 0,
                        count: 10 + i as u16,
                    },
                    count: 20 + i as u16,
                })
                .collect();
            let mut bytes = [0; 96];
            let mut store = SliceStore::new(&mut bytes);
            FixedProjections::initialize(
                &mut store,
                FixedProjectionsInit {
                    rows: &inputs,
                    suffix: "tail",
                },
            )
            .unwrap();
            let used = 24 + 5 * count;
            store.bytes_mut()[used..].fill(0xa5);
            let mut expected = store.bytes().to_vec();
            let mut root = FixedProjections::load_mut(&mut store).unwrap();
            let mut visits = 0;
            let failure = Error::Program { code: 701 };
            let result = root.rows_mut().try_for_each_mut(|row| {
                let mut payload = row.payload_mut();
                let mut flag = payload.flag_mut();
                flag.set(!flag.get())?;
                payload.count_mut().set(90 + visits as u16)?;
                row.count_mut().set(100 + visits as u16)?;
                let fail = visits == stop;
                visits += 1;
                if fail {
                    Err(failure)
                } else {
                    Ok(())
                }
            });
            assert_eq!(result, if stop < count { Err(failure) } else { Ok(()) });
            assert_eq!(visits, if stop < count { stop + 1 } else { count });
            for (i, input) in inputs.iter().enumerate().take(visits) {
                let base = 20 + i * 5;
                expected[base] = u8::from(!input.payload.flag);
                expected[base + 1..base + 3].copy_from_slice(&(90 + i as u16).to_le_bytes());
                expected[base + 3..base + 5].copy_from_slice(&(100 + i as u16).to_le_bytes());
            }
            assert_eq!(root.suffix(), "tail");
            root.validate().unwrap();
            assert_eq!(store.bytes(), expected);
            assert!(!store.is_poisoned());
        }
    }
}

#[test]
fn generated_fixed_slot_benchmark_source_has_independent_host_oracles() {
    use source_contract::{WalkRowInit, WalkRows, WalkRowsInit};
    for count in 0..=4 {
        for op in 55..=61 {
            let inputs: Vec<_> = (0..count).map(|_| WalkRowInit { value: 17 }).collect();
            let mut bytes = [0; 80];
            let mut store = SliceStore::new(&mut bytes);
            WalkRows::initialize(&mut store, WalkRowsInit { value: &inputs }).unwrap();
            let mut expected = store.bytes().to_vec();
            assert_eq!(
                source_contract::slots(&mut store, WalkRows::DISCRIMINATOR, op, |_| {}).unwrap(),
                if matches!(op, 57 | 60) {
                    17 * count as u64
                } else {
                    0
                }
            );
            if matches!(op, 58 | 61) {
                for i in 0..count {
                    expected[16 + i * 8..24 + i * 8].copy_from_slice(&123u64.to_le_bytes());
                }
            }
            assert!(!store.is_poisoned());
            assert!(store.is_closed());
            assert_eq!(bytes.as_slice(), expected);
        }
    }
}

#[test]
fn consuming_and_generic_projections_keep_their_checked_paths() {
    let mut bytes = [0; 512];
    let mut store = SliceStore::new(&mut bytes);
    Catalog::initialize(&mut store, init()).unwrap();
    {
        let table = <Catalog as blubber::RootSchema>::__layout();
        let edit = Edit::load::<Catalog>(&mut store, Catalog::DISCRIMINATOR)
            .unwrap()
            .field_certified(0, None, table.child(0, true));
        let mut numbers = blubber::raw::project::<List<u16>>(edit);
        assert_eq!(numbers.try_len(), Ok(3));
        numbers.reborrow().at_mut(2).unwrap().set(44).unwrap();
    }
    {
        let edit = Edit::load::<Catalog>(&mut store, Catalog::DISCRIMINATOR)
            .unwrap()
            .field_static(1, None, 1);
        let mut flags = blubber::raw::project::<List<bool>>(edit);
        assert_eq!(flags.try_len(), Ok(3));
        flags.reborrow().at_mut(1).unwrap().set(true).unwrap();
    }
    let root = Catalog::load(store.bytes()).unwrap();
    assert_eq!(root.numbers().get(2), Some(44));
    assert_eq!(root.flags().get(1), Some(true));
    root.validate().unwrap();

    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    let mut root = GenericProjections::initialize(
        &mut store,
        GenericProjectionsInit {
            rows: &[
                GenericProjectionInit {
                    payload: "a",
                    count: 1,
                },
                GenericProjectionInit {
                    payload: "last",
                    count: 2,
                },
            ],
            suffix: "preserved",
        },
    )
    .unwrap();
    root.rows_mut()
        .at_mut(1)
        .unwrap()
        .payload_mut()
        .set("longer value")
        .unwrap();
    root.rows_mut()
        .at_mut(1)
        .unwrap()
        .count_mut()
        .set(33)
        .unwrap();
    let row = root.rows().get(1).unwrap();
    assert_eq!(row.payload(), "longer value");
    assert_eq!(row.count(), 33);
    assert_eq!(root.suffix(), "preserved");
    root.validate().unwrap();
}

// A manual projection exposes the exact handle while retaining a generated
// schema's certificate. Hidden hint APIs must still select inherited fields.
struct ExposedFixed<T>(core::marker::PhantomData<T>);
impl<T: Schema> Schema for ExposedFixed<T> {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:cursor:ExposedFixed<T>",
                epoch: 0,
            },
            wire: T::WIRE_DESCRIPTOR,
        };
    const KIND: blubber::Kind = T::KIND;
    const FIXED_SIZE: Option<u32> = T::FIXED_SIZE;
    const MIN_SIZE: u32 = T::MIN_SIZE;
    const LAYOUT: &'static blubber_core::Layout = T::LAYOUT;
    const __LEN: usize = T::__LEN;
    const __HEADER: &'static [u32] = T::__HEADER;
    type __Children = T::__Children;
    type Read<'a> = T::Read<'a>;
    type Mut<'a> = Edit<'a>;
    type Init<'a> = T::Init<'a>;
    type Removed = T::Removed;
    fn read<'a>(view: View<'a>) -> blubber::Result<Self::Read<'a>> {
        T::read(view)
    }
    fn project(edit: blubber::__private::Projection<'_, Self>) -> Edit<'_> {
        blubber::raw::into_edit(edit)
    }
    fn read_removed(view: View<'_>) -> blubber::Result<Self::Removed> {
        T::read_removed(view)
    }
    fn measure(value: &Self::Init<'_>) -> blubber::Result<u32> {
        T::measure(value)
    }
    fn encode(value: &Self::Init<'_>, destination: &mut [u8]) {
        T::encode(value, destination);
    }
    #[cfg(feature = "alloc")]
    type Owned = T::Owned;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self::Owned) -> blubber::Result<u32> {
        T::measure_owned(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self::Owned, destination: &mut [u8]) {
        T::encode_owned(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> blubber::Result<Self::Owned> {
        T::decode_owned(view)
    }
}
blubber::__root_schema!(__ExposedFixed, List<ExposedFixed<FixedChild>>);

#[test]
fn fixed_slot_hints_retain_inherited_identity_offsets_and_checked_fallbacks() {
    for (index, offset) in [(1, 1), (0, 0), (1, 0), (u16::MAX, u32::MAX)] {
        let mut bytes = [0; 64];
        let mut store = SliceStore::new(&mut bytes);
        let root = Edit::initialize::<__ExposedFixed>(
            &mut store,
            *b"slotpair",
            &[
                FixedChildInit {
                    flag: true,
                    count: 0x1234,
                },
                FixedChildInit {
                    flag: false,
                    count: 0x5678,
                },
            ],
        )
        .unwrap();
        let mut cursor = blubber::raw::project::<List<ExposedFixed<FixedChild>>>(root)
            .cursor_mut()
            .unwrap();
        let mut first = cursor.next().unwrap().unwrap();
        // The supplied certificate is deliberately foreign. Exact loans use
        // their actual child certificate, including its original depth budget.
        let foreign = <__WalkFlags as blubber::RootSchema>::__layout();
        let mut child = first.field_reborrow_fixed(1, index, offset, foreign);
        assert_eq!(child.view().unwrap().base(), 17);
        assert_eq!(child.view().unwrap().read_fixed::<u16>(), Ok(0x1234));
        assert_eq!(child.used_len(), Ok(22));
        assert_eq!(child.storage_len(), Ok(64));
        blubber::raw::project::<u16>(child.reborrow())
            .set(77)
            .unwrap();
        assert_eq!(child.view().unwrap().read_fixed::<u16>(), Ok(77));
        let second = cursor.next().unwrap().unwrap();
        assert_eq!(
            second.view().unwrap().field(1).unwrap().read_fixed::<u16>(),
            Ok(0x5678)
        );
        assert!(cursor.next().unwrap().is_none());
        assert_eq!(&store.bytes()[16..22], &[1, 77, 0, 0, 0x78, 0x56]);
        assert!(!store.is_poisoned());
    }
}

// Deliberately short encoding: a mismatched typed handle must reject SIZE,
// even when this faulty codec happens to match the destination byte length.
#[derive(Clone, Copy)]
struct ShortWord(u32);
impl blubber::Fixed for ShortWord {
    const SIZE: u32 = 4;
    type Encoded = [u8; 3];
    fn encode(self) -> Self::Encoded {
        let bytes = self.0.to_le_bytes();
        [bytes[0], bytes[1], bytes[2]]
    }
    fn decode(bytes: &[u8]) -> blubber::Result<Self> {
        <u32 as blubber::Fixed>::decode(bytes).map(Self)
    }
}

#[test]
fn fixed_slot_exact_width_rejects_short_codecs_from_wrong_typed_handles() {
    for cursor in [false, true] {
        let mut bytes = [0; 64];
        let mut store = SliceStore::new(&mut bytes);
        Edit::initialize::<__ExposedFixed>(
            &mut store,
            *b"slotpair",
            &[FixedChildInit {
                flag: true,
                count: 0x1234,
            }],
        )
        .unwrap();
        let before = store.bytes().to_vec();
        {
            let edit = Edit::load::<__ExposedFixed>(&mut store, *b"slotpair").unwrap();
            let list = blubber::raw::project::<List<ExposedFixed<FixedChild>>>(edit);
            fn reject(edit: Edit<'_>) {
                let mut value = blubber::raw::scalar::<ShortWord>(edit);
                assert_eq!(
                    value.try_get().map(|word| word.0),
                    Err(Error::new(ErrorKind::Truncated, 19))
                );
                assert_eq!(
                    value.set(ShortWord(0x0909_0909)),
                    Err(ErrorKind::InvalidBoundary.into())
                );
            }
            if cursor {
                let mut walk = list.cursor_mut().unwrap();
                reject(walk.next().unwrap().unwrap());
                assert!(walk.next().unwrap().is_none());
            } else {
                reject(list.get_mut(0).unwrap().unwrap());
            }
        }
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
        View::load::<__ExposedFixed>(store.bytes(), *b"slotpair")
            .unwrap()
            .validate()
            .unwrap();
    }
}

#[blubber::schema]
struct MapSlotValue {
    flag: bool,
    number: u16,
}
#[blubber::schema(account)]
struct MapSlots {
    prefix: u16,
    values: Map<[u8; 32], MapSlotValue>,
    suffix: Str,
}

#[test]
fn fixed_map_slots_copy_keys_and_frame_nested_values_suffix_and_spare() {
    for count in 0..=4 {
        let mut keys = [[42u8; 32]; 4];
        for (i, key) in keys.iter_mut().enumerate() {
            key[31] = i as u8;
        }
        let inputs: Vec<_> = keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                (
                    *key,
                    MapSlotValueInit {
                        flag: i % 2 == 0,
                        number: 100 + i as u16,
                    },
                )
            })
            .collect();
        let mut bytes = [0xa5; 256];
        bytes[..12].fill(0);
        {
            let mut store = SliceStore::new(&mut bytes);
            MapSlots::initialize(
                &mut store,
                MapSlotsInit {
                    prefix: 91,
                    values: &inputs[..count],
                    suffix: "outer suffix",
                },
            )
            .unwrap()
            .commit()
            .unwrap();
        }
        // Restore observable physical spare after initialization's zero fill.
        let used = MapSlots::load(&bytes).unwrap().used_len() as usize;
        bytes[used..].fill(0xa5);
        let before = bytes;
        let base = View::load::<MapSlots>(&bytes, MapSlots::DISCRIMINATOR)
            .unwrap()
            .field(1)
            .unwrap()
            .base() as usize;
        let mut expected = before;
        {
            let mut store = SliceStore::new(&mut bytes);
            let mut root = MapSlots::load_mut(&mut store).unwrap();
            let mut cursor = root.values_mut().cursor_mut().unwrap();
            for (i, original) in keys.iter().enumerate().take(count) {
                let (mut key, mut value) = cursor.next().unwrap().unwrap();
                assert_eq!(&key, original);
                key.fill(255); // A copied key never aliases the account.
                assert_eq!(key, [255; 32]);
                assert_eq!(value.number(), 100 + i as u16);
                let mut field = value.number_mut();
                field.set(900 + i as u16).unwrap();
                #[allow(clippy::forget_non_drop)]
                core::mem::forget(field);
                let at = base + 4 + i * 35 + 33;
                expected[at..at + 2].copy_from_slice(&(900 + i as u16).to_le_bytes());
            }
            assert!(cursor.next().unwrap().is_none());
            assert!(cursor.next().unwrap().is_none());
            assert_eq!(root.suffix(), "outer suffix");
            root.validate().unwrap();
            root.commit().unwrap();
        }
        assert_eq!(bytes, expected);
    }
}

blubber::__root_schema!(__MapFlagsQ6, Map<bool, bool>);
#[cfg(not(feature = "verify-on-load"))]
#[test]
fn lazy_fixed_map_admission_decodes_visited_keys_and_repairs_leaves() {
    for keys in [[0, 0], [1, 0], [0, 2], [0, 1]] {
        let mut bytes = [0xa5; 32];
        bytes[..8].copy_from_slice(b"mapwalk6");
        bytes[8..12].copy_from_slice(&20u32.to_le_bytes());
        bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&[keys[0], 7, keys[1], 8]);
        let before = bytes;
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::load_trusted::<__MapFlagsQ6>(&mut store, *b"mapwalk6").unwrap();
        let mut cursor = blubber::raw::project::<Map<bool, bool>>(edit)
            .cursor_mut()
            .unwrap();
        for i in 0..2 {
            if keys[i as usize] > 1 {
                assert!(
                    matches!(cursor.next(), Err(e) if e == Error::new(ErrorKind::InvalidBool,16+i*2))
                );
                assert!(cursor.next().unwrap().is_none());
                break;
            }
            let (key, mut value) = cursor.next().unwrap().unwrap();
            assert_eq!(key, keys[i as usize] != 0);
            assert_eq!(
                value.try_get(),
                Err(Error::new(ErrorKind::InvalidBool, 17 + i * 2))
            );
            value.set(true).unwrap();
        }
        #[allow(clippy::drop_non_drop)]
        drop(cursor);
        let mut expected = before;
        expected[17] = 1;
        if keys[1] <= 1 {
            expected[19] = 1;
        }
        assert_eq!(store.bytes(), expected);
        assert_eq!(store.is_poisoned(), keys.iter().any(|key| *key > 1));
    }
}

#[test]
fn fixed_map_callbacks_keep_partial_progress() {
    for stop in 0..=3 {
        let mut bytes = [0; 512];
        let mut store = SliceStore::new(&mut bytes);
        let mut root = Catalog::initialize(&mut store, init()).unwrap();
        let mut visits = 0;
        let result = root.fixed_mut().try_for_each_mut(|key, value| {
            assert_eq!(key, [i16::MIN, 0, i16::MAX][visits]);
            value.set(77)?;
            visits += 1;
            if visits == stop + 1 {
                Err(ErrorKind::Capacity.into())
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result,
            if stop < 3 {
                Err(ErrorKind::Capacity.into())
            } else {
                Ok(())
            }
        );
        assert_eq!(
            root.fixed().iter().map(|(_, v)| v).collect::<Vec<_>>(),
            [10, 20, 30]
                .into_iter()
                .enumerate()
                .map(|(i, v)| if i < visits { 77 } else { v })
                .collect::<Vec<_>>()
        );
        root.validate().unwrap();
        root.commit().unwrap();
    }
}

blubber::__root_schema!(__MapWalkCodec, Map<u8, WalkCodec>);
#[test]
fn fixed_map_codec_failures_preserve_keys_and_disable_poisoned_loans() {
    for mode in 1..=5 {
        let mut bytes = [0; 40];
        let mut store = SliceStore::new(&mut bytes);
        let mut root = Edit::initialize::<__MapWalkCodec>(
            &mut store,
            *b"mapcode6",
            &[
                (1, WalkCodec { value: 7, mode: 0 }),
                (3, WalkCodec { value: 9, mode: 0 }),
            ],
        )
        .unwrap();
        {
            let mut cursor = blubber::raw::project::<Map<u8, WalkCodec>>(root.reborrow())
                .cursor_mut()
                .unwrap();
            let (key, mut value) = cursor.next().unwrap().unwrap();
            assert_eq!(key, 1);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                value.set(WalkCodec { value: 33, mode })
            }));
            if mode < 3 {
                assert_eq!(result.unwrap(), Err(ErrorKind::InvalidBoundary.into()));
            } else {
                assert!(result.is_err());
            }
            if mode < 4 {
                assert_eq!(value.try_get().unwrap().value, 7);
                let (key, mut second) = cursor.next().unwrap().unwrap();
                assert_eq!(key, 3);
                second.set(WalkCodec { value: 44, mode: 0 }).unwrap();
                assert!(cursor.next().unwrap().is_none());
            } else {
                assert_eq!(value.try_get(), Err(ErrorKind::Poisoned.into()));
                assert!(matches!(cursor.next(), Err(e) if e == ErrorKind::Poisoned.into()));
                assert!(cursor.next().unwrap().is_none());
            }
        }
        if mode < 4 {
            root.commit().unwrap();
        } else {
            assert_eq!(root.commit(), Err(ErrorKind::Poisoned.into()));
            assert!(store.is_closed());
        }
        #[allow(clippy::drop_non_drop)]
        drop(store);
        assert_eq!((bytes[16], bytes[18]), (1, 3));
    }
}

blubber::__root_schema!(__MapWideQ6, Map<u8, u16>);
#[test]
fn fixed_map_foreign_widths_are_rejected_before_lending() {
    let mut bytes = [0; 40];
    let mut store = SliceStore::new(&mut bytes);
    let mut root =
        Edit::initialize::<__MapWideQ6>(&mut store, *b"mapwide6", &[(1, 257), (3, 514)]).unwrap();
    assert!(
        matches!(blubber::raw::project::<Map<u16,u8>>(root.reborrow()).cursor_mut(), Err(e) if e == ErrorKind::InvalidSchema.into())
    );
    assert!(
        matches!(blubber::raw::project::<Map<u8,u8>>(root.reborrow()).cursor_mut(), Err(e) if e == ErrorKind::InvalidSchema.into())
    );
    root.validate().unwrap();
}

#[blubber::schema(account)]
struct IngressContainersQ6 {
    items: List<u16>,
    other: Map<bool, bool>,
}

#[test]
fn fixed_map_ingress_rechecks_forged_hint_identity_before_reusing_keys() {
    for foreign_table in [true, false] {
        let mut bytes = [0; 64];
        let mut store = SliceStore::new(&mut bytes);
        IngressContainersQ6::initialize(
            &mut store,
            IngressContainersQ6Init {
                items: &[0x0501, 0x0601],
                other: &[],
            },
        )
        .unwrap()
        .commit()
        .unwrap();
        let mut store = SliceStore::new(&mut bytes);
        let before = store.bytes().to_vec();
        {
            let table = <IngressContainersQ6 as blubber::RootSchema>::__layout();
            let actual = table.child(0, true);
            let selected = if foreign_table {
                let selected = <__MapFlagsQ6 as blubber::RootSchema>::__layout();
                assert_eq!(selected.root(), actual.root());
                assert_ne!(selected.words(), actual.words());
                selected
            } else {
                let selected = table.child(1, true);
                assert_ne!(selected.root(), actual.root());
                assert_eq!(selected.words(), actual.words());
                selected
            };
            assert_eq!(selected.budget(), actual.budget());
            let mut root =
                Edit::load::<IngressContainersQ6>(&mut store, IngressContainersQ6::DISCRIMINATOR)
                    .unwrap();
            // The real selector succeeds, but this certificate interprets two
            // valid u16 values as duplicate bool keys. No encoder ran after load.
            let forged = root.field_reborrow_certified(0, None, selected, None);
            assert!(
                matches!(blubber::raw::project::<Map<bool,bool>>(forged).cursor_mut(),
                Err(error) if error == Error::new(ErrorKind::DuplicateKey,22))
            );
            root.validate().unwrap();
        }
        assert_eq!(store.bytes(), before);
    }
}

#[blubber::schema(account)]
struct IngressSpanQ6 {
    items: Map<bool, bool>,
    raw: List<u16>,
}

#[test]
fn fixed_map_ingress_rechecks_a_forged_span_with_the_same_descriptor() {
    let mut bytes = [0; 64];
    let mut store = SliceStore::new(&mut bytes);
    IngressSpanQ6::initialize(
        &mut store,
        IngressSpanQ6Init {
            items: &[(false, false), (true, false)],
            raw: &[0x0501, 0x0601],
        },
    )
    .unwrap()
    .commit()
    .unwrap();
    let mut store = SliceStore::new(&mut bytes);
    let before = store.bytes().to_vec();
    {
        let selected = <IngressSpanQ6 as blubber::RootSchema>::__layout().child(0, true);
        let mut root =
            Edit::load::<IngressSpanQ6>(&mut store, IngressSpanQ6::DISCRIMINATOR).unwrap();
        // The actual map and this forged tail each occupy eight bytes. Their
        // table, root and budget match; only their account coordinates differ.
        let forged = root.field_reborrow_certified(0, None, selected, Some(12));
        assert!(
            matches!(blubber::raw::project::<Map<bool,bool>>(forged).cursor_mut(),
            Err(error) if error == Error::new(ErrorKind::DuplicateKey,30))
        );
        root.validate().unwrap();
    }
    assert_eq!(store.bytes(), before);
}

#[test]
fn fixed_map_ingress_is_affine_across_reborrow_and_storage_mutation() {
    let mut bytes = [0; 512];
    {
        let mut store = SliceStore::new(&mut bytes);
        Catalog::initialize(&mut store, init())
            .unwrap()
            .commit()
            .unwrap();
    }
    let mut store = SliceStore::new(&mut bytes);
    let mut root = Catalog::load_mut(&mut store).unwrap();
    // A completed first loan must not leave reusable ingress in its parent.
    let mut first = root.fixed_mut().cursor_mut().unwrap();
    let (_, mut value) = first.next().unwrap().unwrap();
    value.set(88).unwrap();
    #[allow(clippy::drop_non_drop)]
    drop(first);
    root.fixed_mut().insert(1, 99).unwrap();
    let mut cursor = root.fixed_mut().cursor_mut().unwrap();
    let mut found = Vec::new();
    while let Some((key, value)) = cursor.next().unwrap() {
        found.push((key, value.get()));
    }
    assert_eq!(found, [(i16::MIN, 88), (0, 20), (1, 99), (i16::MAX, 30)]);
    root.validate().unwrap();
    root.commit().unwrap();
}

#[test]
fn zero_width_fixed_map_descriptors_are_rejected() {
    let layout = blubber_core::Layout::Map {
        key: blubber_core::KeyLayout {
            width: 1,
            domain: blubber_core::KeyDomain::Raw,
        },
        value: &blubber_core::Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[],
        },
    };
    assert!(
        matches!(View::new(&[0;4],layout,0),Err(error) if error == ErrorKind::InvalidSchema.into())
    );
}

#[blubber::schema]
struct NestedFixedMap {
    values: Map<u16, u16>,
    tail: Str,
}
#[blubber::schema(account)]
struct IndexedFixedMaps {
    prefix: u8,
    items: List<NestedFixedMap>,
    suffix: Str,
}

#[test]
fn nested_fixed_map_walks_match_independent_bytes_at_every_small_stop() {
    use blubber_model::{IntWidth, Key, KeySchema, Schema as ModelSchema, Value};
    use std::collections::BTreeMap;
    let schema = ModelSchema::Struct(vec![
        ModelSchema::Unsigned(IntWidth::W1),
        ModelSchema::List(Box::new(ModelSchema::Struct(vec![
            ModelSchema::Map(
                KeySchema::Unsigned(IntWidth::W2),
                Box::new(ModelSchema::Unsigned(IntWidth::W2)),
            ),
            ModelSchema::Str,
        ]))),
        ModelSchema::Str,
    ]);
    for count in 0..=4 {
        let entries: Vec<_> = (0..count)
            .map(|i| ((2 * i + 1) as u16, (10 + i) as u16))
            .collect();
        let items = [
            NestedFixedMapInit {
                values: &entries,
                tail: "first child tail",
            },
            NestedFixedMapInit {
                values: &entries,
                tail: "",
            },
        ];
        for selected in 0..2 {
            for stop in 0..=count {
                for callback in [false, true] {
                    let mut bytes = [0; 512];
                    {
                        let mut store = SliceStore::new(&mut bytes);
                        IndexedFixedMaps::initialize(
                            &mut store,
                            IndexedFixedMapsInit {
                                prefix: 42,
                                items: &items,
                                suffix: "outer suffix",
                            },
                        )
                        .unwrap()
                        .commit()
                        .unwrap();
                    }
                    let used = IndexedFixedMaps::load(&bytes).unwrap().used_len() as usize;
                    bytes[used..].fill(0xa5);
                    {
                        let mut store = SliceStore::new(&mut bytes);
                        let mut root = IndexedFixedMaps::load_mut(&mut store).unwrap();
                        let mut item = root.items_mut().at_mut(selected).unwrap();
                        let mut map = item.values_mut();
                        if callback {
                            let mut ordinal = 0;
                            let result = map.try_for_each_mut(|key, value| {
                                assert_eq!(key, (2 * ordinal + 1) as u16);
                                if ordinal == stop {
                                    return Err(ErrorKind::Capacity.into());
                                }
                                assert_eq!(value.get(), (10 + ordinal) as u16);
                                value.set((110 + ordinal) as u16)?;
                                ordinal += 1;
                                Ok(())
                            });
                            assert_eq!(
                                result,
                                if stop < count {
                                    Err(ErrorKind::Capacity.into())
                                } else {
                                    Ok(())
                                }
                            );
                            assert_eq!(ordinal, stop);
                        } else {
                            let mut cursor = map.cursor_mut().unwrap();
                            for i in 0..stop {
                                let (key, mut value) = cursor.next().unwrap().unwrap();
                                assert_eq!(
                                    (key, value.get()),
                                    ((2 * i + 1) as u16, (10 + i) as u16)
                                );
                                value.set((110 + i) as u16).unwrap();
                            }
                        }
                        root.validate().unwrap();
                        root.commit().unwrap();
                    }
                    let children = (0..2)
                        .map(|child| {
                            let values: BTreeMap<_, _> = (0..count)
                                .map(|i| {
                                    (
                                        Key::Unsigned((2 * i + 1) as u128),
                                        Value::Unsigned(
                                            (if child == selected && i < stop {
                                                110 + i
                                            } else {
                                                10 + i
                                            }) as u128,
                                        ),
                                    )
                                })
                                .collect();
                            Value::Struct(vec![
                                Value::Map(values),
                                Value::Str(if child == 0 { "first child tail" } else { "" }.into()),
                            ])
                        })
                        .collect();
                    let value = Value::Struct(vec![
                        Value::Unsigned(42),
                        Value::List(children),
                        Value::Str("outer suffix".into()),
                    ]);
                    let mut expected = schema
                        .encode_account(IndexedFixedMaps::DISCRIMINATOR, &value, 512)
                        .unwrap();
                    expected[used..].fill(0xa5);
                    assert_eq!(
                        bytes.as_slice(),
                        expected.as_slice(),
                        "count={count} selected={selected} stop={stop} callback={callback}"
                    );
                }
            }
        }
    }
}

#[test]
fn fixed_map_key_codec_panic_exhausts_without_writes_or_poison() {
    let mut bytes = [0; 64];
    {
        let mut store = SliceStore::new(&mut bytes);
        Edit::initialize::<__ConcreteRoot1>(
            &mut store,
            *b"keypanic",
            &[
                (CheckedKey(1), 10),
                (CheckedKey(4), 40),
                (CheckedKey(5), 50),
            ],
        )
        .unwrap()
        .commit()
        .unwrap();
    }
    let before = bytes;
    {
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::load_trusted::<__ConcreteRoot1>(&mut store, *b"keypanic").unwrap();
        let mut cursor = blubber::raw::project::<Map<CheckedKey, u16>>(edit)
            .cursor_mut()
            .unwrap();
        assert_eq!(cursor.next().unwrap().unwrap().1.try_get(), Ok(10));
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = cursor.next();
        }))
        .is_err());
        assert!(cursor.next().unwrap().is_none());
        assert!(cursor.next().unwrap().is_none());
        assert!(!store.is_poisoned());
        assert_eq!(store.bytes(), before);
    }
    assert_eq!(bytes, before);
}

#[blubber::schema(account)]
struct IndexedHistory {
    marker: u16,
    list: List<Str>,
    map: Map<u16, Str>,
    tail: Str,
}

#[test]
fn indexed_mixed_histories_match_independent_bytes_at_every_stop() {
    use blubber_model::{IntWidth, Key, KeySchema, Schema as S, Value as V};
    let schema = S::Struct(vec![
        S::Unsigned(IntWidth::W2),
        S::List(Box::new(S::Str)),
        S::Map(KeySchema::Unsigned(IntWidth::W2), Box::new(S::Str)),
        S::Str,
    ]);
    let inputs = ["", "a", "é", "12345678", "a longer 🦀 value"];
    let max_count = if cfg!(miri) { 2 } else { 4 };
    for count in 0..=max_count {
        for seed in 0..inputs.len() {
            for stop in 0..=count {
                let list: Vec<_> = (0..count)
                    .map(|i| inputs[(seed + i) % inputs.len()])
                    .collect();
                let map: Vec<_> = list
                    .iter()
                    .enumerate()
                    .map(|(i, s)| ((i * 257 + 1) as u16, *s))
                    .collect();
                let mut expected_list: Vec<String> = list.iter().map(|s| (*s).into()).collect();
                let mut expected_map = expected_list.clone();
                let mut room = [0; 1024];
                let mut store = SliceStore::new(&mut room);
                {
                    let mut root = IndexedHistory::initialize(
                        &mut store,
                        IndexedHistoryInit {
                            marker: 0x7193,
                            list: &list,
                            map: &map,
                            tail: "outer suffix 🦀",
                        },
                    )
                    .unwrap();
                    let mut cursor = root.list_mut().cursor_mut().unwrap();
                    for i in 0..stop {
                        let mut child = cursor.next().unwrap().unwrap();
                        assert_eq!(child.get(), expected_list[i]);
                        // Same child, including a reborrow followed by reuse.
                        child.reborrow().push_str("+").unwrap();
                        assert_eq!(child.get(), format!("{}+", expected_list[i]));
                        child.set(inputs[(seed + i + 3) % inputs.len()]).unwrap();
                        child.set(inputs[(seed + i + 1) % inputs.len()]).unwrap();
                        expected_list[i] = inputs[(seed + i + 1) % inputs.len()].into();
                    }
                    let mut cursor = root.map_mut().cursor_mut().unwrap();
                    for i in 0..stop {
                        let (key, mut child) = cursor.next().unwrap().unwrap();
                        assert_eq!(key, map[i].0);
                        assert_eq!(child.get(), expected_map[i]);
                        child.reborrow().set("a longer 🦀 value").unwrap();
                        assert_eq!(child.get(), "a longer 🦀 value");
                        child.clear().unwrap();
                        child.set(inputs[(seed + i + 2) % inputs.len()]).unwrap();
                        expected_map[i] = inputs[(seed + i + 2) % inputs.len()].into();
                    }
                    root.validate().unwrap();
                    assert_eq!(root.tail(), "outer suffix 🦀");
                }
                let value = V::Struct(vec![
                    V::Unsigned(0x7193),
                    V::List(expected_list.into_iter().map(V::Str).collect()),
                    V::Map(
                        expected_map
                            .into_iter()
                            .enumerate()
                            .map(|(i, s)| (Key::Unsigned((i * 257 + 1) as u128), V::Str(s)))
                            .collect(),
                    ),
                    V::Str("outer suffix 🦀".into()),
                ]);
                assert_eq!(
                    store.bytes(),
                    schema
                        .encode_account(IndexedHistory::DISCRIMINATOR, &value, 1024)
                        .unwrap(),
                    "count={count} seed={seed} stop={stop}"
                );
                assert!(!store.is_poisoned());
            }
        }
    }
}

#[blubber::schema(account)]
struct CallbackHistory {
    prefix: u16,
    list: List<Record>,
    map: Map<u16, Record>,
    tail: Str,
}

#[test]
fn nested_callbacks_keep_old_dependent_partial_progress_in_packed_roots() {
    use blubber_model::{IntWidth, Key, KeySchema, Schema as S, Value as V};
    let record_schema = S::Struct(vec![S::Str, S::List(Box::new(S::Unsigned(IntWidth::W4)))]);
    let schema = S::Struct(vec![
        S::Unsigned(IntWidth::W2),
        S::List(Box::new(record_schema.clone())),
        S::Map(KeySchema::Unsigned(IntWidth::W2), Box::new(record_schema)),
        S::Str,
    ]);
    let names = ["first", "é", ""];
    let tags: &[u32] = &[1, 20];
    let records: Vec<_> = names.iter().map(|name| RecordInit { name, tags }).collect();
    let entries: Vec<_> = records
        .iter()
        .enumerate()
        .map(|(i, record)| ((i * 257 + 1) as u16, *record))
        .collect();
    for count in 0..=names.len() {
        for stop in 0..=count {
            let mut room = [0; 1024];
            let mut store = SliceStore::new(&mut room);
            let mut root = CallbackHistory::initialize(
                &mut store,
                CallbackHistoryInit {
                    prefix: 0x7193,
                    list: &records[..count],
                    map: &entries[..count],
                    tail: "outer suffix 🦀",
                },
            )
            .unwrap();
            let mut list_calls = 0;
            let mut map_calls = 0;
            let edit = |record: &mut records::RecordMut<'_>, ordinal: usize| {
                let old = record.name().to_owned();
                record.name_mut().push_str(" temporary growth 🦀")?;
                record
                    .tags_mut()
                    .try_for_each_mut(|tag| tag.set(tag.get() + 1))?;
                record.tags_mut().push(ordinal as u32 + 50)?;
                record.name_mut().clear()?;
                if ordinal.is_multiple_of(2) {
                    record.name_mut().set(&format!("{old} 🦀"))?;
                }
                if ordinal == stop {
                    Err(ErrorKind::Capacity.into())
                } else {
                    Ok(())
                }
            };
            let list_result = root.list_mut().try_for_each_mut(|record| {
                let ordinal = list_calls;
                list_calls += 1;
                edit(record, ordinal)
            });
            let map_result = root.map_mut().try_for_each_mut(|key, record| {
                let ordinal = map_calls;
                map_calls += 1;
                assert_eq!(key, entries[ordinal].0);
                edit(record, ordinal)
            });
            let result = if stop < count {
                Err(ErrorKind::Capacity.into())
            } else {
                Ok(())
            };
            assert_eq!(list_result, result);
            assert_eq!(map_result, result);
            let completed = count.min(stop + 1);
            assert_eq!((list_calls, map_calls), (completed, completed));
            root.validate().unwrap();

            let expected_record = |i: usize| {
                let (name, values) = if i < completed {
                    (
                        if i.is_multiple_of(2) {
                            format!("{} 🦀", names[i])
                        } else {
                            String::new()
                        },
                        vec![2, 21, i as u128 + 50],
                    )
                } else {
                    (names[i].into(), vec![1, 20])
                };
                V::Struct(vec![
                    V::Str(name),
                    V::List(values.into_iter().map(V::Unsigned).collect()),
                ])
            };
            let expected = V::Struct(vec![
                V::Unsigned(0x7193),
                V::List((0..count).map(expected_record).collect()),
                V::Map(
                    (0..count)
                        .map(|i| (Key::Unsigned(entries[i].0 as u128), expected_record(i)))
                        .collect(),
                ),
                V::Str("outer suffix 🦀".into()),
            ]);
            assert_eq!(
                store.bytes(),
                schema
                    .encode_account(CallbackHistory::DISCRIMINATOR, &expected, 1024)
                    .unwrap(),
                "count={count} stop={stop}"
            );
            assert!(!store.is_poisoned());
        }
    }
}

#[test]
fn callback_error_after_caught_nested_encoder_unwind_keeps_damage_poison() {
    for map in [false, true] {
        let mut room = [0; 128];
        let mut store = SliceStore::new(&mut room);
        let mut edit = if map {
            Edit::initialize::<__ConcreteRoot3>(&mut store, *b"mapboom!", &[(1, &[1]), (2, &[2])])
                .unwrap()
        } else {
            Edit::initialize::<__ConcreteRoot2>(&mut store, *b"listboom", &[&[1], &[2]]).unwrap()
        };
        let mut calls = 0;
        let mut callback = |child: &mut blubber::ListMut<'_, Interrupt>| {
            calls += 1;
            child.push(7)?;
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| child.push(99)));
            assert!(unwind.is_err());
            assert_eq!(child.push(8), Err(ErrorKind::Poisoned.into()));
            Err(ErrorKind::InvalidBoundary.into())
        };
        let result = if map {
            blubber::raw::project::<Map<u16, List<Interrupt>>>(edit.reborrow())
                .try_for_each_mut(|_, child| callback(child))
        } else {
            blubber::raw::project::<List<List<Interrupt>>>(edit.reborrow())
                .try_for_each_mut(callback)
        };
        assert_eq!(result, Err(ErrorKind::InvalidBoundary.into()));
        assert_eq!(calls, 1);
        assert_eq!(edit.commit(), Err(ErrorKind::Poisoned.into()));
        assert!(store.is_poisoned());
        assert!(store.is_closed());
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn indexed_equal_writes_frame_all_ends_and_lazy_utf8_errors_rebase_once() {
    let mut room = [0; 160];
    let mut store = SliceStore::new(&mut room);
    IndexedHistory::initialize(
        &mut store,
        IndexedHistoryInit {
            marker: 7,
            list: &["a", "b", "c"],
            map: &[(1, "d"), (2, "e")],
            tail: "tail",
        },
    )
    .unwrap();
    let before = store.bytes().to_vec();
    let loaded = IndexedHistory::load(store.bytes()).unwrap();
    let corrupt_at =
        loaded.list().get(1).unwrap().as_ptr() as usize - store.bytes().as_ptr() as usize;
    store.bytes_mut()[corrupt_at] = 255;
    {
        let mut root = IndexedHistory::load_trusted_mut(&mut store).unwrap();
        let mut cursor = root.list_mut().cursor_mut().unwrap();
        assert_eq!(cursor.next().unwrap().unwrap().get(), "a");
        let mut child = cursor.next().unwrap().unwrap();
        assert_eq!(
            child.try_get(),
            Err(Error::new(ErrorKind::InvalidUtf8, corrupt_at as u32))
        );
        child.set("B").unwrap();
        child.set("b").unwrap();
        assert_eq!(child.get(), "b");
        assert_eq!(cursor.next().unwrap().unwrap().get(), "c");
        assert!(cursor.next().unwrap().is_none());
        root.validate().unwrap();
    }
    assert_eq!(store.bytes(), before);
}

#[test]
fn indexed_optional_equal_encodes_and_unwind_keep_the_directory_frame() {
    blubber::__root_schema!(__OptionalWalk, List<Option<Interrupt>>);
    let mut room = [0; 96];
    let mut store = SliceStore::new(&mut room);
    {
        let edit =
            Edit::initialize::<__OptionalWalk>(&mut store, *b"optwalk!", &[None, None]).unwrap();
        let mut cursor = blubber::raw::project::<List<Option<Interrupt>>>(edit)
            .cursor_mut()
            .unwrap();
        let mut child = cursor.next().unwrap().unwrap();
        child.set(None).unwrap();
        child.set(Some(3)).unwrap();
        child.set(Some(4)).unwrap();
        assert_eq!(child.get(), Some(4));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| child.set(Some(99)))).is_err()
        );
        assert_eq!(child.try_get(), Err(ErrorKind::Poisoned.into()));
        assert!(matches!(cursor.next(), Err(error) if error == ErrorKind::Poisoned.into()));
        assert!(cursor.next().unwrap().is_none());
    }
    assert!(store.is_poisoned());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn indexed_utf8_windows_ignore_neighbor_faults_and_preserve_exact_borrows() {
    let values = ["éééé", "abcdefgh", "🦀🦀", "QRSTUVWX", "01234567", "λéxxxx"];
    for count in 1..=values.len() {
        for bad in 0..count {
            let mut room = [0; 256];
            let mut store = SliceStore::new(&mut room);
            Edit::initialize::<__ConcreteRoot0>(&mut store, *b"winutf8!", &values[..count])
                .unwrap();
            let before = store.bytes().to_vec();
            let base = store.bytes().as_ptr() as usize;
            let payload = 12 + 4 * count;
            let at = payload + 8 * bad;
            store.bytes_mut()[at] = 255;
            {
                let edit = Edit::load_trusted::<__ConcreteRoot0>(&mut store, *b"winutf8!").unwrap();
                let mut cursor = blubber::raw::project::<List<Str>>(edit)
                    .cursor_mut()
                    .unwrap();
                for (index, value) in values[..count].iter().enumerate() {
                    let mut child = cursor.next().unwrap().unwrap();
                    if index == bad {
                        assert_eq!(
                            child.try_get(),
                            Err(Error::new(ErrorKind::InvalidUtf8, at as u32))
                        );
                        child.set(value).unwrap();
                    }
                    assert_eq!(child.get(), *value);
                    // Returned references still point at the exact account slot.
                    assert_eq!(child.get().as_ptr() as usize, base + payload + 8 * index);
                }
                assert!(cursor.next().unwrap().is_none());
            }
            assert_eq!(store.bytes(), before);
        }
    }
}

#[test]
fn cursor_debug_observes_fixed_and_empty_state_without_advancing() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Catalog::initialize(&mut store, init()).unwrap();
    {
        let mut cursor = root.numbers_mut().cursor_mut().unwrap();
        assert!(format!("{cursor:?}").contains("remaining_bytes: 6"));
        assert_eq!(cursor.next().unwrap().unwrap().get(), 10);
        assert!(format!("{cursor:?}").contains("remaining_bytes: 4"));
        assert_eq!(cursor.next().unwrap().unwrap().get(), 20);
        assert_eq!(cursor.next().unwrap().unwrap().get(), 30);
        assert!(format!("{cursor:?}").contains("remaining_bytes: 0"));
        assert!(cursor.next().unwrap().is_none());
        assert!(cursor.next().unwrap().is_none());
    }
    {
        let mut cursor = root.fixed_mut().cursor_mut().unwrap();
        assert!(format!("{cursor:?}").contains("remaining_bytes: 12"));
        assert_eq!(cursor.next().unwrap().unwrap().0, i16::MIN);
        assert!(format!("{cursor:?}").contains("remaining_bytes: 8"));
    }
    root.numbers_mut().clear().unwrap();
    let mut cursor = root.numbers_mut().cursor_mut().unwrap();
    assert!(format!("{cursor:?}").contains("empty"));
    assert!(cursor.next().unwrap().is_none());
    assert!(cursor.next().unwrap().is_none());
}
