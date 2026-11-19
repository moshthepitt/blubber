//! Typed list edits, borrowed inputs, exact removal and atomic failures.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "support/record.rs"]
mod record;

#[path = "support/exhaustive_record.rs"]
mod exhaustive;

#[path = "support/rewrite_api.rs"]
mod rewrite_api;

#[test]
fn bounded_exhaustive_oracle_subset() {
    exhaustive::miri_subset();
}

#[blubber::schema(account)]
struct WideValues {
    values: blubber::List<[u8; 513]>,
}

#[test]
fn removal_returns_large_fixed_values_through_the_large_preparation_path() {
    let first = [7; 513];
    let second = [9; 513];
    let mut bytes = [0; 1200];
    let mut store = blubber::SliceStore::new(&mut bytes);
    let mut root = WideValues::initialize(
        &mut store,
        WideValuesInit {
            values: &[first, second],
        },
    )
    .unwrap();
    assert_eq!(root.values_mut().remove(0), Ok(first));
    assert_eq!(root.values().get(0), Some(second));
    assert_eq!(root.values_mut().pop(), Ok(Some(second)));
    assert!(root.values().is_empty());
    root.validate().unwrap();
    assert!(!store.is_poisoned());
}

use blubber::raw::Resize;
use blubber::Resize as _;
use blubber::{Error, ErrorKind, List, Map, ReadStorage, SliceStore, Str, View};
use record::{RecordInit, Records, RecordsInit};

#[blubber::schema(account)]
struct Lists {
    numbers: List<u16>,
    strings: List<Str>,
    tail: Str,
}

fn init() -> ListsInit<'static> {
    ListsInit {
        numbers: &[10, 20],
        strings: &["a", "", "é"],
        tail: "untouched suffix",
    }
}

fn fault(kind: ErrorKind) -> Error {
    kind.into()
}

#[test]
fn fixed_edits_return_copied_values_and_preserve_outer_suffixes() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Lists::initialize(&mut store, init()).unwrap();
    let mut list = root.numbers_mut();
    list.push(u16::MAX).unwrap();
    list.insert(0, 1).unwrap();
    list.insert(2, 3).unwrap();
    list.insert(5, 7).unwrap();
    list.replace(1, 11).unwrap();
    assert_eq!(list.iter().collect::<Vec<_>>(), [1, 11, 3, 20, 65535, 7]);
    assert_eq!(list.remove(2), Ok(3));
    assert_eq!(list.remove(0), Ok(1));
    assert_eq!(list.pop(), Ok(Some(7)));
    list.truncate(2).unwrap();
    assert_eq!(list.iter().collect::<Vec<_>>(), [11, 20]);
    list.clear().unwrap();
    assert_eq!(list.pop(), Ok(None));
    list.push(42).unwrap();
    assert_eq!(list.pop(), Ok(Some(42)));
    assert!(list.is_empty());
    assert_eq!(root.strings().iter().collect::<Vec<_>>(), ["a", "", "é"]);
    assert_eq!(root.tail(), "untouched suffix");
    root.validate().unwrap();
}

#[test]
fn variable_edits_resize_values_and_discard_removed_views() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Lists::initialize(&mut store, init()).unwrap();
    let mut list = root.strings_mut();
    list.push("last").unwrap();
    list.insert(0, "first").unwrap();
    list.insert(2, "middle").unwrap();
    list.insert(6, "").unwrap();
    list.replace(1, "long replacement 🦀").unwrap();
    list.replace(2, "").unwrap();
    list.replace(4, "xy").unwrap();
    assert_eq!(
        list.iter().collect::<Vec<_>>(),
        ["first", "long replacement 🦀", "", "", "xy", "last", ""]
    );
    let _: () = list.remove(1).unwrap();
    let popped: Option<()> = list.pop().unwrap();
    assert_eq!(popped, Some(()));
    list.truncate(2).unwrap();
    assert_eq!(list.iter().collect::<Vec<_>>(), ["first", ""]);
    list.clear().unwrap();
    assert_eq!(list.pop(), Ok(None));
    assert_eq!(root.numbers().iter().collect::<Vec<_>>(), [10, 20]);
    assert_eq!(root.tail(), "untouched suffix");
    root.validate().unwrap();
}

#[test]
fn indexed_zero_one_two_transitions_have_exact_wire_bytes() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    Lists::initialize(&mut store, init()).unwrap();
    for (operation, expected) in [
        (0, vec![0, 0, 0, 0]),
        (1, vec![1, 0, 0, 0]),
        (2, vec![2, 0, 0, 0, 0, 0, 0, 0, b'x']),
        (3, vec![1, 0, 0, 0, b'x']),
        (4, vec![0, 0, 0, 0]),
    ] {
        {
            let mut root = Lists::load_mut(&mut store).unwrap();
            let mut list = root.strings_mut();
            match operation {
                0 => list.clear().unwrap(),
                1 => list.push("").unwrap(),
                2 => list.push("x").unwrap(),
                3 => list.remove(0).unwrap(),
                _ => assert_eq!(list.pop(), Ok(Some(()))),
            }
            root.validate().unwrap();
        }
        let view = View::load::<Lists>(store.bytes(), Lists::DISCRIMINATOR).unwrap();
        assert_eq!(view.field(1).unwrap().bytes(), expected);
    }
}

#[test]
fn every_insert_remove_position_preserves_empty_and_nonempty_neighbors() {
    for initial in [&[][..], &[""][..], &["", "x"][..], &["a", "", "z"][..]] {
        for index in 0..=initial.len() {
            for value in ["", "🦀"] {
                let mut room = [0; 128];
                let mut store = SliceStore::new(&mut room);
                let mut root = Lists::initialize(
                    &mut store,
                    ListsInit {
                        numbers: &[],
                        strings: initial,
                        tail: "suffix",
                    },
                )
                .unwrap();
                let mut expected = initial.to_vec();
                root.strings_mut().insert(index, value).unwrap();
                expected.insert(index, value);
                assert_eq!(root.strings().iter().collect::<Vec<_>>(), expected);
                root.validate().unwrap();
                assert_eq!(root.strings_mut().remove(index), Ok(()));
                assert_eq!(root.strings().iter().collect::<Vec<_>>(), initial);
                assert_eq!(root.tail(), "suffix");
                root.validate().unwrap();
            }
        }
    }
}

#[test]
fn borrowed_records_and_retained_handles_survive_nested_list_growth() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Records::initialize(&mut store, RecordsInit { records: &[] }).unwrap();
    let name = String::from("borrowed");
    let tags = [7, 9];
    let mut records = root.records_mut();
    records
        .push(RecordInit {
            name: &name,
            tags: &tags,
        })
        .unwrap();
    records
        .insert(
            0,
            RecordInit {
                name: "first",
                tags: &[],
            },
        )
        .unwrap();
    records
        .replace(
            1,
            RecordInit {
                name: "replacement",
                tags: &[3],
            },
        )
        .unwrap();
    let mut record = records.reborrow().at_mut(1).unwrap();
    record.name_mut().push_str(" retained").unwrap();
    let mut tags = record.tags_mut();
    tags.push(8).unwrap();
    tags.insert(0, 2).unwrap();
    tags.replace(1, 4).unwrap();
    assert_eq!(tags.remove(0), Ok(2));
    assert_eq!(tags.pop(), Ok(Some(8)));
    tags.truncate(1).unwrap();
    record.name_mut().set("short").unwrap();
    assert_eq!(record.tags().get(0), Some(4));
    assert_eq!(records.get(1).unwrap().name(), "short");
    assert_eq!(records.remove(0), Ok(()));
    assert_eq!(records.pop(), Ok(Some(())));
    assert_eq!(records.pop(), Ok(None));
    root.validate().unwrap();
    assert_eq!(name, "borrowed");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, blubber::Fixed)]
struct Native {
    flag: bool,
    number: u16,
}

#[blubber::schema]
struct Flag {
    flag: bool,
}

#[blubber::schema(account)]
struct Shapes {
    native: List<Native>,
    views: List<Flag>,
    lists: List<List<u8>>,
    options: List<Option<Str>>,
    maps: List<Map<u16, Str>>,
}

#[test]
fn fixed_schema_views_discard_while_native_fixed_structs_return_values() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let mut root = Shapes::initialize(
        &mut store,
        ShapesInit {
            native: &[],
            views: &[],
            lists: &[],
            options: &[],
            maps: &[],
        },
    )
    .unwrap();
    let native = Native {
        flag: true,
        number: u16::MAX,
    };
    root.native_mut().push(native).unwrap();
    root.views_mut().push(FlagInit { flag: true }).unwrap();
    assert_eq!(root.native_mut().remove(0), Ok(native));
    let _: () = root.views_mut().remove(0).unwrap();
    root.validate().unwrap();
}

#[test]
fn nested_collection_and_option_inputs_are_borrowed_and_measured_before_writes() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Shapes::initialize(
        &mut store,
        ShapesInit {
            native: &[],
            views: &[],
            lists: &[],
            options: &[],
            maps: &[],
        },
    )
    .unwrap();
    root.lists_mut().push(&[1, 2]).unwrap();
    root.lists_mut().insert(0, &[]).unwrap();
    root.lists_mut().at_mut(1).unwrap().push(3).unwrap();
    assert_eq!(
        root.lists().get(1).unwrap().iter().collect::<Vec<_>>(),
        [1, 2, 3]
    );
    root.options_mut().push(Some("é")).unwrap();
    root.options_mut().push(None).unwrap();
    root.options_mut().replace(1, Some("")).unwrap();
    assert_eq!(root.options_mut().pop(), Ok(Some(())));
    root.maps_mut().push(&[(1, "one"), (256, "")]).unwrap();
    let used = root.used_len();
    assert_eq!(
        root.maps_mut().push(&[(2, "bad"), (1, "order")]),
        Err(fault(ErrorKind::UnsortedKeys))
    );
    assert_eq!(
        root.maps_mut().replace(0, &[(1, "a"), (1, "b")]),
        Err(fault(ErrorKind::DuplicateKey))
    );
    assert_eq!(root.used_len(), used);
    assert_eq!(root.maps().get(0).unwrap().get(&1), Some("one"));
    assert_eq!(root.lists_mut().remove(0), Ok(()));
    root.validate().unwrap();
}

#[test]
fn oversized_indices_and_truncation_follow_usize_collection_semantics() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    Lists::initialize(&mut store, init()).unwrap();
    let before = store.bytes().to_vec();
    {
        let mut root = Lists::load_mut(&mut store).unwrap();
        for index in [3, u32::MAX as usize, usize::MAX]
            .into_iter()
            .chain((u32::MAX as usize).checked_add(1))
        {
            assert_eq!(
                root.numbers_mut().insert(index, 7),
                Err(fault(ErrorKind::IndexOutOfBounds))
            );
            assert_eq!(
                root.numbers_mut().remove(index),
                Err(fault(ErrorKind::IndexOutOfBounds))
            );
            assert_eq!(
                root.numbers_mut().replace(index, 7),
                Err(fault(ErrorKind::IndexOutOfBounds))
            );
        }
        root.numbers_mut().truncate(usize::MAX).unwrap();
        root.strings_mut().truncate(usize::MAX).unwrap();
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[test]
fn capacity_failures_preserve_bytes_and_the_bound_list_can_retry() {
    let used = Lists::encoded_len_of(&init()).unwrap() as usize;
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room[..used]);
    Lists::initialize(&mut store, init()).unwrap();
    let before = store.bytes().to_vec();
    {
        let mut root = Lists::load_mut(&mut store).unwrap();
        let mut strings = root.strings_mut();
        assert_eq!(strings.push("x"), Err(fault(ErrorKind::Capacity)));
        assert_eq!(strings.insert(0, ""), Err(fault(ErrorKind::Capacity)));
        assert_eq!(
            strings.replace(0, "longer"),
            Err(fault(ErrorKind::Capacity))
        );
        assert_eq!(strings.get(0), Some("a"));
    }
    assert_eq!(store.bytes(), before);
    let mut root = Lists::load_mut(&mut store).unwrap();
    let mut strings = root.strings_mut();
    strings.remove(2).unwrap();
    strings.push("").unwrap();
    strings.validate().unwrap();
    root.validate().unwrap();
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn corrupt_removed_values_fail_with_absolute_offsets_before_compaction() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    Lists::initialize(&mut store, init()).unwrap();
    let view = View::load_trusted::<Lists>(store.bytes(), Lists::DISCRIMINATOR).unwrap();
    let base = view.field(1).unwrap().base() as usize + 12;
    let original = store.bytes()[base];
    store.bytes_mut()[base] = 0xff;
    let before = store.bytes().to_vec();
    {
        let mut root = Lists::load_trusted_mut(&mut store).unwrap();
        assert_eq!(
            root.strings_mut().remove(0),
            Err(Error::new(ErrorKind::InvalidUtf8, base as u32))
        );
    }
    assert_eq!(store.bytes(), before);
    assert!(store.is_poisoned());
    assert!(matches!(Lists::load_trusted_mut(&mut store), Err(error)
        if error == ErrorKind::Poisoned.into()));
    store.bytes_mut()[base] = original;
    let mut root = Lists::load_checked_mut(&mut store).unwrap();
    root.strings_mut().replace(0, "repaired").unwrap();
    root.validate().unwrap();
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn removed_native_and_schema_values_check_bool_and_nested_utf8_domains() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    Shapes::initialize(
        &mut store,
        ShapesInit {
            native: &[Native {
                flag: true,
                number: 3,
            }],
            views: &[FlagInit { flag: true }],
            lists: &[],
            options: &[Some("x")],
            maps: &[],
        },
    )
    .unwrap();
    for (field, offset, kind) in [
        (0, 4, ErrorKind::InvalidBool),
        (1, 4, ErrorKind::InvalidBool),
        (3, 5, ErrorKind::InvalidUtf8),
    ] {
        let view = View::load_trusted::<Shapes>(store.bytes(), Shapes::DISCRIMINATOR).unwrap();
        let base = view.field(field).unwrap().base() as usize + offset;
        let original = store.bytes()[base];
        store.bytes_mut()[base] = 0xff;
        let before = store.bytes().to_vec();
        {
            let mut root = Shapes::load_trusted_mut(&mut store).unwrap();
            let result = match field {
                0 => root.native_mut().pop().map(|_| ()),
                1 => root.views_mut().pop().map(|_| ()),
                _ => root.options_mut().pop().map(|_| ()),
            };
            assert_eq!(result, Err(Error::new(kind, base as u32)));
        }
        assert_eq!(store.bytes(), before);
        // Fixed-entry ancestor publication now follows the lazy source-fault policy.
        assert!(store.is_poisoned());
        store.bytes_mut()[base] = original;
        Shapes::load_checked_mut(&mut store).unwrap();
        assert!(!store.is_poisoned());
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn structural_corruption_rejects_even_clear_and_empty_pop_without_writes() {
    for (field, relative) in [(0, 0), (1, 4), (1, 8)] {
        let mut room = [0; 128];
        let mut store = SliceStore::new(&mut room);
        Lists::initialize(&mut store, init()).unwrap();
        let view = View::load_trusted::<Lists>(store.bytes(), Lists::DISCRIMINATOR).unwrap();
        let base = view.field(field).unwrap().base() as usize + relative;
        store.bytes_mut()[base..base + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = store.bytes().to_vec();
        assert!(Lists::load_checked_mut(&mut store).is_err());
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn structural_edits_leave_unrelated_leaf_domains_unvisited() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    Lists::initialize(&mut store, init()).unwrap();
    let view = View::load_trusted::<Lists>(store.bytes(), Lists::DISCRIMINATOR).unwrap();
    let tail = view.field(2).unwrap().base() as usize;
    let first = view.field(1).unwrap().base() as usize + 12;
    store.bytes_mut()[tail] = 0xff;
    store.bytes_mut()[first] = 0xff;
    let mut root = Lists::load_trusted_mut(&mut store).unwrap();
    root.strings_mut().extend_from_slice(&["new", ""]).unwrap();
    root.numbers_mut().insert(0, 9).unwrap();
    root.strings_mut().truncate(0).unwrap();
    assert!(root.try_tail().is_err());
    root.tail_mut().set("repaired").unwrap();
    root.validate().unwrap();
}

#[test]
fn shrinking_clears_the_old_used_tail_and_preserves_existing_spare() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let old_used = Lists::initialize(&mut store, init()).unwrap().used_len() as usize;
    store.bytes_mut()[old_used..].fill(0xa5);
    let new_used;
    {
        let mut root = Lists::load_mut(&mut store).unwrap();
        root.strings_mut().clear().unwrap();
        root.numbers_mut().truncate(1).unwrap();
        root.validate().unwrap();
        assert_eq!(root.tail(), "untouched suffix");
        new_used = root.used_len() as usize;
    }
    assert!(store.bytes()[new_used..old_used]
        .iter()
        .all(|&byte| byte == 0));
    assert!(store.bytes()[old_used..].iter().all(|&byte| byte == 0xa5));
}

type DeepList = Option<Option<Option<Option<Option<List<Str>>>>>>;

#[blubber::schema(account)]
struct Paths {
    deep: DeepList,
    maps: Map<u16, List<Str>>,
    tail: Str,
}

#[test]
fn list_plans_patch_option_map_and_maximum_depth_ancestors() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Paths::initialize(
        &mut store,
        PathsInit {
            deep: Some(Some(Some(Some(Some(&["a"]))))),
            maps: &[(1, &["first"]), (2, &["last"])],
            tail: "suffix",
        },
    )
    .unwrap();
    let mut list = root
        .deep_mut()
        .some_mut()
        .unwrap()
        .some_mut()
        .unwrap()
        .some_mut()
        .unwrap()
        .some_mut()
        .unwrap()
        .some_mut()
        .unwrap();
    list.insert(0, "a longer value").unwrap();
    list.replace(1, "").unwrap();
    list.remove(0).unwrap();
    assert_eq!(list.pop(), Ok(Some(())));
    list.extend_from_slice(&["retained", "second"]).unwrap();
    list.insert_from_slice(1, &["", "é"]).unwrap();
    let mut map_value = root.maps_mut().at_mut(&1).unwrap();
    map_value
        .extend_from_slice(&["new element", "bulk tail"])
        .unwrap();
    map_value.insert_from_slice(1, &[""]).unwrap();
    map_value.remove(1).unwrap();
    map_value.remove(0).unwrap();
    assert_eq!(root.maps().get(&1).unwrap().get(0), Some("new element"));
    assert_eq!(root.maps().get(&2).unwrap().get(0), Some("last"));
    assert_eq!(root.tail(), "suffix");
    root.validate().unwrap();
}

#[test]
fn closed_sessions_reject_every_list_operation() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let mut root = Lists::initialize(&mut store, init()).unwrap();
    root.reborrow().commit().unwrap();
    let mut list = root.numbers_mut();
    assert_eq!(
        list.extend_from_slice(&[1, 2]),
        Err(fault(ErrorKind::Closed))
    );
    assert_eq!(
        list.insert_from_slice(0, &[]),
        Err(fault(ErrorKind::Closed))
    );
    assert_eq!(list.push(1), Err(fault(ErrorKind::Closed)));
    assert_eq!(list.insert(0, 1), Err(fault(ErrorKind::Closed)));
    assert_eq!(list.replace(0, 1), Err(fault(ErrorKind::Closed)));
    assert_eq!(list.remove(0), Err(fault(ErrorKind::Closed)));
    assert_eq!(list.pop(), Err(fault(ErrorKind::Closed)));
    assert_eq!(list.truncate(0), Err(fault(ErrorKind::Closed)));
    assert_eq!(list.clear(), Err(fault(ErrorKind::Closed)));
}

struct Interrupt;

impl blubber::Schema for Interrupt {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:list_edits:Interrupt",
                epoch: 0,
            },
            wire: <u8 as blubber::Schema>::WIRE_DESCRIPTOR,
        };
    const KIND: blubber::Kind = blubber::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <u8 as blubber::Schema>::LAYOUT;
    const __LEN: usize = <u8 as blubber::Schema>::__LEN;
    const __HEADER: &'static [u32] = <u8 as blubber::Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = u8;
    type Mut<'a> = blubber::raw::Edit<'a>;
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
    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> blubber::raw::Edit<'a> {
        blubber::raw::into_edit(edit)
    }
    fn read_removed(view: View<'_>) -> blubber::Result<u8> {
        Self::read(view)
    }
    fn measure(_: &u8) -> blubber::Result<u32> {
        Ok(1)
    }
    fn encode(input: &u8, destination: &mut [u8]) {
        destination[0] = *input;
        assert_ne!(*input, 99, "injected post-move encoder unwind");
    }
    fn encode_slot(input: &u8, destination: &mut [u8]) -> blubber::Result<usize> {
        if *input == 97 {
            return Ok(0);
        }
        Self::encode(input, destination);
        match input {
            96 => Err(fault(ErrorKind::InvalidTag)),
            98 => Ok(2),
            _ => Ok(destination.len()),
        }
    }
}

#[blubber::schema(account)]
struct Interrupted {
    list: List<Interrupt>,
}

#[test]
fn encoder_unwind_poison_rejects_all_list_methods_until_validated_recovery() {
    let mut room = [0; 32];
    let mut store = SliceStore::new(&mut room);
    {
        let mut root = Interrupted::initialize(&mut store, InterruptedInit { list: &[1] }).unwrap();
        let mut list = root.list_mut();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            list.insert(0, 99).unwrap();
        }));
        assert!(panic.is_err());
        assert_eq!(list.push(1), Err(fault(ErrorKind::Poisoned)));
        assert_eq!(list.insert(0, 1), Err(fault(ErrorKind::Poisoned)));
        assert_eq!(list.replace(0, 1), Err(fault(ErrorKind::Poisoned)));
        assert_eq!(list.remove(0), Err(fault(ErrorKind::Poisoned)));
        assert_eq!(list.pop(), Err(fault(ErrorKind::Poisoned)));
        assert_eq!(list.truncate(0), Err(fault(ErrorKind::Poisoned)));
        assert_eq!(list.clear(), Err(fault(ErrorKind::Poisoned)));
        assert_eq!(list.extend_from_slice(&[]), Err(fault(ErrorKind::Poisoned)));
    }
    assert!(store.is_poisoned());
    // The interrupted insertion wrote beyond the old logical extent. Full
    // validation still checks the published extent before allowing recovery.
    Interrupted::load_mut(&mut store).unwrap();
    assert!(!store.is_poisoned());
}

#[test]
fn bulk_encoder_unwind_poison_blocks_reads_and_retry() {
    let mut room = [0; 64];
    let mut store = SliceStore::new(&mut room);
    {
        let mut root = Interrupted::initialize(&mut store, InterruptedInit { list: &[1] }).unwrap();
        let mut list = root.list_mut();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            list.insert_from_slice(0, &[2, 99]).unwrap();
        }));
        assert!(panic.is_err());
        assert_eq!(list.try_len(), Err(fault(ErrorKind::Poisoned)));
        assert_eq!(
            list.extend_from_slice(&[3]),
            Err(fault(ErrorKind::Poisoned))
        );
    }
    assert!(store.is_poisoned());
}

#[test]
fn bulk_encoder_error_and_short_or_long_count_poison_insert_and_extend() {
    for bad in [96, 97, 98] {
        for extend in [false, true] {
            let mut room = [0; 64];
            let mut store = SliceStore::new(&mut room);
            {
                let mut root =
                    Interrupted::initialize(&mut store, InterruptedInit { list: &[1] }).unwrap();
                let mut list = root.list_mut();
                let result = if extend {
                    list.extend_from_slice(&[2, bad])
                } else {
                    list.insert_from_slice(0, &[2, bad])
                };
                let expected = if bad == 96 {
                    ErrorKind::InvalidTag
                } else {
                    ErrorKind::InvalidBoundary
                };
                assert_eq!(result, Err(fault(expected)));
                assert_eq!(list.try_len(), Err(fault(ErrorKind::Poisoned)));
                assert_eq!(
                    list.insert_from_slice(0, &[3]),
                    Err(fault(ErrorKind::Poisoned))
                );
            }
            assert!(store.is_poisoned());
        }
    }
}

struct InterruptText;

impl blubber::Schema for InterruptText {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:list_edits:InterruptText",
                epoch: 0,
            },
            wire: <Str as blubber::Schema>::WIRE_DESCRIPTOR,
        };
    const KIND: blubber::Kind = blubber::Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static blubber_core::Layout = <Str as blubber::Schema>::LAYOUT;
    const __LEN: usize = <Str as blubber::Schema>::__LEN;
    const __HEADER: &'static [u32] = <Str as blubber::Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = &'a str;
    type Mut<'a> = blubber::StrMut<'a>;
    type Init<'a> = &'a str;
    type Removed = ();
    #[cfg(feature = "alloc")]
    type Owned = String;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &String) -> blubber::Result<u32> {
        <Str as blubber::Schema>::measure_owned(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &String, destination: &mut [u8]) {
        <Str as blubber::Schema>::encode_owned(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> blubber::Result<String> {
        <Str as blubber::Schema>::decode_owned(view)
    }
    fn read<'a>(view: View<'a>) -> blubber::Result<&'a str> {
        <Str as blubber::Schema>::read(view)
    }
    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> blubber::StrMut<'a> {
        blubber::raw::project::<Str>(blubber::raw::into_edit(edit))
    }
    fn read_removed(view: View<'_>) -> blubber::Result<()> {
        <Str as blubber::Schema>::read_removed(view)
    }
    fn measure(input: &&str) -> blubber::Result<u32> {
        <Str as blubber::Schema>::measure(input)
    }
    fn encode(input: &&str, destination: &mut [u8]) {
        <Str as blubber::Schema>::encode(input, destination);
    }
    fn encode_slot(input: &&str, destination: &mut [u8]) -> blubber::Result<usize> {
        if *input == "short" {
            let written = destination.len() - 1;
            destination[..written].copy_from_slice(&input.as_bytes()[..written]);
            return Ok(written);
        }
        Self::encode(input, destination);
        match *input {
            "err" => Err(fault(ErrorKind::InvalidTag)),
            "long" => Ok(destination.len() + 1),
            _ => Ok(destination.len()),
        }
    }
}

#[blubber::schema(account)]
struct InterruptedTexts {
    list: List<InterruptText>,
    tail: u32,
}

#[test]
fn indexed_bulk_encoding_failures_poison_after_empty_slots_and_relocation() {
    for bad in ["err", "short", "long"] {
        for old in [&[][..], &["old", "", "last"][..]] {
            for extend in [false, true] {
                let mut room = [0; 128];
                let mut store = SliceStore::new(&mut room);
                {
                    let mut root = InterruptedTexts::initialize(
                        &mut store,
                        InterruptedTextsInit {
                            list: old,
                            tail: 42,
                        },
                    )
                    .unwrap();
                    let mut list = root.list_mut();
                    let result = if extend {
                        list.extend_from_slice(&["", bad])
                    } else {
                        list.insert_from_slice(0, &["", bad])
                    };
                    let expected = if bad == "err" {
                        ErrorKind::InvalidTag
                    } else {
                        ErrorKind::InvalidBoundary
                    };
                    assert_eq!(result, Err(fault(expected)));
                    assert_eq!(list.try_len(), Err(fault(ErrorKind::Poisoned)));
                }
                assert!(store.is_poisoned());
            }
        }
    }
}

#[cfg(feature = "alloc")]
#[test]
fn vec_growth_and_compaction_keep_bound_list_paths_current() {
    let mut bytes = Vec::new();
    let mut store = blubber::VecStore::new(&mut bytes);
    let mut root = Records::initialize(&mut store, RecordsInit { records: &[] }).unwrap();
    let mut records = root.records_mut();
    for name in ["", "one", "a much longer record"] {
        records
            .push(RecordInit {
                name,
                tags: &[1, 2],
            })
            .unwrap();
    }
    records.remove(0).unwrap();
    records
        .replace(
            1,
            RecordInit {
                name: "x",
                tags: &[],
            },
        )
        .unwrap();
    assert_eq!(records.get(0).unwrap().name(), "one");
    assert_eq!(records.get(1).unwrap().name(), "x");
    root.validate().unwrap();
    root.shrink_to_fit().unwrap();
    let receipt = root.commit().unwrap();
    assert_eq!(receipt.used_len, receipt.storage_len);
    Records::load(&bytes).unwrap();
}

#[test]
fn bulk_runs_preserve_mixed_empty_elements_and_outer_suffixes() {
    for initial in [&[][..], &[""][..], &["", "x"][..], &["a", "", "z"][..]] {
        for index in 0..=initial.len() {
            for values in [&[][..], &[""][..], &["", "é", "", "long"][..]] {
                let mut room = [0; 256];
                let mut store = SliceStore::new(&mut room);
                let mut root = Lists::initialize(
                    &mut store,
                    ListsInit {
                        numbers: &[10, 20],
                        strings: initial,
                        tail: "suffix",
                    },
                )
                .unwrap();
                let mut expected = initial.to_vec();
                expected.splice(index..index, values.iter().copied());
                root.strings_mut().insert_from_slice(index, values).unwrap();
                assert_eq!(root.strings().into_iter().collect::<Vec<_>>(), expected);
                root.numbers_mut().insert_from_slice(1, &[1, 2, 3]).unwrap();
                root.numbers_mut().extend_from_slice(&[4, 5]).unwrap();
                assert_eq!(
                    root.numbers().into_iter().collect::<Vec<_>>(),
                    [10, 1, 2, 3, 20, 4, 5]
                );
                assert_eq!(root.tail(), "suffix");
                root.validate().unwrap();
            }
        }
    }
}

#[test]
fn bulk_preflights_later_inputs_and_empty_run_bounds_without_writes() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    Shapes::initialize(
        &mut store,
        ShapesInit {
            native: &[],
            views: &[],
            lists: &[],
            options: &[],
            maps: &[],
        },
    )
    .unwrap();
    let before = store.bytes().to_vec();
    {
        let mut root = Shapes::load_mut(&mut store).unwrap();
        assert_eq!(
            root.maps_mut()
                .extend_from_slice(&[&[(1, "valid")], &[(2, "bad"), (1, "order")]]),
            Err(fault(ErrorKind::UnsortedKeys))
        );
        for index in [1, u32::MAX as usize, usize::MAX]
            .into_iter()
            .chain((u32::MAX as usize).checked_add(1))
        {
            assert_eq!(
                root.lists_mut().insert_from_slice(index, &[]),
                Err(fault(ErrorKind::IndexOutOfBounds))
            );
            assert_eq!(
                root.lists_mut().insert_from_slice(index, &[&[1, 2]]),
                Err(fault(ErrorKind::IndexOutOfBounds))
            );
        }
        root.lists_mut().extend_from_slice(&[]).unwrap();
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn bulk_rejects_corrupt_structure_even_for_empty_runs() {
    for values in [&[][..], &["", "x"][..]] {
        let mut room = [0; 128];
        let mut store = SliceStore::new(&mut room);
        Lists::initialize(&mut store, init()).unwrap();
        let base = View::load_trusted::<Lists>(store.bytes(), Lists::DISCRIMINATOR)
            .unwrap()
            .field(1)
            .unwrap()
            .base() as usize
            + 4;
        store.bytes_mut()[base..base + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = store.bytes().to_vec();
        let _ = values;
        assert!(Lists::load_checked_mut(&mut store).is_err());
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
    }
}

#[test]
fn bytes_extend_accepts_an_ordinary_byte_slice() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let edit =
        blubber::raw::Edit::initialize::<blubber::List<u8>>(&mut store, *b"bytebulk", &[]).unwrap();
    let mut list = blubber::raw::project::<blubber::List<u8>>(edit);
    list.extend_from_slice(b"abc").unwrap();
    list.insert_from_slice(1, b"de").unwrap();
    assert_eq!((&list).into_iter().collect::<Vec<_>>(), b"adebc");
    list.validate().unwrap();
}

#[test]
fn mutable_lengths_follow_empty_single_and_bulk_edits() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Lists::initialize(&mut store, init()).unwrap();
    let mut list = root.numbers_mut();
    assert_eq!(list.len(), 2);
    assert_eq!(list.get(1), Some(20));
    list.clear().unwrap();
    assert_eq!(list.len(), 0);
    assert!(list.is_empty());
    assert_eq!(list.get(0), None);
    list.extend_from_slice(&[7]).unwrap();
    assert_eq!(list.len(), 1);
    assert!(!list.is_empty());
    list.insert_from_slice(0, &[3, 5]).unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list.get(2), Some(7));
    assert_eq!(
        list.try_iter()
            .unwrap()
            .collect::<blubber::Result<Vec<_>>>(),
        Ok(vec![3, 5, 7])
    );
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn selected_list_validation_rejects_corrupt_directory() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    Lists::initialize(&mut store, init()).unwrap();
    let boundary = View::load_trusted::<Lists>(store.bytes(), Lists::DISCRIMINATOR)
        .unwrap()
        .field(1)
        .unwrap()
        .base() as usize
        + 4;
    store.bytes_mut()[boundary..boundary + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let before = store.bytes().to_vec();
    assert_eq!(
        Lists::load_trusted_mut(&mut store)
            .unwrap()
            .strings_mut()
            .validate(),
        Err(Error::new(ErrorKind::InvalidBoundary, boundary as u32))
    );
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[path = "../../blubber-core/tests/support/patch_pass.rs"]
mod patch_pass;

#[path = "support/list_patch.rs"]
mod list_patch;

#[path = "../../blubber-core/tests/support/word_moves.rs"]
mod word_moves;
