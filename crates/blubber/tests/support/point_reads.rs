//! Retained geometry preserves point results and exact first faults.
#![forbid(unsafe_code)]

use blubber::{Address, Error, ErrorKind, List, Map, Schema, Str, View};

#[test]
fn point_offsets_and_full_width_indices() {
    let bytes = super::encoded::<List<u64>>(&&[7, 91, u64::MAX][..]);
    for layout in [
        *<List<u64>>::LAYOUT,
        blubber_core::Layout::Flat {
            checked: blubber::__layout_of!(List<u64>),
        },
    ] {
        let list = View::new(&bytes, layout, u32::MAX - bytes.len() as u32)
            .unwrap()
            .read::<List<u64>>()
            .unwrap();
        for (index, value) in [7, 91, u64::MAX].into_iter().enumerate() {
            assert_eq!(list.try_get(index), Ok(Some(value)));
            assert_eq!(list.try_get(index), list.try_iter().nth(index).transpose());
        }
        for index in [3, u32::MAX as usize, usize::MAX] {
            assert_eq!(list.try_get(index), Ok(None));
        }
        #[cfg(target_pointer_width = "64")]
        assert_eq!(list.try_get(1usize << 32), Ok(None));
    }
}

#[test]
fn point_domains_precede_equality_and_value_reads() {
    for byte in 0..=255u8 {
        for base in [0, 41, u32::MAX - 6] {
            let bytes = [1, 0, 0, 0, byte, byte];
            let map = View::new(&bytes, <Map<bool, bool>>::LAYOUT, base)
                .unwrap()
                .read::<Map<bool, bool>>()
                .unwrap();
            for query in [false, true] {
                let expected = if byte > 1 {
                    Err(Error::new(ErrorKind::InvalidBool, base + 4))
                } else if query == (byte == 1) {
                    Ok(Some(query))
                } else {
                    Ok(None)
                };
                assert_eq!(map.try_get(&query), expected);
                assert_eq!(map.try_contains_key(&query), expected.map(|v| v.is_some()));
            }
            let bytes = [1, 0, 0, 0, byte];
            let list = View::new(&bytes, <List<bool>>::LAYOUT, base)
                .unwrap()
                .read::<List<bool>>()
                .unwrap();
            assert_eq!(
                list.try_get(0),
                if byte <= 1 {
                    Ok(Some(byte == 1))
                } else {
                    Err(Error::new(ErrorKind::InvalidBool, base + 4))
                }
            );
            assert_eq!(list.try_get(usize::MAX), Ok(None));
        }
        let bytes = [1, 0, 0, 0, 1, byte];
        let map = View::new(&bytes, <Map<bool, bool>>::LAYOUT, 23)
            .unwrap()
            .read::<Map<bool, bool>>()
            .unwrap();
        assert_eq!(
            map.try_get(&true),
            if byte <= 1 {
                Ok(Some(byte == 1))
            } else {
                Err(Error::new(ErrorKind::InvalidBool, 28))
            }
        );
        assert_eq!(map.try_contains_key(&true), Ok(true));
    }
}

#[test]
fn point_foreign_layouts_keep_both_directions() {
    let fixed = super::encoded::<List<u16>>(&&[0x6261, 0x6463][..]);
    let list = View::new(&fixed, <List<u16>>::LAYOUT, 29)
        .unwrap()
        .read::<List<Str>>()
        .unwrap();
    assert_eq!(list.try_get(0), Ok(Some("ab")));
    assert_eq!(list.try_get(1), Ok(Some("cd")));
    let narrow = View::new(&fixed, <List<u16>>::LAYOUT, 29)
        .unwrap()
        .read::<List<u8>>()
        .unwrap();
    assert_eq!(
        narrow.try_get(0),
        Err(Error::new(ErrorKind::InvalidBoundary, 34))
    );
    let variable = super::encoded::<List<Str>>(&&["ab", "cd"][..]);
    let list = View::new(&variable, <List<Str>>::LAYOUT, 29)
        .unwrap()
        .read::<List<u16>>()
        .unwrap();
    assert_eq!(list.try_get(0), Ok(Some(0x6261)));
    assert_eq!(list.try_get(1), Ok(Some(0x6463)));
    let fixed = super::encoded::<Map<u8, u16>>(&&[(1, 0x6261), (3, 0x6463)][..]);
    let map = View::new(&fixed, <Map<u8, u16>>::LAYOUT, 29)
        .unwrap()
        .read::<Map<u8, Str>>()
        .unwrap();
    assert_eq!(map.try_get(&1), Ok(Some("ab")));
    assert_eq!(map.try_get(&3), Ok(Some("cd")));
    let variable = super::encoded::<Map<u8, Str>>(&&[(1, "ab"), (3, "cd")][..]);
    let map = View::new(&variable, <Map<u8, Str>>::LAYOUT, 29)
        .unwrap()
        .read::<Map<u8, u16>>()
        .unwrap();
    assert_eq!(map.try_get(&1), Ok(Some(0x6261)));
    assert_eq!(map.try_get(&3), Ok(Some(0x6463)));
}

#[test]
fn point_probe_equality_full_keys_halves_and_misses() {
    let mut seed = 0xe742_9081_f5ca_3d67u64;
    let mut keys = Vec::new();
    let count = if cfg!(miri) { 15 } else { 127 };
    for _ in 0..count {
        let mut key = [0u8; 32];
        for chunk in key.as_chunks_mut::<8>().0 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            chunk.copy_from_slice(&seed.to_le_bytes());
        }
        keys.push(key);
    }
    keys.sort();
    keys.dedup();
    let fixed: Vec<_> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (Address::new_from_array(*k), i as u64))
        .collect();
    let variable: Vec<_> = keys
        .iter()
        .map(|k| (Address::new_from_array(*k), "é"))
        .collect();
    let fixed_bytes = super::encoded::<Map<Address, u64>>(&fixed.as_slice());
    let variable_bytes = super::encoded::<Map<Address, Str>>(&variable.as_slice());
    let fixed = View::new(&fixed_bytes, <Map<Address, u64>>::LAYOUT, 19)
        .unwrap()
        .read::<Map<Address, u64>>()
        .unwrap();
    let variable = View::new(&variable_bytes, <Map<Address, Str>>::LAYOUT, 19)
        .unwrap()
        .read::<Map<Address, Str>>()
        .unwrap();
    for (i, key) in keys.iter().enumerate() {
        let query = Address::new_from_array(*key);
        assert_eq!(fixed.try_get(&query), Ok(Some(i as u64)));
        assert_eq!(variable.try_get(&query), Ok(Some("é")));
        for at in 0..32 {
            let mut absent = *key;
            absent[at] ^= 0x80;
            if keys.binary_search(&absent).is_err() {
                let query = Address::new_from_array(absent);
                assert_eq!(fixed.try_get(&query), Ok(None));
                assert_eq!(variable.try_get(&query), Ok(None));
            }
        }
    }
    assert_eq!(fixed.try_len(), Ok(keys.len()));
    assert_eq!(fixed.try_iter().count(), keys.len());
    assert_eq!(fixed.try_get(&Address::new_from_array([0; 32])), Ok(None));
    assert_eq!(fixed.try_get(&Address::new_from_array([255; 32])), Ok(None));
}

std::thread_local! {
    static PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProbeKey(u8);

impl blubber::Fixed for ProbeKey {
    const SIZE: u32 = 1;
    type Encoded = [u8; 1];
    fn decode(bytes: &[u8]) -> blubber::Result<Self> {
        <u8 as blubber::Fixed>::decode(bytes).map(Self)
    }
    fn encode(self) -> Self::Encoded {
        [self.0]
    }
}

impl blubber::MapKey for ProbeKey {
    const KEY_LAYOUT: blubber_core::KeyLayout = <u8 as blubber::MapKey>::KEY_LAYOUT;
    type KeyBytes = [u8; 1];
    fn encode_key(self) -> Self::KeyBytes {
        [self.0]
    }
    fn decode_key(bytes: &[u8]) -> blubber::Result<Self> {
        PROBES.with(|count| {
            count.set(count.get() + 1);
            assert!(count.get() <= 33, "point lookup exceeded its probe bound");
        });
        <u8 as blubber::MapKey>::decode_key(bytes).map(Self)
    }
}

#[test]
fn point_search_progress_and_once_per_probe_decoding() {
    let bytes = [3, 0, 0, 0, 1, 7, 3, 9, 5, 11];
    let map = View::new(&bytes, <Map<ProbeKey, u8>>::LAYOUT, 17)
        .unwrap()
        .read::<Map<ProbeKey, u8>>()
        .unwrap();
    for query in [0, 1, 2, 3, 4, 5, 6, 255] {
        PROBES.with(|count| count.set(0));
        assert_eq!(
            map.try_get(&ProbeKey(query)),
            Ok(match query {
                1 => Some(7),
                3 => Some(9),
                5 => Some(11),
                _ => None,
            })
        );
        PROBES.with(|count| assert_eq!(count.get(), if query == 3 { 2 } else { 3 }));
    }
}
