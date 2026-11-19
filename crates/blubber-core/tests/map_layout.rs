#![forbid(unsafe_code)]

//! Map partitions and exact value intervals against independent byte models.

use blubber_core::{
    map_value, partition_map, Error, ErrorKind, FixedField, KeyDomain, KeyLayout, Layout, Leaf,
    MapPartition, Span,
};

static BYTE: Layout = Layout::Leaf(Leaf::Unsigned(1));
static BOOL: Layout = Layout::Leaf(Leaf::Bool);
static WORD: Layout = Layout::Leaf(Leaf::Signed(2));
static STR: Layout = Layout::Str;
static OPTION: Layout = Layout::Option { value: &STR };
static BYTES: Layout = Layout::List { element: &BYTE };
static PAIR: Layout = Layout::Struct {
    fixed_bytes: 3,
    fixed: &[
        FixedField {
            declaration: 0,
            offset: 0,
            layout: &BOOL,
        },
        FixedField {
            declaration: 1,
            offset: 1,
            layout: &WORD,
        },
    ],
    variable: &[],
};
static CYCLE: Layout = Layout::Map {
    key: KeyLayout {
        width: 1,
        domain: KeyDomain::Raw,
    },
    value: &CYCLE,
};

fn map(key_width: u32, value: &'static Layout) -> Layout {
    Layout::Map {
        key: KeyLayout {
            width: key_width,
            domain: KeyDomain::Raw,
        },
        value,
    }
}

fn fault(kind: ErrorKind, offset: u32) -> Error {
    Error::new(kind, offset)
}

fn indexed(key_width: usize, ends: &[u32], payload: &[u8]) -> Vec<u8> {
    let mut bytes = (ends.len() as u32).to_le_bytes().to_vec();
    for (i, end) in ends.iter().enumerate() {
        bytes.extend(vec![i as u8; key_width]);
        bytes.extend_from_slice(&end.to_le_bytes());
    }
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn empty_maps_require_only_the_count_even_with_unrepresentable_unused_strides() {
    for layout in [
        map(1, &BYTE),
        map(2, &STR),
        map(32, &PAIR),
        map(u32::MAX, &STR),
        map(u32::MAX, &Layout::Leaf(Leaf::Raw(u32::MAX))),
    ] {
        let partition = partition_map(&[0; 4], &layout).unwrap();
        assert_eq!(partition.count, 0);
        assert_eq!(partition.directory, Span { start: 4, end: 4 });
        assert_eq!(partition.payload, Span { start: 4, end: 4 });
        for index in [0, 1, u16::MAX as u32, u32::MAX] {
            assert_eq!(
                map_value(&[0; 4], &layout, index),
                Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
            );
        }
        for len in 0..4 {
            assert_eq!(
                partition_map(&[0; 4][..len], &layout),
                Err(fault(ErrorKind::Truncated, 0))
            );
        }
        for len in 5..=10 {
            assert_eq!(
                partition_map(&[0; 10][..len], &layout),
                Err(fault(ErrorKind::InvalidBoundary, 4))
            );
        }
    }
}

#[test]
fn fixed_maps_interleave_exact_key_and_value_strides() {
    for key_width in [1, 2, 3, 16, 32] {
        for (value, width) in [(&BYTE, 1), (&WORD, 2), (&PAIR, 3)] {
            let layout = map(key_width, value);
            let stride = key_width + width;
            for count in 0..=5u32 {
                let expected = 4 + count * stride;
                for len in 4..=expected + stride + 1 {
                    let mut bytes = vec![0xff; len as usize];
                    bytes[..4].copy_from_slice(&count.to_le_bytes());
                    let result = partition_map(&bytes, &layout);
                    if len < expected {
                        assert_eq!(result, Err(fault(ErrorKind::Truncated, 4)));
                    } else if len > expected {
                        assert_eq!(result, Err(fault(ErrorKind::InvalidBoundary, expected)));
                    } else {
                        assert_eq!(
                            result,
                            Ok(MapPartition {
                                count,
                                key_width,
                                value_width: Some(width),
                                directory: Span { start: 4, end: 4 },
                                payload: Span {
                                    start: 4,
                                    end: expected,
                                },
                            })
                        );
                        for i in 0..count {
                            assert_eq!(
                                map_value(&bytes, &layout, i),
                                Ok(Span {
                                    start: 4 + i * stride + key_width,
                                    end: 4 + (i + 1) * stride,
                                })
                            );
                        }
                        assert_eq!(
                            map_value(&bytes, &layout, count),
                            Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn indexed_maps_always_store_the_terminal_end_including_singletons() {
    for key_width in [1, 2, 3, 32] {
        let layout = map(key_width, &STR);
        let p = 4 + key_width + 4;
        for payload in [&[][..], &[0xff][..], b"abcdef" as &[u8]] {
            let one = indexed(key_width as usize, &[payload.len() as u32], payload);
            assert_eq!(
                partition_map(&one, &layout),
                Ok(MapPartition {
                    count: 1,
                    key_width,
                    value_width: None,
                    directory: Span { start: 4, end: p },
                    payload: Span {
                        start: p,
                        end: one.len() as u32,
                    },
                })
            );
            assert_eq!(
                map_value(&one, &layout, 0),
                Ok(Span {
                    start: p,
                    end: one.len() as u32,
                })
            );
            for cut in 0..=payload.len() as u32 {
                let two = indexed(key_width as usize, &[cut, payload.len() as u32], payload);
                let p = 4 + 2 * (key_width + 4);
                assert_eq!(
                    map_value(&two, &layout, 0),
                    Ok(Span {
                        start: p,
                        end: p + cut,
                    })
                );
                assert_eq!(
                    map_value(&two, &layout, 1),
                    Ok(Span {
                        start: p + cut,
                        end: two.len() as u32,
                    })
                );
            }
        }
    }
}

#[test]
fn worked_u16_string_map_matches_the_spec_bytes() {
    let bytes = [2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, b'x'];
    let layout = Layout::Map {
        key: KeyLayout {
            width: 2,
            domain: KeyDomain::Unsigned,
        },
        value: &STR,
    };
    let before = bytes;
    assert_eq!(
        partition_map(&bytes, &layout).unwrap().payload,
        Span { start: 16, end: 17 }
    );
    assert_eq!(
        map_value(&bytes, &layout, 0),
        Ok(Span { start: 16, end: 17 })
    );
    assert_eq!(
        map_value(&bytes, &layout, 1),
        Ok(Span { start: 17, end: 17 })
    );
    assert_eq!(bytes, before);
}

#[test]
fn small_directories_match_independent_interval_and_terminal_rules() {
    for key_width in [1, 3] {
        let layout = map(key_width, &STR);
        for count in 1..=4u32 {
            for len in 0..=4u32 {
                for mut digits in 0..(len + 2).pow(count) {
                    let mut ends = Vec::new();
                    for _ in 0..count {
                        ends.push(digits % (len + 2));
                        digits /= len + 2;
                    }
                    let bytes = indexed(key_width as usize, &ends, &vec![0xff; len as usize]);
                    let mut cuts = vec![0];
                    cuts.extend_from_slice(&ends);
                    let valid =
                        cuts.windows(2).all(|pair| pair[0] <= pair[1]) && ends.last() == Some(&len);
                    let partition = partition_map(&bytes, &layout);
                    assert_eq!(partition.is_ok(), valid, "ends {ends:?}, payload {len}");
                    for (index, pair) in cuts.windows(2).enumerate() {
                        let result = map_value(&bytes, &layout, index as u32);
                        if valid {
                            let p = partition.unwrap().payload.start;
                            assert_eq!(
                                result,
                                Ok(Span {
                                    start: p + pair[0],
                                    end: p + pair[1],
                                })
                            );
                        } else {
                            assert_eq!(result.unwrap_err(), partition.unwrap_err());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn short_directories_identify_the_first_incomplete_record() {
    for key_width in [1, 2, 3, 32] {
        let layout = map(key_width, &STR);
        for count in 1..=5usize {
            let bytes = indexed(key_width as usize, &vec![0; count], &[]);
            for len in 0..bytes.len() {
                let at = if len < 4 {
                    0
                } else {
                    4 + (len - 4) / (key_width as usize + 4) * (key_width as usize + 4)
                };
                assert_eq!(
                    partition_map(&bytes[..len], &layout),
                    Err(fault(ErrorKind::Truncated, at as u32))
                );
            }
        }
    }
}

#[test]
fn corrupted_ends_and_inexact_terminal_ends_report_their_word() {
    let layout = map(2, &STR);
    for ends in [
        [4, 0, 0],
        [0, u32::MAX, 0],
        [0, 0, 4],
        [2, 1, u32::MAX],
        [0, 0, 0],
        [1, 2, 2],
    ] {
        let bytes = indexed(2, &ends, b"abc");
        let first = ends
            .iter()
            .enumerate()
            .find(|(i, end)| **end > 3 || (*i > 0 && **end < ends[*i - 1]))
            .map(|(i, _)| i)
            .unwrap_or(ends.len() - 1);
        let error = fault(ErrorKind::InvalidBoundary, 4 + first as u32 * 6 + 2);
        assert_eq!(partition_map(&bytes, &layout), Err(error));
        for index in [0, 1, 2, 3, u32::MAX] {
            assert_eq!(map_value(&bytes, &layout, index), Err(error));
        }
    }
}

#[test]
fn full_width_stride_product_and_header_overflow_precede_truncation() {
    for key_width in [1, 2, 16, u32::MAX - 4, u32::MAX - 3, u32::MAX] {
        for value_width in [None, Some(1), Some(16), Some(u32::MAX)] {
            let value = match value_width {
                None => &STR,
                Some(w) => Box::leak(Box::new(Layout::Leaf(Leaf::Raw(w)))),
            };
            let layout = map(key_width, value);
            let stride = u128::from(key_width) + u128::from(value_width.unwrap_or(4));
            let last_count = ((u128::from(u32::MAX) - 4) / stride) as u32;
            for count in [0, 1, last_count, last_count + 1, u32::MAX] {
                let extent = 4 + u128::from(count) * stride;
                let result = partition_map(&count.to_le_bytes(), &layout);
                if extent > u128::from(u32::MAX) {
                    assert_eq!(
                        result,
                        Err(fault(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET))
                    );
                } else if count == 0 {
                    assert!(result.is_ok());
                } else {
                    assert_eq!(result, Err(fault(ErrorKind::Truncated, 4)));
                }
            }
        }
    }
}

#[test]
fn metadata_precedes_bytes_and_invalid_indices() {
    let invalid = fault(ErrorKind::InvalidSchema, Error::NO_OFFSET);
    for layout in [
        STR,
        BYTE,
        BYTES,
        map(0, &STR),
        map(1, &CYCLE),
        map(1, &Layout::Leaf(Leaf::Raw(0))),
        map(1, &Layout::Leaf(Leaf::Unsigned(3))),
        Layout::Map {
            key: KeyLayout {
                width: 3,
                domain: KeyDomain::Unsigned,
            },
            value: &STR,
        },
        Layout::Map {
            key: KeyLayout {
                width: 2,
                domain: KeyDomain::Bool,
            },
            value: &BYTE,
        },
    ] {
        assert_eq!(partition_map(&[], &layout), Err(invalid));
        assert_eq!(map_value(&[], &layout, u32::MAX), Err(invalid));
    }
}

#[test]
fn key_domains_order_and_child_bytes_are_separate_from_partitioning() {
    let bool_keys = Layout::Map {
        key: KeyLayout {
            width: 1,
            domain: KeyDomain::Bool,
        },
        value: &BOOL,
    };
    // Invalid key/value bools and duplicate/descending keys remain structural.
    for keys in [[0xff, 0xff], [1, 0], [0, 0]] {
        let bytes = [2, 0, 0, 0, keys[0], 0xff, keys[1], 0xff];
        assert!(partition_map(&bytes, &bool_keys).is_ok());
    }
    for (value, payload) in [(&STR, &[0xff][..]), (&BYTES, &[0xff]), (&OPTION, &[0xff])] {
        let layout = map(1, value);
        assert!(partition_map(&indexed(1, &[1], payload), &layout).is_ok());
    }
}

#[test]
fn full_u32_indices_and_unaligned_slices_preserve_exact_regions() {
    let count = u16::MAX as u32 + 2;
    for value in [&BYTE, &STR] {
        let layout = map(3, value);
        let bytes = if matches!(value, Layout::Str) {
            indexed(3, &vec![0; count as usize], &[])
        } else {
            let mut bytes = count.to_le_bytes().to_vec();
            bytes.extend(vec![0xff; count as usize * 4]);
            bytes
        };
        let mut storage = vec![0xfa];
        storage.extend_from_slice(&bytes);
        storage.extend_from_slice(&[0xfb, 0xfc]);
        let before = storage.clone();
        let slice = &storage[1..1 + bytes.len()];
        let partition = partition_map(slice, &layout).unwrap();
        assert_eq!(partition.count, count);
        for index in [0, 1, u16::MAX as u32, count - 1] {
            let span = map_value(slice, &layout, index).unwrap();
            if matches!(value, Layout::Str) {
                assert_eq!(span, partition.payload);
            } else {
                assert_eq!(span.start, 4 + index * 4 + 3);
                assert_eq!(span.end, 4 + (index + 1) * 4);
            }
        }
        assert_eq!(
            map_value(slice, &layout, count),
            Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
        );
        assert_eq!(storage, before);
    }
}
