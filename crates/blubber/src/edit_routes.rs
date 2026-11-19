//! Route and work controls for the reviewed edit shortcuts.
extern crate std;
mod counters {
    use super::std;
    use core::cell::Cell;
    std::thread_local! { static COUNTS: Cell<[usize; 6]> = const { Cell::new([0; 6]) }; }
    pub(crate) fn hit(index: usize) {
        COUNTS.with(|c| {
            let mut n = c.get();
            n[index] += 1;
            c.set(n);
        });
    }
    pub(super) fn reset() {
        COUNTS.with(|c| c.set([0; 6]));
    }
    pub(super) fn get(index: usize) -> usize {
        COUNTS.with(|c| c.get()[index])
    }
}
use blubber::{List, Map, ReadStorage, Resize, Schema, SliceStore, Str};
pub(crate) use counters::hit;
#[blubber::schema]
struct Wrap<T: Schema> {
    value: T,
}
#[derive(Clone, Copy, blubber::Fixed)]
struct Record {
    flag: bool,
    value: u16,
}
#[blubber::schema(account)]
struct TailGeneric {
    wrapper: Wrap<List<u16>>,
}
#[blubber::schema(account)]
struct Numbers {
    list: List<u16>,
}
#[blubber::schema(account)]
struct Records {
    list: List<Record>,
}
#[blubber::schema(account)]
struct Nested {
    list: List<Map<u8, u16>>,
    map: Map<u8, Map<u8, u16>>,
    fixed: Map<u8, u16>,
}
#[blubber::schema(account)]
struct Text {
    first: Str,
    last: Str,
}

#[blubber::schema(account)]
struct TextList {
    list: List<Str>,
}

#[test]
fn lazy_list_clear_retains_policy_route() {
    for trusted in [false, true] {
        let mut bytes = [0; 128];
        {
            let mut store = SliceStore::new(&mut bytes);
            TextList::initialize(
                &mut store,
                TextListInit {
                    list: &["old", "tail"],
                },
            )
            .unwrap();
        }
        let before = bytes;
        let result = {
            let mut store = SliceStore::new(&mut bytes);
            let mut edit = if trusted {
                crate::edit::AccountEdit::load_trusted::<TextList>(
                    crate::raw::backend(&mut store),
                    TextList::DISCRIMINATOR,
                )
            } else {
                crate::edit::AccountEdit::load::<TextList>(
                    crate::raw::backend(&mut store),
                    TextList::DISCRIMINATOR,
                )
            }
            .unwrap()
            .field(0);
            edit.try_clear_list()
        };
        if trusted && !cfg!(feature = "verify-on-load") {
            assert_eq!(result, Some(Ok(())));
            let mut expected = before;
            expected[8..12].copy_from_slice(&16u32.to_le_bytes());
            expected[12..].fill(0);
            assert_eq!(bytes, expected);
        } else {
            assert_eq!(result, None);
            assert_eq!(bytes, before);
        }
    }
}

#[test]
fn bulk_tail_growth_uses_exact_capacity() {
    for capacity in [23, 64] {
        for trusted in [false, true] {
            let mut bytes = [0; 64];
            let mut store = SliceStore::new(&mut bytes[..capacity]);
            TextList::initialize(&mut store, TextListInit { list: &["a"] }).unwrap();
            {
                let mut root = if trusted {
                    TextList::load_trusted_mut(&mut store)
                } else {
                    TextList::load_mut(&mut store)
                }
                .unwrap();
                root.list_mut().extend_from_slice(&["bc"]).unwrap();
            }
            assert_eq!(&store.bytes()[8..12], &23u32.to_le_bytes());
            assert_eq!(store.storage_len(), capacity);
            assert!(!store.is_poisoned());
            let root = TextList::load(store.bytes()).unwrap();
            assert_eq!(
                root.list().iter().collect::<std::vec::Vec<_>>(),
                ["a", "bc"]
            );
        }
    }
}
#[test]
fn offset_field_retains_tail_path() {
    let mut b = [0; 128];
    let mut s = SliceStore::new(&mut b);
    TailGeneric::initialize(
        &mut s,
        TailGenericInit {
            wrapper: WrapInit { value: &[1, 2] },
        },
    )
    .unwrap();
    let mut r = TailGeneric::load_mut(&mut s).unwrap();
    counters::reset();
    r.wrapper_mut()
        .value_mut()
        .try_for_each_mut(|_| Ok(()))
        .unwrap();
    assert_eq!(counters::get(0), 1);
    assert_eq!(counters::get(1), 1);
}
#[test]
fn borrowed_fixed_list_retains_primitive_and_record_tail_paths() {
    let mut b = [0; 128];
    let mut s = SliceStore::new(&mut b);
    Numbers::initialize(&mut s, NumbersInit { list: &[1, 2] }).unwrap();
    let mut r = Numbers::load_mut(&mut s).unwrap();
    counters::reset();
    r.list_mut().try_for_each_mut(|_| Ok(())).unwrap();
    assert_eq!(counters::get(1), 1);
    let mut b = [0; 128];
    let mut s = SliceStore::new(&mut b);
    Records::initialize(
        &mut s,
        RecordsInit {
            list: &[Record {
                flag: true,
                value: 9,
            }],
        },
    )
    .unwrap();
    let mut r = Records::load_mut(&mut s).unwrap();
    counters::reset();
    r.list_mut().try_for_each_mut(|_| Ok(())).unwrap();
    assert_eq!(counters::get(1), 1);
}
#[test]
fn consumed_fixed_list_retains_primitive_and_record_tail_paths() {
    let mut b = [0; 128];
    let mut s = SliceStore::new(&mut b);
    Numbers::initialize(&mut s, NumbersInit { list: &[1, 2] }).unwrap();
    let mut r = Numbers::load_mut(&mut s).unwrap();
    counters::reset();
    let mut c = r.list_mut().cursor_mut().unwrap();
    assert!(c.next().unwrap().is_some());
    assert_eq!(counters::get(2), 1);
    let mut b = [0; 128];
    let mut s = SliceStore::new(&mut b);
    Records::initialize(
        &mut s,
        RecordsInit {
            list: &[Record {
                flag: true,
                value: 9,
            }],
        },
    )
    .unwrap();
    let mut r = Records::load_mut(&mut s).unwrap();
    counters::reset();
    let mut c = r.list_mut().cursor_mut().unwrap();
    assert!(c.next().unwrap().is_some());
    assert_eq!(counters::get(2), 1);
}
fn nested_bytes() -> [u8; 256] {
    let mut b = [0; 256];
    Nested::initialize(
        &mut SliceStore::new(&mut b),
        NestedInit {
            list: &[&[(1, 10), (3, 30)]],
            map: &[(1, &[(1, 10), (3, 30)])],
            fixed: &[(1, 10), (3, 30)],
        },
    )
    .unwrap();
    b
}
#[test]
fn checked_ingress_avoids_redundant_key_scans_at_all_paths() {
    for path in 0..3 {
        let mut b = nested_bytes();
        let mut s = SliceStore::new(&mut b);
        let mut r = Nested::load_mut(&mut s).unwrap();
        counters::reset();
        match path {
            0 => r.fixed_mut().try_for_each_mut(|_, _| Ok(())).unwrap(),
            1 => r
                .list_mut()
                .get_mut(0)
                .unwrap()
                .unwrap()
                .try_for_each_mut(|_, _| Ok(()))
                .unwrap(),
            _ => r
                .map_mut()
                .get_mut(&1)
                .unwrap()
                .unwrap()
                .try_for_each_mut(|_, _| Ok(()))
                .unwrap(),
        }
        assert_eq!(counters::get(3), 0, "path={path}");
    }
}

#[test]
fn equal_layout_words_reuse_ingress_after_a_pointer_miss() {
    let actual = <Nested as crate::RootSchema>::__layout().child(2, true);
    static WORDS: std::sync::OnceLock<std::boxed::Box<[u32]>> = std::sync::OnceLock::new();
    let words = WORDS.get_or_init(|| actual.words().to_vec().into_boxed_slice());
    let selected = blubber_core::certify(words, actual.root(), actual.budget()).unwrap();
    assert!(!core::ptr::eq(actual.words(), selected.words()));
    assert_eq!(actual.words(), selected.words());
    let mut bytes = nested_bytes();
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    {
        let mut root =
            blubber::raw::Edit::load::<Nested>(&mut store, Nested::DISCRIMINATOR).unwrap();
        let edit = root.field_reborrow_certified(2, None, selected, None);
        let mut map = blubber::raw::project::<Map<u8, u16>>(edit);
        counters::reset();
        let mut visited = std::vec::Vec::new();
        map.try_for_each_mut(|key, value| {
            visited.push((key, value.try_get()?));
            value.set(value.try_get()? + 1)
        })
        .unwrap();
        assert_eq!(visited, [(1, 10), (3, 30)]);
        assert_eq!(counters::get(3), 0);
    }
    let root = Nested::load(store.bytes()).unwrap();
    assert_eq!(root.fixed().get(&1), Some(11));
    assert_eq!(root.fixed().get(&3), Some(31));
    let mut expected = before;
    let used = u32::from_le_bytes(before[8..12].try_into().unwrap()) as usize;
    expected[used - 5..used - 3].copy_from_slice(&11u16.to_le_bytes());
    expected[used - 2..used].copy_from_slice(&31u16.to_le_bytes());
    assert_eq!(store.bytes(), expected);
}

#[blubber::schema(account)]
struct IngressWords {
    items: List<u16>,
    other: Map<bool, bool>,
}

#[blubber::schema(account)]
struct IngressFlags {
    items: Map<bool, bool>,
    other: List<u16>,
}

#[test]
fn different_layout_words_scan_and_reject_duplicate_keys() {
    let actual = <IngressWords as crate::RootSchema>::__layout().child(0, true);
    let selected = <IngressFlags as crate::RootSchema>::__layout().child(0, true);
    assert!(!core::ptr::eq(actual.words(), selected.words()));
    assert_ne!(actual.words(), selected.words());
    assert_eq!(actual.root(), selected.root());
    assert_eq!(actual.budget(), selected.budget());
    let mut bytes = [0; 64];
    IngressWords::initialize(
        &mut SliceStore::new(&mut bytes),
        IngressWordsInit {
            items: &[0x0501, 0x0601],
            other: &[],
        },
    )
    .unwrap()
    .commit()
    .unwrap();
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    {
        let mut root =
            blubber::raw::Edit::load::<IngressWords>(&mut store, IngressWords::DISCRIMINATOR)
                .unwrap();
        let edit = root.field_reborrow_certified(0, None, selected, None);
        counters::reset();
        assert!(matches!(
            blubber::raw::project::<Map<bool, bool>>(edit).cursor_mut(),
            Err(error) if error == crate::Error::new(crate::ErrorKind::DuplicateKey, 22)
        ));
        assert_eq!(counters::get(3), 1);
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[test]
fn singleton_map_keeps_direct_key_scan() {
    let mut b = [0; 256];
    let mut s = SliceStore::new(&mut b);
    Nested::initialize(
        &mut s,
        NestedInit {
            list: &[],
            map: &[],
            fixed: &[(1, 10)],
        },
    )
    .unwrap();
    let mut r = Nested::load_mut(&mut s).unwrap();
    counters::reset();
    r.fixed_mut().try_for_each_mut(|_, _| Ok(())).unwrap();
    assert_eq!(counters::get(3), 1);
}
#[test]
fn string_dispatch_tries_only_final_variable_field() {
    for final_field in [false, true] {
        let mut b = [0; 128];
        let mut s = SliceStore::new(&mut b);
        Text::initialize(
            &mut s,
            TextInit {
                first: "abc",
                last: "def",
            },
        )
        .unwrap();
        let mut r = Text::load_trusted_mut(&mut s).unwrap();
        counters::reset();
        if final_field {
            r.last_mut().set("new").unwrap();
            assert_eq!(counters::get(4), 1);
            assert_eq!(counters::get(5), 0);
        } else {
            r.first_mut().set("new").unwrap();
            assert_eq!(counters::get(4), 0);
        }
    }
}
