#![forbid(unsafe_code)]

//! Exact list partitions and element regions against independent byte models.

use blubber_core::{
    list_element, partition_list, Error, ErrorKind, FixedField, KeyDomain, KeyLayout, Layout, Leaf,
    ListPartition, Span,
};

static BYTE: Layout = Layout::Leaf(Leaf::Unsigned(1));
static BOOL: Layout = Layout::Leaf(Leaf::Bool);
static WORD: Layout = Layout::Leaf(Leaf::Signed(2));
static STR: Layout = Layout::Str;
static BYTES: Layout = Layout::List { element: &BYTE };
static STRINGS: Layout = Layout::List { element: &STR };
static OPTION: Layout = Layout::Option { value: &STR };
static BAD_MAP: Layout = Layout::Map {
    key: KeyLayout {
        width: 0,
        domain: KeyDomain::Raw,
    },
    value: &STR,
};
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
static CYCLE: Layout = Layout::List { element: &CYCLE };

fn fault(kind: ErrorKind, offset: u32) -> Error {
    Error::new(kind, offset)
}

fn indexed(count: u32, ends: &[u32], payload: &[u8]) -> Vec<u8> {
    let mut bytes = count.to_le_bytes().to_vec();
    for end in ends {
        bytes.extend_from_slice(&end.to_le_bytes());
    }
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn empty_lists_have_only_a_count_and_no_elements() {
    for layout in [&BYTES, &STRINGS, &Layout::List { element: &PAIR }] {
        let partition = partition_list(&[0; 4], layout).unwrap();
        assert_eq!(partition.count, 0);
        assert_eq!(partition.directory, Span { start: 4, end: 4 });
        assert_eq!(partition.payload, Span { start: 4, end: 4 });
        for index in [0, 1, u16::MAX as u32, u32::MAX] {
            assert_eq!(
                list_element(&[0; 4], layout, index),
                Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
            );
        }
        for len in 5..=9 {
            assert_eq!(
                partition_list(&[0; 9][..len], layout),
                Err(fault(ErrorKind::InvalidBoundary, 4))
            );
        }
        for len in 0..4 {
            assert_eq!(
                partition_list(&[0; 4][..len], layout),
                Err(fault(ErrorKind::Truncated, 0))
            );
        }
    }
}

#[test]
fn fixed_stride_requires_exact_count_times_width() {
    for element in [
        Layout::Leaf(Leaf::Unsigned(1)),
        Layout::Leaf(Leaf::Unsigned(2)),
        Layout::Leaf(Leaf::Unsigned(4)),
        Layout::Leaf(Leaf::Unsigned(8)),
        Layout::Leaf(Leaf::Unsigned(16)),
        Layout::Leaf(Leaf::Raw(32)),
        PAIR,
    ] {
        let width = match element {
            Layout::Leaf(Leaf::Unsigned(w)) => w as usize,
            Layout::Leaf(Leaf::Raw(w)) => w as usize,
            _ => 3,
        };
        let layout = Layout::List {
            element: Box::leak(Box::new(element)),
        };
        for count in 0..=5u32 {
            let expected = 4 + count as usize * width;
            for len in 4..=expected + width + 1 {
                let mut bytes = vec![0xff; len];
                bytes[..4].copy_from_slice(&count.to_le_bytes());
                let result = partition_list(&bytes, &layout);
                if len < expected {
                    assert_eq!(result, Err(fault(ErrorKind::Truncated, 4)));
                } else if len > expected {
                    assert_eq!(
                        result,
                        Err(fault(ErrorKind::InvalidBoundary, expected as u32))
                    );
                } else {
                    assert_eq!(
                        result,
                        Ok(ListPartition {
                            count,
                            element_width: Some(width as u32),
                            directory: Span { start: 4, end: 4 },
                            payload: Span {
                                start: 4,
                                end: expected as u32
                            },
                        })
                    );
                    for i in 0..count {
                        assert_eq!(
                            list_element(&bytes, &layout, i),
                            Ok(Span {
                                start: 4 + i * width as u32,
                                end: 4 + (i + 1) * width as u32,
                            })
                        );
                    }
                    assert_eq!(
                        list_element(&bytes, &layout, count),
                        Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
                    );
                }
            }
        }
    }
}

#[test]
fn indexed_counts_one_and_two_elide_the_terminal_end() {
    for payload in [&[][..], &[0xff][..], b"abcdef" as &[u8]] {
        let one = indexed(1, &[], payload);
        let partition = partition_list(&one, &STRINGS).unwrap();
        assert_eq!(partition.element_width, None);
        assert_eq!(partition.directory, Span { start: 4, end: 4 });
        assert_eq!(list_element(&one, &STRINGS, 0), Ok(partition.payload));
        for cut in 0..=payload.len() {
            let two = indexed(2, &[cut as u32], payload);
            let partition = partition_list(&two, &STRINGS).unwrap();
            assert_eq!(partition.count, 2);
            assert_eq!(partition.directory, Span { start: 4, end: 8 });
            assert_eq!(
                list_element(&two, &STRINGS, 0),
                Ok(Span {
                    start: 8,
                    end: (8 + cut) as u32
                })
            );
            assert_eq!(
                list_element(&two, &STRINGS, 1),
                Ok(Span {
                    start: (8 + cut) as u32,
                    end: two.len() as u32
                })
            );
        }
    }
    let example = indexed(3, &[2, 2], b"abxyz");
    for (index, expected) in [(12, 14), (14, 14), (14, 17)].into_iter().enumerate() {
        assert_eq!(
            list_element(&example, &STRINGS, index as u32),
            Ok(Span {
                start: expected.0,
                end: expected.1,
            })
        );
    }
}

#[test]
fn small_directories_match_independent_interval_partitioning() {
    for count in 2..=4u32 {
        for len in 0..=5u32 {
            let possibilities = (len + 2).pow(count - 1);
            for mut digits in 0..possibilities {
                let mut ends = Vec::new();
                for _ in 1..count {
                    ends.push(digits % (len + 2));
                    digits /= len + 2;
                }
                let bytes = indexed(count, &ends, &vec![0xff; len as usize]);
                let mut cuts = vec![0];
                cuts.extend_from_slice(&ends);
                cuts.push(len);
                let valid = cuts.windows(2).all(|pair| pair[0] <= pair[1]);
                let partition = partition_list(&bytes, &STRINGS);
                assert_eq!(partition.is_ok(), valid, "cuts {cuts:?}");
                for (index, pair) in cuts.windows(2).enumerate() {
                    let result = list_element(&bytes, &STRINGS, index as u32);
                    if valid {
                        let span = result.unwrap();
                        let p = partition.unwrap().payload.start;
                        assert_eq!(span.start, p + pair[0]);
                        assert_eq!(span.end, p + pair[1]);
                        assert!(p <= span.start && span.start <= span.end);
                        assert!(span.end <= bytes.len() as u32);
                    } else {
                        assert_eq!(result.unwrap_err(), partition.unwrap_err());
                    }
                }
            }
        }
    }
}

#[test]
fn truncated_directories_identify_the_first_incomplete_word() {
    for count in 2..=5u32 {
        let bytes = indexed(count, &vec![0; count as usize - 1], &[]);
        for len in 0..bytes.len() {
            let at = if len < 4 { 0 } else { 4 + 4 * ((len - 4) / 4) };
            assert_eq!(
                partition_list(&bytes[..len], &STRINGS),
                Err(fault(ErrorKind::Truncated, at as u32))
            );
        }
    }
}

#[test]
fn corrupted_ends_fail_in_wire_order_before_element_selection() {
    for ends in [[4, 0, 0], [0, u32::MAX, 0], [0, 0, 4], [2, 1, u32::MAX]] {
        let bytes = indexed(4, &ends, b"abc");
        let first = ends
            .iter()
            .enumerate()
            .find(|(i, end)| **end > 3 || (*i > 0 && **end < ends[*i - 1]))
            .unwrap()
            .0;
        let error = fault(ErrorKind::InvalidBoundary, 4 + first as u32 * 4);
        assert_eq!(partition_list(&bytes, &STRINGS), Err(error));
        for index in [0, 1, 3, 4, u32::MAX] {
            assert_eq!(list_element(&bytes, &STRINGS, index), Err(error));
        }
    }
}

#[test]
fn full_width_counts_and_sizes_reject_overflow_before_truncation() {
    for width in [1, 2, 3, 16, u32::MAX - 4, u32::MAX - 3, u32::MAX] {
        let layout = Layout::List {
            element: Box::leak(Box::new(Layout::Leaf(Leaf::Raw(width)))),
        };
        let last_count = (u32::MAX - 4) / width;
        for count in [0, 1, last_count, last_count + 1, u32::MAX] {
            let expected = 4 + u64::from(count) * u64::from(width);
            let result = partition_list(&count.to_le_bytes(), &layout);
            if expected > u64::from(u32::MAX) {
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
    for count in [u32::MAX / 4, u32::MAX / 4 + 1, u32::MAX / 4 + 2, u32::MAX] {
        let kind = if u64::from(count) * 4 > u64::from(u32::MAX) {
            ErrorKind::ArithmeticOverflow
        } else {
            ErrorKind::Truncated
        };
        let at = if kind == ErrorKind::ArithmeticOverflow {
            Error::NO_OFFSET
        } else {
            4
        };
        assert_eq!(
            partition_list(&count.to_le_bytes(), &STRINGS),
            Err(fault(kind, at))
        );
    }
}

#[test]
fn metadata_precedes_bytes_and_child_byte_validation_is_deferred() {
    let invalid = fault(ErrorKind::InvalidSchema, Error::NO_OFFSET);
    for layout in [
        Layout::Str,
        BYTE,
        Layout::List { element: &CYCLE },
        Layout::List {
            element: &Layout::Leaf(Leaf::Raw(0)),
        },
        Layout::List {
            element: &Layout::Leaf(Leaf::Signed(3)),
        },
        Layout::List { element: &BAD_MAP },
    ] {
        assert_eq!(partition_list(&[], &layout), Err(invalid));
        assert_eq!(list_element(&[], &layout, u32::MAX), Err(invalid));
    }
    for (element, payload) in [
        (&BOOL, &[0xff][..]),
        (&PAIR, &[0xff; 3]),
        (&STR, &[0xff]),
        (&BYTES, &[0xff]),
        (&OPTION, &[0xff]),
    ] {
        let layout = Layout::List { element };
        assert!(partition_list(&indexed(1, &[], payload), &layout).is_ok());
    }
}

#[test]
fn counts_above_u16_and_empty_elements_keep_full_u32_indices() {
    let count = u16::MAX as u32 + 2;
    let bytes = indexed(count, &vec![0; count as usize - 1], &[]);
    let partition = partition_list(&bytes, &STRINGS).unwrap();
    assert_eq!(partition.count, count);
    assert_eq!(partition.payload.start, count * 4);
    for index in [0, 1, u16::MAX as u32, count - 1] {
        assert_eq!(list_element(&bytes, &STRINGS, index), Ok(partition.payload));
    }
    assert_eq!(
        list_element(&bytes, &STRINGS, count),
        Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
    );
}

#[test]
fn immutable_fixed_payload_matches_the_existing_partition_gateway() {
    const LEAVES: &[Layout] = &[
        Layout::Leaf(blubber_core::Leaf::Raw(0)),
        Layout::Leaf(blubber_core::Leaf::Raw(1)),
        Layout::Leaf(blubber_core::Leaf::Raw(2)),
        Layout::Leaf(blubber_core::Leaf::Raw(3)),
        Layout::Leaf(blubber_core::Leaf::Raw(4)),
        Layout::Leaf(blubber_core::Leaf::Raw(32)),
        Layout::Leaf(blubber_core::Leaf::Raw(u32::MAX)),
    ];
    for leaf in LEAVES {
        let Layout::Leaf(blubber_core::Leaf::Raw(width)) = *leaf else {
            unreachable!()
        };
        let layout = Layout::List { element: leaf };
        for count in [0u32, 1, 2, 4, u32::MAX] {
            let mut bytes = [0xa5; 24];
            bytes[..4].copy_from_slice(&count.to_le_bytes());
            for len in 0..=bytes.len() {
                let input = &bytes[..len];
                let expected = blubber_core::partition_list(input, &layout)
                    .map(|p| &input[p.payload.start as usize..p.payload.end as usize]);
                assert_eq!(
                    blubber_core::raw::fixed_read_payload(input, width),
                    expected,
                    "width={width} count={count} len={len}"
                );
            }
        }
    }
}
