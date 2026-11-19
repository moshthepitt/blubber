//! Sorted typed maps, old values, structural failures and atomic publication.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "support/exhaustive_typed_plan.rs"]
mod exhaustive;

blubber::__root_schema!(__ConcreteRoot0, Map<u8, Interrupt>);

#[cfg(not(feature = "verify-on-load"))]
use blubber::raw::Resize;
#[cfg(not(feature = "verify-on-load"))]
use blubber::Error;
use blubber::Resize as _;
use blubber::{ErrorKind, List, Map, ReadStorage, SliceStore, Str, View};

#[blubber::schema(account)]
struct Maps {
    fixed: Map<i16, u16>,
    strings: Map<u16, Str>,
    tail: Str,
}

fn init() -> MapsInit<'static> {
    MapsInit {
        fixed: &[(-1, 10), (256, 20)],
        strings: &[(1, "a"), (256, "")],
        tail: "untouched suffix",
    }
}

#[test]
fn fixed_upserts_and_removals_copy_values_and_keep_signed_key_order() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Maps::initialize(&mut store, init()).unwrap();
    let mut map = root.fixed_mut();
    for (key, value) in [(i16::MIN, 1), (0, 2), (i16::MAX, 3)] {
        assert_eq!(map.insert(key, value), Ok(None));
    }
    assert_eq!(map.insert(-1, 99), Ok(Some(10)));
    assert_eq!(
        map.iter().collect::<Vec<_>>(),
        [(i16::MIN, 1), (-1, 99), (0, 2), (256, 20), (i16::MAX, 3)]
    );
    assert_eq!(map.remove(&0), Ok(Some(2)));
    assert_eq!(map.remove(&i16::MIN), Ok(Some(1)));
    assert_eq!(map.remove(&i16::MAX), Ok(Some(3)));
    assert_eq!(map.remove(&42), Ok(None));
    map.clear().unwrap();
    map.clear().unwrap();
    assert_eq!(map.insert(7, 0), Ok(None));
    assert_eq!(map.remove(&7), Ok(Some(0)));
    assert_eq!(root.tail(), "untouched suffix");
    assert_eq!(root.strings().get(&1), Some("a"));
    root.validate().unwrap();
}

#[test]
fn variable_upserts_resize_values_and_return_presence_without_borrowed_views() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Maps::initialize(&mut store, init()).unwrap();
    let text = String::from("borrowed 🦀");
    let mut map = root.strings_mut();
    assert_eq!(map.insert(0, ""), Ok(None));
    assert_eq!(map.insert(2, &text), Ok(None));
    assert_eq!(map.insert(65535, "last"), Ok(None));
    assert_eq!(map.insert(1, "a longer value"), Ok(Some(())));
    assert_eq!(map.insert(2, ""), Ok(Some(())));
    assert_eq!(map.insert(256, "é"), Ok(Some(())));
    assert_eq!(
        map.iter().collect::<Vec<_>>(),
        [
            (0, ""),
            (1, "a longer value"),
            (2, ""),
            (256, "é"),
            (65535, "last")
        ]
    );
    for key in [0, 2, 65535, 1, 256] {
        assert_eq!(map.remove(&key), Ok(Some(())));
    }
    assert_eq!(map.remove(&1), Ok(None));
    map.insert(1, "").unwrap();
    map.clear().unwrap();
    assert_eq!(root.tail(), "untouched suffix");
    root.validate().unwrap();
    assert_eq!(text, "borrowed 🦀");
}

#[test]
fn stored_terminal_ends_and_zero_one_two_transitions_are_exact() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    Maps::initialize(&mut store, init()).unwrap();
    for (operation, expected) in [
        (0, vec![0, 0, 0, 0]),
        (1, vec![1, 0, 0, 0, 0, 1, 0, 0, 0, 0]),
        (
            2,
            vec![2, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, b'x'],
        ),
        (3, vec![1, 0, 0, 0, 1, 0, 1, 0, 0, 0, b'x']),
        (4, vec![0, 0, 0, 0]),
    ] {
        {
            let mut root = Maps::load_mut(&mut store).unwrap();
            let mut map = root.strings_mut();
            match operation {
                0 => map.clear().unwrap(),
                1 => {
                    map.insert(1, "").unwrap();
                }
                2 => {
                    map.insert(256, "x").unwrap();
                }
                3 => {
                    map.remove(&1).unwrap();
                }
                _ => {
                    map.remove(&256).unwrap();
                }
            }
            root.validate().unwrap();
        }
        let view = View::load::<Maps>(store.bytes(), Maps::DISCRIMINATOR).unwrap();
        assert_eq!(view.field(1).unwrap().bytes(), expected);
    }
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
    native: Map<bool, Native>,
    views: Map<u8, Flag>,
    lists: Map<u8, List<u16>>,
    options: Map<u8, Option<Str>>,
    maps: Map<u8, Map<u16, Str>>,
}

fn shapes_init() -> ShapesInit<'static> {
    ShapesInit {
        native: &[],
        views: &[],
        lists: &[],
        options: &[],
        maps: &[],
    }
}

#[test]
fn native_fixed_and_schema_fixed_values_have_distinct_removed_types() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let mut root = Shapes::initialize(&mut store, shapes_init()).unwrap();
    let value = Native {
        flag: true,
        number: 9,
    };
    assert_eq!(root.native_mut().insert(true, value), Ok(None));
    assert_eq!(root.native_mut().insert(false, value), Ok(None));
    assert_eq!(root.native_mut().insert(true, value), Ok(Some(value)));
    assert_eq!(root.native_mut().remove(&false), Ok(Some(value)));
    assert_eq!(
        root.views_mut().insert(1, FlagInit { flag: true }),
        Ok(None)
    );
    assert_eq!(
        root.views_mut().insert(1, FlagInit { flag: false }),
        Ok(Some(()))
    );
    assert_eq!(root.views_mut().remove(&1), Ok(Some(())));
    root.validate().unwrap();
}

#[test]
fn nested_borrowed_inputs_and_retained_value_paths_patch_all_ancestors() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Shapes::initialize(&mut store, shapes_init()).unwrap();
    root.lists_mut().insert(1, &[2, 3]).unwrap();
    root.lists_mut().at_mut(&1).unwrap().insert(1, 4).unwrap();
    root.options_mut().insert(2, Some("é")).unwrap();
    root.options_mut()
        .at_mut(&2)
        .unwrap()
        .some_mut()
        .unwrap()
        .push_str(" 🦀")
        .unwrap();
    root.maps_mut().insert(3, &[(1, "one"), (256, "")]).unwrap();
    let mut child = root.maps_mut().at_mut(&3).unwrap();
    child.insert(2, "middle").unwrap();
    child.remove(&1).unwrap();
    assert_eq!(child.get(&256), Some(""));
    assert_eq!(root.options().get(&2), Some(Some("é 🦀")));
    assert_eq!(
        root.lists().get(&1).unwrap().iter().collect::<Vec<_>>(),
        [2, 4, 3]
    );
    root.validate().unwrap();
}

#[test]
fn malformed_input_and_capacity_rejections_preserve_all_bytes() {
    let used = Shapes::encoded_len_of(&shapes_init()).unwrap() as usize;
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room[..used]);
    Shapes::initialize(&mut store, shapes_init()).unwrap();
    let before = store.bytes().to_vec();
    {
        let mut root = Shapes::load_mut(&mut store).unwrap();
        assert_eq!(
            root.maps_mut().insert(1, &[(2, "x"), (1, "y")]),
            Err(ErrorKind::UnsortedKeys.into())
        );
        assert_eq!(
            root.maps_mut().insert(1, &[(1, "x"), (1, "y")]),
            Err(ErrorKind::DuplicateKey.into())
        );
        assert_eq!(
            root.views_mut().insert(0, FlagInit { flag: true }),
            Err(ErrorKind::Capacity.into())
        );
        assert_eq!(
            root.options_mut().insert(0, None),
            Err(ErrorKind::Capacity.into())
        );
        assert_eq!(root.options_mut().remove(&0), Ok(None));
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn old_value_faults_preserve_offsets_and_poison_lazy_map_edits() {
    for remove in [false, true] {
        let mut room = [0; 128];
        let mut store = SliceStore::new(&mut room);
        Maps::initialize(&mut store, init()).unwrap();
        let view = View::load_trusted::<Maps>(store.bytes(), Maps::DISCRIMINATOR).unwrap();
        let base = view.field(1).unwrap().base() + 16;
        store.bytes_mut()[base as usize] = 0xff;
        let before = store.bytes().to_vec();
        {
            let mut root = Maps::load_trusted_mut(&mut store).unwrap();
            let result = if remove {
                root.strings_mut().remove(&1)
            } else {
                root.strings_mut().insert(1, "repair")
            };
            assert_eq!(result, Err(Error::new(ErrorKind::InvalidUtf8, base)));
            assert_eq!(
                root.strings_mut().remove(&2),
                Err(ErrorKind::Poisoned.into())
            );
        }
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
        assert!(Maps::load_checked_mut(&mut store).is_err());
        store.bytes_mut()[base as usize] = b'a';
        Maps::load_checked_mut(&mut store)
            .unwrap()
            .verify()
            .unwrap();
        assert!(!store.is_poisoned());
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn removed_native_and_fixed_schema_domains_are_checked() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let mut input = shapes_init();
    input.native = &[(
        true,
        Native {
            flag: true,
            number: 1,
        },
    )];
    input.views = &[(1, FlagInit { flag: true })];
    Shapes::initialize(&mut store, input).unwrap();
    for field in [0, 1] {
        let view = View::load_trusted::<Shapes>(store.bytes(), Shapes::DISCRIMINATOR).unwrap();
        let base = view.field(field).unwrap().base() + 5;
        store.bytes_mut()[base as usize] = 2;
        let before = store.bytes().to_vec();
        {
            let mut root = Shapes::load_trusted_mut(&mut store).unwrap();
            let result = if field == 0 {
                root.native_mut().remove(&true).map(|_| ())
            } else {
                root.views_mut().remove(&1).map(|_| ())
            };
            assert_eq!(result, Err(Error::new(ErrorKind::InvalidBool, base)));
        }
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
        assert!(matches!(Shapes::load_trusted_mut(&mut store), Err(error)
            if error == ErrorKind::Poisoned.into()));
        store.bytes_mut()[base as usize] = 1;
        Shapes::load_checked_mut(&mut store).unwrap();
        assert!(!store.is_poisoned());
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn strict_ingress_rejects_global_order_and_local_edits_reject_bad_terminal_ends() {
    for (relative, bytes) in [(4, [1, 0, 0, 0]), (10, [0, 1, 0, 0]), (12, [7, 0, 0, 0])] {
        let mut room = [0; 128];
        let mut store = SliceStore::new(&mut room);
        Maps::initialize(&mut store, init()).unwrap();
        let view = View::load_trusted::<Maps>(store.bytes(), Maps::DISCRIMINATOR).unwrap();
        let at = view.field(1).unwrap().base() as usize + relative;
        store.bytes_mut()[at..at + 4].copy_from_slice(&bytes);
        let before = store.bytes().to_vec();
        {
            assert!(Maps::load(store.bytes()).is_err());
            let mut root = Maps::load_trusted_mut(&mut store).unwrap();
            assert!(root.validate().is_err());
            assert_eq!(root.strings_mut().cursor_mut().is_ok(), relative == 4);
            if relative == 4 {
                root.strings_mut().clear().unwrap();
                root.validate().unwrap();
                continue;
            }
            assert!(root.strings_mut().insert(2, "new").is_err());
            assert!(root.strings_mut().remove(&2).is_err());
            assert!(root.strings_mut().clear().is_err());
            assert!(root.strings_mut().into_entry(2).is_err());
        }
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn clear_skips_discarded_value_domains_and_zeros_only_vacated_tail() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let old_used = Maps::initialize(&mut store, init()).unwrap().used_len() as usize;
    let view = View::load_trusted::<Maps>(store.bytes(), Maps::DISCRIMINATOR).unwrap();
    let at = view.field(1).unwrap().base() as usize + 16;
    store.bytes_mut()[at] = 0xff;
    store.bytes_mut()[old_used..].fill(0xa5);
    let new_used;
    {
        let mut root = Maps::load_trusted_mut(&mut store).unwrap();
        root.strings_mut().clear().unwrap();
        root.fixed_mut().clear().unwrap();
        root.validate().unwrap();
        assert_eq!(root.tail(), "untouched suffix");
        new_used = root.used_len() as usize;
    }
    assert!(store.bytes()[new_used..old_used].iter().all(|&b| b == 0));
    assert!(store.bytes()[old_used..].iter().all(|&b| b == 0xa5));
}

#[test]
fn closed_sessions_reject_edits_and_entry_selection() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let mut root = Maps::initialize(&mut store, init()).unwrap();
    root.reborrow().commit().unwrap();
    let mut map = root.strings_mut();
    assert_eq!(map.insert(1, "x"), Err(ErrorKind::Closed.into()));
    assert_eq!(map.remove(&1), Err(ErrorKind::Closed.into()));
    assert_eq!(map.clear(), Err(ErrorKind::Closed.into()));
    assert!(matches!(map.entry(1), Err(e) if e == ErrorKind::Closed.into()));
}

#[test]
fn bounded_exhaustive_oracle_subset() {
    exhaustive::miri_subset();
}

#[cfg(feature = "alloc")]
#[test]
fn vec_map_growth_and_entry_projection_keep_paths_current() {
    let mut bytes = Vec::new();
    let mut store = blubber::VecStore::new(&mut bytes);
    let mut root = Maps::initialize(&mut store, init()).unwrap();
    let mut map = root.strings_mut();
    map.insert(2, "a longer borrowed input that grows the root")
        .unwrap();
    map.entry(0)
        .unwrap()
        .or_insert("")
        .unwrap()
        .push_str("grown through entry")
        .unwrap();
    map.remove(&1).unwrap();
    map.insert(256, "").unwrap();
    assert_eq!(map.get(&0), Some("grown through entry"));
    assert_eq!(root.tail(), "untouched suffix");
    root.validate().unwrap();
}

struct Interrupt;

impl blubber::Schema for Interrupt {
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
    fn encode(value: &u8, destination: &mut [u8]) {
        destination[0] = *value;
        assert_ne!(*value, 99, "injected post-move encoder unwind");
    }
}

#[test]
fn encoder_unwind_poison_blocks_map_edits_and_entry_until_validated_recovery() {
    use blubber::raw::Edit;
    const ID: [u8; 8] = *b"mappanic";
    let mut room = [0; 32];
    let mut store = SliceStore::new(&mut room);
    {
        let mut edit = Edit::initialize::<__ConcreteRoot0>(&mut store, ID, &[(1, 1)]).unwrap();
        let mut map = blubber::raw::project::<Map<u8, Interrupt>>(edit.reborrow());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| map.insert(0, 99))).is_err()
        );
        assert_eq!(map.insert(2, 1), Err(ErrorKind::Poisoned.into()));
        assert_eq!(map.remove(&1), Err(ErrorKind::Poisoned.into()));
        assert_eq!(map.clear(), Err(ErrorKind::Poisoned.into()));
        assert!(matches!(map.entry(1), Err(e) if e == ErrorKind::Poisoned.into()));
    }
    assert!(store.is_poisoned());
    Edit::load::<__ConcreteRoot0>(&mut store, ID).unwrap();
    assert!(!store.is_poisoned());
}

type DeepMap = Option<Option<Option<Option<Option<Map<u16, Str>>>>>>;

#[blubber::schema(account)]
struct Paths {
    deep: DeepMap,
    lists: List<Map<u16, Str>>,
    tail: Str,
}

#[test]
fn map_and_entry_edits_patch_option_list_and_maximum_depth_ancestors() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Paths::initialize(
        &mut store,
        PathsInit {
            deep: Some(Some(Some(Some(Some(&[(2, "")]))))),
            lists: &[&[(1, "one")], &[(2, "two")]],
            tail: "suffix",
        },
    )
    .unwrap();
    let mut map = root
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
    map.insert(1, "first").unwrap();
    map.insert(2, "grown").unwrap();
    map.entry(3)
        .unwrap()
        .or_insert("last")
        .unwrap()
        .push_str("!")
        .unwrap();
    map.remove(&1).unwrap();
    assert_eq!(map.get(&3), Some("last!"));
    let mut child = root.lists_mut().at_mut(0).unwrap();
    child.insert(3, "a longer nested value").unwrap();
    child
        .entry(1)
        .unwrap()
        .or_insert("unused")
        .unwrap()
        .clear()
        .unwrap();
    assert_eq!(root.lists().get(1).unwrap().get(&2), Some("two"));
    assert_eq!(root.tail(), "suffix");
    root.validate().unwrap();
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn bool_key_domains_and_unrelated_value_corruption_are_checked_selectively() {
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    let mut input = shapes_init();
    input.native = &[
        (
            false,
            Native {
                flag: true,
                number: 1,
            },
        ),
        (
            true,
            Native {
                flag: true,
                number: 2,
            },
        ),
    ];
    Shapes::initialize(&mut store, input).unwrap();
    let view = View::load_trusted::<Shapes>(store.bytes(), Shapes::DISCRIMINATOR).unwrap();
    let base = view.field(0).unwrap().base() as usize;
    store.bytes_mut()[base + 8] = 2;
    let before = store.bytes().to_vec();
    {
        let mut root = Shapes::load_trusted_mut(&mut store).unwrap();
        assert!(root.validate().is_err());
        assert_eq!(
            root.native_mut().remove(&false),
            Err(Error::new(ErrorKind::InvalidBool, (base + 8) as u32))
        );
    }
    assert_eq!(store.bytes(), before);
    assert!(store.is_poisoned());
    store.bytes_mut()[base + 8] = 1;
    Shapes::load_checked_mut(&mut store).unwrap();
    assert!(!store.is_poisoned());
    store.bytes_mut()[base + 9] = 2;
    let mut root = Shapes::load_trusted_mut(&mut store).unwrap();
    root.native_mut().remove(&false).unwrap();
    assert!(root.validate().is_err());
    root.native_mut().clear().unwrap();
    root.validate().unwrap();
}

#[test]
fn mutable_map_queries_follow_upsert_and_removal() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Maps::initialize(&mut store, init()).unwrap();
    let mut map = root.fixed_mut();
    assert_eq!(map.len(), 2);
    assert!(!map.is_empty());
    assert!(map.contains_key(&256));
    assert!(!map.contains_key(&42));
    assert_eq!(map.get(&-1), Some(10));
    assert_eq!(map.get(&42), None);
    map.clear().unwrap();
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    assert_eq!(map.insert(42, 77), Ok(None));
    assert_eq!(map.len(), 1);
    assert_eq!(map.insert(42, 99), Ok(Some(77)));
    assert_eq!(
        map.try_iter().unwrap().collect::<blubber::Result<Vec<_>>>(),
        Ok(vec![(42, 99)])
    );
    assert_eq!(map.remove(&42), Ok(Some(99)));
    assert!(!map.contains_key(&42));
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn selected_map_validation_rejects_invalid_value_domain() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    Maps::initialize(&mut store, init()).unwrap();
    let payload = View::load_trusted::<Maps>(store.bytes(), Maps::DISCRIMINATOR)
        .unwrap()
        .field(1)
        .unwrap()
        .base() as usize
        + 16;
    store.bytes_mut()[payload] = 0xff;
    let before = store.bytes().to_vec();
    assert_eq!(
        Maps::load_trusted_mut(&mut store)
            .unwrap()
            .strings_mut()
            .validate(),
        Err(Error::new(ErrorKind::InvalidUtf8, payload as u32))
    );
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}
