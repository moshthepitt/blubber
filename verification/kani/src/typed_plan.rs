//! Tractable production fixed-map edits and corrupted early rejection paths.

#[path = "../../../crates/blubber/src/list_bounds.rs"]
mod bounds;

blubber::__root_schema!(__ConcreteRoot0, Map<u8, u8>);
blubber::__root_schema!(__ConcreteRoot1, Map<u8, [u8; 32]>);

use blubber::raw::Edit;
use blubber::Resize as _;
use blubber::{Error, ErrorKind, Map, SliceStore};

const ID: [u8; 8] = *b"mapproof";

// All wire consumers lend a 32-byte physical account; admission separately covers 64.
fn wire(entries: &[(u8, u8)]) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..8].copy_from_slice(&ID);
    bytes[8..12].copy_from_slice(&(16 + 2 * entries.len() as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&(entries.len() as u32).to_le_bytes());
    for (i, &(key, value)) in entries.iter().enumerate() {
        bytes[16 + 2 * i] = key;
        bytes[17 + 2 * i] = value;
    }
    bytes
}

fn fixed<const N: usize, const KEY: u8, const OP: u8>() {
    let values: [u8; 2] = kani::any();
    let value: u8 = kani::any();
    let initial = [(2, values[0]), (4, values[1])];
    let mut bytes = wire(&initial[..N]);
    let result;
    {
        let mut store = SliceStore::new(&mut bytes[..32]);
        let mut map = blubber::raw::project::<Map<u8, u8>>(
            Edit::load_trusted::<__ConcreteRoot0>(&mut store, ID).unwrap(),
        );
        result = match OP {
            0 => map.insert(KEY, value),
            1 => map.remove(&KEY),
            _ => map.clear().map(|_| None),
        };
    }
    let mut expected = [(0, 0); 3];
    let mut count = 0;
    let mut old = None;
    let mut inserted = false;
    if OP != 2 {
        for &(key, prior) in &initial[..N] {
            if OP == 0 && !inserted && KEY < key {
                expected[count] = (KEY, value);
                count += 1;
                inserted = true;
            }
            if key == KEY {
                old = Some(prior);
                if OP == 0 {
                    expected[count] = (KEY, value);
                    count += 1;
                    inserted = true;
                }
            } else {
                expected[count] = (key, prior);
                count += 1;
            }
        }
        if OP == 0 && !inserted {
            expected[count] = (KEY, value);
            count += 1;
        }
    }
    assert_eq!(result, Ok(old));
    let oracle = wire(&expected[..count]);
    // Four little-endian words compare every physical byte without a 32-step loop.
    for word in 0..4 {
        let at = word * 8;
        let actual: [u8; 8] = bytes[at..at + 8].try_into().unwrap();
        let expected: [u8; 8] = oracle[at..at + 8].try_into().unwrap();
        assert_eq!(u64::from_le_bytes(actual), u64::from_le_bytes(expected));
    }
    kani::cover!(true, "fixed map edit returns");
}

fn admission<const CAP: usize>() {
    let bytes: [u8; CAP] = kani::any();
    let mut storage = bytes;
    {
        let mut store = SliceStore::new(&mut storage);
        let result = Edit::load_trusted::<__ConcreteRoot0>(&mut store, ID);
        if CAP < 12 {
            assert!(matches!(result, Err(e) if e == Error::new(ErrorKind::Truncated, 0)));
        } else {
            let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
            let identity = bytes[..8] == ID;
            let valid = identity && (16..=CAP as u32).contains(&used);
            assert_eq!(result.is_ok(), valid);
        }
    }
    assert_eq!(storage, bytes);
    kani::cover!(true, "corrupted admission returns");
}

#[kani::proof]
#[kani::unwind(65)]
fn typed_plan_corrupted_count_32() {
    let count: u32 = kani::any();
    kani::assume(count > 24);
    let mut bytes = wire(&[]);
    bytes[8..12].copy_from_slice(&32u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&count.to_le_bytes());
    let before = bytes;
    {
        let mut store = SliceStore::new(&mut bytes[..32]);
        {
            let map = blubber::raw::project::<Map<u8, u8>>(
                Edit::load_trusted::<__ConcreteRoot0>(&mut store, ID).unwrap(),
            );
            assert!(map.try_len().is_err());
        }
        assert!(!store.is_poisoned());
    }
    assert_eq!(bytes, before);
    kani::cover!(true, "corrupted count rejected");
}

include!(concat!(env!("OUT_DIR"), "/typed_plan_cases.rs"));

#[kani::proof]
#[kani::unwind(65)]
fn typed_plan_capacity_32() {
    let mut bytes = wire(&[]);
    let before = bytes;
    {
        let mut store = SliceStore::new(&mut bytes[..32]);
        {
            let mut map = blubber::raw::project::<Map<u8, [u8; 32]>>(
                Edit::load_trusted::<__ConcreteRoot1>(&mut store, ID).unwrap(),
            );
            assert_eq!(map.insert(2, [0; 32]), Err(ErrorKind::Capacity.into()));
        }
        assert!(!store.is_poisoned());
    }
    assert_eq!(bytes, before);
    kani::cover!(true, "capacity rejects before writes");
}

#[kani::proof]
#[kani::unwind(65)]
fn typed_plan_index_full_width() {
    let index: usize = kani::any();
    let count: u32 = kani::any();
    assert_eq!(
        bounds::truncate_len(index, count) as usize,
        index.min(count as usize)
    );
    let result = bounds::checked_list_index(index, count, false);
    let valid = index < count as usize;
    assert_eq!(result.is_ok(), valid);
    match result {
        Ok(ordinal) => assert_eq!(ordinal as usize, index),
        Err(error) => assert_eq!(error, ErrorKind::IndexOutOfBounds.into()),
    }
    kani::cover!(valid, "full-width index accepts valid selections");
}
