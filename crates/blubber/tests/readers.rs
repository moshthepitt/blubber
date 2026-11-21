//! Checked roots, direct selection, temporary lifetimes and both collection layouts.

#![forbid(unsafe_code)]

mod support;

use blubber::{Address, Fixed, List, Map, MapKey, Schema, Str, View};
use core::iter::FusedIterator;
use support::{account, encoded, read, Pair, Record, DISCRIMINATOR};

#[test]
fn root_load_excludes_spare_and_preserves_base() {
    let bytes = account::<Record>(&("é", true, &[0x1234, 7]));
    let root = View::load::<Record>(&bytes, DISCRIMINATOR).unwrap();
    assert_eq!(root.base(), 12);
    assert_eq!(root.bytes().len(), 15);
    assert_eq!(root.field(0).unwrap().base(), 17);
    let record = root.read::<Record>().unwrap();
    assert_eq!(record.name(), "é");
    assert_eq!(record.enabled(), Ok(true));
    assert_eq!(record.tags().iter().collect::<Vec<_>>(), [0x1234, 7]);
    root.validate().unwrap();
    View::load::<Record>(&bytes, DISCRIMINATOR).unwrap();
}

#[test]
fn borrowed_children_outlive_temporary_parents_and_iterators() {
    let bytes =
        encoded::<List<Record>>(&&[("one", true, &[1][..]), ("two", false, &[2, 3][..])][..]);
    let name = read::<List<Record>>(&bytes).unwrap().get(1).unwrap().name();
    assert_eq!(name, "two");
    let name_from_iterator = read::<List<Record>>(&bytes)
        .unwrap()
        .iter()
        .next()
        .unwrap()
        .name();
    assert_eq!(name_from_iterator, "one");
    let tags = read::<List<Record>>(&bytes).unwrap().get(1).unwrap().tags();
    assert_eq!(tags.get(1), Some(3));
}

#[test]
fn fixed_lists_and_bytes_return_copied_values() {
    let bytes = encoded::<List<u32>>(&&[0x12345678, 0, u32::MAX][..]);
    let list = read::<List<u32>>(&bytes).unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list.try_len(), Ok(3));
    assert_eq!(list.try_is_empty(), Ok(false));
    assert!(!list.is_empty());
    assert_eq!(list.iter().collect::<Vec<_>>(), [0x12345678, 0, u32::MAX]);
    assert_eq!(list.get(3), None);
    assert_eq!(list.try_get(usize::MAX), Ok(None));
    list.validate().unwrap();
    let bytes = encoded::<List<u8>>(&&[0, 128, 255][..]);
    assert_eq!(
        read::<List<u8>>(&bytes).unwrap().iter().collect::<Vec<_>>(),
        [0, 128, 255]
    );
}

#[test]
fn indexed_lists_cover_elision_and_empty_elements() {
    for values in [
        &[][..],
        &[""][..],
        &["", ""][..],
        &["", "x", "", "yz", ""][..],
    ] {
        let bytes = encoded::<List<Str>>(&values);
        let list = read::<List<Str>>(&bytes).unwrap();
        assert_eq!(list.len(), values.len());
        assert_eq!(list.is_empty(), values.is_empty());
        for (index, &value) in values.iter().enumerate() {
            assert_eq!(list.try_get(index), Ok(Some(value)));
        }
        assert_eq!(list.try_get(values.len()), Ok(None));
        assert_eq!(list.iter().collect::<Vec<_>>(), values);
        list.validate().unwrap();
    }
}

#[test]
fn handwritten_fixed_struct_views_use_strides() {
    let bytes = encoded::<List<Pair>>(&&[(513, true), (7, false)][..]);
    let list = read::<List<Pair>>(&bytes).unwrap();
    let second = list.get(1).unwrap();
    assert_eq!(second.base(), 7);
    assert_eq!(second.field(0).unwrap().read::<u16>(), Ok(7));
    assert_eq!(second.field(1).unwrap().read::<bool>(), Ok(false));
    list.validate().unwrap();
    let bytes = encoded::<Map<u8, Pair>>(&&[(1, (33, true)), (2, (44, false))][..]);
    let map = read::<Map<u8, Pair>>(&bytes).unwrap();
    assert_eq!(map.get(&2).unwrap().field(0).unwrap().read::<u16>(), Ok(44));
    assert_eq!(map.iter().map(|(k, _)| k).collect::<Vec<_>>(), [1, 2]);
    map.validate().unwrap();
}

#[test]
fn options_keep_none_distinct_from_empty_some() {
    assert_eq!(read::<Option<Str>>(&[0]), Ok(None));
    assert_eq!(read::<Option<Str>>(&[1]), Ok(Some("")));
    let bytes = encoded::<Option<Option<Str>>>(&Some(Some("text")));
    assert_eq!(read::<Option<Option<Str>>>(&bytes), Ok(Some(Some("text"))));
    let bytes = encoded::<Option<List<u16>>>(&Some(&[17, 28][..]));
    let list = read::<Option<List<u16>>>(&bytes).unwrap().unwrap();
    assert_eq!(list.iter().collect::<Vec<_>>(), [17, 28]);
    let bytes = encoded::<List<Option<Str>>>(&&[None, Some(""), Some("yes")][..]);
    assert_eq!(
        read::<List<Option<Str>>>(&bytes)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [None, Some(""), Some("yes")]
    );
}

#[test]
fn fixed_maps_search_and_decode_keys_with_the_key_codec() {
    let values = [(-32768i16, 0x1234u16), (-1, 2), (0, 3), (32767, 4)];
    let bytes = encoded::<Map<i16, u16>>(&&values[..]);
    let map = read::<Map<i16, u16>>(&bytes).unwrap();
    assert_eq!(map.len(), 4);
    assert_eq!(map.try_len(), Ok(4));
    assert!(!map.is_empty());
    assert_eq!(map.try_is_empty(), Ok(false));
    for &(key, value) in &values {
        assert_eq!(map.get(&key), Some(value));
        assert!(map.contains_key(&key));
        assert_eq!(map.try_contains_key(&key), Ok(true));
    }
    assert_eq!(map.try_get(&-2), Ok(None));
    assert!(!map.contains_key(&1));
    assert_eq!(map.iter().collect::<Vec<_>>(), values);
    map.validate().unwrap();
}

#[test]
fn indexed_maps_keep_stored_terminal_ends_and_zero_byte_values() {
    for values in [
        &[][..],
        &[(0u16, "")][..],
        &[(1, ""), (2, "x"), (3, "")][..],
    ] {
        let bytes = encoded::<Map<u16, Str>>(&values);
        let map = read::<Map<u16, Str>>(&bytes).unwrap();
        assert_eq!(map.len(), values.len());
        assert_eq!(map.is_empty(), values.is_empty());
        for &(key, value) in values {
            assert_eq!(map.try_get(&key), Ok(Some(value)));
        }
        assert_eq!(map.get(&u16::MAX), None);
        assert_eq!(map.iter().collect::<Vec<_>>(), values);
        map.validate().unwrap();
    }
}

#[test]
fn nested_maps_lists_and_options_read_without_owned_intermediates() {
    type Nested = Map<u16, List<Option<Str>>>;
    let init = &[(1u16, &[None, Some(""), Some("ok")][..]), (2, &[][..])][..];
    let bytes = encoded::<Nested>(&init);
    let map = read::<Nested>(&bytes).unwrap();
    assert_eq!(map.get(&1).unwrap().get(2), Some(Some("ok")));
    assert!(map.get(&2).unwrap().is_empty());
    map.validate().unwrap();
    let bytes = encoded::<List<Map<bool, Str>>>(&&[&[(false, "no"), (true, "yes")][..]][..]);
    assert_eq!(
        read::<List<Map<bool, Str>>>(&bytes)
            .unwrap()
            .get(0)
            .unwrap()
            .get(&true),
        Some("yes")
    );
}

#[test]
fn raw_and_pubkey_map_keys_are_copied() {
    let a = Address::new_from_array([1; 32]);
    let b = Address::new_from_array([2; 32]);
    let bytes = encoded::<Map<Address, bool>>(&&[(a, false), (b, true)][..]);
    let map = read::<Map<Address, bool>>(&bytes).unwrap();
    assert_eq!(map.iter().collect::<Vec<_>>(), [(a, false), (b, true)]);
    assert_eq!(map.get(&b), Some(true));
    let bytes = encoded::<Map<[u8; 3], Str>>(&&[([0, 1, 2], "a"), ([255, 0, 0], "b")][..]);
    assert_eq!(
        read::<Map<[u8; 3], Str>>(&bytes).unwrap().get(&[255, 0, 0]),
        Some("b")
    );
}

fn scalar<T>(value: T)
where
    T: Fixed + Schema<Removed = T> + core::fmt::Debug + PartialEq,
    for<'a> T: Schema<Read<'a> = T>,
{
    let bytes = value.encode();
    assert_eq!(read::<T>(bytes.as_ref()).unwrap(), value);
    let view = View::new(bytes.as_ref(), T::LAYOUT, 41).unwrap();
    assert_eq!(T::read_removed(view).unwrap(), value);
}

#[test]
fn all_fixed_reader_families_and_removed_values_decode_by_value() {
    scalar(u8::MAX);
    scalar(u16::MAX);
    scalar(u32::MAX);
    scalar(u64::MAX);
    scalar(u128::MAX);
    scalar(i8::MIN);
    scalar(i16::MIN);
    scalar(i32::MIN);
    scalar(i64::MIN);
    scalar(i128::MIN);
    scalar(true);
    scalar(false);
    scalar([9; 37]);
    scalar(Address::new_from_array([42; 32]));
    let bytes = encoded::<Str>(&"discarded");
    assert_eq!(
        Str::read_removed(View::new(&bytes, Str::LAYOUT, 0).unwrap()),
        Ok(())
    );
}

#[test]
fn iterators_are_cloneable_fused_and_keep_remaining_bounds() {
    fn fused<I: FusedIterator + Clone>(_: &I) {}
    let bytes = encoded::<List<Str>>(&&["a", "b"][..]);
    let mut iter = read::<List<Str>>(&bytes).unwrap().try_iter();
    fused(&iter);
    assert_eq!(iter.size_hint(), (1, Some(2)));
    assert_eq!(iter.next(), Some(Ok("a")));
    let mut clone = iter.clone();
    assert_eq!(clone.next(), Some(Ok("b")));
    assert_eq!(iter.next(), Some(Ok("b")));
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    fused(&read::<List<Str>>(&bytes).unwrap().iter());
    let bytes = encoded::<Map<u16, Str>>(&&[(1, "x"), (2, "y")][..]);
    let map = read::<Map<u16, Str>>(&bytes).unwrap();
    let mut iter = map.try_iter();
    fused(&iter);
    fused(&map.iter());
    assert_eq!(iter.next(), Some(Ok((1, "x"))));
    assert_eq!(iter.clone().collect::<Vec<_>>(), [Ok((2, "y"))]);
    assert_eq!(iter.next(), Some(Ok((2, "y"))));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(map.iter().clone().collect::<Vec<_>>(), [(1, "x"), (2, "y")]);
}

#[test]
fn memcomparable_lookup_agrees_for_every_integer_width() {
    fn check<K: MapKey + core::fmt::Debug + Schema>(keys: &[K]) {
        let values: Vec<_> = keys
            .iter()
            .copied()
            .enumerate()
            .map(|(i, key)| (key, i as u8))
            .collect();
        let bytes = encoded::<Map<K, u8>>(&&values[..]);
        let map = read::<Map<K, u8>>(&bytes).unwrap();
        for &(key, value) in &values {
            assert_eq!(map.get(&key), Some(value));
        }
        assert_eq!(map.iter().collect::<Vec<_>>(), values);
    }
    check(&[0u8, 1, u8::MAX]);
    check(&[0u16, 1, u16::MAX]);
    check(&[0u32, 1, u32::MAX]);
    check(&[0u64, 1, u64::MAX]);
    check(&[0u128, 1, u128::MAX]);
    check(&[i8::MIN, -1, 0, i8::MAX]);
    check(&[i16::MIN, -1, 0, i16::MAX]);
    check(&[i32::MIN, -1, 0, i32::MAX]);
    check(&[i64::MIN, -1, 0, i64::MAX]);
    check(&[i128::MIN, -1, 0, i128::MAX]);
    check(&[false, true]);
}

#[path = "support/region.rs"]
mod region;

#[test]
fn fixed_chunk_default_preserves_domains_clones_and_retained_borrows() {
    use blubber::{Error, ErrorKind};
    for count in 0..=4 {
        for bad in 0..=count {
            let mut bytes = (count as u32).to_le_bytes().to_vec();
            bytes.extend((0..count).map(|i| if i == bad { 255 } else { i as u8 }));
            let list = View::new(&bytes, <List<region::Domain>>::LAYOUT, 71)
                .unwrap()
                .read::<List<region::Domain>>()
                .unwrap();
            let mut iter = list.try_iter();
            for i in 0..count {
                assert_eq!(iter.size_hint(), (1, Some(count - i)));
                let expected = list.try_get(i).map(Option::unwrap);
                assert_eq!(iter.clone().next(), Some(expected));
                assert_eq!(iter.next(), Some(expected));
                if i == bad {
                    assert_eq!(
                        expected,
                        Err(Error::new(ErrorKind::InvalidTag, 75 + i as u32))
                    );
                    break;
                }
            }
            assert_eq!(iter.size_hint(), (0, Some(0)));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.clone().next(), None);
            assert_eq!(iter.next(), None);
            if bad > 0 && count > 0 {
                let retained = list.try_iter().next().unwrap().unwrap();
                assert_eq!(retained.as_ptr(), bytes[4..].as_ptr());
                assert_eq!(retained, &[0]);
            }
        }
    }
}

#[test]
fn fixed_chunks_use_inherited_width_and_descriptor_for_checked_defaults() {
    // A caller may explicitly ask for another codec on an admitted View. Its
    // actual descriptor determines chunk width; the codec still rejects it.
    let bytes = [2, 0, 0, 0, 1, 0, 2, 0];
    let view = View::new(&bytes, <List<u16>>::LAYOUT, 40).unwrap();
    let list = view.read::<List<u8>>().unwrap();
    assert_eq!(
        list.try_iter().next(),
        Some(list.try_get(0).map(Option::unwrap))
    );
    assert_eq!(
        list.try_get(0),
        Err(blubber::Error::new(blubber::ErrorKind::InvalidBoundary, 45))
    );
    let bytes = [1, 0, 0, 0, 11, 22, 33];
    let inherited = View::new(&bytes, <List<[u8; 3]>>::LAYOUT, 60)
        .unwrap()
        .read::<List<Pair>>()
        .unwrap()
        .iter()
        .next()
        .unwrap();
    assert_eq!(inherited.read_fixed::<[u8; 3]>(), Ok([11, 22, 33]));
    assert_eq!(
        inherited.field(0).unwrap_err(),
        blubber::ErrorKind::InvalidSchema.into()
    );
    let bytes = encoded::<List<Pair>>(&&[(513, true), (7, false)][..]);
    let list = View::new(&bytes, <List<Pair>>::LAYOUT, 100)
        .unwrap()
        .read::<List<Pair>>()
        .unwrap();
    let mut iter = list.iter();
    let first = iter.next().unwrap();
    let second = iter.next().unwrap();
    assert!(iter.next().is_none());
    assert_eq!((first.base(), second.base()), (104, 107));
    assert_eq!(first.field(0).unwrap().read::<u16>(), Ok(513));
    assert_eq!(second.field(1).unwrap().read::<bool>(), Ok(false));
}

#[test]
fn fixed_chunks_match_selection_for_builtin_aliases_and_every_small_partition() {
    type Alias = u64;
    for count in 0..=4 {
        let values = [0, 1, 17, u64::MAX];
        let bytes = encoded::<List<Alias>>(&&values[..count]);
        for base in [0, 93, u32::MAX - bytes.len() as u32] {
            let list = View::new(&bytes, <List<Alias>>::LAYOUT, base)
                .unwrap()
                .read::<List<Alias>>()
                .unwrap();
            assert_eq!(
                list.try_iter().collect::<Vec<_>>(),
                (0..count)
                    .map(|i| list.try_get(i).map(Option::unwrap))
                    .collect::<Vec<_>>()
            );
        }
    }
    for first in 0..=255u8 {
        let bytes = [3, 0, 0, 0, first, 1, 2];
        // The provided bool decoder ignores metadata even when the admitted
        // one-byte stride belongs to another scalar domain.
        let list = View::new(&bytes, <List<u8>>::LAYOUT, 100)
            .unwrap()
            .read::<List<bool>>()
            .unwrap();
        let mut expected = Vec::new();
        for i in 0..3 {
            let item = list.try_get(i).map(Option::unwrap);
            expected.push(item);
            if item.is_err() {
                break;
            }
        }
        assert_eq!(list.try_iter().collect::<Vec<_>>(), expected);
    }
}

#[cfg(feature = "derive")]
#[blubber::schema]
struct ChunkRecord {
    number: u16,
    flag: bool,
}

#[cfg(feature = "derive")]
#[blubber::schema]
struct GenericChunk<T> {
    value: T,
}

#[cfg(feature = "derive")]
#[test]
fn generated_fixed_view_chunks_keep_fields_lazy_and_items_alive() {
    let bytes = [2, 0, 0, 0, 7, 0, 9, 8, 0, 1];
    let list = View::new(&bytes, <List<ChunkRecord>>::LAYOUT, 50)
        .unwrap()
        .read::<List<ChunkRecord>>()
        .unwrap();
    let mut iter = list.try_iter();
    let first = iter.next().unwrap().unwrap();
    let second = iter.next().unwrap().unwrap();
    assert!(iter.next().is_none());
    assert_eq!(first.number(), 7);
    assert_eq!(
        first.try_flag(),
        Err(blubber::Error::new(blubber::ErrorKind::InvalidBool, 56))
    );
    assert!(second.flag());
    let bytes = [2, 0, 0, 0, 0, 9];
    let list = View::new(&bytes, <List<GenericChunk<bool>>>::LAYOUT, 70)
        .unwrap()
        .read::<List<GenericChunk<bool>>>()
        .unwrap();
    let values = list.iter().collect::<Vec<_>>();
    assert_eq!(values[0].try_value(), Ok(false));
    assert_eq!(
        values[1].try_value(),
        Err(blubber::Error::new(blubber::ErrorKind::InvalidBool, 75))
    );
}

#[test]
fn fixed_map_chunks_preserve_key_domains_lazy_order_errors_and_borrowed_items() {
    for keys in [[0u8, 1], [0, 0], [1, 0], [0, 2]] {
        for values in [[0, 1], [7, 1], [0, 8]] {
            let bytes = [2, 0, 0, 0, keys[0], values[0], keys[1], values[1]];
            for base in [0, 91, u32::MAX - bytes.len() as u32] {
                let map = View::new(&bytes, <Map<bool, bool>>::LAYOUT, base)
                    .unwrap()
                    .read::<Map<bool, bool>>()
                    .unwrap();
                let mut iter = map.try_iter();
                let mut clone = iter.clone();
                assert_eq!(iter.size_hint(), (1, Some(2)));
                for i in 0..2 {
                    let expected = if keys[i] > 1 {
                        Err(blubber::Error::new(
                            blubber::ErrorKind::InvalidBool,
                            base + 4 + 2 * i as u32,
                        ))
                    } else if values[i] > 1 {
                        Err(blubber::Error::new(
                            blubber::ErrorKind::InvalidBool,
                            base + 5 + 2 * i as u32,
                        ))
                    } else {
                        Ok((keys[i] != 0, values[i] != 0))
                    };
                    assert_eq!(iter.next(), Some(expected));
                    assert_eq!(clone.next(), Some(expected));
                    if expected.is_err() {
                        break;
                    }
                }
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
                assert_eq!(iter.size_hint(), (0, Some(0)));
            }
        }
    }
    let bytes = [2, 0, 0, 0, 1, 9, 3, 8];
    let map = View::new(&bytes, <Map<u8, u8>>::LAYOUT, 55)
        .unwrap()
        .read::<Map<u8, region::Domain>>()
        .unwrap();
    let mut iter = map.try_iter();
    let first = iter.next().unwrap().unwrap();
    let second = iter.next().unwrap().unwrap();
    assert!(iter.next().is_none());
    assert_eq!((first.0, second.0), (1, 3));
    // The default hook retains the inherited descriptor and exact value slice.
    assert_eq!((first.1, second.1), (&bytes[5..6], &bytes[7..8]));
}

#[test]
fn streamed_lists_match_point_reads_for_every_small_byte_mutation() {
    let fixtures: &[&[&str]] = &[&[], &[""], &["é"], &["", "λ", "", "🙂"]];
    for values in fixtures {
        let original = encoded::<List<Str>>(values);
        let mut bytes = original.clone();
        for at in 0..original.len() {
            for value in 0..=u8::MAX {
                bytes[at] = value;
                let base = u32::MAX - bytes.len() as u32;
                let view = View::new(&bytes, <List<Str>>::LAYOUT, base).unwrap();
                let Ok(list) = view.read::<List<Str>>() else {
                    continue;
                };
                let mut iter = list.try_iter();
                for index in 0..list.len() {
                    let expected = list.try_get(index).map(Option::unwrap);
                    assert_eq!(
                        iter.next(),
                        Some(expected),
                        "byte {at}={value}, index {index}"
                    );
                    if expected.is_err() {
                        break;
                    }
                }
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
                assert_eq!(iter.size_hint(), (0, Some(0)));
            }
            bytes[at] = original[at];
        }
    }
}

#[test]
fn streamed_items_retain_original_unicode_regions_and_borrow() {
    let bytes = encoded::<List<Str>>(&&["", "é", "🙂", ""][..]);
    let list = View::new(&bytes, <List<Str>>::LAYOUT, 37)
        .unwrap()
        .read::<List<Str>>()
        .unwrap();
    let (empty, text) = {
        let mut iter = list.try_iter();
        let empty = iter.next().unwrap().unwrap();
        let text = iter.next().unwrap().unwrap();
        let mut clone = iter.clone();
        assert_eq!(iter.next(), Some(Ok("🙂")));
        assert_eq!(clone.next(), Some(Ok("🙂")));
        (empty, text)
    };
    assert_eq!(text, "é");
    assert!(core::ptr::eq(text.as_ptr(), bytes[16..].as_ptr()));
    assert!(core::ptr::eq(empty.as_ptr(), text.as_ptr()));
    assert_eq!(list.iter().collect::<Vec<_>>(), ["", "é", "🙂", ""]);
}

#[test]
fn streamed_foreign_layouts_match_both_point_read_directions() {
    for bytes in [
        [2, 0, 0, 0, 0, 0, 0, 0, 195, 169],
        [2, 0, 0, 0, 1, 0, 0, 0, 195, 169],
        [2, 0, 0, 0, 2, 0, 0, 0, 195, 169],
    ] {
        let view = View::new(&bytes, <List<Str>>::LAYOUT, 123).unwrap();
        let list = view.read::<List<u16>>().unwrap();
        let mut iter = list.try_iter();
        for index in 0..2 {
            let expected = list.try_get(index).map(Option::unwrap);
            assert_eq!(iter.next(), Some(expected));
            if expected.is_err() {
                break;
            }
        }
        assert_eq!(iter.next(), None);
    }
    for payload in [[0, 0, 0, 0], [195, 169, 0, 0], [255, 255, 195, 169]] {
        let mut bytes = [2, 0, 0, 0, 0, 0, 0, 0];
        bytes[4..].copy_from_slice(&payload);
        let view = View::new(&bytes, <List<u16>>::LAYOUT, 123).unwrap();
        let strings = view.read::<List<Str>>().unwrap();
        let mut iter = strings.try_iter();
        for index in 0..2 {
            let expected = strings.try_get(index).map(Option::unwrap);
            assert_eq!(iter.next(), Some(expected));
            if expected.is_err() {
                break;
            }
        }
        assert_eq!(iter.next(), None);
        let narrow = view.read::<List<u8>>().unwrap();
        assert_eq!(
            narrow.try_iter().next(),
            Some(narrow.try_get(0).map(Option::unwrap))
        );
    }
}

#[test]
fn infallible_folds_match_selection_before_and_after_partial_consumption() {
    for values in [
        &[][..],
        &[0][..],
        &[7, 9][..],
        &[0x0123, 0xabcd, 0x789a][..],
    ] {
        let bytes = encoded::<List<u16>>(&values);
        let list = read::<List<u16>>(&bytes).unwrap();
        assert_eq!(
            list.iter().fold(Vec::new(), |mut out, item| {
                out.push(item);
                out
            }),
            values
        );
        let mut iter = list.iter();
        assert_eq!(
            iter.size_hint(),
            (usize::from(!values.is_empty()), Some(values.len()))
        );
        assert_eq!(iter.next(), values.first().copied());
        assert_eq!(
            iter.size_hint(),
            (
                usize::from(values.len() > 1),
                Some(values.len().saturating_sub(1))
            )
        );
        let clone = iter.clone();
        let expected = &values[usize::from(!values.is_empty())..];
        assert_eq!(
            iter.fold(Vec::new(), |mut out, item| {
                out.push(item);
                out
            }),
            expected
        );
        assert_eq!(clone.collect::<Vec<_>>(), expected);
    }
    for values in [&[][..], &[""][..], &["é", "", "🙂"][..]] {
        let bytes = encoded::<List<Str>>(&values);
        let list = read::<List<Str>>(&bytes).unwrap();
        assert_eq!(
            list.iter().fold(Vec::new(), |mut out, item| {
                out.push(item);
                out
            }),
            values
        );
        let mut iter = list.iter();
        assert_eq!(
            iter.size_hint(),
            (usize::from(!values.is_empty()), Some(values.len()))
        );
        assert_eq!(iter.next(), values.first().copied());
        assert_eq!(
            iter.size_hint(),
            (
                usize::from(values.len() > 1),
                Some(values.len().saturating_sub(1))
            )
        );
        assert_eq!(
            iter.fold(Vec::new(), |mut out, item| {
                out.push(item);
                out
            }),
            &values[usize::from(!values.is_empty())..]
        );
    }
    let bytes = [2, 0, 0, 0, 195, 169, 0, 0];
    let view = View::new(&bytes, <List<u16>>::LAYOUT, 29).unwrap();
    let strings = view.read::<List<Str>>().unwrap();
    assert_eq!(
        strings.iter().fold(Vec::new(), |mut out, item| {
            out.push(item);
            out
        }),
        ["é", "\0\0"]
    );
    let narrow = view.read::<List<u8>>().unwrap();
    assert!(narrow.try_get(0).is_err());
    assert!(std::panic::catch_unwind(|| narrow
        .iter()
        .fold(0u64, |sum, item| sum + u64::from(item)))
    .is_err());
    let variable = [2, 0, 0, 0, 1, 0, 0, 0, 3, 7];
    let numbers = View::new(&variable, <List<Str>>::LAYOUT, 29)
        .unwrap()
        .read::<List<u8>>()
        .unwrap();
    assert_eq!(
        numbers.iter().fold(0u64, |sum, item| sum + u64::from(item)),
        10
    );
}

#[test]
fn infallible_iterator_keeps_error_fusion_after_panic() {
    let bytes = [3, 0, 0, 0, 0, 2, 1];
    let list = View::new(&bytes, <List<u8>>::LAYOUT, 29)
        .unwrap()
        .read::<List<bool>>()
        .unwrap();
    let mut iter = list.iter();
    assert_eq!(iter.next(), Some(false));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| iter.next())).is_err());
    assert_eq!(iter.next(), None);
    assert_eq!(iter.clone().next(), None);
    assert_eq!(iter.size_hint(), (0, Some(0)));
}

struct PanickingStr;

impl Schema for PanickingStr {
    const KIND: blubber::Kind = blubber::Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static blubber_core::Layout = <Str as Schema>::LAYOUT;
    const __LEN: usize = <Str as Schema>::__LEN;
    const __HEADER: &'static [u32] = <Str as Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = &'a str;
    type Mut<'a> = blubber::raw::Edit<'a>;
    type Init<'a> = &'a str;
    type Removed = ();

    fn project(edit: blubber::__private::Projection<'_, Self>) -> Self::Mut<'_> {
        blubber::raw::into_edit(edit)
    }

    fn read(view: View<'_>) -> blubber::Result<Self::Read<'_>> {
        assert_ne!(view.bytes(), b"panic", "reader panic control");
        <Str as Schema>::read(view)
    }

    fn read_removed(view: View<'_>) -> blubber::Result<()> {
        view.validate()
    }

    fn measure(value: &&str) -> blubber::Result<u32> {
        <Str as Schema>::measure(value)
    }

    fn encode(value: &&str, bytes: &mut [u8]) {
        <Str as Schema>::encode(value, bytes);
    }

    #[cfg(feature = "alloc")]
    type Owned = <Str as Schema>::Owned;

    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self::Owned) -> blubber::Result<u32> {
        <Str as Schema>::measure_owned(value)
    }

    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self::Owned, destination: &mut [u8]) {
        <Str as Schema>::encode_owned(value, destination);
    }

    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> blubber::Result<Self::Owned> {
        <Str as Schema>::decode_owned(view)
    }
}

#[test]
fn streamed_iterator_retains_region_when_decoder_panics() {
    for values in [&["panic", "tail"][..], &["ok", "panic", ""][..]] {
        let bytes = encoded::<List<Str>>(&values);
        let list = read::<List<PanickingStr>>(&bytes).unwrap();
        let mut iter = list.try_iter();
        if values[0] == "ok" {
            assert_eq!(iter.next(), Some(Ok("ok")));
        }
        let hint = iter.size_hint();
        for _ in 0..2 {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| iter.next())).is_err()
            );
            assert_eq!(iter.size_hint(), hint);
            let mut clone = iter.clone();
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| clone.next())).is_err()
            );
        }
    }
}

#[test]
fn fixed_wide_iterator_hints_count_slots() {
    let bytes = encoded::<List<u16>>(&&[1, 2, 3][..]);
    let list = read::<List<u16>>(&bytes).unwrap();
    let mut iter = list.try_iter();
    let mut plain = list.iter();
    for remaining in (0..=3).rev() {
        assert_eq!(
            iter.size_hint(),
            (usize::from(remaining > 0), Some(remaining))
        );
        assert_eq!(iter.clone().size_hint(), iter.size_hint());
        assert_eq!(plain.size_hint(), iter.size_hint());
        if remaining > 0 {
            assert!(iter.next().unwrap().is_ok());
            let _ = plain.next();
        }
    }
    assert!(iter.next().is_none());
}

#[test]
fn streamed_maps_match_point_reads_for_each_small_end_and_payload_mutation() {
    let original = encoded::<Map<bool, Str>>(&&[(false, "é"), (true, "x")][..]);
    for layout in [
        *<Map<bool, Str>>::LAYOUT,
        blubber_core::Layout::Flat {
            checked: blubber::__layout_of!(Map<bool, Str>),
        },
    ] {
        let mut bytes = original.clone();
        for at in (5..9).chain(10..original.len()) {
            for value in 0..=u8::MAX {
                bytes[at] = value;
                let base = u32::MAX - bytes.len() as u32;
                let map = View::new(&bytes, layout, base)
                    .unwrap()
                    .read::<Map<bool, Str>>()
                    .unwrap();
                let mut iter = map.try_iter();
                for key in [false, true] {
                    let expected = map.try_get(&key).map(|value| (key, value.unwrap()));
                    let mut clone = iter.clone();
                    assert_eq!(iter.next(), Some(expected), "byte {at}={value}, key {key}");
                    assert_eq!(clone.next(), Some(expected));
                    if expected.is_err() {
                        break;
                    }
                }
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
                assert_eq!(iter.size_hint(), (0, Some(0)));
            }
            bytes[at] = original[at];
        }
    }
}

#[test]
fn streamed_map_keys_keep_domains_physical_order_and_absence() {
    let mut bytes = encoded::<Map<bool, Str>>(&&[(false, "a"), (true, "b")][..]);
    bytes[4] = 2;
    bytes[5..9].copy_from_slice(&u32::MAX.to_le_bytes());
    let map = View::new(&bytes, <Map<bool, Str>>::LAYOUT, 37)
        .unwrap()
        .read::<Map<bool, Str>>()
        .unwrap();
    let mut iter = map.try_iter();
    assert_eq!(
        iter.next(),
        Some(Err(blubber::Error::new(
            blubber::ErrorKind::InvalidBool,
            41
        )))
    );
    assert_eq!(iter.next(), None);
    let mut plain = map.iter();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| plain.next())).is_err());
    assert_eq!(plain.next(), None);
    bytes[4] = 0;
    bytes[5..9].copy_from_slice(&1u32.to_le_bytes());
    bytes[9] = 0;
    let map = read::<Map<bool, Str>>(&bytes).unwrap();
    assert_eq!(map.iter().collect::<Vec<_>>(), [(false, "a"), (false, "b")]);
    assert_eq!(map.try_get(&true), Ok(None));
    assert!(matches!(
        map.validate(),
        Err(blubber::Error::Fault {
            kind: blubber::ErrorKind::DuplicateKey,
            offset: 9
        })
    ));
    bytes[4] = 1;
    let map = read::<Map<bool, Str>>(&bytes).unwrap();
    assert_eq!(map.iter().collect::<Vec<_>>(), [(true, "a"), (false, "b")]);
    assert!(matches!(
        map.validate(),
        Err(blubber::Error::Fault {
            kind: blubber::ErrorKind::UnsortedKeys,
            offset: 9
        })
    ));
}

#[test]
fn streamed_map_foreign_layouts_keep_point_read_behavior() {
    for first in 0u32..=3 {
        let mut bytes = encoded::<Map<u8, Str>>(&&[(1, "é"), (2, "xy")][..]);
        bytes[5..9].copy_from_slice(&first.to_le_bytes());
        let map = View::new(&bytes, <Map<u8, Str>>::LAYOUT, 123)
            .unwrap()
            .read::<Map<u8, u16>>()
            .unwrap();
        let mut iter = map.try_iter();
        for key in [1, 2] {
            let expected = map.try_get(&key).map(|value| (key, value.unwrap()));
            assert_eq!(iter.next(), Some(expected));
            if expected.is_err() {
                break;
            }
        }
        assert_eq!(iter.next(), None);
    }
    let bytes = encoded::<Map<u8, u16>>(&&[(1, 0xa9c3), (2, 0)][..]);
    let map = View::new(&bytes, <Map<u8, u16>>::LAYOUT, 123)
        .unwrap()
        .read::<Map<u8, Str>>()
        .unwrap();
    assert_eq!(map.iter().collect::<Vec<_>>(), [(1, "é"), (2, "\0\0")]);
    let narrow = View::new(&bytes, <Map<u8, u16>>::LAYOUT, 123)
        .unwrap()
        .read::<Map<u8, u8>>()
        .unwrap();
    assert_eq!(
        narrow.try_iter().next(),
        Some(narrow.try_get(&1).map(|v| (1, v.unwrap())))
    );
    let bytes = encoded::<Map<u16, Str>>(&&[(1, "a"), (2, "b")][..]);
    let map = View::new(&bytes, <Map<u16, Str>>::LAYOUT, 123)
        .unwrap()
        .read::<Map<u8, Str>>()
        .unwrap();
    let mut iter = map.try_iter();
    assert!(iter.next().unwrap().is_err());
    assert_eq!(iter.next(), None);
    let bytes = encoded::<Map<u8, List<Str>>>(&&[(1, &["é", "🙂"][..])][..]);
    let map = read::<Map<u8, List<Str>>>(&bytes).unwrap();
    assert_eq!(
        map.iter().next().unwrap().1.iter().collect::<Vec<_>>(),
        ["é", "🙂"]
    );
}

#[test]
fn streamed_maps_retain_full_keys_borrows_and_slot_hints() {
    for values in [
        &[][..],
        &[(0u8, "")][..],
        &[(1, ""), (2, "é"), (3, "🙂")][..],
    ] {
        let entries: Vec<_> = values
            .iter()
            .map(|&(k, v)| {
                let mut key = [0u8; 32];
                for (i, byte) in key.iter_mut().enumerate() {
                    *byte = k.wrapping_mul(17).wrapping_add(i as u8);
                }
                (Address::new_from_array(key), v)
            })
            .collect();
        let bytes = encoded::<Map<Address, Str>>(&entries.as_slice());
        let map = read::<Map<Address, Str>>(&bytes).unwrap();
        let (checksum, borrowed) = {
            let mut iter = map.try_iter();
            let mut checksum = 0u64;
            let mut borrowed = None;
            for (i, &expected) in entries.iter().enumerate() {
                assert_eq!(iter.size_hint(), (1, Some(entries.len() - i)));
                assert_eq!(iter.clone().next(), Some(Ok(expected)));
                let (key, value) = iter.next().unwrap().unwrap();
                checksum += key
                    .encode_key()
                    .as_ref()
                    .iter()
                    .chain(value.as_bytes())
                    .map(|&b| u64::from(b))
                    .sum::<u64>();
                let point = map.get(&key).unwrap();
                assert!(core::ptr::eq(value.as_ptr(), point.as_ptr()));
                borrowed = Some(value);
            }
            assert_eq!(iter.next(), None);
            assert_eq!(iter.size_hint(), (0, Some(0)));
            (checksum, borrowed)
        };
        if let Some(value) = borrowed {
            assert_eq!(value, entries.last().unwrap().1);
        }
        let expected: u64 = entries
            .iter()
            .map(|(k, v)| {
                k.encode_key()
                    .as_ref()
                    .iter()
                    .chain(v.as_bytes())
                    .map(|&b| u64::from(b))
                    .sum::<u64>()
            })
            .sum();
        assert_eq!(checksum, expected);
    }
}

#[test]
fn streamed_map_decoder_unwind_retains_the_selected_entry() {
    for values in [
        &[(1u8, "panic"), (2, "tail")][..],
        &[(1, "ok"), (2, "panic"), (3, "")][..],
    ] {
        let bytes = encoded::<Map<u8, Str>>(&values);
        let map = read::<Map<u8, PanickingStr>>(&bytes).unwrap();
        let mut iter = map.try_iter();
        if values[0].1 == "ok" {
            assert_eq!(iter.next(), Some(Ok((1, "ok"))));
        }
        let hint = iter.size_hint();
        for _ in 0..2 {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| iter.next())).is_err()
            );
            assert_eq!(iter.size_hint(), hint);
            let mut clone = iter.clone();
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| clone.next())).is_err()
            );
        }
    }
}

#[test]
fn infallible_map_hints_count_wide_slots_through_clones_and_exhaustion() {
    for values in [&[][..], &[(1u16, 7u32)][..], &[(1, 7), (2, 19)][..]] {
        let bytes = encoded::<Map<u16, u32>>(&values);
        let map = read::<Map<u16, u32>>(&bytes).unwrap();
        let mut iter = map.iter();
        for (index, expected) in values.iter().copied().enumerate() {
            let hint = (1, Some(values.len() - index));
            assert_eq!(iter.size_hint(), hint);
            let mut clone = iter.clone();
            assert_eq!(clone.size_hint(), hint);
            assert_eq!(clone.next(), Some(expected));
            assert_eq!(iter.next(), Some(expected));
        }
        assert_eq!(iter.size_hint(), (0, Some(0)));
        assert_eq!(iter.clone().size_hint(), (0, Some(0)));
        assert_eq!(iter.next(), None);
    }
    let bytes = encoded::<Map<u16, Str>>(&&[(1, ""), (2, "é"), (3, "🙂")][..]);
    let map = read::<Map<u16, Str>>(&bytes).unwrap();
    let mut iter = map.iter();
    for remaining in (0..=3).rev() {
        assert_eq!(
            iter.size_hint(),
            (usize::from(remaining > 0), Some(remaining))
        );
        assert_eq!(iter.clone().size_hint(), iter.size_hint());
        if remaining > 0 {
            assert!(iter.next().is_some());
        }
    }
    assert_eq!(iter.next(), None);
}

#[test]
fn infallible_map_foreign_widths_report_once_before_value_decoding() {
    let bytes = encoded::<Map<u16, u32>>(&&[(1, 7), (2, 19)][..]);
    let map = View::new(&bytes, <Map<u16, u32>>::LAYOUT, 37)
        .unwrap()
        .read::<Map<u8, u32>>()
        .unwrap();
    let mut iter = map.iter();
    assert_eq!(iter.size_hint(), (1, Some(2)));
    let mut clone = iter.clone();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| iter.next())).is_err());
    assert_eq!(iter.next(), None);
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| clone.next())).is_err());
    assert_eq!(clone.next(), None);
    let map = View::new(&bytes, <Map<u16, u32>>::LAYOUT, 37)
        .unwrap()
        .read::<Map<u16, u16>>()
        .unwrap();
    assert_eq!(
        map.try_iter().next(),
        Some(Err(blubber::Error::new(
            blubber::ErrorKind::InvalidBoundary,
            45
        )))
    );
    let mut iter = map.iter();
    assert_eq!(iter.size_hint(), (1, Some(2)));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| iter.next())).is_err());
    assert_eq!(iter.next(), None);
    assert_eq!(iter.size_hint(), (0, Some(0)));
}

std::thread_local! {
    static MAP_REGION_CALLS: core::cell::Cell<usize> = const {core::cell::Cell::new(0)};
}

struct RegionCountedStr;
impl Schema for RegionCountedStr {
    const KIND: blubber::Kind = blubber::Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static blubber_core::Layout = <Str as Schema>::LAYOUT;
    const __LEN: usize = <Str as Schema>::__LEN;
    const __HEADER: &'static [u32] = <Str as Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = &'a str;
    type Mut<'a> = blubber::raw::Edit<'a>;
    type Init<'a> = &'a str;
    type Removed = ();
    fn project(edit: blubber::__private::Projection<'_, Self>) -> Self::Mut<'_> {
        blubber::raw::into_edit(edit)
    }
    fn read(view: View<'_>) -> blubber::Result<&str> {
        Str::read(view)
    }
    fn read_region(bytes: &[u8], layout: blubber_core::Layout, base: u32) -> blubber::Result<&str> {
        MAP_REGION_CALLS.with(|calls| calls.set(calls.get() + 1));
        Str::read_region(bytes, layout, base)
    }
    fn read_removed(view: View<'_>) -> blubber::Result<()> {
        view.validate()
    }
    fn measure(value: &&str) -> blubber::Result<u32> {
        Str::measure(value)
    }
    fn encode(value: &&str, bytes: &mut [u8]) {
        Str::encode(value, bytes)
    }
    #[cfg(feature = "alloc")]
    type Owned = <Str as Schema>::Owned;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self::Owned) -> blubber::Result<u32> {
        Str::measure_owned(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self::Owned, bytes: &mut [u8]) {
        Str::encode_owned(value, bytes)
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> blubber::Result<Self::Owned> {
        Str::decode_owned(view)
    }
}

#[test]
fn streamed_maps_use_one_exact_region_hook_per_visited_value() {
    let original = encoded::<Map<u8, Str>>(&&[(1, "a"), (2, "b")][..]);
    for invalid in [false, true] {
        let mut bytes = original.clone();
        if invalid {
            *bytes.last_mut().unwrap() = 255;
        }
        let map = read::<Map<u8, RegionCountedStr>>(&bytes).unwrap();
        MAP_REGION_CALLS.with(|calls| calls.set(0));
        let mut iter = map.try_iter();
        assert_eq!(MAP_REGION_CALLS.with(|calls| calls.get()), 0);
        assert_eq!(iter.next(), Some(Ok((1, "a"))));
        assert_eq!(MAP_REGION_CALLS.with(|calls| calls.get()), 1);
        let result = iter.next().unwrap();
        assert_eq!(result.is_err(), invalid);
        if !invalid {
            assert_eq!(result, Ok((2, "b")));
        }
        assert_eq!(MAP_REGION_CALLS.with(|calls| calls.get()), 2);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(MAP_REGION_CALLS.with(|calls| calls.get()), 2);
    }
}

fn same_text(left: blubber::Result<&str>, right: blubber::Result<&str>) {
    assert_eq!(left, right);
    if let (Ok(left), Ok(right)) = (left, right) {
        assert!(core::ptr::eq(left.as_ptr(), right.as_ptr()));
    }
}

#[test]
fn batched_unicode_clone_fusion_and_held_borrows() {
    fn fused(_: &impl FusedIterator) {}
    for values in [&[][..], &[""][..], &["", "é", "€", "🙂", "tail", ""][..]] {
        let bytes = encoded::<List<Str>>(&values);
        let frame = bytes.clone();
        let list = read::<List<Str>>(&bytes).unwrap();
        let mut iter = list.try_iter_batched();
        fused(&iter);
        let mut ordinary = list.try_iter();
        let held = iter.clone().collect::<blubber::Result<Vec<_>>>().unwrap();
        for _ in 0..values.len() + 2 {
            assert_eq!(iter.size_hint(), ordinary.size_hint());
            assert_eq!(
                iter.clone().collect::<Vec<_>>(),
                ordinary.clone().collect::<Vec<_>>()
            );
            match (iter.next(), ordinary.next()) {
                (Some(a), Some(b)) => same_text(a, b),
                (None, None) => {}
                _ => panic!("batch length differs"),
            }
        }
        assert_eq!(held, values);
        assert_eq!(bytes, frame);
    }
    let entries = [(false, "é🙂"), (true, "")];
    let bytes = encoded::<Map<bool, Str>>(&&entries[..]);
    let frame = bytes.clone();
    let map = read::<Map<bool, Str>>(&bytes).unwrap();
    let mut batch = map.try_iter_batched();
    fused(&batch);
    assert_eq!(batch.size_hint(), (1, Some(2)));
    let held = batch.next().unwrap().unwrap().1;
    let mut clone = batch.clone();
    assert_eq!(batch.next(), Some(Ok((true, ""))));
    assert_eq!(clone.next(), Some(Ok((true, ""))));
    assert_eq!(batch.next(), None);
    assert_eq!(clone.next(), None);
    assert_eq!(batch.size_hint(), (0, Some(0)));
    assert_eq!(held, "é🙂");
    assert_eq!(bytes, frame);
}

#[test]
fn batched_fallback_preserves_prefixes_and_first_error_offsets() {
    // The complete payload is valid; the first stored end splits a character.
    let split = [2, 0, 0, 0, 1, 0, 0, 0, 0xc3, 0xa9];
    let list = View::new(&split, <List<Str>>::LAYOUT, 29)
        .unwrap()
        .read::<List<Str>>()
        .unwrap();
    let mut batch = list.try_iter_batched();
    assert_eq!(batch.next(), list.try_iter().next());
    assert_eq!(batch.next(), None);
    assert_eq!(batch.clone().next(), None);
    // Aggregate failure must not suppress the valid first item.
    let invalid = [2, 0, 0, 0, 1, 0, 0, 0, b'x', b'y', 0xff];
    let list = View::new(&invalid, <List<Str>>::LAYOUT, 29)
        .unwrap()
        .read::<List<Str>>()
        .unwrap();
    let mut batch = list.try_iter_batched();
    let mut lazy = list.try_iter();
    assert_eq!(batch.next(), Some(Ok("x")));
    assert_eq!(lazy.next(), Some(Ok("x")));
    let fault = blubber::Error::new(blubber::ErrorKind::InvalidUtf8, 39);
    assert_eq!(batch.next(), Some(Err(fault)));
    assert_eq!(lazy.next(), Some(Err(fault)));
    assert_eq!(batch.next(), None);
    // A future invalid end stays lazy in the ordinary iterator.
    let invalid_end = [3, 0, 0, 0, 1, 0, 0, 0, 255, 255, 255, 255, b'x', b'y'];
    let list = View::new(&invalid_end, <List<Str>>::LAYOUT, 29)
        .unwrap()
        .read::<List<Str>>()
        .unwrap();
    assert_eq!(list.try_iter().next(), Some(Ok("x")));
    assert_eq!(
        list.try_iter_batched().collect::<Vec<_>>(),
        list.try_iter().collect::<Vec<_>>()
    );
}

#[test]
fn batched_lists_match_all_small_byte_mutations() {
    let original = encoded::<List<Str>>(&&["x", "é", "🙂", ""][..]);
    for layout in [
        *<List<Str>>::LAYOUT,
        blubber_core::Layout::Flat {
            checked: blubber::__layout_of!(List<Str>),
        },
    ] {
        for at in 4..original.len() {
            for byte in 0..=u8::MAX {
                let mut bytes = original.clone();
                bytes[at] = byte;
                let base = u32::MAX - bytes.len() as u32;
                let list = View::new(&bytes, layout, base)
                    .unwrap()
                    .read::<List<Str>>()
                    .unwrap();
                let mut batch = list.try_iter_batched();
                let mut ordinary = list.try_iter();
                for _ in 0..6 {
                    assert_eq!(batch.size_hint(), ordinary.size_hint());
                    match (batch.next(), ordinary.next()) {
                        (Some(a), Some(b)) => same_text(a, b),
                        (None, None) => {}
                        _ => panic!("batch length differs"),
                    }
                }
            }
        }
    }
}

#[test]
fn batched_maps_match_key_boundary_value_error_order() {
    let original = encoded::<Map<bool, Str>>(&&[(false, "é"), (true, "x")][..]);
    for layout in [
        *<Map<bool, Str>>::LAYOUT,
        blubber_core::Layout::Flat {
            checked: blubber::__layout_of!(Map<bool,Str>),
        },
    ] {
        for at in 4..original.len() {
            for byte in 0..=u8::MAX {
                let mut bytes = original.clone();
                bytes[at] = byte;
                let map = View::new(&bytes, layout, u32::MAX - bytes.len() as u32)
                    .unwrap()
                    .read::<Map<bool, Str>>()
                    .unwrap();
                let mut batch = map.try_iter_batched();
                let mut ordinary = map.try_iter();
                for _ in 0..4 {
                    assert_eq!(batch.size_hint(), ordinary.size_hint());
                    match (batch.next(), ordinary.next()) {
                        (Some(Ok((key, a))), Some(Ok((other, b)))) => {
                            assert_eq!(key, other);
                            same_text(Ok(a), Ok(b));
                        }
                        (a, b) => assert_eq!(a, b),
                    }
                }
            }
        }
    }
    // Simultaneous invalid key, boundary and text: key error wins.
    let bytes = [2, 0, 0, 0, 2, 255, 255, 255, 255, 1, 2, 0, 0, 0, 255, 255];
    let map = View::new(&bytes, <Map<bool, Str>>::LAYOUT, 29)
        .unwrap()
        .read::<Map<bool, Str>>()
        .unwrap();
    assert_eq!(
        map.try_iter_batched().next(),
        Some(Err(blubber::Error::new(
            blubber::ErrorKind::InvalidBool,
            33
        )))
    );
}

#[test]
fn batched_foreign_layouts_retain_ordinary_selection() {
    let bytes = [2, 0, 0, 0, 195, 169, 0, 0];
    let list = View::new(&bytes, <List<u16>>::LAYOUT, 29)
        .unwrap()
        .read::<List<Str>>()
        .unwrap();
    assert_eq!(
        list.try_iter_batched().collect::<Vec<_>>(),
        list.try_iter().collect::<Vec<_>>()
    );
    let bytes = [2, 0, 0, 0, 0, 195, 169, 1, 0, 0];
    let map = View::new(&bytes, <Map<bool, u16>>::LAYOUT, 29)
        .unwrap()
        .read::<Map<bool, Str>>()
        .unwrap();
    assert_eq!(
        map.try_iter_batched().collect::<Vec<_>>(),
        map.try_iter().collect::<Vec<_>>()
    );
    // Repeated and descending physical keys keep the ordinary iterator contract.
    for bytes in [
        [2, 0, 0, 0, 1, 1, 0, 0, 0, 0, 2, 0, 0, 0, b'a', b'b'],
        [2, 0, 0, 0, 1, 1, 0, 0, 0, 1, 2, 0, 0, 0, b'a', b'b'],
    ] {
        let map = View::new(&bytes, <Map<bool, Str>>::LAYOUT, 29)
            .unwrap()
            .read::<Map<bool, Str>>()
            .unwrap();
        assert_eq!(
            map.try_iter_batched().collect::<Vec<_>>(),
            map.try_iter().collect::<Vec<_>>()
        );
        assert!(map.validate().is_err());
    }
}

#[test]
fn concatenated_text_matches_valid_items_and_empty_setup() {
    for values in [
        &[][..],
        &[""][..],
        &["é"][..],
        &["", "a", "é", "中", "🦀", ""][..],
    ] {
        let bytes = encoded::<List<Str>>(&values);
        let list = read::<List<Str>>(&bytes).unwrap();
        let text = list.try_concat_str().unwrap();
        assert_eq!(text, values.concat());
        let sum = |s: &str| s.bytes().map(u64::from).sum::<u64>();
        assert_eq!(sum(text), list.iter().map(sum).sum::<u64>());
        list.validate().unwrap();
        assert_eq!(text.as_ptr(), bytes[list_payload(values.len())..].as_ptr());
    }
    fn list_payload(n: usize) -> usize {
        4 + n.saturating_sub(1) * 4
    }
}

#[test]
fn concatenated_text_admits_split_and_invalid_item_ends() {
    use blubber::{Error, ErrorKind};
    for end in [0u32, 1, 2, 3, u32::MAX] {
        let mut bytes = [2, 0, 0, 0, 0, 0, 0, 0, 0xc3, 0xa9];
        bytes[4..8].copy_from_slice(&end.to_le_bytes());
        let list = read::<List<Str>>(&bytes).unwrap();
        assert_eq!(list.try_concat_str(), Ok("é"));
        match end {
            0 | 2 => list.validate().unwrap(),
            1 => {
                assert_eq!(
                    list.try_iter().next(),
                    Some(Err(Error::new(ErrorKind::InvalidUtf8, 8)))
                );
                assert!(list.validate().is_err());
            }
            _ => assert_eq!(
                list.validate(),
                Err(Error::new(ErrorKind::InvalidBoundary, 4))
            ),
        }
    }
}

#[test]
fn concatenated_text_reports_aggregate_faults_at_the_payload_base() {
    use blubber::{Error, ErrorKind};
    for (payload, offset) in [
        (&[255u8][..], 0),
        (&[b'a', 0xc3][..], 1),
        (&[0xc3, 0xa9, 255][..], 2),
    ] {
        let mut bytes = vec![2, 0, 0, 0, 0, 0, 0, 0];
        bytes.extend_from_slice(payload);
        let list = View::new(&bytes, *List::<Str>::LAYOUT, u32::MAX - bytes.len() as u32)
            .unwrap()
            .read::<List<Str>>()
            .unwrap();
        assert_eq!(
            list.try_concat_str(),
            Err(Error::new(
                ErrorKind::InvalidUtf8,
                u32::MAX - payload.len() as u32 + offset
            ))
        );
    }
}

#[test]
fn concatenated_text_checks_actual_tree_and_flat_schemas() {
    use blubber::{Error, ErrorKind, ListRef};
    for layout in [
        *List::<List<u8>>::LAYOUT,
        blubber_core::Layout::Flat {
            checked: blubber::__layout_of!(List<List<u8>>),
        },
        *List::<u8>::LAYOUT,
    ] {
        let bytes = [1, 0, 0, 0, b'a'];
        let list = ListRef::<Str>::from_view(View::new(&bytes, layout, 19).unwrap()).unwrap();
        assert_eq!(list.try_concat_str(), Err(ErrorKind::InvalidSchema.into()));
    }
    for layout in [
        *List::<Str>::LAYOUT,
        blubber_core::Layout::Flat {
            checked: blubber::__layout_of!(List<Str>),
        },
    ] {
        for (bytes, expected) in [
            (&[0u8, 0, 0, 0][..], ""),
            (&[1, 0, 0, 0, 0xc3, 0xa9][..], "é"),
        ] {
            let list = ListRef::<Str>::from_view(View::new(bytes, layout, 19).unwrap()).unwrap();
            assert_eq!(list.try_concat_str(), Ok(expected));
        }
    }
    assert_eq!(
        blubber_core::concat_read::payload(&[0, 0, 0, 0, 1]),
        Err(Error::new(ErrorKind::InvalidBoundary, 4))
    );
}

#[test]
fn concatenated_text_header_errors_and_full_width_counts() {
    use blubber::{Error, ErrorKind};
    for (bytes, expected) in [
        (&[][..], Error::new(ErrorKind::Truncated, 0)),
        (&[1, 0, 0][..], Error::new(ErrorKind::Truncated, 0)),
        (
            &[3, 0, 0, 0, 0, 0, 0][..],
            Error::new(ErrorKind::Truncated, 4),
        ),
        (
            &[3, 0, 0, 0, 0, 0, 0, 0, 0][..],
            Error::new(ErrorKind::Truncated, 8),
        ),
        (
            &[255, 255, 255, 255][..],
            Error::new(ErrorKind::ArithmeticOverflow, u32::MAX),
        ),
        (
            &[0, 0, 0, 64][..],
            Error::new(ErrorKind::ArithmeticOverflow, u32::MAX),
        ),
    ] {
        assert_eq!(blubber_core::concat_read::payload(bytes), Err(expected));
    }
    let bytes = encoded::<List<Str>>(&&["a", "é"][..]);
    let text = read::<List<Str>>(&bytes).unwrap().try_concat_str().unwrap();
    let mut clone = read::<List<Str>>(&bytes).unwrap().try_iter();
    assert_eq!(clone.next(), Some(Ok("a")));
    assert_eq!(text, "aé");
    assert_eq!(clone.next(), Some(Ok("é")));
}

#[path = "support/fixed_fold.rs"]
mod fixed_fold;

#[path = "support/map_values.rs"]
mod map_values;

#[test]
fn strict32_checked_reader_preserves_prefix_faults_and_values() {
    use blubber::{Error, ErrorKind};
    for position in 0..32 {
        let mut first = [0xa5; 32];
        let mut second = first;
        first[position] = 0x7f;
        second[position] = 0x80;
        first[position + 1..].fill(0xff);
        second[position + 1..].fill(0);
        let entries = [
            (Address::new_from_array(first), false),
            (Address::new_from_array(second), true),
        ];
        let mut bytes = encoded::<Map<Address, bool>>(&entries.as_slice());
        let checked = View::new(&bytes, <Map<Address, bool>>::LAYOUT, 31).unwrap();
        checked.validate().unwrap();
        assert_eq!(
            checked.read::<Map<Address, bool>>().unwrap().iter().count(),
            2
        );
        bytes[37..69].copy_from_slice(&first);
        let checked = View::new(&bytes, <Map<Address, bool>>::LAYOUT, 31).unwrap();
        assert_eq!(
            checked.validate(),
            Err(Error::new(ErrorKind::DuplicateKey, 68))
        );
        bytes[4..36].copy_from_slice(&second);
        let checked = View::new(&bytes, <Map<Address, bool>>::LAYOUT, 31).unwrap();
        assert_eq!(
            checked.validate(),
            Err(Error::new(ErrorKind::UnsortedKeys, 68))
        );
    }
}

#[path = "support/point_reads.rs"]
mod point_reads;

#[test]
fn shared_iteration_retains_underlying_frame_after_handle_scope() {
    let bytes = encoded::<List<Str>>(&&["one", "two"][..]);
    let mut iter = {
        let list = read::<List<Str>>(&bytes).unwrap();
        (&list).into_iter()
    };
    assert_eq!(iter.next(), Some("one"));
    assert_eq!(iter.clone().collect::<Vec<_>>(), ["two"]);
    assert_eq!(iter.next(), Some("two"));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    let bytes = encoded::<Map<u16, Str>>(&&[(1, "one"), (2, "two")][..]);
    let mut iter = {
        let map = read::<Map<u16, Str>>(&bytes).unwrap();
        (&map).into_iter()
    };
    assert_eq!(iter.next(), Some((1, "one")));
    assert_eq!(iter.clone().collect::<Vec<_>>(), [(2, "two")]);
    assert_eq!(iter.next(), Some((2, "two")));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
}

#[cfg(feature = "derive")]
#[test]
fn collection_and_generated_traits_have_no_child_bounds() {
    #[blubber::schema]
    struct Generic<T: Schema> {
        value: T,
    }
    fn copy<T: Copy>() {}
    fn debug<T: core::fmt::Debug>() {}
    fn auto<T: Send + Sync + Unpin>() {}
    copy::<List<Generic<Str>>>();
    copy::<Map<u16, Generic<Str>>>();
    copy::<blubber::ListRef<'_, Generic<Str>>>();
    copy::<blubber::MapRef<'_, u16, Generic<Str>>>();
    copy::<GenericRef<'_, Str>>();
    debug::<List<Generic<Str>>>();
    debug::<Map<u16, Generic<Str>>>();
    debug::<blubber::ListRef<'_, Generic<Str>>>();
    debug::<blubber::MapRef<'_, u16, Generic<Str>>>();
    debug::<GenericRef<'_, Str>>();
    debug::<GenericMut<'_, Str>>();
    fn iter<I: Clone + FusedIterator>() {}
    iter::<blubber::ListIter<'_, Generic<Str>>>();
    iter::<blubber::TryListIter<'_, Generic<Str>>>();
    iter::<blubber::MapIter<'_, u16, Generic<Str>>>();
    iter::<blubber::TryMapIter<'_, u16, Generic<Str>>>();
    iter::<blubber::TryBatchedListIter<'_>>();
    iter::<blubber::TryBatchedMapIter<'_, u16>>();
    iter::<blubber::TryMapValues<'_, u16, u64>>();
    auto::<blubber::ListRef<'_, Generic<Str>>>();
    auto::<blubber::MapRef<'_, u16, Generic<Str>>>();
    let bytes = encoded::<List<Generic<Str>>>(
        &&[GenericInit {
            value: "private text",
        }][..],
    );
    let list = read::<List<Generic<Str>>>(&bytes).unwrap();
    assert!(!format!("{list:?}").contains("private text"));
    assert_eq!(list.get(0).unwrap().value(), "private text");
    assert!(!format!("{:?}", list.get(0).unwrap()).contains("private text"));
}

#[test]
fn reader_debug_does_not_admit_or_decode_invalid_values() {
    let bytes = [1, 0, 0, 0, 0xff];
    let list = View::new(&bytes, <List<RegionCountedStr>>::LAYOUT, 37)
        .unwrap()
        .read::<List<RegionCountedStr>>()
        .unwrap();
    MAP_REGION_CALLS.with(|calls| calls.set(0));
    assert_eq!(format!("{list:?}"), "ListRef { base: 37, bytes: 5, .. }");
    MAP_REGION_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    assert!(list.try_iter().next().unwrap().is_err());
    MAP_REGION_CALLS.with(|calls| assert_eq!(calls.get(), 1));
    let bytes = [1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0xff];
    let map = View::new(&bytes, <Map<u16, RegionCountedStr>>::LAYOUT, 37)
        .unwrap()
        .read::<Map<u16, RegionCountedStr>>()
        .unwrap();
    MAP_REGION_CALLS.with(|calls| calls.set(0));
    assert_eq!(format!("{map:?}"), "MapRef { base: 37, bytes: 11, .. }");
    MAP_REGION_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    assert!(map.try_iter().next().unwrap().is_err());
    MAP_REGION_CALLS.with(|calls| assert_eq!(calls.get(), 1));
}
