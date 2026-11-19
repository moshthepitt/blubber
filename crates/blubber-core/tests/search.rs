#![forbid(unsafe_code)]

//! Key/order/search gateways against slice order and an independent BTreeMap.

use std::{cmp::Ordering, collections::BTreeMap};

use blubber_core::{
    compare_keys, map_key, map_value, partition_map, search_map, validate_map_keys, Error,
    ErrorKind, KeyDomain, KeyLayout, Layout, Leaf, Search, Span,
};

static BYTE: Layout = Layout::Leaf(Leaf::Unsigned(1));
static BOOL: Layout = Layout::Leaf(Leaf::Bool);
static STR: Layout = Layout::Str;
static CYCLE: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Raw,
    },
    value: &CYCLE,
};

fn map(width: u32, domain: KeyDomain, value: &'static Layout) -> Layout {
    Layout::Map {
        key: KeyLayout { width, domain },
        value,
    }
}

fn fault(kind: ErrorKind, offset: u32) -> Error {
    Error::new(kind, offset)
}

fn encode(keys: &[Vec<u8>], indexed: bool) -> Vec<u8> {
    let mut bytes = (keys.len() as u32).to_le_bytes().to_vec();
    let mut payload = Vec::new();
    for (i, key) in keys.iter().enumerate() {
        bytes.extend_from_slice(key);
        if indexed {
            // Empty values exercise equal ends, including the terminal end.
            payload.extend(vec![0xff; i % 3]);
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        } else {
            bytes.push(0xff);
        }
    }
    bytes.extend_from_slice(&payload);
    bytes
}

fn check_lookup(bytes: &[u8], layout: &Layout, keys: &[Vec<u8>], query: &[u8]) {
    let model: BTreeMap<_, _> = keys
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, k)| (k, i))
        .collect();
    let found = model.get(query).copied();
    let index = found.unwrap_or_else(|| model.keys().take_while(|k| k.as_slice() < query).count());
    assert_eq!(
        search_map(bytes, layout, query),
        Ok(Search {
            index: index as u32,
            found: found.is_some(),
        }),
        "keys {keys:?}, query {query:?}"
    );
}

#[test]
fn exhaustive_byte_pairs_and_equal_prefixes_match_slice_order() {
    for a in 0..=255u8 {
        for b in 0..=255u8 {
            for (left, right) in [
                (vec![a], vec![b]),
                (vec![0, 255, a], vec![0, 255, b]),
                (vec![a, 0], vec![b, 255]),
                (vec![a], vec![b, 0]),
                (vec![a, 255], vec![b]),
            ] {
                assert_eq!(compare_keys(&left, &right), left.cmp(&right));
                assert_eq!(
                    compare_keys(&left, &right),
                    compare_keys(&right, &left).reverse()
                );
            }
        }
    }
    for a in [&[][..], &[0][..], &[255], &[0, 255]] {
        assert_eq!(compare_keys(a, a), Ordering::Equal);
        assert_eq!(compare_keys(&[], a), [].as_slice().cmp(a));
        assert_eq!(compare_keys(a, &[]), a.cmp(&[]));
    }
}

#[test]
fn arbitrary_lengths_totality_transitivity_and_long_prefixes() {
    let mut state = 0x9e37_79b9u32;
    let mut make = |len: usize| {
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect::<Vec<_>>()
    };
    for n in 0..10_000 {
        let a = make(n % 83);
        let b = make(n % 79);
        let c = make(n % 71);
        assert_eq!(compare_keys(&a, &b), a.cmp(&b));
        assert_eq!(compare_keys(&a, &b) == Ordering::Equal, a == b);
        if compare_keys(&a, &b).is_lt() && compare_keys(&b, &c).is_lt() {
            assert!(compare_keys(&a, &c).is_lt());
        }
    }
    let mut a = vec![0x80; 65_537];
    let mut b = a.clone();
    assert_eq!(compare_keys(&a, &b), Ordering::Equal);
    b.push(0);
    assert_eq!(compare_keys(&a, &b), Ordering::Less);
    a[65_536] = 0xff;
    assert_eq!(compare_keys(&a, &b), Ordering::Greater);
}

#[test]
fn exact_key_intervals_exclude_values_and_ends_on_unaligned_slices() {
    for width in [1, 2, 3, 16, 32] {
        for indexed in [false, true] {
            let layout = map(width, KeyDomain::Raw, if indexed { &STR } else { &BYTE });
            let keys: Vec<_> = (0..5).map(|i| vec![i; width as usize]).collect();
            let bytes = encode(&keys, indexed);
            let mut storage = vec![0xfa];
            storage.extend_from_slice(&bytes);
            storage.push(0xfb);
            let before = storage.clone();
            let slice = &storage[1..storage.len() - 1];
            assert_eq!(validate_map_keys(slice, &layout), Ok(()));
            for (i, key) in keys.iter().enumerate() {
                let span = map_key(slice, &layout, i as u32).unwrap();
                let start = 4 + i as u32 * (width + if indexed { 4 } else { 1 });
                assert_eq!(
                    span,
                    Span {
                        start,
                        end: start + width
                    }
                );
                assert_eq!(&slice[span.start as usize..span.end as usize], key);
                assert!(span.end <= map_value(slice, &layout, i as u32).unwrap().start);
                check_lookup(slice, &layout, &keys, key);
            }
            for i in [5, 6, u32::MAX] {
                assert_eq!(
                    map_key(slice, &layout, i),
                    Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
                );
            }
            assert_eq!(storage, before);
        }
    }
}

#[test]
fn every_small_ordered_subset_has_exact_found_and_absent_partitions() {
    for mask in 0..256u32 {
        let keys: Vec<_> = (0..8)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| vec![2 * i + 1])
            .collect();
        for indexed in [false, true] {
            let layout = map(1, KeyDomain::Raw, if indexed { &STR } else { &BYTE });
            let bytes = encode(&keys, indexed);
            assert_eq!(validate_map_keys(&bytes, &layout), Ok(()));
            for query in 0..=17 {
                check_lookup(&bytes, &layout, &keys, &[query]);
            }
        }
    }
}

#[test]
fn exhaustive_small_key_sequences_report_the_first_bad_adjacent_pair() {
    for count in 0..=5u32 {
        for mut code in 0..4usize.pow(count) {
            let keys: Vec<_> = (0..count)
                .map(|_| {
                    let key = vec![(code % 4) as u8];
                    code /= 4;
                    key
                })
                .collect();
            for indexed in [false, true] {
                let layout = map(1, KeyDomain::Raw, if indexed { &STR } else { &BYTE });
                let bytes = encode(&keys, indexed);
                let expected = keys.windows(2).enumerate().find_map(|(i, pair)| {
                    let kind = match pair[0].cmp(&pair[1]) {
                        Ordering::Less => return None,
                        Ordering::Equal => ErrorKind::DuplicateKey,
                        Ordering::Greater => ErrorKind::UnsortedKeys,
                    };
                    Some(fault(
                        kind,
                        4 + (i as u32 + 1) * if indexed { 5 } else { 2 },
                    ))
                });
                assert_eq!(
                    validate_map_keys(&bytes, &layout),
                    expected.map_or(Ok(()), Err)
                );
                for query in 0..=4 {
                    let result = search_map(&bytes, &layout, &[query]).unwrap();
                    assert!(result.index <= count);
                    if result.found {
                        assert_eq!(keys[result.index as usize], [query]);
                    }
                    if expected.is_none() {
                        check_lookup(&bytes, &layout, &keys, &[query]);
                    }
                }
            }
        }
    }
}

#[test]
fn bool_domains_and_query_widths_have_exact_faults() {
    for indexed in [false, true] {
        let layout = map(1, KeyDomain::Bool, if indexed { &STR } else { &BYTE });
        for key in 0..=255u8 {
            let keys = vec![vec![key]];
            let bytes = encode(&keys, indexed);
            assert!(map_key(&bytes, &layout, 0).is_ok());
            assert_eq!(
                validate_map_keys(&bytes, &layout),
                if key <= 1 {
                    Ok(())
                } else {
                    Err(fault(ErrorKind::InvalidBool, 4))
                }
            );
            assert_eq!(
                search_map(&bytes, &layout, &[key]),
                if key <= 1 {
                    Ok(Search {
                        index: 0,
                        found: true,
                    })
                } else {
                    Err(fault(ErrorKind::InvalidBool, Error::NO_OFFSET))
                }
            );
        }
        for keys in [vec![], vec![vec![0], vec![1]]] {
            let bytes = encode(&keys, indexed);
            for query in [vec![], vec![0, 0], vec![2, 2], vec![0; 33]] {
                assert_eq!(
                    search_map(&bytes, &layout, &query),
                    Err(fault(ErrorKind::InvalidBoundary, Error::NO_OFFSET))
                );
            }
            for query in [0, 1] {
                check_lookup(&bytes, &layout, &keys, &[query]);
            }
        }
        let stride = if indexed { 5 } else { 2 };
        for (keys, kind) in [
            (vec![vec![1], vec![2]], ErrorKind::InvalidBool),
            (vec![vec![1], vec![0]], ErrorKind::UnsortedKeys),
            (vec![vec![1], vec![1]], ErrorKind::DuplicateKey),
        ] {
            assert_eq!(
                validate_map_keys(&encode(&keys, indexed), &layout),
                Err(fault(kind, 4 + stride))
            );
        }
    }
}

#[test]
fn malformed_metadata_precedes_bytes_indices_and_query_faults() {
    for layout in [
        STR,
        BYTE,
        CYCLE,
        map(0, KeyDomain::Raw, &STR),
        map(3, KeyDomain::Unsigned, &STR),
        map(2, KeyDomain::Bool, &BYTE),
        map(1, KeyDomain::Raw, &Layout::Leaf(Leaf::Raw(0))),
    ] {
        let error = fault(ErrorKind::InvalidSchema, Error::NO_OFFSET);
        assert_eq!(validate_map_keys(&[], &layout), Err(error));
        assert_eq!(map_key(&[], &layout, u32::MAX).unwrap_err(), error);
        assert_eq!(search_map(&[], &layout, &[]).unwrap_err(), error);
    }
}

#[test]
fn partition_faults_precede_key_access_order_and_query_faults() {
    for indexed in [false, true] {
        let layout = map(2, KeyDomain::Raw, if indexed { &STR } else { &BYTE });
        let valid = encode(&[vec![0, 1], vec![1, 0], vec![255, 255]], indexed);
        let mut cases: Vec<_> = (0..valid.len()).map(|len| valid[..len].to_vec()).collect();
        let mut trailing = valid.clone();
        trailing.push(0);
        cases.push(trailing);
        for word in [0, 1, 2, u32::MAX / 4, u32::MAX] {
            let mut bad = valid.clone();
            bad[..4].copy_from_slice(&word.to_le_bytes());
            cases.push(bad);
            if indexed {
                for at in [6, 12, 18] {
                    let mut bad = valid.clone();
                    bad[at..at + 4].copy_from_slice(&word.to_le_bytes());
                    cases.push(bad);
                }
            }
        }
        for bytes in cases {
            if let Err(error) = partition_map(&bytes, &layout) {
                let before = bytes.clone();
                assert_eq!(validate_map_keys(&bytes, &layout), Err(error));
                assert_eq!(map_key(&bytes, &layout, u32::MAX), Err(error));
                assert_eq!(search_map(&bytes, &layout, &[]), Err(error));
                assert_eq!(bytes, before);
            }
        }
    }
}

#[test]
fn empty_maps_never_compute_unused_overflowing_strides() {
    for value in [&STR, &Layout::Leaf(Leaf::Raw(u32::MAX))] {
        let layout = map(u32::MAX, KeyDomain::Raw, value);
        assert_eq!(validate_map_keys(&[0; 4], &layout), Ok(()));
        assert_eq!(
            map_key(&[0; 4], &layout, 0),
            Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
        );
        assert_eq!(
            search_map(&[0; 4], &layout, &[]),
            Err(fault(ErrorKind::InvalidBoundary, Error::NO_OFFSET))
        );
        for count in [1u32, u32::MAX] {
            let bytes = count.to_le_bytes();
            assert_eq!(
                validate_map_keys(&bytes, &layout),
                Err(fault(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET))
            );
            assert_eq!(
                search_map(&bytes, &layout, &[]),
                Err(fault(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET))
            );
        }
    }
}

#[test]
fn numeric_sign_bias_and_big_endian_keys_use_byte_order() {
    for width in [1, 2, 4, 8, 16] {
        let max = if width == 16 {
            u128::MAX
        } else {
            (1u128 << (width * 8)) - 1
        };
        let values = [0, 1, 127, 128, 255, max / 2, max / 2 + 1, max - 1, max];
        let mut unsigned: Vec<_> = values.into_iter().filter(|v| *v <= max).collect();
        unsigned.sort_unstable();
        unsigned.dedup();
        let keys: Vec<_> = unsigned
            .iter()
            .map(|n| n.to_be_bytes()[16 - width..].to_vec())
            .collect();
        for indexed in [false, true] {
            let layout = map(
                width as u32,
                KeyDomain::Unsigned,
                if indexed { &STR } else { &BYTE },
            );
            let bytes = encode(&keys, indexed);
            assert_eq!(validate_map_keys(&bytes, &layout), Ok(()));
            for key in &keys {
                check_lookup(&bytes, &layout, &keys, key);
            }
        }
        let sign = 1u128 << (width * 8 - 1);
        let min = if width == 16 {
            i128::MIN
        } else {
            -(sign as i128)
        };
        let max = (sign - 1) as i128;
        let mut signed = vec![min, min + 1, -128, -1, 0, 1, 127, max - 1, max];
        signed.sort_unstable();
        signed.dedup();
        let keys: Vec<_> = signed
            .iter()
            .map(|n| ((*n as u128) ^ sign).to_be_bytes()[16 - width..].to_vec())
            .collect();
        for indexed in [false, true] {
            let layout = map(
                width as u32,
                KeyDomain::Signed,
                if indexed { &STR } else { &BYTE },
            );
            let bytes = encode(&keys, indexed);
            assert_eq!(validate_map_keys(&bytes, &layout), Ok(()));
            for key in &keys {
                check_lookup(&bytes, &layout, &keys, key);
            }
        }
    }
}

#[test]
fn key_validation_and_search_leave_child_validation_to_the_schema_walk() {
    let layout = map(1, KeyDomain::Raw, &BOOL);
    let bytes = encode(&[vec![0], vec![1]], false);
    assert_eq!(validate_map_keys(&bytes, &layout), Ok(()));
    assert_eq!(
        search_map(&bytes, &layout, &[0]),
        Ok(Search {
            index: 0,
            found: true
        })
    );
    let layout = map(1, KeyDomain::Raw, &STR);
    let bytes = encode(&[vec![0], vec![1], vec![2]], true);
    assert_eq!(validate_map_keys(&bytes, &layout), Ok(()));
    assert_eq!(
        search_map(&bytes, &layout, &[2]),
        Ok(Search {
            index: 2,
            found: true
        })
    );
}

#[test]
fn counts_above_u16_and_extreme_bytes_keep_exact_indices() {
    let count = u16::MAX as usize + 2;
    let keys: Vec<_> = (0..count as u32)
        .map(|i| i.to_be_bytes()[1..].to_vec())
        .collect();
    for indexed in [false, true] {
        let layout = map(3, KeyDomain::Raw, if indexed { &STR } else { &BYTE });
        let bytes = encode(&keys, indexed);
        assert_eq!(validate_map_keys(&bytes, &layout), Ok(()));
        for i in [0, 1, 32768, 65535, 65536] {
            assert_eq!(
                search_map(&bytes, &layout, &keys[i]),
                Ok(Search {
                    index: i as u32,
                    found: true
                })
            );
            assert_eq!(
                map_key(&bytes, &layout, i as u32).unwrap().start,
                4 + i as u32 * if indexed { 7 } else { 4 }
            );
        }
        assert_eq!(
            search_map(&bytes, &layout, &[255; 3]),
            Ok(Search {
                index: count as u32,
                found: false
            })
        );
    }
}

#[test]
fn strict32_every_differing_byte_and_key_frames() {
    for indexed in [false, true] {
        let layout = map(32, KeyDomain::Raw, if indexed { &STR } else { &BYTE });
        for position in 0..32 {
            for (low, high) in [(0, 1), (0x7f, 0x80), (0xfe, 0xff), (1, 0xff)] {
                // The suffix orders in the opposite direction to the first difference.
                let mut left = vec![0xff; 32];
                let mut right = vec![0; 32];
                left[..position].fill(0xa5);
                right[..position].fill(0xa5);
                left[position] = low;
                right[position] = high;
                let keys = [left.clone(), right.clone()];
                let bytes = encode(&keys, indexed);
                assert_eq!(validate_map_keys(&bytes, &layout), Ok(()));
                // An unaligned input and changed values cannot change key order.
                let mut framed = vec![0x77];
                framed.extend_from_slice(&bytes);
                assert_eq!(validate_map_keys(&framed[1..], &layout), Ok(()));
                if !indexed {
                    framed[1 + 4 + 32] = 0;
                    framed[1 + 4 + 33 + 32] = 0;
                    assert_eq!(validate_map_keys(&framed[1..], &layout), Ok(()));
                }
                let reversed = encode(&[right.clone(), left.clone()], indexed);
                let offset = 4 + 32 + if indexed { 4 } else { 1 };
                assert_eq!(
                    validate_map_keys(&reversed, &layout),
                    Err(fault(ErrorKind::UnsortedKeys, offset))
                );
                let duplicate = encode(&[left.clone(), left], indexed);
                assert_eq!(
                    validate_map_keys(&duplicate, &layout),
                    Err(fault(ErrorKind::DuplicateKey, offset))
                );
            }
        }
    }
}

#[test]
fn strict32_empty_singleton_and_first_faults() {
    for indexed in [false, true] {
        let layout = map(32, KeyDomain::Raw, if indexed { &STR } else { &BYTE });
        assert_eq!(validate_map_keys(&encode(&[], indexed), &layout), Ok(()));
        assert_eq!(
            validate_map_keys(&encode(&[vec![0xff; 32]], indexed), &layout),
            Ok(())
        );
        let key = |last| {
            let mut bytes = vec![0x91; 32];
            bytes[31] = last;
            bytes
        };
        let stride = 32 + if indexed { 4 } else { 1 };
        for (keys, kind, index) in [
            (
                vec![key(0), key(1), key(1), key(0)],
                ErrorKind::DuplicateKey,
                2,
            ),
            (
                vec![key(0), key(2), key(1), key(1)],
                ErrorKind::UnsortedKeys,
                2,
            ),
            (
                vec![key(1), key(1), key(0), key(0)],
                ErrorKind::DuplicateKey,
                1,
            ),
            (
                vec![key(2), key(1), key(1), key(0)],
                ErrorKind::UnsortedKeys,
                1,
            ),
        ] {
            assert_eq!(
                validate_map_keys(&encode(&keys, indexed), &layout),
                Err(fault(kind, 4 + index * stride))
            );
        }
    }
    // Empty partitions admit a huge declared stride without forming its sum.
    static HUGE: Layout = Layout::Leaf(Leaf::Raw(u32::MAX));
    let layout = map(32, KeyDomain::Raw, &HUGE);
    assert_eq!(validate_map_keys(&[0; 4], &layout), Ok(()));
}

/// Local word probes retain scalar order at every byte and every key width.
#[test]
fn local_word_search_matches_scalar_order_and_fallback() {
    use blubber_core::search_map_local;
    let mut state = 0xd18a_912c_4e73_b205u64;
    for width in [1, 2, 3, 4, 7, 8, 16, 31, 32, 33, 64] {
        for differing in 0..width {
            let prefix: Vec<u8> = (0..width)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                })
                .collect();
            let keys: Vec<Vec<u8>> = [1, 3, 5, 7, 9]
                .into_iter()
                .map(|byte| {
                    let mut key = prefix.clone();
                    key[differing] = byte;
                    // The first different byte decides even when every later byte
                    // would put the keys in the reverse order.
                    key[differing + 1..].fill(255 - byte);
                    key
                })
                .collect();
            for indexed in [false, true] {
                let layout = map(
                    width as u32,
                    KeyDomain::Raw,
                    if indexed { &STR } else { &BYTE },
                );
                for count in [0, 1, 2, 5] {
                    let bytes = encode(&keys[..count], indexed);
                    for byte in 0..=10 {
                        let mut query = prefix.clone();
                        query[differing] = byte;
                        query[differing + 1..].fill(255 - byte);
                        let index = keys[..count].partition_point(|key| key < &query);
                        let expected = Search {
                            index: index as u32,
                            found: keys
                                .get(index)
                                .is_some_and(|key| index < count && key == &query),
                        };
                        assert_eq!(
                            search_map_local(&bytes, &layout, &query),
                            Ok(expected),
                            "width={width} differing={differing} count={count} byte={byte}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn local_word_search_random_keys_and_exact_errors() {
    use blubber_core::search_map_local;
    let mut state = 0x284a_e07b_913c_265du64;
    let mut key = || -> Vec<u8> {
        (0..32)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect()
    };
    let mut keys: Vec<_> = (0..67).map(|_| key()).collect();
    keys.sort();
    keys.dedup();
    let queries: Vec<_> = keys
        .iter()
        .cloned()
        .chain((0..100).map(|_| key()))
        .collect();
    for indexed in [false, true] {
        let layout = map(32, KeyDomain::Raw, if indexed { &STR } else { &BYTE });
        let bytes = encode(&keys, indexed);
        for query in &queries {
            let index = keys.partition_point(|stored| stored < query);
            assert_eq!(
                search_map_local(&bytes, &layout, query),
                Ok(Search {
                    index: index as u32,
                    found: keys.get(index) == Some(query),
                })
            );
        }
        for len in [0, 1, 31, 33] {
            assert_eq!(
                search_map_local(&bytes, &layout, &vec![0; len]),
                Err(fault(ErrorKind::InvalidBoundary, Error::NO_OFFSET))
            );
        }
        assert_eq!(
            search_map_local(&[0, 0, 0], &layout, &[0; 32]),
            Err(fault(ErrorKind::Truncated, 0))
        );
    }
    let layout = map(1, KeyDomain::Bool, &BYTE);
    for query in [2, 255] {
        assert_eq!(
            search_map_local(&[0, 0, 0, 0], &layout, &[query]),
            Err(fault(ErrorKind::InvalidBool, Error::NO_OFFSET))
        );
    }
    // Last endpoint is checked first. Domain faults retain that key's offset.
    assert_eq!(
        search_map_local(&[2, 0, 0, 0, 0, 8, 2, 9], &layout, &[0]),
        Err(fault(ErrorKind::InvalidBool, 6))
    );
    assert_eq!(
        search_map_local(&[2, 0, 0, 0, 2, 8, 1, 9], &layout, &[0]),
        Err(fault(ErrorKind::InvalidBool, 4))
    );
}
