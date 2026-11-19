//! Production typed edits on independently checked, finite structural geometries.

#[path = "../../../crates/blubber/src/list_bounds.rs"]
mod bounds;
#[path = "../../../crates/blubber/tests/support/record.rs"]
mod shapes;

blubber::__root_schema!(__ConcreteRoot0, List<u32>);
blubber::__root_schema!(__ConcreteRoot1, List<Record>);
blubber::__root_schema!(__PreparedString, blubber::Str);

use blubber::raw::{Edit, Resize};
use blubber::Resize as _;
use blubber::{Error, ErrorKind, List, Map, ReadStorage, Schema, SliceStore};
use shapes::{Record, RecordInit, Records};

// Check the same complete byte frame in short slices. This keeps the final
// equality's memcmp bound from expanding every unrelated production loop.
fn assert_same_bytes<const N: usize>(actual: &[u8; N], expected: &[u8; N]) {
    for (actual, expected) in actual.chunks(4).zip(expected.chunks(4)) {
        assert_eq!(actual, expected);
    }
}

// Independent bounded format oracle: no kernel, typed reader or schema encoder.
fn word(bytes: &mut [u8], offset: usize, value: usize) {
    bytes[offset..offset + 4].copy_from_slice(&(value as u32).to_le_bytes());
}

fn wire(records: &[RecordInit<'_>]) -> [u8; 48] {
    let mut expected = [0; 48];
    expected[..8].copy_from_slice(&Records::DISCRIMINATOR);
    word(&mut expected, 12, records.len());
    let table = records.len().saturating_sub(1) * 4;
    let payload = 16 + table;
    let mut end = payload;
    for (index, record) in records.iter().enumerate() {
        word(&mut expected, end, record.name.len());
        end += 4;
        expected[end..end + record.name.len()].copy_from_slice(record.name.as_bytes());
        end += record.name.len();
        word(&mut expected, end, record.tags.len());
        end += 4;
        for tag in record.tags {
            expected[end..end + 4].copy_from_slice(&tag.to_le_bytes());
            end += 4;
        }
        if index + 1 < records.len() {
            word(&mut expected, 16 + index * 4, end - payload);
        }
    }
    word(&mut expected, 8, end);
    expected
}

fn assert_bytes(store: &SliceStore<'_>, expected: &[u8; 48]) {
    let bytes = store.bytes();
    assert!(bytes.len() <= 48);
    // Word equality preserves every byte, including partial physical words.
    for word in 0..6 {
        let at = word * 8;
        if at + 8 <= bytes.len() {
            let actual: [u8; 8] = bytes[at..at + 8].try_into().unwrap();
            let oracle: [u8; 8] = expected[at..at + 8].try_into().unwrap();
            assert_eq!(u64::from_le_bytes(actual), u64::from_le_bytes(oracle));
        } else {
            for byte in 0..8 {
                let index = at + byte;
                if index < bytes.len() {
                    assert_eq!(bytes[index], expected[index]);
                }
            }
        }
    }
}

fn assert_full_bytes(actual: &[u8; 48], expected: &[u8; 48]) {
    for word in 0..6 {
        let at = word * 8;
        let actual: [u8; 8] = actual[at..at + 8].try_into().unwrap();
        let expected: [u8; 8] = expected[at..at + 8].try_into().unwrap();
        assert_eq!(u64::from_le_bytes(actual), u64::from_le_bytes(expected));
    }
}

fn fixed_wire(tags: &[u32]) -> [u8; 48] {
    let mut bytes = [0; 48];
    bytes[..8].copy_from_slice(&Records::DISCRIMINATOR);
    word(&mut bytes, 12, tags.len());
    let mut end = 16;
    for tag in tags {
        bytes[end..end + 4].copy_from_slice(&tag.to_le_bytes());
        end += 4;
    }
    word(&mut bytes, 8, end);
    bytes
}

fn initial(tags: &[u32; 2]) -> [RecordInit<'_>; 2] {
    [
        RecordInit {
            name: "",
            tags: &[],
        },
        RecordInit {
            name: "x",
            tags: &tags[..1],
        },
    ]
}

fn admission<const FIXED: bool, const COUNT: usize>() {
    let capacity: u8 = kani::any();
    kani::assume(capacity <= 48);
    let capacity = capacity as usize;
    let tags: [u32; 2] = kani::any();
    let records = initial(&tags);
    let mut bytes = if FIXED {
        fixed_wire(&tags[..COUNT])
    } else {
        wire(&records[..COUNT])
    };
    let before = bytes;
    let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut store = SliceStore::new(&mut bytes[..capacity]);
    {
        let result = if FIXED {
            Edit::load_trusted::<__ConcreteRoot0>(&mut store, Records::DISCRIMINATOR)
        } else {
            Edit::load_trusted::<__ConcreteRoot1>(&mut store, Records::DISCRIMINATOR)
        };
        assert_eq!(result.is_ok(), capacity >= used);
        if let Err(error) = result {
            assert_eq!(
                error,
                blubber::Error::new(
                    if capacity < 12 {
                        ErrorKind::Truncated
                    } else {
                        ErrorKind::UsedLength
                    },
                    if capacity < 12 { 0 } else { 8 }
                )
            );
        }
    }
    assert_bytes(&store, &before);
    assert!(!store.is_poisoned());
    kani::cover!(true, "admission returns");
}

fn fixed_case<const COUNT: usize, const INDEX: usize, const CAP: usize, const INSERT: bool>() {
    let tags: [u32; 2] = kani::any();
    let value: u32 = kani::any();
    let mut bytes = fixed_wire(&tags[..COUNT]);
    let before = bytes;
    let result;
    {
        let mut store = SliceStore::new(&mut bytes[..CAP]);
        {
            let mut list = blubber::raw::project::<List<u32>>(
                Edit::load_trusted::<__ConcreteRoot0>(&mut store, Records::DISCRIMINATOR).unwrap(),
            );
            result = if INSERT {
                list.insert(INDEX, value)
            } else {
                list.remove(INDEX).map(|old| {
                    assert_eq!(old, tags[INDEX]);
                })
            };
        }
        assert!(!store.is_poisoned());
        assert_eq!(store.bytes().len(), CAP);
    }
    let new_count = if INSERT { COUNT + 1 } else { COUNT - 1 };
    assert_eq!(
        result.is_ok(),
        CAP >= (16 + 4 * COUNT).max(16 + 4 * new_count)
    );
    match result {
        Err(error) => {
            assert_eq!(error, ErrorKind::Capacity.into());
            assert_full_bytes(&bytes, &before);
        }
        Ok(()) => {
            let mut expected = [0; 3];
            expected[..COUNT].copy_from_slice(&tags[..COUNT]);
            if INSERT {
                for at in (INDEX..COUNT).rev() {
                    expected[at + 1] = expected[at];
                }
                expected[INDEX] = value;
            } else {
                for at in INDEX..COUNT - 1 {
                    expected[at] = expected[at + 1];
                }
            }
            assert_full_bytes(&bytes, &fixed_wire(&expected[..new_count]));
        }
    }
    kani::cover!(true, "fixed edit returns");
}

#[kani::proof]
fn record_indices_full_width() {
    let index: usize = kani::any();
    let count: u32 = kani::any();
    let insertion: bool = kani::any();
    let result = bounds::checked_list_index(index, count, insertion);
    let valid = index <= count as usize && (insertion || index < count as usize);
    assert_eq!(result.is_ok(), valid);
    match result {
        Ok(ordinal) => assert_eq!(ordinal as usize, index),
        Err(error) => assert_eq!(error, ErrorKind::IndexOutOfBounds.into()),
    }
    kani::cover!(valid, "accepted ordinal exists");
}

#[kani::proof]
fn record_truncation_full_width() {
    let requested: usize = kani::any();
    let count: u32 = kani::any();
    let kept = bounds::truncate_len(requested, count);
    assert_eq!(kept as usize, requested.min(count as usize));
    assert!(kept <= count);
    kani::cover!(kept == 0, "empty prefix exists");
}

include!(concat!(env!("OUT_DIR"), "/record_cases.rs"));

#[kani::proof]
#[kani::unwind(9)]
fn record_prepared_capacity_reject() {
    let mut bytes: [u8; 16] = kani::any();
    bytes[8..12].copy_from_slice(&16u32.to_le_bytes());
    let before = bytes;
    let layout = blubber_core::Layout::Flat {
        checked: <__PreparedString as blubber::RootSchema>::__layout(),
    };
    let prepared = blubber_core::raw::resolve_edit(
        &mut bytes,
        16,
        &layout,
        &blubber_core::raw::InlinePath::new(),
    )
    .unwrap()
    .prepare_replace(8, 24)
    .unwrap();
    assert_eq!(prepared.prepared_len(), 20);
    assert!(matches!(
        prepared.begin_prepared(),
        Err(blubber::Error::Fault {
            kind: ErrorKind::InvalidPlan,
            ..
        })
    ));
    // Two exact words check all 16 bytes without memcmp's longer loop.
    let mut at = 0;
    while at < 16 {
        let actual: [u8; 8] = bytes[at..at + 8].try_into().unwrap();
        let expected: [u8; 8] = before[at..at + 8].try_into().unwrap();
        assert_eq!(u64::from_le_bytes(actual), u64::from_le_bytes(expected));
        at += 8;
    }
    kani::cover!(
        true,
        "insufficient prepared capacity rejects without writes"
    );
}

#[kani::proof]
#[kani::unwind(49)]
fn record_fixed_destination() {
    let mut bytes: [u8; 32] = kani::any();
    bytes[..8].copy_from_slice(b"prepared");
    bytes[8..12].copy_from_slice(&24u32.to_le_bytes());
    let before = bytes;
    let offset: u32 = kani::any();
    let width: u32 = kani::any();
    let valid = offset <= 12 && width <= 12 - offset;
    let result =
        blubber_core::raw::fixed_destination(&mut bytes, *b"prepared", 12, Some(12), offset, width);
    assert_eq!(result.is_ok(), valid);
    if let Ok(destination) = result {
        assert_eq!(destination.len(), width as usize);
        destination.fill(0x5a);
    }
    for i in 0..32 {
        assert_eq!(
            bytes[i],
            if valid && i >= 12 + offset as usize && i < 12 + offset as usize + width as usize {
                0x5a
            } else {
                before[i]
            }
        );
    }
    kani::cover!(valid && width > 0, "contained fixed overwrite exists");
}

blubber::__root_schema!(__WalkFlags, List<bool>);

#[kani::proof]
#[kani::unwind(33)]
fn record_walk_split() {
    let mut bytes: [u8; 4] = kani::any();
    let width: u8 = kani::any();
    kani::assume(width <= 4);
    let mut expected = bytes;
    let (head, tail) = blubber_core::raw::split_fixed_slot(&mut bytes, width as usize);
    if width > 0 {
        head[0] = 7;
        expected[0] = 7;
    }
    if width < 4 {
        tail[0] = 9;
        expected[width as usize] = 9;
    }
    assert_eq!(bytes, expected);
    kani::cover!(width == 4);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_walk_admission() {
    let n: u8 = kani::any();
    kani::assume(n <= 4);
    let payload: [u8; 4] = kani::any();
    let mut bytes = [0xa5u8; 24];
    bytes[..8].copy_from_slice(b"walkkani");
    bytes[8..12].copy_from_slice(&(16 + n as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&(n as u32).to_le_bytes());
    bytes[16..20].copy_from_slice(&payload);
    let mut expected = bytes;
    let (slot, used, count) =
        blubber_core::raw::fixed_list_payload(&mut bytes, *b"walkkani", 4, 0, 1).unwrap();
    assert_eq!(
        (slot.len(), used, count),
        (n as usize, 16 + n as u32, n as u32)
    );
    if n > 0 {
        slot[0] = 7;
        expected[16] = 7;
    }
    assert_eq!(bytes, expected);
    kani::cover!(n == 4);
}

fn walk_cursor<const N: usize>() {
    let n = N as u8;
    let flags: [bool; 4] = kani::any();
    let mut bytes = [0u8; 24];
    bytes[..8].copy_from_slice(b"walkkani");
    bytes[8..12].copy_from_slice(&(16 + n as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&(n as u32).to_le_bytes());
    for i in 0..n as usize {
        bytes[16 + i] = u8::from(flags[i]);
    }
    let mut expected = bytes;
    {
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::load_trusted::<__WalkFlags>(&mut store, *b"walkkani").unwrap();
        let mut cursor = blubber::raw::project::<List<bool>>(edit)
            .cursor_mut()
            .unwrap();
        for i in 0..n as usize {
            let mut value = cursor.next().unwrap().unwrap();
            assert_eq!(value.get(), flags[i]);
            value.set(!flags[i]).unwrap();
            expected[16 + i] = u8::from(!flags[i]);
        }
        assert!(cursor.next().unwrap().is_none());
        assert!(cursor.next().unwrap().is_none());
        assert!(!store.is_poisoned());
        kani::cover!(!store.is_poisoned());
    }
    assert_same_bytes(&bytes, &expected);
}

#[kani::proof]
#[kani::unwind(9)]
fn record_walk_cursor_n0() {
    walk_cursor::<0>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_walk_cursor_n1() {
    walk_cursor::<1>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_walk_cursor_n2() {
    walk_cursor::<2>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_walk_cursor_n3() {
    walk_cursor::<3>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_walk_cursor_n4() {
    walk_cursor::<4>();
}

#[kani::proof]
#[kani::unwind(33)]
fn record_walk_state() {
    let close: bool = kani::any();
    let mut bytes = [0u8; 24];
    bytes[..8].copy_from_slice(b"walkkani");
    bytes[8..12].copy_from_slice(&16u32.to_le_bytes());
    let expected = bytes;
    {
        let mut store = SliceStore::new(&mut bytes);
        store.set_poisoned(true);
        if close {
            assert_eq!(store.commit(16), Err(ErrorKind::Poisoned.into()));
        }
        let error = Edit::load_trusted::<__WalkFlags>(&mut store, *b"walkkani").err();
        assert_eq!(
            error,
            Some(if close {
                ErrorKind::Closed.into()
            } else {
                ErrorKind::Poisoned.into()
            })
        );
    }
    assert_eq!(bytes, expected);
    kani::cover!(close);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_slot_region() {
    let mut bytes: [u8; 8] = kani::any();
    let offset: u32 = kani::any();
    let width: u32 = kani::any();
    let mut expected = bytes;
    let valid = offset as u64 + width as u64 <= 8;
    let result = blubber_core::raw::fixed_slot_region(&mut bytes, offset, width);
    assert_eq!(result.is_ok(), valid);
    if let Ok(slot) = result {
        assert_eq!(slot.len(), width as usize);
        if width > 0 {
            slot[0] = 7;
            expected[offset as usize] = 7;
        }
    }
    assert_eq!(bytes, expected);
    kani::cover!(valid && width > 0);
}

fn slot_progress<const N: usize>() {
    let flags: [bool; 4] = kani::any();
    let stop: u8 = kani::any();
    kani::assume(stop <= N as u8);
    let mut bytes = [0u8; 24];
    bytes[..8].copy_from_slice(b"walkkani");
    bytes[8..12].copy_from_slice(&(16 + N as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&(N as u32).to_le_bytes());
    for i in 0..N {
        bytes[16 + i] = u8::from(flags[i]);
    }
    let mut expected = bytes;
    let visits;
    {
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::load_trusted::<__WalkFlags>(&mut store, *b"walkkani").unwrap();
        let mut list = blubber::raw::project::<List<bool>>(edit);
        let mut seen = 0;
        let result = list.try_for_each_mut(|value| {
            value.set(!value.get())?;
            let fail = seen == stop as usize;
            seen += 1;
            if fail {
                Err(ErrorKind::InvalidBoundary.into())
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result,
            if (stop as usize) < N {
                Err(ErrorKind::InvalidBoundary.into())
            } else {
                Ok(())
            }
        );
        visits = if (stop as usize) < N {
            stop as usize + 1
        } else {
            N
        };
        assert_eq!(seen, visits);
        assert!(!store.is_poisoned());
        kani::cover!(stop as usize == N);
    }
    for i in 0..visits {
        expected[16 + i] = u8::from(!flags[i]);
    }
    assert_same_bytes(&bytes, &expected);
}
#[kani::proof]
#[kani::unwind(9)]
fn record_slot_progress_n0() {
    slot_progress::<0>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_slot_progress_n1() {
    slot_progress::<1>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_slot_progress_n2() {
    slot_progress::<2>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_slot_progress_n3() {
    slot_progress::<3>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_slot_progress_n4() {
    slot_progress::<4>();
}

#[blubber::schema]
struct SlotPair {
    flag: bool,
    byte: u8,
}
blubber::__root_schema!(__SlotPairs, List<SlotPair>);
#[blubber::schema]
struct SlotWord {
    byte: u8,
}
blubber::__root_schema!(__SlotWords, List<SlotWord>);

#[kani::proof]
#[kani::unwind(9)]
fn record_slot_projection() {
    let value: u8 = kani::any();
    let mut bytes = [0xa5u8; 24];
    bytes[..8].copy_from_slice(b"walkkani");
    bytes[8..12].copy_from_slice(&17u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
    bytes[16] = 7;
    let mut expected = bytes;
    expected[16] = value;
    {
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::load_trusted::<__SlotWords>(&mut store, *b"walkkani").unwrap();
        let mut cursor = blubber::raw::project::<List<SlotWord>>(edit)
            .cursor_mut()
            .unwrap();
        let mut pair = cursor.next().unwrap().unwrap();
        pair.byte_mut().set(value).unwrap();
        assert!(cursor.next().unwrap().is_none());
        assert!(!store.is_poisoned());
        kani::cover!(value == 255);
    }
    assert_same_bytes(&bytes, &expected);
}

#[kani::proof]
#[kani::unwind(9)]
fn record_slot_projection_domain() {
    let flag: u8 = kani::any();
    let mut bytes = [0xa5u8; 24];
    bytes[..8].copy_from_slice(b"walkkani");
    bytes[8..12].copy_from_slice(&18u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
    bytes[16] = flag;
    bytes[17] = 7;
    let expected = bytes;
    {
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::load_trusted::<__SlotPairs>(&mut store, *b"walkkani").unwrap();
        let mut cursor = blubber::raw::project::<List<SlotPair>>(edit)
            .cursor_mut()
            .unwrap();
        let mut pair = cursor.next().unwrap().unwrap();
        assert_eq!(
            pair.flag_mut().try_get(),
            if flag <= 1 {
                Ok(flag != 0)
            } else {
                Err(blubber::Error::new(ErrorKind::InvalidBool, 16))
            }
        );
        assert!(cursor.next().unwrap().is_none());
        assert!(!store.is_poisoned());
        kani::cover!(flag > 1);
    }
    assert_same_bytes(&bytes, &expected);
}

#[kani::proof]
#[kani::unwind(9)]
fn record_slot_projection_pair() {
    let flags: [bool; 2] = kani::any();
    let values: [u8; 2] = kani::any();
    let mut bytes = [0xa5u8; 24];
    bytes[..8].copy_from_slice(b"walkkani");
    bytes[8..12].copy_from_slice(&20u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
    for i in 0..2 {
        bytes[16 + i * 2] = u8::from(flags[i]);
        bytes[17 + i * 2] = values[i];
    }
    let mut expected = bytes;
    {
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::load_trusted::<__SlotPairs>(&mut store, *b"walkkani").unwrap();
        let mut cursor = blubber::raw::project::<List<SlotPair>>(edit)
            .cursor_mut()
            .unwrap();
        for i in 0..2 {
            let mut pair = cursor.next().unwrap().unwrap();
            assert_eq!(pair.flag_mut().try_get(), Ok(flags[i]));
            pair.byte_mut().set(values[i].wrapping_add(1)).unwrap();
            expected[17 + i * 2] = values[i].wrapping_add(1);
        }
        assert!(cursor.next().unwrap().is_none());
        assert!(!store.is_poisoned());
        kani::cover!(!store.is_poisoned());
    }
    assert_same_bytes(&bytes, &expected);
}

blubber::__root_schema!(__MapWalkFlags, blubber::Map<u8, bool>);
#[kani::proof]
#[kani::unwind(17)]
fn record_map_slot_split() {
    let mut bytes: [u8; 4] = kani::any();
    let key: u8 = kani::any();
    let width: u8 = kani::any();
    kani::assume(key <= 4 && width <= 4 - key);
    let before = bytes;
    let (copied, value, tail) =
        blubber_core::raw::split_fixed_map_entry(&mut bytes, key as usize, width as usize);
    assert_eq!(copied, &before[..key as usize]);
    value.fill(7);
    tail.fill(9);
    for i in 0..4 {
        assert_eq!(
            bytes[i],
            if i < key as usize {
                before[i]
            } else if i < (key + width) as usize {
                7
            } else {
                9
            }
        );
    }
    kani::cover!(key == 1 && width == 0);
}
fn map_walk<const N: usize>() {
    let flags: [bool; 4] = kani::any();
    let stop: u8 = kani::any();
    kani::assume(stop <= N as u8);
    let mut bytes = [0xa5u8; 24];
    bytes[..8].copy_from_slice(b"mapkani6");
    bytes[8..12].copy_from_slice(&(16 + 2 * N as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&(N as u32).to_le_bytes());
    for i in 0..N {
        bytes[16 + 2 * i] = (2 * i) as u8;
        bytes[17 + 2 * i] = u8::from(flags[i]);
    }
    let mut expected = bytes;
    {
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::load_trusted::<__MapWalkFlags>(&mut store, *b"mapkani6").unwrap();
        let mut cursor = blubber::raw::project::<blubber::Map<u8, bool>>(edit)
            .cursor_mut()
            .unwrap();
        for i in 0..N {
            let (mut key, mut value) = cursor.next().unwrap().unwrap();
            assert_eq!(key, (2 * i) as u8);
            key = 255;
            assert_eq!(key, 255);
            assert_eq!(value.get(), flags[i]);
            if i < stop as usize {
                value.set(!flags[i]).unwrap();
                expected[17 + 2 * i] = u8::from(!flags[i]);
            }
        }
        assert!(cursor.next().unwrap().is_none());
        assert!(cursor.next().unwrap().is_none());
        assert!(!store.is_poisoned());
        kani::cover!(stop as usize == N);
    }
    assert_same_bytes(&bytes, &expected);
}
#[kani::proof]
#[kani::unwind(9)]
fn record_map_walk_n0() {
    map_walk::<0>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_map_walk_n1() {
    map_walk::<1>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_map_walk_n2() {
    map_walk::<2>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_map_walk_n3() {
    map_walk::<3>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_map_walk_n4() {
    map_walk::<4>();
}

#[kani::proof]
#[kani::unwind(33)]
fn record_map_order_reject() {
    let keys: [u8; 2] = kani::any();
    let mut bytes = [0xa5u8; 24];
    bytes[..8].copy_from_slice(b"mapkani6");
    bytes[8..12].copy_from_slice(&20u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&[keys[0], 7, keys[1], 9]);
    let expected = bytes;
    {
        let layout = blubber_core::Layout::Flat {
            checked: <__MapWalkFlags as blubber::RootSchema>::__layout(),
        };
        let result = blubber_core::validate_map_keys(&bytes[12..20], &layout)
            .map_err(|error| error.rebase(12));
        assert_eq!(result.is_ok(), keys[0] < keys[1]);
        if let Err(error) = result {
            assert_eq!(
                error,
                blubber::Error::new(
                    if keys[0] == keys[1] {
                        ErrorKind::DuplicateKey
                    } else {
                        ErrorKind::UnsortedKeys
                    },
                    18
                )
            );
        }
        kani::cover!(keys[0] >= keys[1]);
    }
    assert_eq!(bytes, expected);
}

blubber::__root_schema!(__MapWalkBoolKeys, blubber::Map<bool, bool>);
#[kani::proof]
#[kani::unwind(17)]
fn record_map_key_domain() {
    let keys: [u8; 2] = kani::any();
    let bytes = [2, 0, 0, 0, keys[0], 7, keys[1], 9];
    let layout = blubber_core::Layout::Flat {
        checked: <__MapWalkBoolKeys as blubber::RootSchema>::__layout(),
    };
    let result = blubber_core::validate_map_keys(&bytes, &layout);
    let expected = if keys[0] > 1 {
        Err(blubber::Error::new(ErrorKind::InvalidBool, 4))
    } else if keys[1] > 1 {
        Err(blubber::Error::new(ErrorKind::InvalidBool, 6))
    } else if keys[0] == keys[1] {
        Err(blubber::Error::new(ErrorKind::DuplicateKey, 6))
    } else if keys[0] > keys[1] {
        Err(blubber::Error::new(ErrorKind::UnsortedKeys, 6))
    } else {
        Ok(())
    };
    assert_eq!(result, expected);
    kani::cover!(keys[0] > 1);
}
fn map_iter<const N: usize>() {
    let values: [u8; 4] = kani::any();
    let mut bytes = [0u8; 12];
    bytes[..4].copy_from_slice(&(N as u32).to_le_bytes());
    for i in 0..N {
        bytes[4 + 2 * i] = (2 * i) as u8;
        bytes[5 + 2 * i] = values[i];
    }
    let view = blubber::View::new(
        &bytes[..4 + 2 * N],
        blubber_core::Layout::Flat {
            checked: <__MapWalkFlags as blubber::RootSchema>::__layout(),
        },
        91,
    )
    .unwrap();
    let map = view.read::<blubber::Map<u8, bool>>().unwrap();
    let mut iter = map.try_iter();
    let mut clone = iter.clone();
    for i in 0..N {
        let expected = if values[i] <= 1 {
            Ok(((2 * i) as u8, values[i] != 0))
        } else {
            Err(blubber::Error::new(
                ErrorKind::InvalidBool,
                96 + 2 * i as u32,
            ))
        };
        assert_eq!(iter.next(), Some(expected));
        assert_eq!(clone.next(), Some(expected));
        if expected.is_err() {
            break;
        }
    }
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert_eq!(iter.size_hint(), (0, Some(0)));
    kani::cover!(N == 0 || values[N - 1] > 1);
}
#[kani::proof]
#[kani::unwind(17)]
fn record_map_iter_n0() {
    map_iter::<0>();
}
#[kani::proof]
#[kani::unwind(17)]
fn record_map_iter_n1() {
    map_iter::<1>();
}
#[kani::proof]
#[kani::unwind(17)]
fn record_map_iter_n2() {
    map_iter::<2>();
}
#[kani::proof]
#[kani::unwind(17)]
fn record_map_iter_n3() {
    map_iter::<3>();
}
#[kani::proof]
#[kani::unwind(17)]
fn record_map_iter_n4() {
    map_iter::<4>();
}

blubber::__root_schema!(__MapWalkBytes, blubber::Map<u8,u8>);
fn map_ingress(high: bool) {
    let value: u8 = kani::any();
    if (value >= 128) != high {
        return;
    }
    let mut bytes = [0xa5u8; 24];
    bytes[..8].copy_from_slice(b"mapkani6");
    bytes[8..12].copy_from_slice(&18u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
    bytes[16] = 3;
    bytes[17] = value;
    let mut expected = bytes;
    expected[17] = value.wrapping_add(1);
    {
        let mut store = SliceStore::new(&mut bytes);
        let mut root = Edit::load::<__MapWalkBytes>(&mut store, *b"mapkani6").unwrap();
        {
            let mut cursor = blubber::raw::project::<blubber::Map<u8, u8>>(root.reborrow())
                .cursor_mut()
                .unwrap();
            let (key, mut child) = cursor.next().unwrap().unwrap();
            assert_eq!(key, 3);
            child.set(value.wrapping_add(1)).unwrap();
            assert!(cursor.next().unwrap().is_none());
        }
        let mut cursor = blubber::raw::project::<blubber::Map<u8, u8>>(root.reborrow())
            .cursor_mut()
            .unwrap();
        let (key, child) = cursor.next().unwrap().unwrap();
        assert_eq!((key, child.get()), (3, value.wrapping_add(1)));
        assert!(cursor.next().unwrap().is_none());
        kani::cover!(value == if high { 255 } else { 127 });
    }
    assert_same_bytes(&bytes, &expected);
}

#[kani::proof]
#[kani::unwind(9)]
fn record_map_ingress_low() {
    map_ingress(false);
}
#[kani::proof]
#[kani::unwind(9)]
fn record_map_ingress_high() {
    map_ingress(true);
}
#[kani::proof]
#[kani::unwind(9)]
fn record_indexed_end() {
    let previous: u8 = kani::any();
    let current: u8 = kani::any();
    let extent: u8 = kani::any();
    let terminal: bool = kani::any();
    let result = blubber_core::raw::indexed_next_end(
        previous as usize,
        current as usize,
        extent as usize,
        terminal,
    );
    let valid = previous <= current && current <= extent && (!terminal || current == extent);
    assert_eq!(result.is_ok(), valid);
    if let Ok(end) = result {
        assert_eq!(end, current as usize);
    }
    kani::cover!(valid && terminal && previous == current);
}

#[kani::proof]
#[kani::unwind(9)]
fn record_indexed_slot() {
    let mut bytes: [u8; 4] = kani::any();
    let before = bytes;
    let start: u8 = kani::any();
    let end: u8 = kani::any();
    kani::assume(start <= end && end <= 4);
    let value: u8 = kani::any();
    let slot = blubber_core::raw::indexed_slot_region(&mut bytes, start as usize, end as usize);
    slot.fill(value);
    for i in 0..4 {
        assert_eq!(
            bytes[i],
            if (start as usize..end as usize).contains(&i) {
                value
            } else {
                before[i]
            }
        );
    }
    let base: u8 = kani::any();
    let index: u8 = kani::any();
    let stride: u8 = kani::any();
    assert_eq!(
        blubber_core::raw::indexed_coordinate(base as usize, index as usize, stride as usize),
        base as usize + index as usize * stride as usize
    );
    kani::cover!(start == end && end == 4);
}

#[kani::proof]
#[kani::unwind(9)]
fn record_indexed_window() {
    let bytes: [u8; 24] = kani::any();
    let start: u8 = kani::any();
    let end: u8 = kani::any();
    kani::assume(start <= end && end <= 24);
    let start = start as usize;
    let end = end as usize;
    let result = blubber_core::raw::indexed_string_window(&bytes, start, end);
    let valid = end - start <= 8 && (start & !7usize) + 16 <= bytes.len();
    assert_eq!(result.is_some(), valid);
    if let Some((window, low, high)) = result {
        assert_eq!(window.len(), 16);
        assert_eq!((low, high), (start & 7, (start & 7) + end - start));
        for i in 0..end - start {
            assert_eq!(window[low + i], bytes[start + i]);
        }
    }
    kani::cover!(result.is_some() && start == 7 && end == 15);
    kani::cover!(result.is_none() && start == 24 && end == 24);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_stream() {
    let source = [2u8, 0, 0, 0, 0, 0, 0, 0];
    let layout = blubber_core::Layout::List {
        element: &blubber_core::Layout::Str,
    };
    let plan = blubber_core::raw::RewritePlan::check(&source, &layout, 2, 4).unwrap();
    let first: u8 = kani::any();
    kani::assume(first <= 4);
    let value: u8 = kani::any();
    let mut bytes = [2u8, 0, 0, 0, 0, 0, 0, 0, 0xa5, 0xa5, 0xa5, 0xa5];
    let mut stream = plan.begin(&mut bytes).unwrap();
    {
        stream.prepare_slot(first as u32).unwrap();
        stream.destination()
    }
    .fill(value);
    stream.advance(first as usize).unwrap();
    {
        stream.prepare_slot(4 - first as u32).unwrap();
        stream.destination()
    }
    .fill(value);
    stream.advance(4 - first as usize).unwrap();
    stream.finish().unwrap();
    assert_eq!(&bytes[..4], &source[..4]);
    assert_eq!(bytes[4], first);
    assert_eq!(&bytes[5..8], &[0, 0, 0]);
    assert_eq!(&bytes[8..], &[value, value, value, value]);
    kani::cover!(first == 0);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_reject() {
    let mut bytes = [1u8, 0, 0, 0, 7];
    let layout = blubber_core::Layout::List {
        element: &blubber_core::Layout::Leaf(blubber_core::Leaf::Unsigned(1)),
    };
    let plan = blubber_core::raw::RewritePlan::check(&bytes, &layout, 1, 1).unwrap();
    let requested: u8 = kani::any();
    kani::assume(requested <= 2);
    let written: u8 = kani::any();
    kani::assume(written <= 2);
    let mut stream = plan.begin(&mut bytes).unwrap();
    if stream.prepare_slot(requested as u32).is_ok() {
        stream.destination();
        assert_eq!(requested, 1);
        assert_eq!(stream.advance(written as usize).is_ok(), written == 1);
        assert_eq!(stream.finish().is_ok(), written == 1);
    } else {
        assert_ne!(requested, 1);
        assert!(stream.finish().is_err());
    }
    assert_eq!(bytes, [1, 0, 0, 0, 7]);
    kani::cover!(requested == 1 && written == 2);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_count_overflow() {
    let bytes = [0u8; 4];
    let count: u32 = kani::any();
    let total: u32 = kani::any();
    let layout = blubber_core::Layout::List {
        element: &blubber_core::Layout::Str,
    };
    assert_eq!(
        blubber_core::raw::RewritePlan::check(&bytes, &layout, count, total).is_ok(),
        count == 0 && total == 0
    );
    let one = [1u8, 0, 0, 0];
    let got = blubber_core::raw::RewritePlan::check(&one, &layout, 1, total);
    assert_eq!(got.is_ok(), total <= u32::MAX - 4);
    kani::cover!(count == 0 && total == u32::MAX);
}

blubber::__root_schema!(__RewriteMap, blubber::Map<u8, blubber::Str>);
#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_map_keys() {
    let source = [2u8, 0, 0, 0, 1, 0, 0, 0, 0, 3, 0, 0, 0, 0];
    let layout = blubber_core::Layout::Flat {
        checked: <__RewriteMap as blubber::RootSchema>::__layout(),
    };
    let plan = blubber_core::raw::RewritePlan::check(&source, &layout, 2, 2).unwrap();
    let mut bytes = [2u8, 0, 0, 0, 1, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0xa5, 0xa5];
    let value: u8 = kani::any();
    let mut stream = plan.begin(&mut bytes).unwrap();
    {
        stream.prepare_slot(0).unwrap();
        stream.destination()
    };
    stream.advance(0).unwrap();
    {
        stream.prepare_slot(2).unwrap();
        stream.destination()
    }
    .fill(value);
    stream.advance(2).unwrap();
    stream.finish().unwrap();
    assert_eq!(bytes[4], 1);
    assert_eq!(bytes[9], 3);
    assert_eq!(&bytes[5..9], &[0, 0, 0, 0]);
    assert_eq!(&bytes[10..14], &[2, 0, 0, 0]);
    assert_eq!(&bytes[14..], &[value, value]);
    kani::cover!(value == 255);
}

#[blubber::schema]
struct RewriteTailRoot {
    values: blubber::List<blubber::Str>,
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_tail() {
    let mut bytes = [0xa5u8; 26];
    bytes[..8].copy_from_slice(b"rewrite8");
    bytes[8..12].copy_from_slice(&22u32.to_le_bytes());
    bytes[12..22].copy_from_slice(&[2, 0, 0, 0, 1, 0, 0, 0, b'a', b'b']);
    let layout = blubber_core::Layout::Flat {
        checked: <RewriteTailRoot as blubber::RootSchema>::__layout(),
    };
    let path = blubber_core::raw::InlinePath::root_field(0);
    let target = blubber_core::raw::resolve_rewrite_tail(&bytes, *b"rewrite8", 4, &layout, &path)
        .unwrap()
        .unwrap();
    let (span, selected) = target.selection();
    assert_eq!((span.start, span.end), (12, 22));
    let plan =
        blubber_core::raw::RewritePlan::check_admitted(&bytes[12..22], &selected, 2, 6).unwrap();
    let mut write = target
        .prepare(&mut bytes, plan.encoded_len())
        .unwrap()
        .unwrap();
    let value: u8 = kani::any();
    let mut stream = plan.begin(write.destination()).unwrap();
    for _ in 0..2 {
        stream.prepare_slot(3).unwrap();
        stream.destination().fill(value);
        stream.advance(3).unwrap();
    }
    stream.finish().unwrap();
    write.finish();
    assert_eq!(&bytes[..8], b"rewrite8");
    assert_eq!(&bytes[8..12], &26u32.to_le_bytes());
    assert_eq!(&bytes[12..16], &2u32.to_le_bytes());
    assert_eq!(&bytes[16..20], &3u32.to_le_bytes());
    assert_eq!(&bytes[20..], &[value; 6]);
    kani::cover!(value == 255);
}

fn typed_rewrite<const N: usize>() {
    let values: [u8; 4] = kani::any();
    let requested: u8 = kani::any();
    kani::assume(requested <= 4);
    let mut bytes = [0xa5u8; 8];
    word(&mut bytes, 0, N);
    let layout = blubber_core::Layout::List {
        element: <u8 as Schema>::LAYOUT,
    };
    let view = blubber::View::new(&bytes[..4 + N], layout, 0).unwrap();
    let before = bytes;
    let result = blubber::raw::prepare_list::<u8>(view, true, &values[..requested as usize]);
    assert_eq!(result.is_ok(), requested as usize == N);
    if let Ok((plan, ())) = result {
        let mut stream = plan.begin(&mut bytes[..4 + N]).unwrap();
        for value in &values[..N] {
            blubber::raw::encode_rewrite_slot::<u8>(value, 1, &mut stream).unwrap();
        }
        stream.finish().unwrap();
        assert_eq!(&bytes[4..4 + N], &values[..N]);
        assert_eq!(&bytes[..4], &before[..4]);
    } else {
        assert_eq!(bytes, before);
    }
    kani::cover!(requested as usize == N);
}

#[kani::proof]
#[kani::unwind(17)]
fn record_rewrite_typed_n0() {
    typed_rewrite::<0>();
}
#[kani::proof]
#[kani::unwind(17)]
fn record_rewrite_typed_n1() {
    typed_rewrite::<1>();
}
#[kani::proof]
#[kani::unwind(17)]
fn record_rewrite_typed_n2() {
    typed_rewrite::<2>();
}
#[kani::proof]
#[kani::unwind(17)]
fn record_rewrite_typed_n3() {
    typed_rewrite::<3>();
}
#[kani::proof]
#[kani::unwind(17)]
fn record_rewrite_typed_n4() {
    typed_rewrite::<4>();
}

blubber::__root_schema!(__RewriteTypedMap, blubber::Map<u8,u8>);
#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_typed_keys() {
    let a: u8 = kani::any();
    let b: u8 = kani::any();
    let value: u8 = kani::any();
    let mut bytes = [0u8; 20];
    bytes[..8].copy_from_slice(b"rwkeys00");
    word(&mut bytes, 8, 20);
    bytes[12..].copy_from_slice(&[2, 0, 0, 0, 1, 9, 3, 10]);
    let before = bytes;
    let view = blubber::View::load_trusted::<__RewriteTypedMap>(&bytes, *b"rwkeys00").unwrap();
    let result = blubber::raw::prepare_map::<u8, u8>(view, true, &[(a, value), (b, value)]);
    assert_eq!(result.is_ok(), a == 1 && b == 3);
    if let Ok((plan, ())) = result {
        let mut stream = plan.begin(&mut bytes[12..]).unwrap();
        for _ in 0..2 {
            blubber::raw::encode_rewrite_slot::<u8>(&value, 1, &mut stream).unwrap();
        }
        stream.finish().unwrap();
        let mut expected = before;
        expected[17] = value;
        expected[19] = value;
        assert_eq!(bytes, expected);
    } else {
        assert_eq!(bytes, before);
    }
    kani::cover!(a == 1 && b == 3);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_typed_fill() {
    let value: u8 = kani::any();
    let mut bytes = [2u8, 0, 0, 0, 9, 10];
    let layout = blubber_core::Layout::List {
        element: <u8 as Schema>::LAYOUT,
    };
    let view = blubber::View::new(&bytes, layout, 0).unwrap();
    let (plan, (count, len)) =
        blubber::raw::prepare_fill::<u8>(view, true, false, 0, &value).unwrap();
    assert_eq!((count, len), (2, 1));
    let mut stream = plan.begin(&mut bytes).unwrap();
    for _ in 0..count {
        blubber::raw::encode_rewrite_slot::<u8>(&value, len, &mut stream).unwrap();
    }
    stream.finish().unwrap();
    assert_eq!(bytes, [2, 0, 0, 0, value, value]);
    kani::cover!(value == 255);
}

struct SlotCodec;
impl Schema for SlotCodec {
    const KIND: blubber::Kind = <u8 as Schema>::KIND;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <u8 as Schema>::LAYOUT;
    const __LEN: usize = <u8 as Schema>::__LEN;
    const __HEADER: &'static [u32] = <u8 as Schema>::__HEADER;
    type __Children = <u8 as Schema>::__Children;
    type Read<'a> = u8;
    type Mut<'a> = blubber::ScalarMut<'a, u8>;
    type Init<'a> = (u8, u8);
    type Removed = u8;
    fn read<'a>(view: blubber::View<'a>) -> blubber::Result<u8> {
        <u8 as Schema>::read(view)
    }
    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::project::<u8>(blubber::raw::into_edit(edit))
    }
    fn read_removed(view: blubber::View<'_>) -> blubber::Result<u8> {
        <u8 as Schema>::read_removed(view)
    }
    fn measure(_: &Self::Init<'_>) -> blubber::Result<u32> {
        Ok(1)
    }
    fn encode(i: &Self::Init<'_>, bytes: &mut [u8]) {
        bytes[0] = i.0;
    }
    fn encode_slot(i: &Self::Init<'_>, bytes: &mut [u8]) -> blubber::Result<usize> {
        Self::encode(i, bytes);
        match i.1 {
            1 => Err(ErrorKind::InvalidBoundary.into()),
            2 => Ok(0),
            3 => Ok(2),
            _ => Ok(1),
        }
    }
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_typed_codec() {
    let value: u8 = kani::any();
    let fault: u8 = kani::any();
    kani::assume(fault <= 3);
    let mut bytes = [1u8, 0, 0, 0, 9];
    let layout = blubber_core::Layout::List {
        element: <u8 as Schema>::LAYOUT,
    };
    let view = blubber::View::new(&bytes, layout, 0).unwrap();
    let (plan, (_, len)) =
        blubber::raw::prepare_fill::<SlotCodec>(view, true, false, 0, &(value, fault)).unwrap();
    let mut stream = plan.begin(&mut bytes).unwrap();
    let result = blubber::raw::encode_rewrite_slot::<SlotCodec>(&(value, fault), len, &mut stream);
    assert_eq!(result.is_ok(), fault == 0);
    assert_eq!(stream.finish().is_ok(), fault == 0);
    assert_eq!(bytes, [1, 0, 0, 0, value]);
    kani::cover!(fault == 3);
}

// Keyless count rejection runs the shared production preflight; geometry and
// symbolic values cover fixed map strides without a variable controller graph.
fn keyless_rewrite<const N: usize>() {
    let values: [u8; 4] = kani::any();
    let requested: u8 = kani::any();
    kani::assume(requested <= 4);
    let mut bytes = [0xa5u8; 24];
    bytes[..8].copy_from_slice(b"rwkeys00");
    word(&mut bytes, 8, 16 + 2 * N);
    word(&mut bytes, 12, N);
    for i in 0..N {
        bytes[16 + 2 * i] = i as u8;
    }
    let before = bytes;
    let view = blubber::View::load_trusted::<__RewriteTypedMap>(&bytes, *b"rwkeys00").unwrap();
    let result =
        blubber::raw::prepare_map_values::<u8, u8>(view, true, &values[..requested as usize]);
    assert_eq!(result.is_ok(), requested as usize == N);
    if let Ok((plan, ())) = result {
        let mut stream = plan.begin(&mut bytes[12..16 + 2 * N]).unwrap();
        for value in &values[..N] {
            blubber::raw::encode_rewrite_slot::<u8>(value, 1, &mut stream).unwrap();
        }
        stream.finish().unwrap();
        let mut expected = before;
        for i in 0..N {
            expected[17 + 2 * i] = values[i];
        }
        assert_eq!(bytes, expected);
    } else {
        assert_eq!(bytes, before);
    }
    kani::cover!(requested as usize == N);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_keyless_n0() {
    keyless_rewrite::<0>();
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_keyless_n1() {
    keyless_rewrite::<1>();
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_keyless_n2() {
    keyless_rewrite::<2>();
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_keyless_n3() {
    keyless_rewrite::<3>();
}

#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_keyless_n4() {
    keyless_rewrite::<4>();
}

blubber::__root_schema!(__RewriteKeylessCodec, blubber::Map<u8,SlotCodec>);
#[kani::proof]
#[kani::unwind(33)]
fn record_rewrite_keyless_codec() {
    let value: u8 = kani::any();
    let fault: u8 = kani::any();
    kani::assume(fault <= 3);
    let mut bytes = [0u8; 18];
    bytes[..8].copy_from_slice(b"rwcodec0");
    word(&mut bytes, 8, 18);
    bytes[12..].copy_from_slice(&[1, 0, 0, 0, 3, 9]);
    let mut store = blubber::SliceStore::new(&mut bytes);
    let edit = Edit::load_trusted::<__RewriteKeylessCodec>(&mut store, *b"rwcodec0").unwrap();
    let mut map = blubber::raw::project::<blubber::Map<u8, SlotCodec>>(edit);
    let result = map.rewrite_values(&[(value, fault)]);
    assert_eq!(result.is_ok(), fault == 0);
    assert_eq!(store.is_poisoned(), fault != 0);
    assert_eq!(store.commit(18).is_ok(), fault == 0);
    kani::cover!(fault == 3);
}

// Fixed counts keep the controller tractable under D14. Arbitrary stored keys
// remain byte-exact, including duplicate/descending keys outside valid history.
fn lazy_map_values<const N: usize, const PART: u8>() {
    let keys: [u8; 4] = kani::any();
    let values: [u8; 4] = kani::any();
    let operation: u8 = if PART < 4 { PART } else { kani::any() };
    kani::assume(operation < 4);
    if PART < 4 && operation != PART {
        return;
    }
    let mut bytes = [0xa5; 24];
    bytes[..8].copy_from_slice(b"walkmap0");
    word(&mut bytes, 8, 16 + 2 * N);
    word(&mut bytes, 12, N);
    for i in 0..N {
        bytes[16 + 2 * i] = keys[i];
    }
    let mut expected = bytes;
    {
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::load_trusted::<__RewriteTypedMap>(&mut store, *b"walkmap0").unwrap();
        let mut map = blubber::raw::project::<blubber::Map<u8, u8>>(edit);
        match operation {
            0 => {
                map.fill_values(values[0]).unwrap();
            }
            1 => {
                map.rewrite_values(&values[..N]).unwrap();
            }
            2 => {
                let mut cursor = map.cursor_mut().unwrap();
                for i in 0..N {
                    let (key, mut slot) = cursor.next().unwrap().unwrap();
                    assert_eq!(key, keys[i]);
                    slot.set(values[i]).unwrap();
                }
                assert!(cursor.next().unwrap().is_none());
            }
            _ => {
                let mut i = 0;
                map.try_for_each_mut(|key, slot| {
                    assert_eq!(key, keys[i]);
                    slot.set(values[i])?;
                    i += 1;
                    Ok(())
                })
                .unwrap();
                assert_eq!(i, N);
            }
        }
        assert!(!store.is_poisoned());
    }
    for i in 0..N {
        expected[17 + 2 * i] = values[if operation == 0 { 0 } else { i }];
    }
    assert_same_bytes(&bytes, &expected);
    kani::cover!(operation == if PART == 4 { 3 } else { PART });
}

#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n0() {
    lazy_map_values::<0, 4>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n1_fill() {
    lazy_map_values::<1, 0>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n1_rewrite() {
    lazy_map_values::<1, 1>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n1_cursor() {
    lazy_map_values::<1, 2>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n1_callbacks() {
    lazy_map_values::<1, 3>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n2_fill() {
    lazy_map_values::<2, 0>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n2_rewrite() {
    lazy_map_values::<2, 1>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n2_cursor() {
    lazy_map_values::<2, 2>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n2_callbacks() {
    lazy_map_values::<2, 3>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n3_fill() {
    lazy_map_values::<3, 0>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n3_rewrite() {
    lazy_map_values::<3, 1>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n3_cursor() {
    lazy_map_values::<3, 2>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n3_callbacks() {
    lazy_map_values::<3, 3>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n4_fill() {
    lazy_map_values::<4, 0>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n4_rewrite() {
    lazy_map_values::<4, 1>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n4_cursor() {
    lazy_map_values::<4, 2>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_map_n4_callbacks() {
    lazy_map_values::<4, 3>();
}

struct FixedFillEncoder {
    value: [u8; 3],
    written: usize,
    fail: bool,
}
impl blubber_core::fill::ValueEncoder for FixedFillEncoder {
    fn geometry(&self) -> (u32, u32) {
        (3, 1)
    }
    fn encode(&self, slot: &mut [u8]) -> blubber::Result<usize> {
        let _ = <[u8; 3] as Schema>::encode_slot(&self.value, slot)?;
        if self.fail {
            Err(ErrorKind::InvalidBoundary.into())
        } else {
            Ok(self.written)
        }
    }
}
#[kani::proof]
#[kani::unwind(33)]
fn record_fixed_fill_fixed_records() {
    let value: [u8; 3] = kani::any();
    let keys: [u8; 2] = kani::any();
    let written: usize = kani::any();
    let fail: bool = kani::any();
    let mut bytes = [0xa5; 16];
    bytes[..12].copy_from_slice(&[2, 0, 0, 0, keys[0], 1, 2, 3, keys[1], 4, 5, 6]);
    let before = bytes;
    let result = blubber_core::fill::values(
        &mut bytes[4..12],
        &FixedFillEncoder {
            value,
            written,
            fail,
        },
    );
    assert_eq!(result.is_ok(), !fail && written == 3);
    assert_eq!(&bytes[..4], &before[..4]);
    assert_eq!(bytes[4], keys[0]);
    assert_eq!(bytes[8], keys[1]);
    assert_eq!(&bytes[12..], &before[12..]);
    assert_eq!(&bytes[5..8], &value);
    if result.is_ok() {
        assert_eq!(&bytes[9..12], &value);
    } else {
        assert_eq!(&bytes[9..12], &before[9..12]);
    }
    kani::cover!(written == 3 && !fail && keys[0] > keys[1]);
}
#[kani::proof]
#[kani::unwind(33)]
fn record_fixed_fill_poison() {
    let value: u8 = kani::any();
    let fault: u8 = kani::any();
    let mut bytes = [0u8; 18];
    bytes[..8].copy_from_slice(b"fillcode");
    word(&mut bytes, 8, 18);
    bytes[12..].copy_from_slice(&[1, 0, 0, 0, 3, 9]);
    let mut store = blubber::SliceStore::new(&mut bytes);
    let edit = Edit::load_trusted::<__RewriteKeylessCodec>(&mut store, *b"fillcode").unwrap();
    let mut map = blubber::raw::project::<blubber::Map<u8, SlotCodec>>(edit);
    let result = map.fill_values((value, fault));
    assert_eq!(result.is_ok(), !(1..=3).contains(&fault));
    assert_eq!(store.is_poisoned(), (1..=3).contains(&fault));
    assert_eq!(store.commit(18).is_ok(), result.is_ok());
    kani::cover!(fault == 3);
}

blubber::__root_schema!(__FixedFillForeignMap,blubber::Map<[u8;2],u8>);
#[kani::proof]
#[kani::unwind(33)]
fn record_fixed_fill_foreign_metadata() {
    let raw: [u8; 3] = kani::any();
    let value: u8 = kani::any();
    let mut bytes = [0u8; 19];
    bytes[..8].copy_from_slice(b"fillmeta");
    word(&mut bytes, 8, 19);
    bytes[12..].copy_from_slice(&[1, 0, 0, 0, raw[0], raw[1], raw[2]]);
    let view = blubber::View::load_trusted::<__FixedFillForeignMap>(&bytes, *b"fillmeta").unwrap();
    assert!(blubber::raw::prepare_fill::<u8>(view, true, true, 1, &value).is_err());
    assert!(blubber::raw::prepare_fill::<u16>(view, true, true, 2, &u16::from(value)).is_err());
    kani::cover!(raw == [255, 255, 255]);
}

blubber::__root_schema!(__FixedFillFixedList, blubber::List<[u8; 3]>);
#[kani::proof]
#[kani::unwind(33)]
fn record_fixed_fill_fixed_list() {
    let value: [u8; 3] = kani::any();
    let mut bytes = [0xa5; 24];
    bytes[..8].copy_from_slice(b"filllist");
    word(&mut bytes, 8, 22);
    word(&mut bytes, 12, 2);
    let before = bytes;
    {
        let mut store = SliceStore::new(&mut bytes);
        let edit = Edit::load_trusted::<__FixedFillFixedList>(&mut store, *b"filllist").unwrap();
        blubber::raw::project::<blubber::List<[u8; 3]>>(edit)
            .fill(value)
            .unwrap();
        assert!(!store.is_poisoned());
    }
    let mut expected = before;
    expected[16..19].copy_from_slice(&value);
    expected[19..22].copy_from_slice(&value);
    assert_eq!(bytes, expected);
    kani::cover!(value == [255, 0, 255]);
}
blubber::__root_schema!(__FixedFillCodecList, blubber::List<SlotCodec>);
#[kani::proof]
#[kani::unwind(33)]
fn record_fixed_fill_list_poison() {
    let value: u8 = kani::any();
    let fault: u8 = kani::any();
    let mut bytes = [0u8; 18];
    bytes[..8].copy_from_slice(b"filllist");
    word(&mut bytes, 8, 17);
    word(&mut bytes, 12, 1);
    let mut store = SliceStore::new(&mut bytes);
    let edit = Edit::load_trusted::<__FixedFillCodecList>(&mut store, *b"filllist").unwrap();
    let result = blubber::raw::project::<blubber::List<SlotCodec>>(edit).fill((value, fault));
    assert_eq!(result.is_ok(), !(1..=3).contains(&fault));
    assert_eq!(store.is_poisoned(), (1..=3).contains(&fault));
    assert_eq!(store.commit(17).is_ok(), result.is_ok());
    kani::cover!(fault == 3);
}

// Byte-view state keeps domain checking separate from the UTF-8 bridge below.
struct ListStreamRegion;
impl Schema for ListStreamRegion {
    const KIND: blubber::Kind = blubber::Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static blubber_core::Layout = <blubber::Str as Schema>::LAYOUT;
    const __LEN: usize = <blubber::Str as Schema>::__LEN;
    const __HEADER: &'static [u32] = <blubber::Str as Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = blubber::View<'a>;
    type Mut<'a> = blubber::raw::Edit<'a>;
    type Init<'a> = &'a str;
    type Removed = ();
    fn project(edit: blubber::__private::Projection<'_, Self>) -> Self::Mut<'_> {
        blubber::raw::into_edit(edit)
    }
    fn read(view: blubber::View<'_>) -> blubber::Result<Self::Read<'_>> {
        if view.bytes().first() == Some(&255) {
            Err(blubber::Error::new(ErrorKind::InvalidTag, view.base()))
        } else {
            Ok(view)
        }
    }
    fn read_removed(view: blubber::View<'_>) -> blubber::Result<()> {
        view.validate()
    }
    fn measure(value: &&str) -> blubber::Result<u32> {
        <blubber::Str as Schema>::measure(value)
    }
    fn encode(value: &&str, bytes: &mut [u8]) {
        <blubber::Str as Schema>::encode(value, bytes);
    }
}
fn list_stream_view_equal(
    observed: blubber::Result<blubber::View<'_>>,
    expected: blubber::Result<blubber::View<'_>>,
) {
    match (observed, expected) {
        (Ok(left), Ok(right)) => {
            assert_eq!(left.base(), right.base());
            assert_eq!(left.bytes().len(), right.bytes().len());
            assert!(core::ptr::eq(left.bytes().as_ptr(), right.bytes().as_ptr()));
            match (left.layout().head(), right.layout().head()) {
                (blubber_core::LayoutHead::Str, blubber_core::LayoutHead::Str) => {}
                (
                    blubber_core::LayoutHead::Leaf(blubber_core::Leaf::Unsigned(a)),
                    blubber_core::LayoutHead::Leaf(blubber_core::Leaf::Unsigned(b)),
                ) => assert_eq!(a, b),
                _ => panic!("selected descriptor differs"),
            }
        }
        (Err(left), Err(right)) => assert_eq!(left, right),
        _ => panic!("iterator and point result differ"),
    }
}

fn list_stream_text_equal(observed: blubber::Result<&str>, expected: blubber::Result<&str>) {
    match (observed, expected) {
        (Ok(left), Ok(right)) => {
            assert_eq!(left.len(), right.len());
            assert!(core::ptr::eq(left.as_ptr(), right.as_ptr()));
        }
        (Err(left), Err(right)) => assert_eq!(left, right),
        _ => panic!("iterator and point result differ"),
    }
}

fn list_stream_stream_input<const END: u32>() -> ([u8; 10], u32) {
    let payload: [u8; 2] = kani::any();
    let end = END.to_le_bytes();
    let bytes = [
        2, 0, 0, 0, end[0], end[1], end[2], end[3], payload[0], payload[1],
    ];
    let base: u32 = kani::any();
    kani::assume(base <= u32::MAX - bytes.len() as u32);
    (bytes, base)
}

fn list_stream_first_case<const END: u32>() {
    let (bytes, base) = list_stream_stream_input::<END>();
    let list = blubber::View::new(&bytes, <List<blubber::Str>>::LAYOUT, base)
        .unwrap()
        .read::<List<ListStreamRegion>>()
        .unwrap();
    let mut iter = list.try_iter();
    let first = iter.next().expect("first slot");
    list_stream_view_equal(first, list.try_get(0).map(Option::unwrap));
    if first.is_err() {
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }
    kani::cover!(first.is_ok() && bytes[8] == 195 && bytes[9] == 169);
}

fn list_stream_stream_case<const END: u32>() {
    let (bytes, base) = list_stream_stream_input::<END>();
    let list = blubber::View::new(&bytes, <List<blubber::Str>>::LAYOUT, base)
        .unwrap()
        .read::<List<ListStreamRegion>>()
        .unwrap();
    let mut iter = list.try_iter();
    let first = iter.next().expect("first slot");
    let mut complete = false;
    if first.is_ok() {
        let second = iter.next().expect("second slot");
        list_stream_view_equal(second, list.try_get(1).map(Option::unwrap));
        complete = second.is_ok();
    }
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert_eq!(iter.size_hint(), (0, Some(0)));
    kani::cover!(complete && bytes[8] == 195 && bytes[9] == 169);
}

fn list_stream_clone_before_case<const END: u32>() {
    let (bytes, base) = list_stream_stream_input::<END>();
    let list = blubber::View::new(&bytes, <List<blubber::Str>>::LAYOUT, base)
        .unwrap()
        .read::<List<ListStreamRegion>>()
        .unwrap();
    let mut iter = list.try_iter();
    let mut clone = iter.clone();
    let first = iter.next().expect("first slot");
    list_stream_view_equal(first, clone.next().expect("cloned first slot"));
    assert_eq!(iter.size_hint(), clone.size_hint());
    if first.is_err() {
        assert!(iter.next().is_none());
        assert!(clone.next().is_none());
        assert!(clone.next().is_none());
    }
    kani::cover!(first.is_ok() && bytes[8] == 195 && bytes[9] == 169);
}

fn list_stream_clone_after_case<const END: u32>() {
    let (bytes, base) = list_stream_stream_input::<END>();
    let list = blubber::View::new(&bytes, <List<blubber::Str>>::LAYOUT, base)
        .unwrap()
        .read::<List<ListStreamRegion>>()
        .unwrap();
    let mut iter = list.try_iter();
    let first = iter.next().expect("first slot");
    let mut clone = iter.clone();
    let mut complete = false;
    if first.is_ok() {
        let second = iter.next().expect("second slot");
        list_stream_view_equal(second, clone.next().expect("cloned second slot"));
        complete = second.is_ok();
    }
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(clone.next().is_none());
    assert!(clone.next().is_none());
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(clone.size_hint(), (0, Some(0)));
    let mut finished = iter.clone();
    assert!(finished.next().is_none());
    assert!(finished.next().is_none());
    assert_eq!(finished.size_hint(), (0, Some(0)));
    kani::cover!(complete && bytes[8] == 195 && bytes[9] == 169);
}

#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_stream_end0() {
    list_stream_stream_case::<0>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_stream_end1() {
    list_stream_stream_case::<1>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_stream_end2() {
    list_stream_stream_case::<2>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_first_end0() {
    list_stream_first_case::<0>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_first_end1() {
    list_stream_first_case::<1>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_first_end2() {
    list_stream_first_case::<2>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_clone_before_end0() {
    list_stream_clone_before_case::<0>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_clone_before_end1() {
    list_stream_clone_before_case::<1>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_clone_before_end2() {
    list_stream_clone_before_case::<2>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_clone_after_end0() {
    list_stream_clone_after_case::<0>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_clone_after_end1() {
    list_stream_clone_after_case::<1>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_clone_after_end2() {
    list_stream_clone_after_case::<2>();
}

#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_end_full_width() {
    let payload: [u8; 2] = kani::any();
    let end: u32 = kani::any();
    let word = end.to_le_bytes();
    let bytes = [
        2, 0, 0, 0, word[0], word[1], word[2], word[3], payload[0], payload[1],
    ];
    let result = blubber_core::list_read::region(&bytes, 8, 0, 2, 0);
    assert_eq!(result.is_ok(), end <= 2);
    match result {
        Ok((chunk, start)) => {
            assert_eq!(start, 8);
            assert_eq!(chunk.len(), end as usize);
            assert!(core::ptr::eq(chunk.as_ptr(), bytes[8..].as_ptr()));
            for i in 0..2 {
                if i < chunk.len() {
                    assert_eq!(chunk[i], payload[i]);
                }
            }
        }
        Err(error) => assert_eq!(error, blubber::Error::new(ErrorKind::InvalidBoundary, 4)),
    }
    kani::cover!(end == u32::MAX);
}

fn list_stream_invalid_case<const END: u32>() {
    let payload: [u8; 2] = kani::any();
    let word = END.to_le_bytes();
    let bytes = [
        2, 0, 0, 0, word[0], word[1], word[2], word[3], payload[0], payload[1],
    ];
    let list = blubber::View::new(&bytes, <List<blubber::Str>>::LAYOUT, 19)
        .unwrap()
        .read::<List<blubber::Str>>()
        .unwrap();
    let first = list.try_get(0).map(Option::unwrap);
    assert_eq!(
        first,
        Err(blubber::Error::new(ErrorKind::InvalidBoundary, 23))
    );
    let mut iter = list.try_iter();
    let mut clone = iter.clone();
    assert_eq!(iter.next(), Some(first));
    assert_eq!(clone.next(), Some(first));
    assert!(iter.next().is_none());
    assert!(clone.next().is_none());
    kani::cover!(payload[0] == 255 && payload[1] == 0);
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_invalid_end3() {
    list_stream_invalid_case::<3>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_invalid_end_max() {
    list_stream_invalid_case::<{ u32::MAX }>();
}

#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_fixed_state() {
    let payload: [u8; 2] = kani::any();
    let bytes = [2, 0, 0, 0, payload[0], payload[1]];
    let base: u32 = kani::any();
    kani::assume(base <= u32::MAX - bytes.len() as u32);
    let list = blubber::View::new(&bytes, <List<u8>>::LAYOUT, base)
        .unwrap()
        .read::<List<bool>>()
        .unwrap();
    let mut iter = list.try_iter();
    for i in 0..2 {
        assert_eq!(iter.size_hint(), (1, Some(2 - i)));
        let expected = list.try_get(i).map(Option::unwrap);
        let mut clone = iter.clone();
        assert_eq!(clone.next(), Some(expected));
        assert_eq!(iter.next(), Some(expected));
        if expected.is_err() {
            break;
        }
    }
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert_eq!(iter.size_hint(), (0, Some(0)));
    kani::cover!(bytes[4] <= 1 && bytes[5] <= 1);
}

#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_foreign_fixed() {
    let payload: [u8; 2] = kani::any();
    let bytes = [2, 0, 0, 0, payload[0], payload[1]];
    let list = blubber::View::new(&bytes, <List<u8>>::LAYOUT, 19)
        .unwrap()
        .read::<List<ListStreamRegion>>()
        .unwrap();
    let mut iter = list.try_iter();
    let first = list.try_get(0).map(Option::unwrap);
    list_stream_view_equal(iter.next().expect("first fixed slot"), first);
    if first.is_ok() {
        list_stream_view_equal(
            iter.next().expect("second fixed slot"),
            list.try_get(1).map(Option::unwrap),
        );
    }
    assert!(iter.next().is_none());
    kani::cover!(bytes[4] == b'a' && bytes[5] == b'b');
}

#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_foreign_width() {
    let payload: [u8; 4] = kani::any();
    let bytes = [2, 0, 0, 0, payload[0], payload[1], payload[2], payload[3]];
    let list = blubber::View::new(&bytes, <List<u16>>::LAYOUT, 19)
        .unwrap()
        .read::<List<u8>>()
        .unwrap();
    let first = list.try_get(0).map(Option::unwrap);
    assert!(first.is_err());
    let mut iter = list.try_iter();
    assert_eq!(iter.next(), Some(first));
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    kani::cover!(bytes[4] == 255 && bytes[5] == 0);
}

#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_foreign_variable() {
    let values: [u8; 2] = kani::any();
    let bytes = [2, 0, 0, 0, 1, 0, 0, 0, values[0], values[1]];
    let list = blubber::View::new(&bytes, <List<blubber::Str>>::LAYOUT, 19)
        .unwrap()
        .read::<List<u8>>()
        .unwrap();
    let mut iter = list.try_iter();
    assert_eq!(iter.next(), Some(list.try_get(0).map(Option::unwrap)));
    assert_eq!(iter.next(), Some(list.try_get(1).map(Option::unwrap)));
    assert!(iter.next().is_none());
    kani::cover!(values[0] == 255 && values[1] == 0);
}

fn list_stream_infallible_case<const N: usize>() {
    let payload: [u8; 2] = kani::any();
    let bytes = [N as u8, 0, 0, 0, payload[0], payload[1]];
    let list = blubber::View::new(&bytes[..4 + N], <List<u8>>::LAYOUT, 19)
        .unwrap()
        .read::<List<u8>>()
        .unwrap();
    let expected =
        u16::from(if N > 0 { bytes[4] } else { 0 }) + u16::from(if N > 1 { bytes[5] } else { 0 });
    assert_eq!(list.iter().map(u16::from).sum::<u16>(), expected);
    let mut iter = list.iter();
    let mut clone = iter.clone();
    let first = if N > 0 { Some(bytes[4]) } else { None };
    assert_eq!(iter.next(), first);
    assert_eq!(clone.next(), first);
    let rest = if N > 1 { u16::from(bytes[5]) } else { 0 };
    assert_eq!(iter.map(u16::from).sum::<u16>(), rest);
    for _ in 0..N {
        let _ = clone.next();
    }
    assert!(clone.next().is_none());
    assert_eq!(clone.size_hint(), (0, Some(0)));
    kani::cover!(N == 0 || bytes[4] == 7);
}

#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_infallible_n0() {
    list_stream_infallible_case::<0>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_infallible_n1() {
    list_stream_infallible_case::<1>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_infallible_n2() {
    list_stream_infallible_case::<2>();
}

fn list_stream_str_region_case<const N: usize>() {
    let bytes: [u8; 2] = kani::any();
    let data = &bytes[..N];
    let view = blubber::View::new(data, <blubber::Str as Schema>::LAYOUT, 19).unwrap();
    let point = <blubber::Str as Schema>::read(view);
    let region = <blubber::Str as Schema>::read_region(data, *<blubber::Str as Schema>::LAYOUT, 19);
    list_stream_text_equal(region, point);
    kani::cover!(N == 0 || (bytes[0] == 195 && bytes[1] == 169));
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_str_region_n0() {
    list_stream_str_region_case::<0>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_str_region_n1() {
    list_stream_str_region_case::<1>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_str_region_n2() {
    list_stream_str_region_case::<2>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_list_stream_codec_fusion() {
    let tail: u8 = kani::any();
    let bytes = [2, 0, 0, 0, 1, 0, 0, 0, 255, tail];
    let list = blubber::View::new(&bytes, <List<blubber::Str>>::LAYOUT, 19)
        .unwrap()
        .read::<List<blubber::Str>>()
        .unwrap();
    let mut iter = list.try_iter();
    let mut clone = iter.clone();
    let expected = blubber::Error::new(ErrorKind::InvalidUtf8, 27);
    assert_eq!(iter.next().expect("first slot").unwrap_err(), expected);
    assert_eq!(
        clone.next().expect("cloned first slot").unwrap_err(),
        expected
    );
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(clone.next().is_none());
    kani::cover!(tail == 0);
}

// Certified tables keep recursive metadata admission outside this state cohort.
// Miri checks both descriptor forms; Verus checks directory admission.
macro_rules! map_stream_layout {
    ($ty:ty) => {
        blubber_core::Layout::Flat {
            checked: blubber::__layout_of!($ty),
        }
    };
}
fn map_stream_input<const END: u32, const TERMINAL: u32>() -> ([u8; 16], u32) {
    let raw: [u8; 4] = kani::any();
    let end = END.to_le_bytes();
    let terminal = TERMINAL.to_le_bytes();
    let bytes = [
        2,
        0,
        0,
        0,
        raw[0],
        end[0],
        end[1],
        end[2],
        end[3],
        raw[1],
        terminal[0],
        terminal[1],
        terminal[2],
        terminal[3],
        raw[2],
        raw[3],
    ];
    let base = if kani::any::<bool>() {
        0
    } else {
        u32::MAX - bytes.len() as u32
    };
    (bytes, base)
}

fn map_stream_expected(
    bytes: &[u8; 16],
    base: u32,
    index: usize,
) -> blubber::Result<(bool, blubber::View<'_>)> {
    let at = 4 + index * 5;
    let key = <bool as blubber::MapKey>::decode_key(&bytes[at..at + 1])
        .map_err(|e| e.rebase(base + at as u32))?;
    let low = if index == 0 {
        0
    } else {
        u32::from_le_bytes(bytes[5..9].try_into().unwrap())
    };
    let high = u32::from_le_bytes(bytes[at + 1..at + 5].try_into().unwrap());
    if low > 2 {
        return Err(Error::new(ErrorKind::InvalidBoundary, base + 5));
    }
    if high < low || high > 2 || (index == 1 && high != 2) {
        return Err(Error::new(ErrorKind::InvalidBoundary, base + at as u32 + 1));
    }
    let view = blubber::View::new(
        &bytes[14 + low as usize..14 + high as usize],
        map_stream_layout!(blubber::Str),
        base + 14 + low,
    )
    .unwrap();
    view.read::<ListStreamRegion>().map(|value| (key, value))
}

fn map_stream_equal(
    observed: blubber::Result<(bool, blubber::View<'_>)>,
    expected: blubber::Result<(bool, blubber::View<'_>)>,
) {
    match (observed, expected) {
        (Ok((key, left)), Ok((other, right))) => {
            assert_eq!(key, other);
            list_stream_view_equal(Ok(left), Ok(right));
        }
        (Err(left), Err(right)) => assert_eq!(left, right),
        _ => panic!("map iterator and point selection differ"),
    }
}

fn map_stream_case<const END: u32, const TERMINAL: u32, const SECOND: bool>() {
    let (bytes, base) = map_stream_input::<END, TERMINAL>();
    let map = blubber::View::new(&bytes, map_stream_layout!(Map<u8,blubber::Str>), base)
        .unwrap()
        .read::<Map<bool, ListStreamRegion>>()
        .unwrap();
    let mut iter = map.try_iter();
    let mut clone = iter.clone();
    let first = iter.next().expect("first entry");
    if !SECOND {
        map_stream_equal(first, map_stream_expected(&bytes, base, 0));
        map_stream_equal(clone.next().expect("clone entry"), first);
    }
    let mut complete = first.is_ok();
    if SECOND && first.is_ok() {
        assert_eq!(iter.size_hint(), (1, Some(1)));
        let mut clone = iter.clone();
        let second = iter.next().expect("second entry");
        map_stream_equal(second, map_stream_expected(&bytes, base, 1));
        map_stream_equal(clone.next().expect("clone second"), second);
        complete = second.is_ok();
    }
    if SECOND || first.is_err() {
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert_eq!(iter.size_hint(), (0, Some(0)));
    }
    kani::cover!(complete && bytes[4] == 0 && bytes[9] == 1 && bytes[14] == 195);
}

#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_first_end0() {
    map_stream_case::<0, 2, false>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_first_end1() {
    map_stream_case::<1, 2, false>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_first_end2() {
    map_stream_case::<2, 2, false>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_second_end0() {
    map_stream_case::<0, 2, true>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_second_end1() {
    map_stream_case::<1, 2, true>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_second_end2() {
    map_stream_case::<2, 2, true>();
}

#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_terminal_fault() {
    let (bytes, base) = map_stream_input::<0, 0>();
    let map = blubber::View::new(&bytes, map_stream_layout!(Map<u8,blubber::Str>), base)
        .unwrap()
        .read::<Map<u8, ListStreamRegion>>()
        .unwrap();
    let mut iter = map.try_iter();
    assert!(iter.next().unwrap().is_ok());
    let result = iter.next().unwrap();
    assert!(
        matches!(result,Err(Error::Fault {kind:ErrorKind::InvalidBoundary,offset}) if offset==base+10)
    );
    assert!(iter.next().is_none());
    kani::cover!(bytes[4] == 255 && bytes[9] == 255);
}

#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_key_fault_precedence() {
    let (bytes, base) = map_stream_input::<{ u32::MAX }, 2>();
    let map = blubber::View::new(&bytes, map_stream_layout!(Map<u8,blubber::Str>), base)
        .unwrap()
        .read::<Map<bool, ListStreamRegion>>()
        .unwrap();
    let mut iter = map.try_iter();
    let result = iter.next().unwrap();
    if bytes[4] > 1 {
        assert!(
            matches!(result,Err(Error::Fault {kind:ErrorKind::InvalidBool,offset}) if offset==base+4)
        );
    } else {
        assert!(
            matches!(result,Err(Error::Fault {kind:ErrorKind::InvalidBoundary,offset}) if offset==base+5)
        );
    }
    assert!(iter.next().is_none());
    assert!(iter.clone().next().is_none());
    kani::cover!(bytes[4] == 2);
}

#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_end_full_width() {
    let end: u32 = kani::any();
    let raw: [u8; 2] = kani::any();
    let e = end.to_le_bytes();
    let bytes = [
        2, 0, 0, 0, 0, e[0], e[1], e[2], e[3], 1, 2, 0, 0, 0, raw[0], raw[1],
    ];
    let last: bool = kani::any();
    let result = blubber_core::map_read::region(&bytes, 14, 5, last, 0);
    assert_eq!(result.is_ok(), end <= 2 && (!last || end == 2));
    if let Ok((chunk, start)) = result {
        assert_eq!(start, 14);
        assert_eq!(chunk.len(), end as usize);
        assert!(core::ptr::eq(chunk.as_ptr(), bytes[14..].as_ptr()));
    } else {
        assert_eq!(
            result.unwrap_err(),
            Error::new(ErrorKind::InvalidBoundary, 5)
        );
    }
    kani::cover!(end == 2 && last);
}

#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_text_codec_fusion() {
    let raw: [u8; 3] = kani::any();
    let bytes = [
        2, 0, 0, 0, raw[0], 1, 0, 0, 0, raw[1], 2, 0, 0, 0, 255, raw[2],
    ];
    let map = blubber::View::new(&bytes, map_stream_layout!(Map<u8,blubber::Str>), 31)
        .unwrap()
        .read::<Map<u8, blubber::Str>>()
        .unwrap();
    let mut iter = map.try_iter();
    let mut clone = iter.clone();
    let expected = Err(Error::new(ErrorKind::InvalidUtf8, 45));
    assert_eq!(iter.next(), Some(expected));
    assert_eq!(clone.next(), Some(expected));
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(clone.next().is_none());
    kani::cover!(raw == [255, 255, 255]);
}

fn map_stream_infallible_case<const N: u8>() {
    let raw: [u8; 4] = kani::any();
    let bytes = [N, 0, 0, 0, raw[0], raw[1], raw[2], raw[3]];
    let map = blubber::View::new(
        &bytes[..4 + 2 * N as usize],
        map_stream_layout!(Map<u8,u8>),
        31,
    )
    .unwrap()
    .read::<Map<u8, u8>>()
    .unwrap();
    let mut iter = map.iter();
    let mut checked = map.try_iter();
    for index in 0..N as usize {
        assert_eq!(iter.size_hint(), (1, Some(N as usize - index)));
        let mut clone = iter.clone();
        let expected = Some((raw[index * 2], raw[index * 2 + 1]));
        assert_eq!(iter.next(), expected);
        assert_eq!(clone.next(), expected);
        assert_eq!(checked.size_hint(), (1, Some(N as usize - index)));
        let mut checked_clone = checked.clone();
        assert_eq!(checked.next(), expected.map(Ok::<_, Error>));
        assert_eq!(checked_clone.next(), expected.map(Ok::<_, Error>));
    }
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(checked.next(), None);
    assert_eq!(checked.next(), None);
    assert_eq!(checked.size_hint(), (0, Some(0)));
    kani::cover!(raw == [255, 0, 255, 255]);
}
#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_infallible_n0() {
    map_stream_infallible_case::<0>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_infallible_n1() {
    map_stream_infallible_case::<1>();
}
#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_infallible_n2() {
    map_stream_infallible_case::<2>();
}

#[kani::proof]
#[kani::unwind(12)]
fn record_map_stream_base_full_width() {
    let base: u32 = kani::any();
    let offset: u8 = kani::any();
    kani::assume(base <= u32::MAX - 16);
    kani::assume(offset <= 16);
    let absolute = blubber_core::raw::fixed_chunk_end(base, offset as usize);
    assert_eq!(absolute, base + u32::from(offset));
    let fault = Error::new(ErrorKind::InvalidBoundary, u32::from(offset)).rebase(base);
    if absolute == Error::NO_OFFSET {
        assert_eq!(
            fault,
            Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)
        );
    } else {
        assert_eq!(fault, Error::new(ErrorKind::InvalidBoundary, absolute));
    }
    kani::cover!(base == 31 && offset == 9);
    kani::cover!(base == u32::MAX - 16 && offset == 16);
}

fn batched_text_list_input<const END: u32>() -> ([u8; 10], u32) {
    let end = END.to_le_bytes();
    let (a, b) = if END == 1 { (b'a', b'b') } else { (195, 169) };
    let bytes = [2, 0, 0, 0, end[0], end[1], end[2], end[3], a, b];
    let base = if kani::any::<bool>() {
        0
    } else {
        u32::MAX - 10
    };
    (bytes, base)
}
fn batched_text_map_input<const END: u32, const TERMINAL: u32>() -> ([u8; 16], u32) {
    let keys = [u8::from(kani::any::<bool>()), u8::from(kani::any::<bool>())];
    let end = END.to_le_bytes();
    let terminal = TERMINAL.to_le_bytes();
    let (a, b) = if END == 1 { (b'a', b'b') } else { (195, 169) };
    let bytes = [
        2,
        0,
        0,
        0,
        keys[0],
        end[0],
        end[1],
        end[2],
        end[3],
        keys[1],
        terminal[0],
        terminal[1],
        terminal[2],
        terminal[3],
        a,
        b,
    ];
    let base = if kani::any::<bool>() {
        0
    } else {
        u32::MAX - 16
    };
    (bytes, base)
}
// BatchedText safe-string bridge and iterator states; all assumptions bound inputs.
fn batched_text_list_expected<const END: u32>(
    bytes: &[u8; 10],
    base: u32,
    second: bool,
) -> blubber::Result<&str> {
    if END > 2 {
        return Err(Error::new(ErrorKind::InvalidBoundary, base + 4));
    }
    let (low, high) = if second {
        (END as usize, 2)
    } else {
        (0, END as usize)
    };
    <blubber::Str as Schema>::read_region(
        &bytes[8 + low..8 + high],
        *blubber::Str::LAYOUT,
        base + 8 + low as u32,
    )
}

fn batched_text_list<const END: u32, const SECOND: bool>() {
    let (bytes, base) = batched_text_list_input::<END>();
    let mut batch = blubber::__private::kani_batched_list_state(&bytes, base);
    let first = batch.next().unwrap();
    if !SECOND {
        list_stream_text_equal(
            first,
            batched_text_list_expected::<END>(&bytes, base, false),
        );
    }
    let mut accepted = first.is_ok();
    if SECOND && first.is_ok() {
        let second = batch.next().unwrap();
        list_stream_text_equal(
            second,
            batched_text_list_expected::<END>(&bytes, base, true),
        );
        accepted = second.is_ok();
    }
    if SECOND || first.is_err() {
        assert!(batch.next().is_none());
        assert!(batch.clone().next().is_none());
        assert_eq!(batch.size_hint(), (0, Some(0)));
    }
    kani::cover!(accepted);
}

fn batched_text_map_equal(
    left: blubber::Result<(bool, &str)>,
    right: blubber::Result<(bool, &str)>,
) {
    match (left, right) {
        (Ok((key, left)), Ok((other, right))) => {
            assert_eq!(key, other);
            list_stream_text_equal(Ok(left), Ok(right));
        }
        (Err(left), Err(right)) => assert_eq!(left, right),
        _ => panic!("batch and ordinary entry differ"),
    }
}

fn batched_text_map_expected<const END: u32, const TERMINAL: u32>(
    bytes: &[u8; 16],
    base: u32,
    second: bool,
) -> blubber::Result<(bool, &str)> {
    let at = if second { 9 } else { 4 };
    let key = <bool as blubber::MapKey>::decode_key(&bytes[at..at + 1])
        .map_err(|e| e.rebase(base + at as u32))?;
    let (low, high) = if second { (END, TERMINAL) } else { (0, END) };
    if low > 2 || high < low || high > 2 || (second && high != 2) {
        return Err(Error::new(ErrorKind::InvalidBoundary, base + at as u32 + 1));
    }
    <blubber::Str as Schema>::read_region(
        &bytes[14 + low as usize..14 + high as usize],
        *blubber::Str::LAYOUT,
        base + 14 + low,
    )
    .map(|value| (key, value))
}

fn batched_text_map<const END: u32, const TERMINAL: u32, const SECOND: bool>() {
    let (bytes, base) = batched_text_map_input::<END, TERMINAL>();
    let mut batch = blubber::__private::kani_batched_map_state(&bytes, base);
    let first = batch.next().unwrap();
    if !SECOND {
        batched_text_map_equal(
            first,
            batched_text_map_expected::<END, TERMINAL>(&bytes, base, false),
        );
    }
    let mut accepted = first.is_ok();
    if SECOND && first.is_ok() {
        let second = batch.next().unwrap();
        batched_text_map_equal(
            second,
            batched_text_map_expected::<END, TERMINAL>(&bytes, base, true),
        );
        accepted = second.is_ok();
    }
    if SECOND || first.is_err() {
        assert!(batch.next().is_none());
        assert!(batch.clone().next().is_none());
        assert_eq!(batch.size_hint(), (0, Some(0)));
    }
    kani::cover!(accepted && bytes[4] == 0 && bytes[9] == 1);
}

#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_list_first_end0() {
    batched_text_list::<0, false>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_list_first_end1() {
    batched_text_list::<1, false>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_list_first_end2() {
    batched_text_list::<2, false>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_list_second_end0() {
    batched_text_list::<0, true>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_list_second_end1() {
    batched_text_list::<1, true>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_list_second_end2() {
    batched_text_list::<2, true>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_map_first_end0() {
    batched_text_map::<0, 2, false>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_map_first_end1() {
    batched_text_map::<1, 2, false>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_map_first_end2() {
    batched_text_map::<2, 2, false>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_map_second_end0() {
    batched_text_map::<0, 2, true>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_map_second_end1() {
    batched_text_map::<1, 2, true>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_map_second_end2() {
    batched_text_map::<2, 2, true>();
}

#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_invalid_list_end() {
    let (bytes, base) = batched_text_list_input::<{ u32::MAX }>();
    let mut batch = blubber::__private::kani_batched_list_state(&bytes, base);
    let result = batch.next().unwrap();
    list_stream_text_equal(
        result,
        batched_text_list_expected::<{ u32::MAX }>(&bytes, base, false),
    );
    assert!(result.is_err());
    assert!(batch.next().is_none());
    kani::cover!(result.is_err());
}

#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_terminal_fault() {
    let (bytes, base) = batched_text_map_input::<0, 1>();
    let mut batch = blubber::__private::kani_batched_map_state(&bytes, base);
    let first = batch.next().unwrap();
    batched_text_map_equal(
        first,
        batched_text_map_expected::<0, 1>(&bytes, base, false),
    );
    if first.is_ok() {
        batched_text_map_equal(
            batch.next().unwrap(),
            batched_text_map_expected::<0, 1>(&bytes, base, true),
        );
    }
    assert!(batch.next().is_none());
    kani::cover!(first.is_ok());
}

#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_list_clone() {
    let (bytes, base) = batched_text_list_input::<2>();
    let mut batch = blubber::__private::kani_batched_list_state(&bytes, base);
    let mut clone = batch.clone();
    let first = batch.next().unwrap();
    list_stream_text_equal(first, clone.next().unwrap());
    let mut clone = batch.clone();
    assert_eq!(batch.size_hint(), clone.size_hint());
    if first.is_ok() {
        list_stream_text_equal(batch.next().unwrap(), clone.next().unwrap());
    }
    assert!(batch.next().is_none());
    assert!(clone.next().is_none());
    kani::cover!(first.is_ok() && bytes[8] == 195 && bytes[9] == 169);
}

#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_map_clone() {
    let (bytes, base) = batched_text_map_input::<2, 2>();
    let mut batch = blubber::__private::kani_batched_map_state(&bytes, base);
    let mut clone = batch.clone();
    let first = batch.next().unwrap();
    batched_text_map_equal(first, clone.next().unwrap());
    let mut clone = batch.clone();
    assert_eq!(batch.size_hint(), clone.size_hint());
    if first.is_ok() {
        batched_text_map_equal(batch.next().unwrap(), clone.next().unwrap());
    }
    assert!(batch.next().is_none());
    assert!(clone.next().is_none());
    kani::cover!(
        first.is_ok() && bytes[4] == 0 && bytes[9] == 1 && bytes[14] == 195 && bytes[15] == 169
    );
}

#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_empty() {
    let bytes = [0u8; 4];
    let list = <List<blubber::Str> as Schema>::read_region(
        &bytes,
        map_stream_layout!(List<blubber::Str>),
        0,
    )
    .unwrap();
    let map = <Map<bool, blubber::Str> as Schema>::read_region(
        &bytes,
        map_stream_layout!(Map<u8,blubber::Str>),
        0,
    )
    .unwrap();
    assert!(list.try_iter_batched().next().is_none());
    assert!(map.try_iter_batched().next().is_none());
    kani::cover!(list.try_iter_batched().size_hint() == (0, Some(0)));
}

fn batched_text_bridge<const END: usize>() {
    let bytes: [u8; 2] = kani::any();
    let text = core::str::from_utf8(&bytes).ok();
    let left = blubber::__private::kani_batched_text(text, 0, &bytes[..END], 29);
    let right = <blubber::Str as Schema>::read_region(&bytes[..END], *blubber::Str::LAYOUT, 29);
    list_stream_text_equal(left, right);
    let left = blubber::__private::kani_batched_text(text, END, &bytes[END..], 29 + END as u32);
    let right = <blubber::Str as Schema>::read_region(
        &bytes[END..],
        *blubber::Str::LAYOUT,
        29 + END as u32,
    );
    list_stream_text_equal(left, right);
    kani::cover!(bytes == [195, 169]);
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_bridge_end0() {
    batched_text_bridge::<0>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_bridge_end1() {
    batched_text_bridge::<1>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_bridge_end2() {
    batched_text_bridge::<2>();
}

fn batched_text_list_fault<const SPLIT: bool>() {
    let bytes = if SPLIT {
        [2, 0, 0, 0, 1, 0, 0, 0, 195, 169]
    } else {
        [2, 0, 0, 0, 1, 0, 0, 0, b'a', 255]
    };
    let list = <List<blubber::Str> as Schema>::read_region(
        &bytes,
        map_stream_layout!(List<blubber::Str>),
        29,
    )
    .unwrap();
    let mut batch = list.try_iter_batched();
    let first = batch.next().unwrap();
    if SPLIT {
        assert_eq!(first, Err(Error::new(ErrorKind::InvalidUtf8, 37)));
    } else {
        assert_eq!(first, Ok("a"));
        assert_eq!(
            batch.next(),
            Some(Err(Error::new(ErrorKind::InvalidUtf8, 38)))
        );
    }
    assert!(batch.next().is_none());
    assert!(batch.clone().next().is_none());
    kani::cover!(first.is_err() == SPLIT);
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_list_split() {
    batched_text_list_fault::<true>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_list_prefix() {
    batched_text_list_fault::<false>();
}

fn batched_text_map_fault<const SPLIT: bool>() {
    let bytes = if SPLIT {
        [2, 0, 0, 0, 0, 1, 0, 0, 0, 1, 2, 0, 0, 0, 195, 169]
    } else {
        [2, 0, 0, 0, 0, 1, 0, 0, 0, 1, 2, 0, 0, 0, b'a', 255]
    };
    let map = <Map<bool, blubber::Str> as Schema>::read_region(
        &bytes,
        map_stream_layout!(Map<u8,blubber::Str>),
        29,
    )
    .unwrap();
    let mut batch = map.try_iter_batched();
    let first = batch.next().unwrap();
    if SPLIT {
        assert_eq!(first, Err(Error::new(ErrorKind::InvalidUtf8, 43)));
    } else {
        assert_eq!(first, Ok((false, "a")));
        assert_eq!(
            batch.next(),
            Some(Err(Error::new(ErrorKind::InvalidUtf8, 44)))
        );
    }
    assert!(batch.next().is_none());
    assert!(batch.clone().next().is_none());
    kani::cover!(first.is_err() == SPLIT);
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_map_split() {
    batched_text_map_fault::<true>();
}
#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_map_prefix() {
    batched_text_map_fault::<false>();
}

#[kani::proof]
#[kani::unwind(4)]
fn record_batched_text_key_fault() {
    let bytes = [2, 0, 0, 0, 2, 255, 255, 255, 255, 1, 2, 0, 0, 0, 255, 255];
    let map = <Map<bool, blubber::Str> as Schema>::read_region(
        &bytes,
        map_stream_layout!(Map<u8,blubber::Str>),
        29,
    )
    .unwrap();
    let mut batch = map.try_iter_batched();
    let result = batch.next().unwrap();
    assert_eq!(result, Err(Error::new(ErrorKind::InvalidBool, 33)));
    assert!(batch.next().is_none());
    assert!(batch.clone().next().is_none());
    kani::cover!(result.is_err());
}

#[kani::proof]
#[kani::unwind(6)]
fn record_text_aggregate_count() {
    let raw: [u8; 4] = kani::any();
    let count = u32::from_le_bytes(raw);
    let bytes = [raw[0], raw[1], raw[2], raw[3], b'a', b'a', b'a', b'a'];
    let expected = if count > u32::MAX / 4 {
        Err(Error::new(ErrorKind::ArithmeticOverflow, u32::MAX))
    } else if count > 2 {
        Err(Error::new(ErrorKind::Truncated, 8))
    } else if count == 0 {
        Err(Error::new(ErrorKind::InvalidBoundary, 4))
    } else {
        Ok((
            if count == 1 { &bytes[4..] } else { &bytes[8..] },
            if count == 1 { 4 } else { 8 },
        ))
    };
    let result = blubber_core::concat_read::payload(&bytes);
    assert_eq!(result, expected);
    kani::cover!(result.is_ok());
    kani::cover!(result.is_err());
}

fn text_aggregate_text<const N: usize>() {
    let mut bytes = [1u8, 0, 0, 0, 0, 0, 0, 0];
    let raw: [u8; 4] = kani::any();
    bytes[4..].copy_from_slice(&raw);
    let base: u32 = kani::any();
    kani::assume(base <= u32::MAX - (4 + N) as u32);
    let bytes = &bytes[..4 + N];
    let list = <List<blubber::Str> as Schema>::read_region(
        bytes,
        map_stream_layout!(List<blubber::Str>),
        base,
    )
    .unwrap();
    let result = list.try_concat_str();
    let expected = core::str::from_utf8(&raw[..N])
        .map_err(|e| Error::new(ErrorKind::InvalidUtf8, base + 4 + e.valid_up_to() as u32));
    assert_eq!(result, expected);
    if let Ok(text) = result {
        assert_eq!(text.as_bytes(), &raw[..N]);
    }
    kani::cover!(result.is_ok());
    kani::cover!(result.is_err());
}
#[kani::proof]
#[kani::unwind(6)]
fn record_text_aggregate_text2() {
    text_aggregate_text::<2>();
}
#[kani::proof]
#[kani::unwind(6)]
fn record_text_aggregate_text4() {
    text_aggregate_text::<4>();
}

#[kani::proof]
#[kani::unwind(6)]
fn record_text_aggregate_split() {
    let end: u32 = kani::any();
    let raw = end.to_le_bytes();
    let bytes = [2, 0, 0, 0, raw[0], raw[1], raw[2], raw[3], 195, 169];
    let list = <List<blubber::Str> as Schema>::read_region(
        &bytes,
        map_stream_layout!(List<blubber::Str>),
        29,
    )
    .unwrap();
    assert_eq!(list.try_concat_str(), Ok("é"));
    kani::cover!(end == 1);
    kani::cover!(end == u32::MAX);
}

#[kani::proof]
#[kani::unwind(6)]
fn record_text_aggregate_schema() {
    let bytes = [0u8; 4];
    let list =
        <List<blubber::Str> as Schema>::read_region(&bytes, map_stream_layout!(List<u8>), 29)
            .unwrap();
    let result = list.try_concat_str();
    assert_eq!(result, Err(ErrorKind::InvalidSchema.into()));
    kani::cover!(result.is_err());
}

#[kani::proof]
#[kani::unwind(6)]
fn record_text_aggregate_empty() {
    let singleton: bool = kani::any();
    let bytes = [u8::from(singleton), 0, 0, 0];
    let base: u32 = kani::any();
    kani::assume(base <= u32::MAX - 4);
    let list = <List<blubber::Str> as Schema>::read_region(
        &bytes,
        map_stream_layout!(List<blubber::Str>),
        base,
    )
    .unwrap();
    let text = list.try_concat_str().unwrap();
    assert_eq!(text, "");
    assert_eq!(text.as_ptr(), bytes[4..].as_ptr());
    kani::cover!(singleton);
    kani::cover!(!singleton);
}

fn directory_suffix_suffix<const KEY: usize, const COUNT: usize>() {
    let mut bytes: [u8; 74] = kani::any();
    let before = bytes;
    let delta: i8 = kani::any();
    let stride = KEY + 4;
    let length = COUNT * stride;
    for i in 0..COUNT {
        let at = 1 + i * stride + KEY;
        let value = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        // These bounds restrict the admitted end-word input domain.
        kani::assume((0..=u32::MAX as i64).contains(&(value as i64 + delta as i64)));
    }
    blubber_core::raw::kani_patch_suffix(
        &mut bytes[1..1 + length],
        KEY,
        stride,
        delta as i64,
        COUNT,
    );
    for i in 0..COUNT {
        let at = 1 + i * stride + KEY;
        let old = u32::from_le_bytes(before[at..at + 4].try_into().unwrap());
        let new = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(new as i64, old as i64 + delta as i64);
        assert_eq!(&bytes[1 + i * stride..at], &before[1 + i * stride..at]);
    }
    assert_eq!(bytes[0], before[0]);
    assert_eq!(&bytes[1 + length..], &before[1 + length..]);
    kani::cover!(delta == 0);
}

#[kani::proof]
#[kani::unwind(80)]
fn record_directory_suffix_empty() {
    directory_suffix_suffix::<32, 0>();
}
#[kani::proof]
#[kani::unwind(80)]
fn record_directory_suffix_word() {
    directory_suffix_suffix::<0, 2>();
}
#[kani::proof]
#[kani::unwind(80)]
fn record_directory_suffix_unaligned() {
    directory_suffix_suffix::<3, 2>();
}
#[kani::proof]
#[kani::unwind(80)]
fn record_directory_suffix_wide() {
    directory_suffix_suffix::<32, 2>();
}

fn fixed_fold_fold<const COUNT: usize>() {
    let payload: [u8; 4] = kani::any();
    let bytes = [
        COUNT as u8,
        0,
        0,
        0,
        payload[0],
        payload[1],
        payload[2],
        payload[3],
    ];
    let consumed: u8 = kani::any();
    kani::assume(consumed as usize <= COUNT);
    let map = blubber::View::new(&bytes[..4 + 2 * COUNT], map_stream_layout!(Map<u8,u8>), 31)
        .unwrap()
        .read::<Map<u8, u8>>()
        .unwrap();
    let mut iter = map.iter();
    for _ in 0..consumed {
        assert!(iter.next().is_some());
    }
    let mut expected = iter.clone();
    let mut sum = 17u32;
    while let Some((key, value)) = expected.next() {
        sum = sum * 3 + key as u32 + value as u32;
    }
    assert_eq!(expected.next(), None);
    assert_eq!(expected.next(), None);
    let mut calls = 0;
    let actual = iter.fold(17u32, |acc, (key, value)| {
        assert_eq!(key, payload[2 * (consumed as usize + calls)]);
        assert_eq!(value, payload[2 * (consumed as usize + calls) + 1]);
        calls += 1;
        acc * 3 + key as u32 + value as u32
    });
    assert_eq!(actual, sum);
    assert_eq!(calls, COUNT - consumed as usize);
    kani::cover!(consumed == 0);
    kani::cover!(consumed as usize == COUNT);
}

#[kani::proof]
#[kani::unwind(6)]
fn record_fixed_fold_n0() {
    fixed_fold_fold::<0>();
}

#[kani::proof]
#[kani::unwind(6)]
fn record_fixed_fold_n1() {
    fixed_fold_fold::<1>();
}

#[kani::proof]
#[kani::unwind(6)]
fn record_fixed_fold_n2() {
    fixed_fold_fold::<2>();
}

#[kani::proof]
#[kani::unwind(6)]
fn record_fixed_fold_domains() {
    let payload: [u8; 4] = kani::any();
    let bytes = [2, 0, 0, 0, payload[0], payload[1], payload[2], payload[3]];
    let base = 31;
    let map =
        <Map<bool, bool> as Schema>::read_region(&bytes, map_stream_layout!(Map<bool, bool>), base)
            .unwrap();
    let mut iter = map.try_iter();
    let mut sum = 0;
    let mut valid = true;
    for i in 0..2 {
        let key = payload[2 * i];
        let value = payload[2 * i + 1];
        let expected = if key > 1 {
            Err(Error::new(ErrorKind::InvalidBool, base + 4 + 2 * i as u32))
        } else if value > 1 {
            Err(Error::new(ErrorKind::InvalidBool, base + 5 + 2 * i as u32))
        } else {
            Ok((key != 0, value != 0))
        };
        assert_eq!(iter.next(), Some(expected));
        if expected.is_err() {
            valid = false;
            break;
        }
        sum = sum * 3 + key as u32 + value as u32;
    }
    assert_eq!(iter.next(), None);
    assert_eq!(iter.clone().next(), None);
    assert_eq!(iter.next(), None);
    if valid {
        assert_eq!(
            map.iter().fold(0u32, |acc, (key, value)| acc * 3
                + key as u32
                + value as u32),
            sum
        );
    }
    kani::cover!(valid);
    kani::cover!(payload[0] > 1 && payload[1] > 1);
    kani::cover!(payload[0] <= 1 && payload[1] <= 1 && payload[2] <= 1 && payload[3] > 1);
}

fn fixed_values_values<const COUNT: usize>() {
    let payload: [u8; 4] = kani::any();
    let bytes = [
        COUNT as u8,
        0,
        0,
        0,
        payload[0],
        payload[1],
        payload[2],
        payload[3],
    ];
    let base: u32 = kani::any();
    kani::assume(base <= u32::MAX - (4 + 2 * COUNT) as u32);
    let map = <Map<u8, u8> as Schema>::read_region(
        &bytes[..4 + 2 * COUNT],
        map_stream_layout!(Map<u8,u8>),
        base,
    )
    .unwrap();
    let consumed: u8 = kani::any();
    kani::assume(consumed as usize <= COUNT);
    let mut iter = map.try_values();
    for i in 0..consumed as usize {
        assert_eq!(iter.next(), Some(Ok(payload[2 * i + 1])));
    }
    let mut cloned = iter.clone();
    assert_eq!(
        iter.size_hint(),
        (COUNT - consumed as usize, Some(COUNT - consumed as usize))
    );
    for i in consumed as usize..COUNT {
        assert_eq!(iter.next(), Some(Ok(payload[2 * i + 1])));
        assert_eq!(cloned.next(), Some(Ok(payload[2 * i + 1])));
    }
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(cloned.next(), None);
    kani::cover!(base == 0);
    kani::cover!(base == u32::MAX - (4 + 2 * COUNT) as u32);
}

#[kani::proof]
#[kani::unwind(6)]
fn record_fixed_values_n0() {
    fixed_values_values::<0>();
}
#[kani::proof]
#[kani::unwind(6)]
fn record_fixed_values_n1() {
    fixed_values_values::<1>();
}
#[kani::proof]
#[kani::unwind(6)]
fn record_fixed_values_n2() {
    fixed_values_values::<2>();
}

#[kani::proof]
#[kani::unwind(6)]
fn record_fixed_values_domains() {
    let payload: [u8; 4] = kani::any();
    let bytes = [2, 0, 0, 0, payload[0], payload[1], payload[2], payload[3]];
    let map =
        <Map<bool, bool> as Schema>::read_region(&bytes, map_stream_layout!(Map<bool,bool>), 31)
            .unwrap();
    let mut iter = map.try_values();
    for i in 0..2 {
        let value = payload[2 * i + 1];
        let expected = if value <= 1 {
            Ok(value != 0)
        } else {
            Err(Error::new(ErrorKind::InvalidBool, 36 + 2 * i as u32))
        };
        assert_eq!(iter.next(), Some(expected));
        assert_eq!(iter.len(), 1 - i);
    }
    assert_eq!(iter.next(), None);
    assert_eq!(iter.clone().next(), None);
    kani::cover!(payload[0] > 1 && payload[2] > 1 && payload[1] <= 1 && payload[3] <= 1);
    kani::cover!(payload[1] > 1 && payload[3] <= 1);
    kani::cover!(payload[1] > 1 && payload[3] > 1);
}

#[kani::proof]
#[kani::unwind(6)]
fn record_fixed_values_schema() {
    let choose: bool = kani::any();
    let bytes = [0u8; 4];
    let layout = if choose {
        map_stream_layout!(Map<u16,u8>)
    } else {
        map_stream_layout!(Map<u8,u16>)
    };
    let view = blubber::View::new(&bytes, layout, 31).unwrap();
    let mut iter = blubber::MapRef::<u8, u8>::from_view(view)
        .unwrap()
        .try_values();
    assert_eq!(iter.len(), 1);
    assert_eq!(iter.next(), Some(Err(ErrorKind::InvalidSchema.into())));
    assert_eq!(iter.len(), 0);
    assert_eq!(iter.clone().next(), None);
    assert_eq!(iter.next(), None);
    kani::cover!(choose);
}

// StrictKeys32 checks the real strict scan against an independent scalar byte oracle.
fn strict_keys32_keys<const COUNT: usize>() -> i8 {
    let payload: [u8; 64] = kani::any();
    let mut bytes = [0u8; 70];
    bytes[0] = COUNT as u8;
    for i in 0..32 {
        if COUNT > 0 {
            bytes[4 + i] = payload[i];
        }
        if COUNT > 1 {
            bytes[37 + i] = payload[32 + i];
        }
    }
    // Values differ from both keys and are outside the comparison frame.
    bytes[36] = kani::any();
    bytes[69] = kani::any();
    let layout = map_stream_layout!(Map<[u8; 32], u8>);
    let mut order = 0i8;
    if COUNT > 1 {
        for i in 0..32 {
            if order == 0 {
                if payload[i] < payload[32 + i] {
                    order = -1;
                }
                if payload[i] > payload[32 + i] {
                    order = 1;
                }
            }
        }
    }
    let partition = blubber_core::MapPartition {
        count: COUNT as u32,
        key_width: 32,
        value_width: Some(1),
        directory: blubber_core::Span { start: 4, end: 4 },
        payload: blubber_core::Span {
            start: 4,
            end: (4 + COUNT * 33) as u32,
        },
    };
    let result =
        blubber_core::__strict_map_partition(&bytes[..4 + COUNT * 33], &layout, &partition);
    let expected = if COUNT < 2 || order < 0 {
        Ok(())
    } else {
        Err(blubber_core::Error::new(
            if order == 0 {
                blubber_core::ErrorKind::DuplicateKey
            } else {
                blubber_core::ErrorKind::UnsortedKeys
            },
            37,
        ))
    };
    assert_eq!(result, expected);
    order
}

#[kani::proof]
#[kani::unwind(34)]
fn record_strict_keys32_n0() {
    strict_keys32_keys::<0>();
    kani::cover!(true);
}
#[kani::proof]
#[kani::unwind(34)]
fn record_strict_keys32_n1() {
    strict_keys32_keys::<1>();
    kani::cover!(true);
}
#[kani::proof]
#[kani::unwind(34)]
fn record_strict_keys32_n2() {
    let order = strict_keys32_keys::<2>();
    kani::cover!(order < 0);
    kani::cover!(order == 0);
    kani::cover!(order > 0);
}

#[kani::proof]
#[kani::unwind(34)]
fn record_strict_keys32_progress() {
    let position: usize = kani::any();
    kani::assume(position < 32);
    let a: u8 = kani::any();
    let b: u8 = kani::any();
    let c: u8 = kani::any();
    let mut bytes = [0u8; 103];
    bytes[0] = 3;
    bytes[4 + position] = a;
    bytes[37 + position] = b;
    bytes[70 + position] = c;
    bytes[36] = kani::any();
    bytes[69] = kani::any();
    bytes[102] = kani::any();
    let layout = map_stream_layout!(Map<[u8; 32], u8>);
    let first = if a >= b {
        Some((a == b, 37))
    } else if b >= c {
        Some((b == c, 70))
    } else {
        None
    };
    let expected = match first {
        None => Ok(()),
        Some((equal, offset)) => Err(blubber_core::Error::new(
            if equal {
                blubber_core::ErrorKind::DuplicateKey
            } else {
                blubber_core::ErrorKind::UnsortedKeys
            },
            offset,
        )),
    };
    let partition = blubber_core::MapPartition {
        count: 3,
        key_width: 32,
        value_width: Some(1),
        directory: blubber_core::Span { start: 4, end: 4 },
        payload: blubber_core::Span { start: 4, end: 103 },
    };
    assert_eq!(
        blubber_core::__strict_map_partition(&bytes, &layout, &partition),
        expected
    );
    kani::cover!(a < b && b < c);
    kani::cover!(a < b && b == c);
    kani::cover!(a < b && b > c);
}

#[kani::proof]
#[kani::unwind(4)]
fn record_point_lookup_index() {
    let values: [u64; 2] = kani::any();
    let mut bytes = [0; 20];
    bytes[0] = 2;
    bytes[4..12].copy_from_slice(&values[0].to_le_bytes());
    bytes[12..20].copy_from_slice(&values[1].to_le_bytes());
    let index: usize = kani::any();
    let base: u32 = kani::any();
    kani::assume(base <= u32::MAX - 20);
    let list = blubber::View::new(&bytes, <List<u64>>::LAYOUT, base)
        .unwrap()
        .read::<List<u64>>()
        .unwrap();
    assert_eq!(list.try_get(index), Ok(values.get(index).copied()));
    assert_eq!(list.try_len(), Ok(2));
    kani::cover!(index < 2);
    kani::cover!(index >= 2);
}

#[kani::proof]
#[kani::unwind(4)]
fn record_point_lookup_bool() {
    let key: u8 = kani::any();
    let value: u8 = kani::any();
    let query: bool = kani::any();
    let bytes = [1, 0, 0, 0, key, value];
    let map = blubber::View::new(&bytes, map_stream_layout!(Map<bool, bool>), 37)
        .unwrap()
        .read::<Map<bool, bool>>()
        .unwrap();
    let expected = if key > 1 {
        Err(Error::new(ErrorKind::InvalidBool, 41))
    } else if (key == 1) != query {
        Ok(None)
    } else if value > 1 {
        Err(Error::new(ErrorKind::InvalidBool, 42))
    } else {
        Ok(Some(value == 1))
    };
    assert_eq!(map.try_get(&query), expected);
    assert_eq!(
        map.try_contains_key(&query),
        if key > 1 {
            Err(Error::new(ErrorKind::InvalidBool, 41))
        } else {
            Ok((key == 1) == query)
        }
    );
    assert_eq!(map.try_len(), Ok(1));
    kani::cover!(key > 1);
    kani::cover!(key <= 1 && (key == 1) == query && value > 1);
    kani::cover!(key <= 1 && (key == 1) == query && value <= 1);
    kani::cover!(key <= 1 && (key == 1) != query);
}

#[kani::proof]
#[kani::unwind(34)]
fn record_point_lookup_keys() {
    let key: [u8; 32] = kani::any();
    let query: [u8; 32] = kani::any();
    let mut equal = true;
    for index in 0..32 {
        equal &= key[index] == query[index];
    }
    assert_eq!(key.as_slice() == query.as_slice(), equal);
    kani::cover!(equal);
    kani::cover!(!equal);
}

#[kani::proof]
#[kani::unwind(6)]
fn record_point_lookup_halves() {
    let query: u8 = kani::any();
    let bytes = [3, 0, 0, 0, 1, 7, 3, 9, 5, 11];
    let map = blubber::View::new(&bytes, map_stream_layout!(Map<u8, u8>), 17)
        .unwrap()
        .read::<Map<u8, u8>>()
        .unwrap();
    assert_eq!(
        map.try_get(&query),
        Ok(match query {
            1 => Some(7),
            3 => Some(9),
            5 => Some(11),
            _ => None,
        })
    );
    kani::cover!(query == 1);
    kani::cover!(query == 3);
    kani::cover!(query == 5);
    kani::cover!(query == 255);
}

#[kani::proof]
#[kani::unwind(4)]
fn record_point_lookup_foreign() {
    let value: u8 = kani::any();
    let bytes = [1, 0, 0, 0, value, 0];
    let list = blubber::View::new(&bytes, map_stream_layout!(List<u16>), 13)
        .unwrap()
        .read::<List<u8>>()
        .unwrap();
    assert_eq!(
        list.try_get(0),
        Err(Error::new(ErrorKind::InvalidBoundary, 18))
    );
    let variable = [1, 0, 0, 0, value];
    let list = blubber::View::new(&variable, map_stream_layout!(List<blubber::Str>), 13)
        .unwrap()
        .read::<List<bool>>()
        .unwrap();
    assert_eq!(
        list.try_get(0),
        if value <= 1 {
            Ok(Some(value == 1))
        } else {
            Err(Error::new(ErrorKind::InvalidBool, 17))
        }
    );
    kani::cover!(value <= 1);
    kani::cover!(value > 1);
}

#[blubber::schema(account)]
struct TailRemoveList {
    value: List<u8>,
}
#[blubber::schema(account)]
struct TailRemoveMap {
    value: Map<u8, u8>,
}

#[kani::proof]
#[kani::unwind(33)]
fn record_tail_remove_routes() {
    let a: u8 = kani::any();
    let b: u8 = kani::any();
    let mut single = [0u8; 19];
    single[..8].copy_from_slice(&<TailRemoveList as blubber::AccountSchema>::DISCRIMINATOR);
    word(&mut single, 8, 18);
    word(&mut single, 12, 2);
    single[16] = a;
    single[17] = b;
    single[18] = 165;
    let expected = single;
    {
        let mut store = SliceStore::new(&mut single);
        let mut root = TailRemoveList::load_trusted_mut(&mut store).unwrap();
        let mut values = root.value_mut();
        assert_eq!(values.reborrow().try_len().unwrap(), 2);
    }
    assert_eq!(single, expected);

    kani::cover!(a == 255 && b == 0);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_tail_remove_n0() {
    let query: u8 = kani::any();
    let mut bytes = [1u8, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 165];
    let before = bytes;
    let mut path = blubber_core::raw::InlinePath::new();
    path.push(blubber_core::raw::Step::Field(0)).unwrap();
    let loan = blubber_core::raw::prepare_tail_remove(
        &mut bytes,
        [1, 0, 0, 0, 0, 0, 0, 0],
        4,
        &map_stream_layout!(TailRemoveMap),
        &path,
        0,
        Some(&[query]),
    )
    .unwrap()
    .unwrap();
    assert_eq!(loan.required_len(), 16);
    assert_eq!(loan.removed_span(), None);
    loan.finish();
    assert_eq!(bytes, before);
    let result = blubber_core::raw::prepare_tail_remove(
        &mut bytes,
        [1, 0, 0, 0, 0, 0, 0, 0],
        4,
        &map_stream_layout!(TailRemoveList),
        &path,
        usize::MAX,
        None,
    );
    assert_eq!(result.err(), Some(ErrorKind::IndexOutOfBounds.into()));
    assert_eq!(bytes, before);
    kani::cover!(query == 255);
}

fn tail_remove_n1<const PART: u8>() {
    let value: u8 = kani::any();
    let index: usize = match PART {
        0 => 0,
        1 => 1,
        _ => kani::any(),
    };
    let included = match PART {
        0 => index == 0,
        1 => index == 1,
        2 => index > 1 && index <= u32::MAX as usize,
        _ => index > u32::MAX as usize,
    };
    if !included {
        return;
    }
    let mut bytes = [
        1u8, 0, 0, 0, 0, 0, 0, 0, 17, 0, 0, 0, 1, 0, 0, 0, value, 165,
    ];
    let before = bytes;
    let mut path = blubber_core::raw::InlinePath::new();
    path.push(blubber_core::raw::Step::Field(0)).unwrap();
    match blubber_core::raw::prepare_tail_remove(
        &mut bytes,
        [1, 0, 0, 0, 0, 0, 0, 0],
        4,
        &map_stream_layout!(TailRemoveList),
        &path,
        index,
        None,
    ) {
        Err(error) => {
            assert!(index > 0);
            assert_eq!(error, ErrorKind::IndexOutOfBounds.into());
        }
        Ok(loan) => {
            assert_eq!(index, 0);
            if PART == 0 {
                let loan = loan.unwrap();
                assert_eq!(
                    loan.removed_span(),
                    Some(blubber_core::Span { start: 16, end: 17 })
                );
                assert_eq!(loan.inspect()[16], value);
                loan.finish();
            }
        }
    }
    if PART == 0 {
        assert_same_bytes(
            &bytes,
            &[1, 0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 165],
        );
    } else {
        assert_same_bytes(&bytes, &before);
    }
    kani::cover!((if PART == 0 { index == 0 } else { index > 0 }) && value == 255);
}

#[kani::proof]
#[kani::unwind(9)]
fn record_tail_remove_n1_index0() {
    tail_remove_n1::<0>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_tail_remove_n1_index1() {
    tail_remove_n1::<1>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_tail_remove_n1_index_word() {
    tail_remove_n1::<2>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_tail_remove_n1_index_large() {
    tail_remove_n1::<3>();
}

fn tail_remove_n2<const PART: u8>() {
    let first: u8 = kani::any();
    let second: u8 = kani::any();
    let query: u8 = match PART {
        0 => 1,
        1 => 3,
        2 => 0,
        3 => 2,
        _ => kani::any(),
    };
    let included = match PART {
        0 => query == 1,
        1 => query == 3,
        2 => query < 1,
        3 => query > 1 && query < 3,
        _ => query > 3,
    };
    if !included {
        return;
    }
    let mut bytes = [
        1u8, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0, 2, 0, 0, 0, 1, first, 3, second, 165,
    ];
    let before = bytes;
    let mut path = blubber_core::raw::InlinePath::new();
    path.push(blubber_core::raw::Step::Field(0)).unwrap();
    let loan = blubber_core::raw::prepare_tail_remove(
        &mut bytes,
        [1, 0, 0, 0, 0, 0, 0, 0],
        4,
        &map_stream_layout!(TailRemoveMap),
        &path,
        0,
        Some(&[query]),
    )
    .unwrap()
    .unwrap();
    let expected = match query {
        1 => Some((17, first)),
        3 => Some((19, second)),
        _ => None,
    };
    assert_eq!(
        loan.removed_span(),
        expected.map(|(start, _)| blubber_core::Span {
            start,
            end: start + 1
        })
    );
    if let Some((at, value)) = expected {
        assert_eq!(loan.inspect()[at as usize], value);
    }
    loan.finish();
    if expected.is_some() {
        let (key, value) = if query == 1 { (3, second) } else { (1, first) };
        assert_same_bytes(
            &bytes,
            &[
                1, 0, 0, 0, 0, 0, 0, 0, 18, 0, 0, 0, 1, 0, 0, 0, key, value, 0, 0, 165,
            ],
        );
    } else {
        assert_same_bytes(&bytes, &before);
    }
    kani::cover!(
        (match PART {
            0 => query == 1,
            1 => query == 3,
            _ => query != 1 && query != 3,
        }) && first == 255
            && second == 254
    );
}

#[kani::proof]
#[kani::unwind(9)]
fn record_tail_remove_n2_first() {
    tail_remove_n2::<0>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_tail_remove_n2_second() {
    tail_remove_n2::<1>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_tail_remove_n2_miss_before() {
    tail_remove_n2::<2>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_tail_remove_n2_miss_between() {
    tail_remove_n2::<3>();
}
#[kani::proof]
#[kani::unwind(9)]
fn record_tail_remove_n2_miss_after() {
    tail_remove_n2::<4>();
}

// WordLookup exercises the production local probe with a separate scalar byte oracle.
fn word_lookup_order(left: &[u8], right: &[u8]) -> i8 {
    let mut same = true;
    let mut less = false;
    for i in 0..left.len() {
        less |= same && left[i] < right[i];
        same &= left[i] == right[i];
    }
    if same {
        0
    } else if less {
        -1
    } else {
        1
    }
}

#[kani::proof]
#[kani::unwind(34)]
fn record_word_lookup_n0() {
    let query: [u8; 32] = kani::any();
    assert_eq!(
        blubber_core::search_map_local(&[0; 4], &map_stream_layout!(Map<[u8; 32], u8>), &query),
        Ok(blubber_core::Search {
            index: 0,
            found: false
        })
    );
    kani::cover!(true);
}

// Full-domain eight-byte pairs exercise every word position. Generic Verus
// composition covers arbitrary surrounding words; no data assumptions are used.
fn word_lookup_word_case<const AT: usize>() -> i8 {
    let stored: [u8; 8] = kani::any();
    let query: [u8; 8] = kani::any();
    let mut left = [0x42u8; 32];
    let mut right = [0x42u8; 32];
    left[AT..AT + 8].copy_from_slice(&stored);
    right[AT..AT + 8].copy_from_slice(&query);
    let order = word_lookup_order(&stored, &query);
    let expected = if order < 0 {
        core::cmp::Ordering::Less
    } else if order > 0 {
        core::cmp::Ordering::Greater
    } else {
        core::cmp::Ordering::Equal
    };
    assert_eq!(blubber_core::__local_key_order(&left, &right), expected);
    order
}

#[kani::proof]
#[kani::unwind(34)]
fn record_word_lookup_word0() {
    let order = word_lookup_word_case::<0>();
    kani::cover!(order < 0);
    kani::cover!(order == 0);
    kani::cover!(order > 0);
}

#[kani::proof]
#[kani::unwind(34)]
fn record_word_lookup_word1() {
    let order = word_lookup_word_case::<8>();
    kani::cover!(order < 0);
    kani::cover!(order == 0);
    kani::cover!(order > 0);
}

#[kani::proof]
#[kani::unwind(34)]
fn record_word_lookup_word2() {
    let order = word_lookup_word_case::<16>();
    kani::cover!(order < 0);
    kani::cover!(order == 0);
    kani::cover!(order > 0);
}

#[kani::proof]
#[kani::unwind(34)]
fn record_word_lookup_word3() {
    let order = word_lookup_word_case::<24>();
    kani::cover!(order < 0);
    kani::cover!(order == 0);
    kani::cover!(order > 0);
}

#[kani::proof]
#[kani::unwind(6)]
fn record_word_lookup_prefix() {
    let shared: [u8; 8] = kani::any();
    let side: bool = kani::any();
    let mut bytes = [0x42u8; 103];
    bytes[..4].copy_from_slice(&3u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&shared);
    bytes[37..45].copy_from_slice(&shared);
    bytes[70..78].copy_from_slice(&shared);
    bytes[35] = 1;
    bytes[68] = 3;
    bytes[101] = 5;
    bytes[36] = kani::any();
    bytes[69] = kani::any();
    bytes[102] = kani::any();
    let mut query = [0x42u8; 32];
    query[..8].copy_from_slice(&shared);
    query[31] = if side { 2 } else { 4 };
    assert_eq!(
        blubber_core::search_map_local(&bytes, &map_stream_layout!(Map<[u8; 32], u8>), &query),
        Ok(blubber_core::Search {
            index: if side { 1 } else { 2 },
            found: false
        })
    );
    kani::cover!(side);
    kani::cover!(!side);
}

#[kani::proof]
#[kani::unwind(34)]
fn record_word_lookup_fallback() {
    let stored: [u8; 31] = kani::any();
    let query: [u8; 31] = kani::any();
    let order = word_lookup_order(&stored, &query);
    let expected = if order < 0 {
        core::cmp::Ordering::Less
    } else if order > 0 {
        core::cmp::Ordering::Greater
    } else {
        core::cmp::Ordering::Equal
    };
    assert_eq!(blubber_core::__local_key_order(&stored, &query), expected);
    kani::cover!(order < 0);
    kani::cover!(order == 0);
    kani::cover!(order > 0);
}

#[kani::proof]
#[kani::unwind(4)]
fn record_word_lookup_byte() {
    let left: u8 = kani::any();
    let right: u8 = kani::any();
    let expected = if left < right {
        core::cmp::Ordering::Less
    } else if left > right {
        core::cmp::Ordering::Greater
    } else {
        core::cmp::Ordering::Equal
    };
    assert_eq!(blubber_core::__local_key_order(&[left], &[right]), expected);
    kani::cover!(left < right);
    kani::cover!(left == right);
    kani::cover!(left > right);
}

#[kani::proof]
#[kani::unwind(6)]
fn record_word_lookup_indexed() {
    let shared: [u8; 8] = kani::any();
    let side: bool = kani::any();
    let mut bytes = [0x42u8; 112];
    bytes[..4].copy_from_slice(&3u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&shared);
    bytes[40..48].copy_from_slice(&shared);
    bytes[76..84].copy_from_slice(&shared);
    bytes[35] = 1;
    bytes[71] = 3;
    bytes[107] = 5;
    // All values are empty; each stored end equals the zero payload length.
    bytes[36..40].copy_from_slice(&0u32.to_le_bytes());
    bytes[72..76].copy_from_slice(&0u32.to_le_bytes());
    bytes[108..112].copy_from_slice(&0u32.to_le_bytes());
    let mut query = [0x42u8; 32];
    query[..8].copy_from_slice(&shared);
    query[31] = if side { 2 } else { 4 };
    assert_eq!(
        blubber_core::search_map_local(
            &bytes,
            &map_stream_layout!(Map<[u8; 32], blubber::Str>),
            &query
        ),
        Ok(blubber_core::Search {
            index: if side { 1 } else { 2 },
            found: false
        })
    );
    kani::cover!(side);
    kani::cover!(!side);
}

struct UniformFillEncoder {
    key: u32,
    calls: core::cell::Cell<usize>,
    value: [u8; 2],
    fail_at: usize,
    written: usize,
    error: bool,
}
impl blubber_core::fill::ValueEncoder for UniformFillEncoder {
    fn geometry(&self) -> (u32, u32) {
        (0, self.key)
    }
    fn encode(&self, slot: &mut [u8]) -> blubber::Result<usize> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        slot.copy_from_slice(&self.value[..slot.len()]);
        if call == self.fail_at {
            if self.error {
                return Err(ErrorKind::InvalidUtf8.into());
            }
            return Ok(self.written);
        }
        Ok(slot.len())
    }
}
fn uniform_fill_encoder() -> UniformFillEncoder {
    UniformFillEncoder {
        key: 0,
        calls: core::cell::Cell::new(0),
        value: kani::any(),
        fail_at: kani::any(),
        written: kani::any(),
        error: kani::any(),
    }
}
fn uniform_fill<const MAP: bool>() {
    let mut encoder = uniform_fill_encoder();
    encoder.key = u32::from(MAP);
    let layout = if MAP {
        blubber_core::Layout::Map {
            key: blubber_core::KeyLayout {
                width: 1,
                domain: blubber_core::KeyDomain::Raw,
            },
            value: &blubber_core::Layout::Str,
        }
    } else {
        blubber_core::Layout::List {
            element: &blubber_core::Layout::Str,
        }
    };
    let mut bytes = [0xa5u8; 18];
    bytes[..4].copy_from_slice(&2u32.to_le_bytes());
    let payload = if MAP { 14 } else { 8 };
    bytes[4..payload].fill(0);
    if MAP {
        bytes[4] = 1;
        bytes[9] = 3;
    }
    let before = bytes;
    let plan = blubber_core::raw::RewritePlan::check(&bytes[..payload], &layout, 2, 4).unwrap();
    let mut stream = plan.begin(&mut bytes[..payload + 4]).unwrap();
    let result = stream.fill_uniform(2, u32::from(MAP), &encoder);
    assert_eq!(stream.finish().is_ok(), result.is_ok());
    let first = encoder.fail_at != 0 || (!encoder.error && encoder.written == 2);
    let second = encoder.fail_at != 1 || (!encoder.error && encoder.written == 2);
    assert_eq!(result.is_ok(), first && second);
    assert_eq!(encoder.calls.get(), if first { 2 } else { 1 });
    assert_eq!(&bytes[..4], &before[..4]);
    assert_eq!(&bytes[payload..payload + 2], &encoder.value);
    assert_eq!(
        &bytes[payload + 2..payload + 4],
        if first {
            &encoder.value[..]
        } else {
            &before[payload + 2..payload + 4]
        }
    );
    let at = if MAP { 5 } else { 4 };
    assert_eq!(
        &bytes[at..at + 4],
        &if first { 2u32 } else { 0 }.to_le_bytes()
    );
    if MAP {
        assert_eq!(bytes[4], 1);
        assert_eq!(bytes[9], 3);
        assert_eq!(
            &bytes[10..14],
            &if first && second { 4u32 } else { 0 }.to_le_bytes()
        );
    }
    kani::cover!(result.is_ok() && encoder.fail_at == 1);
}
#[kani::proof]
#[kani::unwind(33)]
fn record_uniform_fill_list() {
    uniform_fill::<false>();
}
fn uniform_fill_map() {
    let mut encoder = uniform_fill_encoder();
    encoder.key = 1;
    let key: u8 = kani::any();
    let layout = blubber_core::Layout::Flat {
        checked: <__RewriteMap as blubber::RootSchema>::__layout(),
    };
    let mut bytes = [1u8, 0, 0, 0, key, 0, 0, 0, 0, 0xa5, 0xa5];
    let plan = blubber_core::raw::RewritePlan::check(&bytes[..9], &layout, 1, 2).unwrap();
    let mut stream = plan.begin(&mut bytes).unwrap();
    let result = stream.fill_uniform(2, 1, &encoder);
    assert_eq!(stream.finish().is_ok(), result.is_ok());
    let expected = encoder.fail_at != 0 || (!encoder.error && encoder.written == 2);
    assert_eq!(result.is_ok(), expected);
    assert_eq!(encoder.calls.get(), 1);
    assert_eq!(&bytes[..5], &[1, 0, 0, 0, key]);
    assert_eq!(&bytes[5..9], &if expected { 2u32 } else { 0 }.to_le_bytes());
    assert_eq!(&bytes[9..], &encoder.value);
    kani::cover!(result.is_err() && encoder.fail_at == 0);
}
#[kani::proof]
#[kani::unwind(33)]
fn record_uniform_fill_map() {
    uniform_fill_map();
}
fn uniform_fill_small<const N: usize>() {
    let encoder = uniform_fill_encoder();
    let layout = blubber_core::Layout::List {
        element: &blubber_core::Layout::Str,
    };
    let mut bytes = [0xa5u8; 6];
    bytes[..4].copy_from_slice(&(N as u32).to_le_bytes());
    let plan =
        blubber_core::raw::RewritePlan::check(&bytes[..4], &layout, N as u32, (2 * N) as u32)
            .unwrap();
    let mut stream = plan.begin(&mut bytes[..4 + 2 * N]).unwrap();
    let result = stream.fill_uniform(2, 0, &encoder);
    assert_eq!(stream.finish().is_ok(), result.is_ok());
    assert_eq!(encoder.calls.get(), N);
    assert_eq!(
        result.is_ok(),
        N == 0 || encoder.fail_at != 0 || (!encoder.error && encoder.written == 2)
    );
    assert_eq!(&bytes[..4], &(N as u32).to_le_bytes());
    if N == 1 {
        assert_eq!(&bytes[4..6], &encoder.value);
    }
    kani::cover!(result.is_ok());
}
#[kani::proof]
#[kani::unwind(33)]
fn record_uniform_fill_n0() {
    uniform_fill_small::<0>();
}
#[kani::proof]
#[kani::unwind(33)]
fn record_uniform_fill_n1() {
    uniform_fill_small::<1>();
}
fn uniform_fill_poison(part: u8) {
    use blubber::raw::Resize as _;
    use blubber::Resize as _;
    let encoder = uniform_fill_encoder();
    if match part {
        0 => encoder.fail_at != 0,
        1 => encoder.fail_at != 1,
        _ => encoder.fail_at < 2,
    } {
        return;
    }
    let mut bytes = [0xa5u8; 24];
    bytes[..8].copy_from_slice(b"fillloan");
    bytes[8..12].copy_from_slice(&20u32.to_le_bytes());
    bytes[12..20].copy_from_slice(&[2, 0, 0, 0, 0, 0, 0, 0]);
    let layout = blubber_core::Layout::Flat {
        checked: <RewriteTailRoot as blubber::RootSchema>::__layout(),
    };
    let path = blubber_core::raw::InlinePath::root_field(0);
    let mut store = SliceStore::new(&mut bytes);
    let tail =
        blubber_core::raw::resolve_rewrite_tail(store.bytes(), *b"fillloan", 4, &layout, &path)
            .unwrap()
            .unwrap();
    let (span, selected) = tail.selection();
    let plan = blubber_core::raw::RewritePlan::check_admitted(
        &store.bytes()[span.start as usize..span.end as usize],
        &selected,
        2,
        4,
    )
    .unwrap();
    store.set_poisoned(true);
    let mut write = tail
        .prepare(store.bytes_mut(), plan.encoded_len())
        .unwrap()
        .unwrap();
    let result = (|| {
        let mut stream = plan.begin(write.destination())?;
        stream.fill_uniform(2, 0, &encoder)?;
        stream.finish()
    })();
    if result.is_ok() {
        write.finish();
        store.set_poisoned(false);
    } else {
        drop(write);
    }
    let success = encoder.fail_at >= 2 || (!encoder.error && encoder.written == 2);
    let first = encoder.fail_at != 0 || (!encoder.error && encoder.written == 2);
    assert_eq!(result.is_ok(), success);
    assert_eq!(store.is_poisoned(), !success);
    assert_eq!(encoder.calls.get(), if first { 2 } else { 1 });
    assert_eq!(
        &store.bytes()[8..12],
        &if success { 24u32 } else { 20 }.to_le_bytes()
    );
    assert_eq!(
        &store.bytes()[16..20],
        &if first { 2u32 } else { 0 }.to_le_bytes()
    );
    assert_eq!(&store.bytes()[20..22], &encoder.value);
    assert_eq!(
        &store.bytes()[22..24],
        if first {
            &encoder.value[..]
        } else {
            &[0xa5u8; 2][..]
        }
    );
    if !success {
        assert_eq!(store.commit(20), Err(ErrorKind::Poisoned.into()));
    }
    kani::cover!(if part == 1 {
        result.is_err() && encoder.fail_at == 1
    } else if part == 0 {
        result.is_err() && encoder.fail_at == 0
    } else {
        result.is_ok() && encoder.fail_at >= 2
    });
}
#[kani::proof]
#[kani::unwind(33)]
fn record_uniform_fill_poison_first() {
    uniform_fill_poison(0);
}
#[kani::proof]
#[kani::unwind(33)]
fn record_uniform_fill_poison_second() {
    uniform_fill_poison(1);
}
#[kani::proof]
#[kani::unwind(33)]
fn record_uniform_fill_poison_later() {
    uniform_fill_poison(2);
}

blubber::__root_schema!(__ScopedPointWords, List<blubber::Str>);

#[kani::proof]
#[kani::unwind(25)]
fn record_scoped_point_selection() {
    let values: [u8; 2] = kani::any();
    let mut bytes = [
        b'l',
        b'i',
        b's',
        b't',
        b'l',
        b'o',
        b'a',
        b'n',
        22,
        0,
        0,
        0,
        2,
        0,
        0,
        0,
        1,
        0,
        0,
        0,
        values[0] & 0x7f,
        values[1] & 0x7f,
    ];
    let before = bytes;
    let mut store = SliceStore::new(&mut bytes);
    let edit = Edit::load_trusted::<__ScopedPointWords>(&mut store, *b"listloan").unwrap();
    let mut list = blubber::raw::project::<List<blubber::Str>>(edit);
    let index: usize = kani::any();
    let value: u64 = kani::any();
    let mut calls = 0;
    let result = list.with_mut(index, |_| {
        calls += 1;
        Ok(value)
    });
    assert_eq!(result, Ok(if index < 2 { Some(value) } else { None }));
    assert_eq!(calls, usize::from(index < 2));
    assert_eq!(store.bytes(), before);
    kani::cover!(index < 2);
    kani::cover!(index >= 2);
}

#[kani::proof]
#[kani::unwind(25)]
fn record_scoped_point_loan() {
    let mut bytes: [u8; 24] = kani::any();
    let before = bytes;
    let replacement: [u8; 4] = kani::any();
    let split: usize = kani::any();
    kani::assume(split <= 4);
    let second: bool = kani::any();
    let start = if second { 20 + split } else { 20 };
    let end = if second { 24 } else { 20 + split };
    let slot = blubber_core::raw::indexed_slot_region(&mut bytes, start, end);
    assert_eq!(slot.len(), end - start);
    slot.copy_from_slice(&replacement[..end - start]);
    for i in 0..24 {
        assert_eq!(
            bytes[i],
            if start <= i && i < end {
                replacement[i - start]
            } else {
                before[i]
            }
        );
    }
    kani::cover!(start == end);
    kani::cover!(start < end);
}

#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_list_empty_end() {
    let end: u32 = kani::any();
    let lower: u32 = kani::any();
    let upper: u32 = kani::any();
    let delta: i64 = kani::any();
    let at: u32 = kani::any();
    let result = blubber_core::patch_pass::checked_end(end, lower, upper, delta, at);
    let sum = i128::from(end) + i128::from(delta);
    let expected = if end < lower || end > upper {
        Err(Error::new(ErrorKind::InvalidBoundary, at))
    } else if sum < 0 || sum > i128::from(u32::MAX) {
        Err(Error::new(ErrorKind::ArithmeticOverflow, at))
    } else {
        Ok(sum as u32)
    };
    assert_eq!(result, expected);
    kani::cover!(result.is_ok());
    kani::cover!(end < lower || end > upper);
    kani::cover!(lower <= end && end <= upper && sum > i128::from(u32::MAX));
}

#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_list_empty_word() {
    let mut bytes: [u8; 8] = kani::any();
    let before = bytes;
    let source: u32 = kani::any();
    let destination: u32 = kani::any();
    let lower: u32 = kani::any();
    let upper: u32 = kani::any();
    let delta: i64 = kani::any();
    let mut poisoned: bool = kani::any();
    let result = blubber_core::patch_pass::patch_word(
        &mut bytes,
        source,
        destination,
        lower,
        upper,
        delta,
        &mut poisoned,
    );
    assert!(poisoned);
    if result.is_err() {
        assert_eq!(bytes, before);
    } else {
        assert!(source <= 4 && destination <= 4);
        let source = source as usize;
        let end = u32::from_le_bytes([
            before[source],
            before[source + 1],
            before[source + 2],
            before[source + 3],
        ]);
        assert!(lower <= end && end <= upper);
        let sum = i128::from(end) + i128::from(delta);
        assert!(0 <= sum && sum <= i128::from(u32::MAX));
        let word = (sum as u32).to_le_bytes();
        for i in 0..8 {
            let at = destination as usize;
            assert_eq!(
                bytes[i],
                if at <= i && i < at + 4 {
                    word[i - at]
                } else {
                    before[i]
                }
            );
        }
    }
    kani::cover!(result.is_ok() && source == destination);
    kani::cover!(result.is_err());
}

#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_list_empty_publication() {
    let mut poisoned: bool = kani::any();
    let success: bool = kani::any();
    let error = Error::new(ErrorKind::InvalidBoundary, kani::any());
    let publication = if success { Ok(()) } else { Err(error) };
    let result = blubber_core::patch_pass::finish_publication(&mut poisoned, publication);
    assert_eq!(result, publication);
    assert_eq!(poisoned, !success);
    kani::cover!(success);
    kani::cover!(!success);
}

#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_list_empty_empty() {
    let mut bytes: [u8; 8] = kani::any();
    let before = bytes;
    let len: usize = kani::any();
    kani::assume(len <= 8);
    let result = blubber_core::patch_pass::write_empty_list(&mut bytes[..len]);
    assert_eq!(result.is_ok(), len == 4);
    for i in 0..8 {
        assert_eq!(bytes[i], if len == 4 && i < 4 { 0 } else { before[i] });
    }
    kani::cover!(result.is_ok());
    kani::cover!(result.is_err());
}

#[kani::proof]
#[kani::unwind(25)]
fn record_lazy_list_empty_clear() {
    let spare: [u8; 4] = kani::any();
    let mut bytes = [0u8; 20];
    bytes[..8].copy_from_slice(b"listloan");
    bytes[8..12].copy_from_slice(&16u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&spare);
    let layout = blubber_core::Layout::List {
        element: &blubber_core::Layout::Str,
    };
    let resolved = blubber_core::raw::resolve_account_edit(
        &mut bytes,
        *b"listloan",
        4,
        &layout,
        &blubber_core::raw::InlinePath::new(),
    )
    .unwrap();
    let mut poisoned = true;
    let publication = blubber_core::patch_pass::write_empty_list(resolved.into_destination());
    assert_eq!(
        blubber_core::patch_pass::finish_publication(&mut poisoned, publication),
        Ok(())
    );
    assert_eq!(&bytes[8..16], &[16, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(&bytes[16..20], &spare);
    assert!(!poisoned);
    kani::cover!(spare[0] != 0);
}

#[kani::proof]
#[kani::unwind(9)]
fn record_lazy_list_patch_transport() {
    let mut bytes: [u8; 8] = kani::any();
    let before = bytes;
    let end = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let delta: i64 = kani::any();
    let lower: u32 = kani::any();
    let upper: u32 = kani::any();
    let mut poisoned = false;
    let result = blubber_core::patch_pass::transport_contained(
        &mut bytes,
        0,
        4,
        lower,
        upper,
        delta,
        &mut poisoned,
    );
    let sum = i128::from(end) + i128::from(delta);
    let valid = lower <= end && end <= upper && 0 <= sum && sum <= i128::from(u32::MAX);
    assert_eq!(result.is_ok(), valid);
    assert!(poisoned);
    assert_eq!(&bytes[..4], &before[..4]);
    if valid {
        assert_eq!(result, Ok(end));
        assert_eq!(&bytes[4..], &(sum as u32).to_le_bytes());
    } else {
        assert_eq!(bytes, before);
    }
    kani::cover!(valid);
    kani::cover!(!valid);
}

fn lazy_list_patch_wire(a: u32, b: u32, c: u32) -> [u8; 40] {
    let mut bytes = [0; 40];
    bytes[..8].copy_from_slice(b"listloan");
    bytes[8..12].copy_from_slice(&32u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&4u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&a.to_le_bytes());
    bytes[20..24].copy_from_slice(&b.to_le_bytes());
    bytes[24..28].copy_from_slice(&c.to_le_bytes());
    bytes[28..32].copy_from_slice(b"abcd");
    bytes
}

#[kani::proof]
#[kani::unwind(49)]
fn record_lazy_list_patch_insert() {
    let a: u32 = kani::any();
    let b: u32 = kani::any();
    let c: u32 = kani::any();
    let mut bytes = lazy_list_patch_wire(a, b, c);
    let before = bytes;
    let mut poisoned = false;
    let result = blubber_core::raw::__list_insert_model(&mut bytes, &mut poisoned);
    assert_eq!(result.is_ok(), a <= b && b <= c && c <= 4);
    assert!(poisoned);
    assert_eq!(&bytes[..16], &before[..16]);
    assert_eq!(&bytes[37..], &before[37..]);
    if result.is_ok() {
        assert_eq!(&bytes[16..20], &1u32.to_le_bytes());
        assert_eq!(&bytes[20..24], &(a + 1).to_le_bytes());
        assert_eq!(&bytes[24..28], &(b + 1).to_le_bytes());
        assert_eq!(&bytes[28..32], &(c + 1).to_le_bytes());
        assert_eq!(&bytes[33..37], b"abcd");
    }
    kani::cover!(result.is_ok());
    kani::cover!(result.is_err() && c <= 4 && b <= c && a > b);
}

#[kani::proof]
#[kani::unwind(49)]
fn record_lazy_list_patch_splice() {
    let a: u32 = kani::any();
    let b: u32 = kani::any();
    let c: u32 = kani::any();
    let mut bytes = lazy_list_patch_wire(a, b, c);
    let before = bytes;
    let mut poisoned = false;
    let result = blubber_core::raw::__list_splice_model(&mut bytes, &mut poisoned);
    assert_eq!(result.is_ok(), a <= b && b <= c && c <= 4);
    assert!(poisoned);
    assert_eq!(&bytes[..16], &before[..16]);
    assert_eq!(&bytes[32..], &before[32..]);
    if result.is_ok() {
        assert_eq!(&bytes[16..20], &(b - a).to_le_bytes());
        assert_eq!(&bytes[20..24], &(c - a).to_le_bytes());
        for i in 0..(4 - a) as usize {
            assert_eq!(bytes[24 + i], before[28 + a as usize + i]);
        }
    } else if a > 4 {
        assert_eq!(bytes, before);
    } else if a <= b && b <= 4 {
        assert_eq!(&bytes[16..20], &(b - a).to_le_bytes());
    }
    kani::cover!(result.is_ok());
    kani::cover!(result.is_err() && a <= b && b <= 4 && c > 4);
}

#[kani::proof]
#[kani::unwind(25)]
fn record_lazy_list_patch_bulk() {
    let end: u32 = kani::any();
    let mut bytes = [0u8; 15];
    bytes[..4].copy_from_slice(&2u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&end.to_le_bytes());
    bytes[8] = b'a';
    bytes[9] = b'b';
    let layout = blubber_core::Layout::Flat {
        checked: blubber::__layout_of!(blubber::List<blubber::Str>),
    };
    let source = blubber_core::raw::HeaderBulkSource::check(&bytes[..10], &layout).unwrap();
    let plan = source.plan(0, 1, 1).unwrap();
    let mut poisoned = false;
    let success = match plan.begin(&mut bytes, &mut poisoned) {
        Ok(mut layout) => {
            layout.reserve(1).unwrap();
            let mut slots = layout.finish().unwrap();
            slots.slot(0)[0] = b'c';
            true
        }
        Err(_) => false,
    };
    assert!(poisoned);
    assert_eq!(success, end <= 2);
    assert_eq!(
        blubber_core::read_u32(&bytes, 0).unwrap(),
        if success { 3 } else { 2 }
    );
    kani::cover!(success);
    kani::cover!(!success);
}

// Exact fixed words retain every byte of the frame without a 40-byte loop.
fn lazy_map_patch_word(bytes: &[u8; 40], at: usize) -> u64 {
    u64::from_le_bytes([
        bytes[at],
        bytes[at + 1],
        bytes[at + 2],
        bytes[at + 3],
        bytes[at + 4],
        bytes[at + 5],
        bytes[at + 6],
        bytes[at + 7],
    ])
}

fn lazy_map_patch_same_frame(bytes: &[u8; 40], before: &[u8; 40]) {
    assert_eq!(
        lazy_map_patch_word(bytes, 0),
        lazy_map_patch_word(before, 0)
    );
    assert_eq!(
        lazy_map_patch_word(bytes, 8),
        lazy_map_patch_word(before, 8)
    );
    assert_eq!(
        lazy_map_patch_word(bytes, 16),
        lazy_map_patch_word(before, 16)
    );
    assert_eq!(
        lazy_map_patch_word(bytes, 24),
        lazy_map_patch_word(before, 24)
    );
    assert_eq!(
        lazy_map_patch_word(bytes, 32),
        lazy_map_patch_word(before, 32)
    );
}

fn lazy_map_patch_wire(a: u32, b: u32) -> [u8; 40] {
    let mut bytes = [0; 40];
    bytes[..8].copy_from_slice(b"maploans");
    bytes[8..12].copy_from_slice(&34u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&3u32.to_le_bytes());
    for (at, key, end) in [(16, 2, a), (21, 4, b), (26, 6, 3)] {
        bytes[at] = key;
        bytes[at + 1..at + 5].copy_from_slice(&end.to_le_bytes());
    }
    bytes[31..34].copy_from_slice(b"abc");
    bytes
}

#[kani::proof]
#[kani::unwind(17)]
fn record_lazy_map_patch_insert() {
    let a: u32 = kani::any();
    let b: u32 = kani::any();
    let mut bytes = lazy_map_patch_wire(a, b);
    let before = bytes;
    let mut poisoned = false;
    let result = blubber_core::raw::__map_patch_model(
        &mut bytes,
        blubber_core::raw::MapEdit::Insert,
        &mut poisoned,
    );
    assert!(poisoned);
    assert_eq!(
        lazy_map_patch_word(&bytes, 0),
        lazy_map_patch_word(&before, 0)
    );
    assert_eq!(
        lazy_map_patch_word(&bytes, 8),
        lazy_map_patch_word(&before, 8)
    );
    assert_eq!(result.is_ok(), a <= b && b <= 3);
    if result.is_ok() {
        assert_eq!(blubber_core::read_u32(&bytes, 17).unwrap(), 1);
        assert_eq!(blubber_core::read_u32(&bytes, 22).unwrap(), a + 1);
        assert_eq!(blubber_core::read_u32(&bytes, 27).unwrap(), b + 1);
        assert_eq!(blubber_core::read_u32(&bytes, 32).unwrap(), 4);
        assert_eq!(&bytes[37..40], b"abc");
    }
    kani::cover!(result.is_ok());
    kani::cover!(result.is_err() && a <= 3 && b > 3);
}

#[kani::proof]
#[kani::unwind(17)]
fn record_lazy_map_patch_remove() {
    let a: u32 = kani::any();
    let b: u32 = kani::any();
    let mut bytes = lazy_map_patch_wire(a, b);
    let before = bytes;
    let mut poisoned = false;
    let result = blubber_core::raw::__map_patch_model(
        &mut bytes,
        blubber_core::raw::MapEdit::Remove,
        &mut poisoned,
    );
    assert!(poisoned);
    assert_eq!(
        lazy_map_patch_word(&bytes, 0),
        lazy_map_patch_word(&before, 0)
    );
    assert_eq!(
        lazy_map_patch_word(&bytes, 8),
        lazy_map_patch_word(&before, 8)
    );
    assert_eq!(
        lazy_map_patch_word(&bytes, 32) >> 16,
        lazy_map_patch_word(&before, 32) >> 16
    );
    assert_eq!(result.is_ok(), a <= b && b <= 3);
    if a > 3 {
        lazy_map_patch_same_frame(&bytes, &before);
    }
    if result.is_ok() {
        assert_eq!(blubber_core::read_u32(&bytes, 17).unwrap(), b - a);
        assert_eq!(blubber_core::read_u32(&bytes, 22).unwrap(), 3 - a);
        for i in 0..(3 - a) as usize {
            assert_eq!(bytes[26 + i], before[31 + a as usize + i]);
        }
    }
    kani::cover!(result.is_ok());
    kani::cover!(result.is_err() && a <= 3 && b > 3);
}

#[kani::proof]
#[kani::unwind(17)]
fn record_lazy_map_patch_replace() {
    let a: u32 = kani::any();
    let b: u32 = kani::any();
    let mut bytes = lazy_map_patch_wire(a, b);
    let before = bytes;
    let mut poisoned = false;
    let result = blubber_core::raw::__map_patch_model(
        &mut bytes,
        blubber_core::raw::MapEdit::Replace,
        &mut poisoned,
    );
    assert!(poisoned);
    assert_eq!(
        lazy_map_patch_word(&bytes, 0),
        lazy_map_patch_word(&before, 0)
    );
    assert_eq!(
        lazy_map_patch_word(&bytes, 8),
        lazy_map_patch_word(&before, 8)
    );
    assert_eq!(
        lazy_map_patch_word(&bytes, 32) >> 24,
        lazy_map_patch_word(&before, 32) >> 24
    );
    assert_eq!(result.is_ok(), a <= b && b <= 3);
    if a > 3 {
        lazy_map_patch_same_frame(&bytes, &before);
    }
    if result.is_ok() {
        assert_eq!(blubber_core::read_u32(&bytes, 17).unwrap(), 1);
        assert_eq!(blubber_core::read_u32(&bytes, 22).unwrap(), b + 1 - a);
        assert_eq!(blubber_core::read_u32(&bytes, 27).unwrap(), 4 - a);
        for i in 0..(3 - a) as usize {
            assert_eq!(bytes[32 + i], before[31 + a as usize + i]);
        }
    }
    kani::cover!(result.is_ok());
    kani::cover!(result.is_err() && a <= 3 && b > 3);
}

#[kani::proof]
#[kani::unwind(17)]
fn record_lazy_map_patch_clear() {
    let a: u32 = kani::any();
    let b: u32 = kani::any();
    let mut bytes = lazy_map_patch_wire(a, b);
    let before = bytes;
    let mut poisoned = false;
    let result = blubber_core::raw::__map_patch_model(
        &mut bytes,
        blubber_core::raw::MapEdit::Clear,
        &mut poisoned,
    );
    assert!(poisoned);
    assert_eq!(
        lazy_map_patch_word(&bytes, 0),
        lazy_map_patch_word(&before, 0)
    );
    assert_eq!(
        lazy_map_patch_word(&bytes, 8),
        lazy_map_patch_word(&before, 8)
    );
    assert_eq!(
        lazy_map_patch_word(&bytes, 32) >> 16,
        lazy_map_patch_word(&before, 32) >> 16
    );
    assert_eq!(result.is_ok(), true);
    lazy_map_patch_same_frame(&bytes, &before);
    kani::cover!(a > 3 && b > 3);
}

#[kani::proof]
#[kani::unwind(10)]
fn record_lazy_map_patch_walk_key() {
    let bytes: [u8; 8] = kani::any();
    let at: usize = kani::any();
    let stride: usize = kani::any();
    let first: bool = kani::any();
    let result = blubber_core::raw::indexed_map_key(&bytes, at, 1, stride, first);
    let valid = at < 8 && (first || (stride > 0 && stride <= at && bytes[at - stride] < bytes[at]));
    assert_eq!(result.is_ok(), valid);
    if let Ok(key) = result {
        assert_eq!(key, &bytes[at..at + 1]);
    }
    kani::cover!(result.is_ok());
    kani::cover!(result.is_err());
}

// Production mover, arbitrary full-width bytes, every tail and sub-word overlap.
fn chunk_move_move(right: bool) {
    let mut bytes: [u8; 32] = kani::any();
    let before = bytes;
    let tail: u8 = kani::any();
    let distance: u8 = kani::any();
    kani::assume(tail < 8 && distance >= 1 && distance <= 7);
    let count = 8 + tail as usize;
    let source = 4 + if right { 0 } else { distance as usize };
    let destination = 4 + if right { distance as usize } else { 0 };
    assert_eq!(
        blubber_core::raw::move_bytes(
            &mut bytes,
            blubber_core::Span {
                start: source as u32,
                end: (source + count) as u32,
            },
            destination as u32
        ),
        Ok(())
    );
    for i in 0..32 {
        let expected = if destination <= i && i < destination + count {
            before[source + i - destination]
        } else {
            before[i]
        };
        assert_eq!(bytes[i], expected);
    }
    kani::cover!(tail == 0 && distance == 1);
    kani::cover!(tail == 7 && distance == 7);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_chunk_move_move_left() {
    chunk_move_move(false);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_chunk_move_move_right() {
    chunk_move_move(true);
}

// Production mover, arbitrary full-width bytes, every tail and sub-word overlap.
fn fixed_map_remove_move(right: bool) {
    let mut bytes: [u8; 32] = kani::any();
    let before = bytes;
    let tail: u8 = kani::any();
    let distance: u8 = kani::any();
    kani::assume(tail < 8 && distance >= 1 && distance <= 7);
    let count = 8 + tail as usize;
    let source = 4 + if right { 0 } else { distance as usize };
    let destination = 4 + if right { distance as usize } else { 0 };
    assert_eq!(
        blubber_core::raw::fixed_removal_move_model(
            &mut bytes,
            blubber_core::Span {
                start: source as u32,
                end: (source + count) as u32,
            },
            destination as u32
        ),
        Ok(())
    );
    for i in 0..32 {
        let expected = if destination <= i && i < destination + count {
            before[source + i - destination]
        } else {
            before[i]
        };
        assert_eq!(bytes[i], expected);
    }
    kani::cover!(tail == 0 && distance == 1);
    kani::cover!(tail == 7 && distance == 7);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_fixed_map_remove_move_left() {
    fixed_map_remove_move(false);
}

#[kani::proof]
#[kani::unwind(33)]
fn record_fixed_map_remove_move_right() {
    fixed_map_remove_move(true);
}

#[blubber::schema(account)]
struct FixedMapRemoveMap {
    value: Map<u8, u8>,
}

#[kani::proof]
#[kani::unwind(33)]
fn record_fixed_map_remove_remove() {
    let payload: [u8; 5] = kani::any();
    let query: u8 = kani::any();
    let mut bytes = [
        1u8, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    bytes[16..].copy_from_slice(&payload);
    let before = bytes;
    let encoded_query = [query];
    let mut path = blubber_core::raw::InlinePath::new();
    path.push(blubber_core::raw::Step::Field(0)).unwrap();
    let loan = blubber_core::raw::prepare_tail_remove(
        &mut bytes,
        [1, 0, 0, 0, 0, 0, 0, 0],
        4,
        &map_stream_layout!(FixedMapRemoveMap),
        &path,
        0,
        Some(&encoded_query),
    )
    .unwrap()
    .unwrap();
    // The generic gateway checks the last key before the first, even on duplicates.
    let index = if query == payload[2] {
        Some(1usize)
    } else if query < payload[2] && query == payload[0] {
        Some(0usize)
    } else {
        None
    };
    let expected_span = index.map(|i| blubber_core::Span {
        start: 17 + 2 * i as u32,
        end: 18 + 2 * i as u32,
    });
    assert_eq!(loan.removed_span(), expected_span);
    loan.finish_fixed_map();
    let mut expected = before;
    if let Some(index) = index {
        word(&mut expected, 8, 18);
        word(&mut expected, 12, 1);
        let remaining = 1 - index;
        expected[16] = payload[2 * remaining];
        expected[17] = payload[2 * remaining + 1];
        expected[18] = 0;
        expected[19] = 0;
    }
    assert_eq!(bytes, expected);
    kani::cover!(index.is_some() && payload[0] >= payload[2]);
    kani::cover!(index.is_none());
}

// The actual checked ancestor pass, without a variable typed-graph claim.
fn ancestor_patch_ancestor(grow: bool) {
    let mut bytes: [u8; 24] = kani::any();
    let before = bytes;
    let upper: u32 = kani::any();
    let amount: u32 = kani::any();
    kani::assume(if grow {
        amount <= u32::MAX - upper
    } else {
        amount <= upper
    });
    let mut poison = false;
    let result =
        blubber_core::raw::__ancestor_pass_model(&mut bytes, grow, amount, upper, &mut poison);
    let mut expected = before;
    let mut failure = None;
    let mut done = 0usize;
    for i in 0..4 {
        if failure.is_none() {
            let at = 4 + 4 * i;
            let end =
                u32::from_le_bytes([before[at], before[at + 1], before[at + 2], before[at + 3]]);
            if end > upper {
                failure = Some(Error::new(ErrorKind::InvalidBoundary, at as u32));
            } else if !grow && end < amount {
                failure = Some(Error::new(ErrorKind::ArithmeticOverflow, at as u32));
            } else {
                let shifted = if grow {
                    end as u64 + amount as u64
                } else {
                    end as u64 - amount as u64
                };
                assert!(shifted <= u32::MAX as u64);
                expected[at..at + 4].copy_from_slice(&(shifted as u32).to_le_bytes());
                done += 1;
            }
        }
    }
    assert_eq!(
        result,
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    );
    assert_eq!(poison, failure.is_some());
    for i in 0..3 {
        let actual: u64 = u64::from_le_bytes(bytes[8 * i..8 * i + 8].try_into().unwrap());
        let oracle: u64 = u64::from_le_bytes(expected[8 * i..8 * i + 8].try_into().unwrap());
        assert_eq!(actual, oracle);
    }
    kani::cover!(failure.is_none() && amount > 0);
    kani::cover!(failure.is_some() && done == 3);
    kani::cover!(if grow {
        upper == u32::MAX && amount == 0 && failure.is_none()
    } else {
        amount == u32::MAX && done > 0
    });
}
#[kani::proof]
#[kani::unwind(25)]
fn record_ancestor_patch_ancestor_grow() {
    ancestor_patch_ancestor(true);
}
#[kani::proof]
#[kani::unwind(25)]
fn record_ancestor_patch_ancestor_shrink() {
    ancestor_patch_ancestor(false);
}
