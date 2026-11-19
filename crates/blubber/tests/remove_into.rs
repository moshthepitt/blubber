//! Allocation-free encoded extraction with atomic failures in both buffers.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::raw::Resize;
#[cfg(not(feature = "verify-on-load"))]
use blubber::Error;
use blubber::Resize as _;
use blubber::{ErrorKind, List, Map, ReadStorage, Schema, SliceStore, Str, View};

#[blubber::schema]
struct Item {
    text: Str,
    tags: List<u16>,
}

#[blubber::schema(account)]
struct Values {
    fixed: List<u16>,
    items: List<Item>,
    strings: List<Str>,
    fixed_map: Map<i16, bool>,
    map: Map<u16, Item>,
    empty: Map<u16, Str>,
    tail: Str,
}

fn init() -> ValuesInit<'static> {
    ValuesInit {
        fixed: &[1, 256, 3],
        items: &[
            ItemInit {
                text: "é",
                tags: &[1, 256],
            },
            ItemInit {
                text: "last",
                tags: &[],
            },
        ],
        strings: &["", "é", ""],
        fixed_map: &[(-1, true), (256, false)],
        map: &[(
            1,
            ItemInit {
                text: "é",
                tags: &[1, 256],
            },
        )],
        empty: &[(1, "")],
        tail: "unrelated tail",
    }
}

#[test]
fn fixed_list_extraction_copies_little_endian_bytes_and_retains_spare() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let old_used = Values::initialize(&mut store, init()).unwrap().used_len() as usize;
    store.bytes_mut()[old_used..].fill(0xa5);
    let mut destination = [0xcc; 5];
    let new_used;
    {
        let mut root = Values::load_mut(&mut store).unwrap();
        assert_eq!(root.fixed_mut().remove_into(1, &mut destination), Ok(2));
        assert_eq!(root.fixed().iter().collect::<Vec<_>>(), [1, 3]);
        assert_eq!(root.tail(), "unrelated tail");
        root.validate().unwrap();
        new_used = root.used_len() as usize;
    }
    assert_eq!(destination, [0, 1, 0xcc, 0xcc, 0xcc]);
    assert!(store.bytes()[new_used..old_used]
        .iter()
        .all(|byte| *byte == 0));
    assert!(store.bytes()[old_used..].iter().all(|byte| *byte == 0xa5));
}

#[test]
fn variable_subtrees_extract_as_independently_relocatable_encodings() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Values::initialize(&mut store, init()).unwrap();
    let mut destination = [0xcc; 32];
    let size = root.items_mut().remove_into(0, &mut destination).unwrap();
    let view = View::new(&destination[..size as usize], Item::LAYOUT, 500).unwrap();
    view.validate().unwrap();
    let item = Item::read(view).unwrap();
    assert_eq!(item.text(), "é");
    assert_eq!(item.tags().iter().collect::<Vec<_>>(), [1, 256]);
    assert!(destination[size as usize..]
        .iter()
        .all(|byte| *byte == 0xcc));
    assert_eq!(root.items().get(0).unwrap().text(), "last");
    root.validate().unwrap();
}

#[test]
fn map_extraction_handles_both_layouts_and_missing_keys() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Values::initialize(&mut store, init()).unwrap();
    let mut destination = [0xcc; 32];
    assert_eq!(
        root.fixed_map_mut().remove_into(&-1, &mut destination),
        Ok(Some(1))
    );
    assert_eq!(destination[0], 1);
    assert_eq!(root.fixed_map().get(&256), Some(false));
    let len = root
        .map_mut()
        .remove_into(&1, &mut destination)
        .unwrap()
        .unwrap();
    let view = View::new(&destination[..len as usize], Item::LAYOUT, 0).unwrap();
    assert_eq!(Item::read(view).unwrap().text(), "é");
    let before = destination;
    assert_eq!(root.map_mut().remove_into(&1, &mut destination), Ok(None));
    assert_eq!(destination, before);
    root.validate().unwrap();
}

#[test]
fn zero_byte_values_succeed_with_empty_destinations_and_update_directories() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Values::initialize(&mut store, init()).unwrap();
    assert_eq!(root.strings_mut().remove_into(0, &mut []), Ok(0));
    assert_eq!(root.strings_mut().remove_into(1, &mut []), Ok(0));
    let mut destination = [0; 2];
    assert_eq!(root.strings_mut().remove_into(0, &mut destination), Ok(2));
    assert_eq!(&destination, "é".as_bytes());
    assert_eq!(root.empty_mut().remove_into(&1, &mut []), Ok(Some(0)));
    assert_eq!(root.empty_mut().remove_into(&1, &mut []), Ok(None));
    root.validate().unwrap();
}

#[test]
fn insufficient_room_and_invalid_indices_preserve_both_buffers() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    Values::initialize(&mut store, init()).unwrap();
    let before = store.bytes().to_vec();
    let mut destination = [0xcc; 1];
    {
        let mut root = Values::load_mut(&mut store).unwrap();
        assert_eq!(
            root.fixed_mut().remove_into(0, &mut destination),
            Err(ErrorKind::Capacity.into())
        );
        assert_eq!(
            root.items_mut().remove_into(0, &mut destination),
            Err(ErrorKind::Capacity.into())
        );
        assert_eq!(
            root.map_mut().remove_into(&1, &mut destination),
            Err(ErrorKind::Capacity.into())
        );
        assert_eq!(
            root.fixed_mut().remove_into(3, &mut destination),
            Err(ErrorKind::IndexOutOfBounds.into())
        );
        assert_eq!(
            root.items_mut().remove_into(usize::MAX, &mut destination),
            Err(ErrorKind::IndexOutOfBounds.into())
        );
    }
    assert_eq!(destination, [0xcc]);
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn invalid_utf8_and_fixed_domains_leave_both_buffers_unchanged() {
    for field in [1, 3, 4] {
        let mut room = [0; 256];
        let mut store = SliceStore::new(&mut room);
        Values::initialize(&mut store, init()).unwrap();
        let view = View::load_trusted::<Values>(store.bytes(), Values::DISCRIMINATOR)
            .unwrap()
            .field(field)
            .unwrap();
        let (relative, byte, kind) = match field {
            1 => (12, 0xff, ErrorKind::InvalidUtf8), // count, one end, Item's name end
            3 => (6, 2, ErrorKind::InvalidBool),     // count, signed key
            _ => (14, 0xff, ErrorKind::InvalidUtf8), // count, key/end, Item's name end
        };
        let base = view.base() + relative;
        store.bytes_mut()[base as usize] = byte;
        let before = store.bytes().to_vec();
        let mut destination = [0xcc; 64];
        {
            let mut root = Values::load_trusted_mut(&mut store).unwrap();
            let result = match field {
                1 => root.items_mut().remove_into(0, &mut destination),
                3 => root
                    .fixed_map_mut()
                    .remove_into(&-1, &mut destination)
                    .map(Option::unwrap),
                _ => root
                    .map_mut()
                    .remove_into(&1, &mut destination)
                    .map(Option::unwrap),
            };
            assert_eq!(result, Err(Error::new(kind, base)));
        }
        assert_eq!(destination, [0xcc; 64]);
        assert_eq!(store.bytes(), before);
        assert!(store.is_poisoned());
    }
}

#[cfg(not(feature = "verify-on-load"))]
#[test]
fn missing_extraction_preserves_both_buffers_while_ingress_rejects_global_order() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    Values::initialize(&mut store, init()).unwrap();
    let base = View::load_trusted::<Values>(store.bytes(), Values::DISCRIMINATOR)
        .unwrap()
        .field(3)
        .unwrap()
        .base() as usize;
    store.bytes_mut()[base + 4..base + 6].copy_from_slice(&[0xff, 0xff]);
    assert!(Values::load(store.bytes()).is_err());
    let before = store.bytes().to_vec();
    let mut destination = [0xcc; 64];
    assert_eq!(
        Values::load_trusted_mut(&mut store)
            .unwrap()
            .fixed_map_mut()
            .remove_into(&42, &mut destination),
        Ok(None)
    );
    assert_eq!(destination, [0xcc; 64]);
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
    assert!(Values::load(store.bytes()).is_err());
}

#[test]
fn nested_extraction_updates_ancestor_boundaries_and_reuses_retained_handle() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut root = Values::initialize(&mut store, init()).unwrap();
    let mut item = root.items_mut().at_mut(0).unwrap();
    let mut tags = item.tags_mut();
    let mut destination = [0xcc; 2];
    assert_eq!(tags.remove_into(0, &mut destination), Ok(2));
    assert_eq!(destination, [1, 0]);
    assert_eq!(tags.remove_into(0, &mut destination), Ok(2));
    assert_eq!(destination, [0, 1]);
    assert!(tags.is_empty());
    assert_eq!(root.items().get(0).unwrap().text(), "é");
    assert_eq!(root.tail(), "unrelated tail");
    root.validate().unwrap();
}

#[test]
fn closed_and_poisoned_roots_preserve_extraction_destination() {
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    Values::initialize(&mut store, init()).unwrap();
    let before = store.bytes().to_vec();
    store.set_poisoned(true);
    assert!(Values::load_trusted_mut(&mut store).is_err());
    store.set_poisoned(false);
    let mut destination = [0xcc; 64];
    {
        let mut root = Values::load_trusted_mut(&mut store).unwrap();
        root.reborrow().commit().unwrap();
        assert_eq!(
            root.fixed_mut().remove_into(0, &mut destination),
            Err(ErrorKind::Closed.into())
        );
        assert_eq!(
            root.map_mut().remove_into(&1, &mut destination),
            Err(ErrorKind::Closed.into())
        );
    }
    assert_eq!(destination, [0xcc; 64]);
    assert_eq!(room.as_slice(), before);
}
