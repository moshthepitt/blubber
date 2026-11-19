//! Reborrowed field projections, nested replacement and relocation-safe short reads.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::raw::{Edit, Resize};
use blubber::Resize as _;
use blubber::{Error, ErrorKind, List, Map, ReadStorage, Schema, SliceStore, Str};
use blubber_core::MAX_DEPTH;

#[derive(Clone, Copy, Debug, PartialEq, Eq, blubber::Fixed)]
struct Native {
    enabled: bool,
    number: u64,
}

#[blubber::schema]
struct Flags {
    flag: bool,
    number: u16,
}

#[blubber::schema]
struct Child {
    label: Str,
    tags: List<u16>,
    choice: Option<Str>,
}

#[blubber::schema(account)]
struct Account {
    front: Str,
    child: Child,
    maybe: Option<Child>,
    rows: List<Child>,
    fixed_rows: List<Flags>,
    numbers: List<u32>,
    map: Map<i16, u32>,
    objects: Map<u16, Child>,
    tail: Str,
    count: u128,
    native: Native,
    raw: [u8; 3],
}

fn child(label: &str) -> ChildInit<'_> {
    ChildInit {
        label,
        tags: &[3, 8],
        choice: Some("é"),
    }
}

fn init() -> AccountInit<'static> {
    AccountInit {
        front: "α",
        child: child("child"),
        maybe: Some(child("maybe")),
        rows: &[
            ChildInit {
                label: "first",
                tags: &[1],
                choice: None,
            },
            ChildInit {
                label: "last",
                tags: &[],
                choice: Some("x"),
            },
        ],
        fixed_rows: &[FlagsInit {
            flag: true,
            number: 7,
        }],
        numbers: &[10, 20],
        map: &[(-2, 4), (5, 9)],
        objects: &[
            (
                1,
                ChildInit {
                    label: "one",
                    tags: &[2],
                    choice: None,
                },
            ),
            (
                9,
                ChildInit {
                    label: "nine",
                    tags: &[],
                    choice: None,
                },
            ),
        ],
        tail: "tail",
        count: 42,
        native: Native {
            enabled: true,
            number: 10,
        },
        raw: [1, 2, 3],
    }
}

fn error(kind: ErrorKind) -> Error {
    kind.into()
}

#[test]
fn scalar_edits_cover_native_structs_arrays_and_full_width_values() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    let used = root.used_len();
    let mut counter = root.count_mut();
    assert_eq!(counter.try_get(), Ok(42));
    counter.set(u128::MAX).unwrap();
    assert_eq!(counter.try_get(), Ok(u128::MAX));
    root.native_mut()
        .set(Native {
            enabled: false,
            number: u64::MAX,
        })
        .unwrap();
    root.raw_mut().set([0, 255, 128]).unwrap();
    assert_eq!(
        root.native(),
        Native {
            enabled: false,
            number: u64::MAX
        }
    );
    assert_eq!(root.raw(), [0, 255, 128]);
    assert_eq!(root.used_len(), used);
    assert_eq!(root.front(), "α");
    root.validate().unwrap();
}

#[test]
fn nested_fields_keep_only_integer_paths() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    {
        let mut record = root.child_mut();
        assert_eq!(record.label(), "child");
        assert_eq!(record.try_label(), Ok("child"));
        record.label_mut().set("a much longer child").unwrap();
        assert_eq!(record.label(), "a much longer child");
        assert_eq!(record.tags().get(1), Some(8));
        record
            .choice_mut()
            .some_mut()
            .unwrap()
            .set("changed")
            .unwrap();
        record.validate().unwrap();
    }
    assert_eq!(root.child().choice(), Some("changed"));
    assert_eq!(root.tail(), "tail");
    root.validate().unwrap();
}

#[test]
fn reborrow_passes_retained_handles_to_helpers_by_value() {
    fn root_helper(mut root: AccountMut<'_>) -> blubber::Result<()> {
        root.front_mut().set("helper")
    }
    fn record_helper(mut record: ChildMut<'_>) -> blubber::Result<()> {
        record.label_mut().push_str(" helper")?;
        record.choice_mut().set(None)
    }
    fn scalar_helper(mut scalar: blubber::ScalarMut<'_, u16>) -> blubber::Result<()> {
        scalar.set(99)
    }
    fn string_helper(mut text: blubber::StrMut<'_>) -> blubber::Result<()> {
        text.push_str(" passed")
    }
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    root_helper(root.reborrow()).unwrap();
    assert_eq!(root.front(), "helper");
    let mut record = root.rows_mut().at_mut(0).unwrap();
    record_helper(record.reborrow()).unwrap();
    assert_eq!(record.label(), "first helper");
    assert_eq!(record.choice(), None);
    let mut text = record.label_mut();
    string_helper(text.reborrow()).unwrap();
    assert_eq!(text.try_get(), Ok("first helper passed"));
    let mut tags = record.tags_mut();
    let mut tag = tags.reborrow().at_mut(0).unwrap();
    scalar_helper(tag.reborrow()).unwrap();
    assert_eq!(tag.try_get(), Ok(99));
    assert_eq!(tags.get(0), Some(99));
    root.validate().unwrap();
}

#[test]
fn bound_record_and_list_reselect_children_after_sibling_growth() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    let mut record = root.rows_mut().at_mut(0).unwrap();
    record.label_mut().push_str(" still here").unwrap();
    let mut tags = record.tags_mut();
    tags.reborrow().at_mut(0).unwrap().set(98).unwrap();
    assert_eq!(tags.get(0), Some(98));
    record.choice_mut().set(Some("grown sibling")).unwrap();
    record.label_mut().set("short").unwrap();
    assert_eq!(record.label(), "short");
    assert_eq!(record.tags().get(0), Some(98));
    assert_eq!(record.choice(), Some("grown sibling"));
    root.validate().unwrap();
}

#[test]
fn strings_replace_append_and_shrink_without_disturbing_later_subtrees() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    let original_used = root.used_len();
    let mut text = root.front_mut();
    text.push_str("β🦀").unwrap();
    assert_eq!(text.try_get(), Ok("αβ🦀"));
    text.set("long replacement").unwrap();
    text.set("xy").unwrap();
    text.clear().unwrap();
    text.push_str("α").unwrap();
    assert_eq!(root.front(), "α");
    assert_eq!(root.used_len(), original_used);
    assert_eq!(root.child().tags().iter().collect::<Vec<_>>(), [3, 8]);
    assert_eq!(root.objects().get(&9).unwrap().label(), "nine");
    root.validate().unwrap();
}

#[test]
fn string_truncation_checks_utf8_boundaries_and_large_lengths_are_noops() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    root.front_mut().set("aé🦀").unwrap();
    let used = root.used_len();
    assert_eq!(
        root.front_mut().truncate(2),
        Err(error(ErrorKind::Utf8Boundary))
    );
    assert_eq!(root.front(), "aé🦀");
    assert_eq!(root.used_len(), used);
    root.front_mut().truncate(usize::MAX).unwrap();
    root.front_mut().truncate(7).unwrap();
    root.front_mut().truncate(3).unwrap();
    assert_eq!(root.front(), "aé");
    root.front_mut().truncate(0).unwrap();
    assert_eq!(root.front(), "");
    root.validate().unwrap();
}

#[test]
fn option_replacement_some_projection_and_absence_are_atomic() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    let mut option = root.maybe_mut();
    assert_eq!(option.try_get().unwrap().unwrap().label(), "maybe");
    option
        .reborrow()
        .some_mut()
        .unwrap()
        .label_mut()
        .set("nested growth")
        .unwrap();
    option.set(None).unwrap();
    assert!(option.try_get().unwrap().is_none());
    assert!(option.reborrow().get_mut().unwrap().is_none());
    assert!(matches!(option.reborrow().some_mut(), Err(e) if e == error(ErrorKind::MissingValue)));
    option.set(Some(child("new payload"))).unwrap();
    assert_eq!(option.try_get().unwrap().unwrap().label(), "new payload");
    option.some_mut().unwrap().choice_mut().set(None).unwrap();
    assert_eq!(root.maybe().unwrap().choice(), None);
    root.validate().unwrap();
}

#[test]
fn fixed_and_indexed_lists_project_scalars_and_local_mut_types() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    {
        let mut rows = root.rows_mut();
        assert_eq!(rows.try_len(), Ok(2));
        assert!(!rows.is_empty());
        assert_eq!(rows.get(0).unwrap().label(), "first");
        assert_eq!(rows.try_get(usize::MAX).unwrap().map(|r| r.label()), None);
        assert_eq!(rows.iter().count(), 2);
        assert_eq!(rows.try_iter().unwrap().count(), 2);
        rows.reborrow()
            .at_mut(0)
            .unwrap()
            .label_mut()
            .push_str(" longer")
            .unwrap();
        rows.reborrow()
            .at_mut(1)
            .unwrap()
            .label_mut()
            .set("")
            .unwrap();
        assert_eq!(rows.get(0).unwrap().label(), "first longer");
        assert_eq!(rows.get(1).unwrap().label(), "");
        assert!(rows.reborrow().get_mut(usize::MAX).unwrap().is_none());
        assert!(
            matches!(rows.reborrow().at_mut(2), Err(e) if e == error(ErrorKind::IndexOutOfBounds))
        );
        rows.validate().unwrap();
    }
    root.numbers_mut().at_mut(1).unwrap().set(u32::MAX).unwrap();
    root.fixed_rows_mut()
        .at_mut(0)
        .unwrap()
        .number_mut()
        .set(65535)
        .unwrap();
    root.fixed_rows_mut()
        .at_mut(0)
        .unwrap()
        .flag_mut()
        .set(false)
        .unwrap();
    assert_eq!(root.numbers().get(1), Some(u32::MAX));
    assert_eq!(root.fixed_rows().get(0).unwrap().number(), 65535);
    assert!(!root.fixed_rows().get(0).unwrap().flag());
    root.validate().unwrap();
}

#[test]
fn fixed_and_indexed_maps_project_values_without_exposing_keys() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    root.map_mut().at_mut(&-2).unwrap().set(123).unwrap();
    let mut map = root.objects_mut();
    assert_eq!(map.try_len(), Ok(2));
    assert!(!map.is_empty());
    assert!(!map.try_is_empty().unwrap());
    assert!(map.contains_key(&1));
    assert_eq!(map.try_contains_key(&10), Ok(false));
    assert_eq!(map.try_get(&9).unwrap().unwrap().label(), "nine");
    assert_eq!(map.iter().map(|(k, _)| k).collect::<Vec<_>>(), [1, 9]);
    assert_eq!(map.try_iter().unwrap().count(), 2);
    map.reborrow()
        .at_mut(&1)
        .unwrap()
        .label_mut()
        .set("new first map value")
        .unwrap();
    map.reborrow()
        .at_mut(&9)
        .unwrap()
        .label_mut()
        .clear()
        .unwrap();
    assert_eq!(map.get(&1).unwrap().label(), "new first map value");
    assert_eq!(map.get(&9).unwrap().label(), "");
    assert!(map.reborrow().get_mut(&8).unwrap().is_none());
    assert!(matches!(map.reborrow().at_mut(&8), Err(e) if e == error(ErrorKind::MissingKey)));
    map.validate().unwrap();
    assert_eq!(root.map().get(&-2), Some(123));
    assert_eq!(root.tail(), "tail");
    root.validate().unwrap();
}

#[test]
fn capacity_failure_preserves_the_entire_storage_and_retained_projection() {
    let used = Account::encoded_len_of(&init()).unwrap() as usize;
    let mut room = vec![0; used];
    let before;
    {
        let mut store = SliceStore::new(&mut room);
        Account::initialize(&mut store, init()).unwrap();
        before = store.bytes().to_vec();
        let mut root = Account::load_mut(&mut store).unwrap();
        let mut record = root.child_mut();
        let mut text = record.label_mut();
        assert_eq!(text.push_str("extra"), Err(error(ErrorKind::Capacity)));
        assert_eq!(text.try_get(), Ok("child"));
        root.validate().unwrap();
    }
    assert_eq!(room, before);
}

#[test]
fn shrinks_clear_only_the_vacated_used_tail_and_preserve_spare() {
    let mut room = [0; 512];
    let old_used;
    let new_used;
    {
        let mut store = SliceStore::new(&mut room);
        old_used = Account::initialize(&mut store, init()).unwrap().used_len() as usize;
        store.bytes_mut()[old_used..].fill(0xa5);
        let mut root = Account::load_mut(&mut store).unwrap();
        root.child_mut().label_mut().clear().unwrap();
        new_used = root.used_len() as usize;
        assert_eq!(new_used + 5, old_used);
        root.validate().unwrap();
    }
    assert!(room[new_used..old_used].iter().all(|&b| b == 0));
    assert!(room[old_used..].iter().all(|&b| b == 0xa5));
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn unrelated_corrupt_utf8_is_unvisited_but_retained_prefixes_are_checked() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    Account::initialize(&mut store, init()).unwrap();
    let tail = blubber::View::load_trusted::<Account>(store.bytes(), Account::DISCRIMINATOR)
        .unwrap()
        .field(8)
        .unwrap()
        .base() as usize;
    store.bytes_mut()[tail] = 0xff;
    let mut root = Account::load_trusted_mut(&mut store).unwrap();
    root.child_mut()
        .label_mut()
        .set("valid replacement")
        .unwrap();
    assert_eq!(root.child().label(), "valid replacement");
    assert!(matches!(
        root.try_tail(),
        Err(Error::Fault {
            kind: ErrorKind::InvalidUtf8,
            ..
        })
    ));
    assert!(root.validate().is_err());
    root.tail_mut().set("repaired").unwrap();
    root.validate().unwrap();
    let text = root.tail_mut();
    assert_eq!(text.try_get(), Ok("repaired"));
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn malformed_ancestors_fail_before_writing_or_growing_storage() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    Account::initialize(&mut store, init()).unwrap();
    let prefix = match Account::LAYOUT {
        blubber_core::Layout::Struct { fixed_bytes, .. } => *fixed_bytes,
        _ => unreachable!(),
    };
    store.bytes_mut()[12 + prefix as usize..16 + prefix as usize]
        .copy_from_slice(&u32::MAX.to_le_bytes());
    let before = store.bytes().to_vec();
    {
        let mut root = Account::load_trusted_mut(&mut store).unwrap();
        assert!(matches!(
            root.child_mut().label_mut().set("x"),
            Err(Error::Fault {
                kind: ErrorKind::InvalidBoundary,
                ..
            })
        ));
    }
    assert_eq!(store.bytes(), before);
    assert!(store.is_poisoned());
    assert!(matches!(Account::load_trusted_mut(&mut store), Err(error)
        if error == ErrorKind::Poisoned.into()));
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn local_selection_is_bounded_while_full_validation_rejects_descending_keys() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    Account::initialize(&mut store, init()).unwrap();
    let base = blubber::View::load_trusted::<Account>(store.bytes(), Account::DISCRIMINATOR)
        .unwrap()
        .field(7)
        .unwrap()
        .base() as usize;
    // Variable map directory: [key:u16 big endian][end:u32 little endian].
    store.bytes_mut()[base + 4..base + 6].copy_from_slice(&10u16.to_be_bytes());
    let before = store.bytes().to_vec();
    {
        let mut root = Account::load_trusted_mut(&mut store).unwrap();
        assert!(root.validate().is_err());
        assert!(root.objects_mut().get_mut(&5).unwrap().is_none());
        // The checked endpoint remains selectable despite distant bad order.
        assert_eq!(
            root.objects_mut().get_mut(&9).unwrap().unwrap().label(),
            "nine"
        );
    }
    assert_eq!(store.bytes(), before);
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn option_tags_and_fixed_child_domains_are_checked_with_absolute_offsets() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    Account::initialize(&mut store, init()).unwrap();
    let base = blubber::View::load_trusted::<Account>(store.bytes(), Account::DISCRIMINATOR)
        .unwrap()
        .field(2)
        .unwrap()
        .base() as usize;
    store.bytes_mut()[base] = 2;
    let before = store.bytes().to_vec();
    {
        let mut root = Account::load_trusted_mut(&mut store).unwrap();
        assert!(
            matches!(root.maybe_mut().get_mut(), Err(e) if e == Error::new(ErrorKind::InvalidTag, base as u32))
        );
        assert_eq!(root.maybe_mut().set(None), Err(ErrorKind::Poisoned.into()));
    }
    assert_eq!(store.bytes(), before);
}

type ThreeOptions = Option<Option<Option<Str>>>;
type SixOptions = Option<Option<Option<ThreeOptions>>>;

#[blubber::schema(account)]
struct Deep {
    value: SixOptions,
}

#[test]
fn maximum_default_depth_is_accepted_and_hidden_overflow_is_sticky() {
    let mut room = [0; 64];
    let mut store = SliceStore::new(&mut room);
    {
        let mut root = Deep::initialize(
            &mut store,
            DeepInit {
                value: Some(Some(Some(Some(Some(Some("a")))))),
            },
        )
        .unwrap();
        root.value_mut()
            .some_mut()
            .unwrap()
            .some_mut()
            .unwrap()
            .some_mut()
            .unwrap()
            .some_mut()
            .unwrap()
            .some_mut()
            .unwrap()
            .some_mut()
            .unwrap()
            .push_str("b")
            .unwrap();
        assert_eq!(root.value(), Some(Some(Some(Some(Some(Some("ab")))))));
        root.validate().unwrap();
    }
    let mut edit = Edit::load::<Deep>(&mut store, Deep::DISCRIMINATOR).unwrap();
    // Even an erroneous hidden projection cannot overflow its inline buffer.
    for _ in 0..MAX_DEPTH + 2 {
        edit = edit.field(0);
    }
    assert_eq!(edit.view().unwrap_err(), error(ErrorKind::DepthExceeded));
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn scalar_set_repairs_invalid_old_domains_without_changing_width() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    Account::initialize(&mut store, init()).unwrap();
    let base = blubber::View::load_trusted::<Account>(store.bytes(), Account::DISCRIMINATOR)
        .unwrap()
        .field(10)
        .unwrap()
        .base() as usize;
    store.bytes_mut()[base] = 2;
    let mut root = Account::load_trusted_mut(&mut store).unwrap();
    assert_eq!(
        root.native_mut().try_get(),
        Err(Error::new(ErrorKind::InvalidBool, base as u32))
    );
    root.native_mut()
        .set(Native {
            enabled: true,
            number: 1,
        })
        .unwrap();
    root.validate().unwrap();
}

#[cfg(feature = "alloc")]
#[test]
fn vec_growth_retains_nested_handles_and_commit_reports_the_current_extent() {
    let mut bytes = Vec::new();
    let mut store = blubber::VecStore::new(&mut bytes);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    let mut record = root.rows_mut().at_mut(0).unwrap();
    record
        .label_mut()
        .set("a longer first record after growth")
        .unwrap();
    record.choice_mut().set(Some("new option")).unwrap();
    assert_eq!(record.label(), "a longer first record after growth");
    assert_eq!(record.choice(), Some("new option"));
    root.tail_mut().clear().unwrap();
    root.reserve(256).unwrap();
    root.validate().unwrap();
    let used = root.used_len();
    assert!(root.shrink_to_fit().unwrap());
    let receipt = root.commit().unwrap();
    assert_eq!(receipt.used_len, used);
    assert_eq!(receipt.storage_len, used);
    assert!(store.is_closed());
    Account::load(&bytes).unwrap();
}

#[test]
fn closed_and_poisoned_roots_reject_all_terminal_editors() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    Account::initialize(&mut store, init()).unwrap();
    store.set_poisoned(true);
    assert!(
        matches!(Account::load_trusted_mut(&mut store), Err(e) if e == error(ErrorKind::Poisoned))
    );
    let mut root = Account::load_mut(&mut store).unwrap();
    root.reborrow().commit().unwrap();
    assert_eq!(root.front_mut().set("x"), Err(error(ErrorKind::Closed)));
    assert_eq!(root.count_mut().set(0), Err(error(ErrorKind::Closed)));
    assert_eq!(root.maybe_mut().set(None), Err(error(ErrorKind::Closed)));
    assert!(matches!(root.rows_mut().get_mut(0), Err(e) if e == error(ErrorKind::Closed)));
    assert!(matches!(root.objects_mut().get_mut(&1), Err(e) if e == error(ErrorKind::Closed)));
    assert_eq!(root.try_front(), Err(error(ErrorKind::Closed)));
}

#[blubber::schema(account)]
struct OptionalInputs {
    scalar: Option<u64>,
    list: Option<List<Str>>,
    map: Option<Map<u16, Str>>,
}

#[test]
fn option_set_measures_borrowed_collections_and_rejects_bad_map_order() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = OptionalInputs::initialize(
        &mut store,
        OptionalInputsInit {
            scalar: None,
            list: None,
            map: None,
        },
    )
    .unwrap();
    root.scalar_mut().set(Some(u64::MAX)).unwrap();
    root.scalar_mut().some_mut().unwrap().set(7).unwrap();
    root.list_mut().set(Some(&["one", "", "three"])).unwrap();
    root.list_mut()
        .some_mut()
        .unwrap()
        .at_mut(1)
        .unwrap()
        .set("two")
        .unwrap();
    root.map_mut()
        .set(Some(&[(1, "first"), (2, "last")]))
        .unwrap();
    let used = root.used_len();
    assert_eq!(
        root.map_mut().set(Some(&[(2, "bad"), (1, "order")])),
        Err(error(ErrorKind::UnsortedKeys))
    );
    assert_eq!(
        root.map_mut().set(Some(&[(1, "bad"), (1, "duplicate")])),
        Err(error(ErrorKind::DuplicateKey))
    );
    assert_eq!(root.used_len(), used);
    assert_eq!(root.scalar(), Some(7));
    assert_eq!(
        root.list().unwrap().iter().collect::<Vec<_>>(),
        ["one", "two", "three"]
    );
    assert_eq!(root.map().unwrap().get(&2), Some("last"));
    root.validate().unwrap();
}

struct Interrupt;
impl Schema for Interrupt {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:projections:Interrupt",
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
    type Mut<'a> = Edit<'a>;
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
    fn decode_owned(view: blubber::View<'_>) -> blubber::Result<u8> {
        Self::read(view)
    }

    fn read<'a>(view: blubber::View<'a>) -> blubber::Result<u8> {
        view.read::<u8>()
    }
    fn read_removed(view: blubber::View<'_>) -> blubber::Result<u8> {
        Self::read(view)
    }
    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> Edit<'a> {
        blubber::raw::into_edit(edit)
    }
    fn measure(_: &u8) -> blubber::Result<u32> {
        Ok(1)
    }
    fn encode(init: &u8, destination: &mut [u8]) {
        destination[0] = *init;
        assert_ne!(*init, 99, "injected codec unwind");
    }
}

#[blubber::schema(account)]
struct PanicAccount {
    value: Option<Interrupt>,
    text: Str,
    scalar: u64,
    list: List<u8>,
    map: Map<u8, u8>,
}

#[test]
fn post_move_codec_unwind_poison_blocks_read_write_and_lifecycle_operations() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    {
        let mut root = PanicAccount::initialize(
            &mut store,
            PanicAccountInit {
                value: None,
                text: "text",
                scalar: 7,
                list: &[1],
                map: &[(1, 2)],
            },
        )
        .unwrap();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            root.value_mut().set(Some(99)).unwrap();
        }));
        assert!(panic.is_err());
        assert_eq!(root.try_text(), Err(error(ErrorKind::Poisoned)));
        assert_eq!(root.text_mut().try_get(), Err(error(ErrorKind::Poisoned)));
        assert_eq!(root.text_mut().set("x"), Err(error(ErrorKind::Poisoned)));
        assert_eq!(root.scalar_mut().set(1), Err(error(ErrorKind::Poisoned)));
        assert_eq!(root.value_mut().set(None), Err(error(ErrorKind::Poisoned)));
        assert!(matches!(root.list_mut().get_mut(0), Err(e) if e == error(ErrorKind::Poisoned)));
        assert!(matches!(root.map_mut().get_mut(&1), Err(e) if e == error(ErrorKind::Poisoned)));
        assert_eq!(root.reserve(1), Err(error(ErrorKind::Poisoned)));
        assert_eq!(root.shrink_to_fit(), Err(error(ErrorKind::Poisoned)));
        assert_eq!(root.validate(), Err(error(ErrorKind::Poisoned)));
    }
    assert!(store.is_poisoned());
    assert!(PanicAccount::load_mut(&mut store).is_err());
    assert!(store.is_poisoned());
}

#[test]
fn leaf_getters_and_into_iter_match_checked_reads() {
    let mut room = [0; 512];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    assert_eq!(root.count_mut().get(), 42);
    assert_eq!(root.front_mut().get(), "α");
    assert_eq!(root.child_mut().choice_mut().get(), Some("é"));
    root.child_mut().choice_mut().set(None).unwrap();
    assert_eq!(root.child_mut().choice_mut().get(), None);
    let rows = root.rows();
    let names: Vec<_> = rows.into_iter().map(|row| row.label()).collect();
    assert_eq!(names, ["first", "last"]);
    assert_eq!(rows.len(), 2); // Copy views remain usable after iteration.
    let map = root.map();
    assert_eq!(map.into_iter().collect::<Vec<_>>(), [(-2, 4), (5, 9)]);
    assert_eq!(map.len(), 2);
    let mut rows = root.rows_mut();
    assert_eq!(
        (&rows)
            .into_iter()
            .map(|row| row.label())
            .collect::<Vec<_>>(),
        ["first", "last"]
    );
    rows.clear().unwrap();
    assert!((&rows).into_iter().next().is_none());
    let mut map = root.map_mut();
    assert_eq!((&map).into_iter().collect::<Vec<_>>(), [(-2, 4), (5, 9)]);
    map.clear().unwrap();
    assert!((&map).into_iter().next().is_none());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn infallible_leaf_getters_panic_on_the_same_faults_try_get_reports() {
    #[blubber::schema(account)]
    struct Leaves {
        flag: bool,
        text: Str,
        choice: Option<Str>,
    }
    for (field, kind) in [
        (0, ErrorKind::InvalidBool),
        (1, ErrorKind::InvalidUtf8),
        (2, ErrorKind::InvalidTag),
    ] {
        let mut room = [0; 64];
        let mut store = SliceStore::new(&mut room);
        Leaves::initialize(
            &mut store,
            LeavesInit {
                flag: true,
                text: "x",
                choice: Some("y"),
            },
        )
        .unwrap();
        let at = blubber::View::load_trusted::<Leaves>(store.bytes(), Leaves::DISCRIMINATOR)
            .unwrap()
            .field(field)
            .unwrap()
            .base();
        store.bytes_mut()[at as usize] = 0xff;
        let mut root = Leaves::load_trusted_mut(&mut store).unwrap();
        let checked = match field {
            0 => root.flag_mut().try_get().map(|_| ()),
            1 => root.text_mut().try_get().map(|_| ()),
            _ => root.choice_mut().try_get().map(|_| ()),
        };
        assert_eq!(checked, Err(Error::new(kind, at)));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match field {
                0 => {
                    let _ = root.flag_mut().get();
                }
                1 => {
                    let _ = root.text_mut().get();
                }
                _ => {
                    let _ = root.choice_mut().get();
                }
            }))
            .is_err()
        );
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn invalid_hidden_projection_precedes_unvisited_partition_faults() {
    let mut room = [0; 1024];
    let mut store = SliceStore::new(&mut room);
    Account::initialize(&mut store, init()).unwrap();
    let blubber_core::Layout::Struct { fixed_bytes, .. } = Account::LAYOUT else {
        panic!("account struct");
    };
    let boundary = 12 + *fixed_bytes as usize;
    store.bytes_mut()[boundary..boundary + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let before = store.bytes().to_vec();
    let edit = Edit::load_trusted::<Account>(&mut store, Account::DISCRIMINATOR).unwrap();
    assert_eq!(
        edit.field(u16::MAX).view().unwrap_err(),
        error(ErrorKind::IndexOutOfBounds)
    );
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[test]
fn nested_hidden_edits_cannot_run_root_lifecycle_operations() {
    for operation in 0..3 {
        let mut room = [0; 1024];
        let mut store = SliceStore::new(&mut room);
        Account::initialize(&mut store, init()).unwrap();
        let before = store.bytes().to_vec();
        let mut edit = Edit::load::<Account>(&mut store, Account::DISCRIMINATOR)
            .unwrap()
            .field(0);
        let result = match operation {
            0 => edit.reserve(0),
            1 => edit.shrink_to_fit().map(|_| ()),
            _ => edit.commit().map(|_| ()),
        };
        assert_eq!(result, Err(error(ErrorKind::InvalidSchema)));
        assert_eq!(store.bytes(), before);
        assert!(!store.is_closed());
        assert!(!store.is_poisoned());
    }
}

#[test]
fn read_projection_keeps_empty_region_schema_error_precedence() {
    let disc = *b"emptyreg";
    let mut bytes = [0xa5; 32];
    bytes[..8].copy_from_slice(&disc);
    bytes[8..12].copy_from_slice(&12u32.to_le_bytes());
    let before = bytes;
    {
        let mut store = SliceStore::new(&mut bytes);
        {
            let edit = Edit::load_trusted::<Str>(&mut store, disc).unwrap();
            let list = blubber::raw::project::<List<u8>>(edit);
            assert_eq!(list.try_iter().err(), Some(ErrorKind::InvalidSchema.into()));
        }
        assert!(!store.is_poisoned());
    }
    assert_eq!(bytes, before);
}

#[blubber::schema]
struct ReadTail {
    prefix: u8,
    values: List<u8>,
}

#[test]
fn read_projection_rejects_stale_tail_coordinates_before_slicing() {
    use blubber::RootSchema;
    let disc = *b"stalereg";
    let mut bytes = [0xa5; 40];
    bytes[..8].copy_from_slice(&disc);
    bytes[8..12].copy_from_slice(&17u32.to_le_bytes());
    bytes[12..17].fill(0);
    let before = bytes;
    {
        let mut store = SliceStore::new(&mut bytes);
        {
            let mut root = Edit::load_trusted::<ReadTail>(&mut store, disc).unwrap();
            // Model a stale generated coordinate while retaining genuine schema
            // identity and the current valid account. The read must reject it.
            let selected = ReadTail::__layout().child(0, true);
            let child = root.field_reborrow_certified(1, None, selected, Some(20));
            let list = blubber::raw::project::<List<u8>>(child);
            assert_eq!(
                list.try_iter().err(),
                Some(ErrorKind::InvalidBoundary.into())
            );
        }
        assert!(!store.is_poisoned());
    }
    assert_eq!(bytes, before);
}

#[test]
fn mutable_debug_formats_metadata_and_preserves_selections() {
    #[derive(Clone, Copy, blubber::Fixed)]
    struct WithoutDebug {
        value: u64,
    }
    fn debug<T: core::fmt::Debug>() {}
    debug::<blubber::ScalarMut<'_, WithoutDebug>>();
    debug::<blubber::OptionMut<'_, Child>>();
    debug::<blubber::ListMut<'_, Child>>();
    debug::<blubber::MapMut<'_, u16, Child>>();
    debug::<blubber::Entry<'_, u16, Child>>();
    debug::<blubber::OccupiedEntry<'_, u16, Child>>();
    debug::<blubber::VacantEntry<'_, u16, Child>>();
    debug::<blubber::ListCursorMut<'_, Child>>();
    debug::<blubber::MapCursorMut<'_, u16, Child>>();

    let mut room = [0; 2048];
    let mut store = SliceStore::new(&mut room);
    let mut root = Account::initialize(&mut store, init()).unwrap();
    assert_eq!(format!("{root:?}"), "AccountMut { .. }");
    assert!(format!("{:?}", root.front_mut()).starts_with("StrMut"));
    assert!(format!("{:?}", root.count_mut()).starts_with("ScalarMut"));
    assert!(format!("{:?}", root.maybe_mut()).starts_with("OptionMut"));
    {
        let mut list = root.rows_mut();
        assert!(format!("{list:?}").starts_with("ListMut"));
        {
            let mut cursor = list.reborrow().cursor_mut().unwrap();
            let metadata = format!("{cursor:?}");
            assert!(metadata.contains("indexed"));
            assert_eq!(cursor.next().unwrap().unwrap().label(), "first");
            assert!(format!("{cursor:?}").contains("next: 1"));
            assert_eq!(cursor.next().unwrap().unwrap().label(), "last");
        }
        assert_eq!(
            (&list).into_iter().map(|v| v.label()).collect::<Vec<_>>(),
            ["first", "last"]
        );
    }
    let mut map = root.objects_mut();
    assert!(format!("{map:?}").starts_with("MapMut"));
    let mut cursor = map.reborrow().cursor_mut().unwrap();
    assert!(format!("{cursor:?}").contains("indexed"));
    assert_eq!(cursor.next().unwrap().unwrap().0, 1);
    assert_eq!(cursor.next().unwrap().unwrap().0, 9);
    assert!(cursor.next().unwrap().is_none());
    assert!(cursor.next().unwrap().is_none());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn mutable_debug_does_not_admit_invalid_text_or_poison() {
    let mut room = [0; 2048];
    let mut store = SliceStore::new(&mut room);
    drop(Account::initialize(&mut store, init()).unwrap());
    let at = blubber::View::load::<Account>(store.bytes(), Account::DISCRIMINATOR)
        .unwrap()
        .field(0)
        .unwrap()
        .base() as usize;
    store.bytes_mut()[at] = 0xff;
    let before = store.bytes().to_vec();
    let mut root = Account::load_trusted_mut(&mut store).unwrap();
    assert_eq!(format!("{root:?}"), "AccountMut { .. }");
    assert!(format!("{:?}", root.front_mut()).starts_with("StrMut"));
    drop(root);
    assert!(!store.is_poisoned());
    assert_eq!(store.bytes(), before);
}
