#![forbid(unsafe_code)]

//! Descriptor metadata and exact, nonrecursive struct partition acceptance.

use blubber_core::{
    check_layout, fixed_region, partition_struct, struct_variable_field, Error, ErrorKind,
    FixedField, KeyDomain, KeyLayout, Layout, Leaf, Span, VariableField, MAX_DEPTH,
};

static BYTE: Layout = Layout::Leaf(Leaf::Unsigned(1));
static BOOL: Layout = Layout::Leaf(Leaf::Bool);
static WORD: Layout = Layout::Leaf(Leaf::Signed(2));
static STR: Layout = Layout::Str;
static FIXED: Layout = Layout::Struct {
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
static ONE: Layout = Layout::Struct {
    fixed_bytes: 1,
    fixed: &[FixedField {
        declaration: 1,
        offset: 0,
        layout: &BYTE,
    }],
    variable: &[VariableField {
        declaration: 0,
        layout: &STR,
    }],
};
static MIXED: Layout = Layout::Struct {
    fixed_bytes: 3,
    fixed: &[
        FixedField {
            declaration: 1,
            offset: 0,
            layout: &WORD,
        },
        FixedField {
            declaration: 3,
            offset: 2,
            layout: &BYTE,
        },
    ],
    variable: &[
        VariableField {
            declaration: 0,
            layout: &STR,
        },
        VariableField {
            declaration: 2,
            layout: &STR,
        },
        VariableField {
            declaration: 4,
            layout: &STR,
        },
        VariableField {
            declaration: 5,
            layout: &STR,
        },
    ],
};
static CYCLE: Layout = Layout::Option { value: &CYCLE };

fn fault(kind: ErrorKind, offset: u32) -> Error {
    Error::new(kind, offset)
}

fn variable_struct(count: usize) -> Layout {
    let fields = (0..count)
        .map(|i| VariableField {
            declaration: i as u16,
            layout: &STR,
        })
        .collect::<Vec<_>>();
    Layout::Struct {
        fixed_bytes: 0,
        fixed: &[],
        variable: Box::leak(fields.into_boxed_slice()),
    }
}

#[test]
fn exact_fixed_regions_reject_short_and_trailing_bytes() {
    for layout in [&BYTE, &BOOL, &WORD, &FIXED] {
        check_layout(layout, MAX_DEPTH).unwrap();
    }
    assert_eq!(fixed_region(&[0xff], &BOOL), Ok(Span { start: 0, end: 1 }));
    assert_eq!(
        fixed_region(&[0xff, 2, 3], &FIXED),
        Ok(Span { start: 0, end: 3 })
    );
    assert_eq!(
        fixed_region(&[0; 2], &FIXED),
        Err(fault(ErrorKind::Truncated, 0))
    );
    assert_eq!(
        fixed_region(&[0; 4], &FIXED),
        Err(fault(ErrorKind::InvalidBoundary, 3))
    );
    assert_eq!(
        fixed_region(&[], &STR),
        Err(fault(ErrorKind::InvalidSchema, Error::NO_OFFSET))
    );
    let partition = partition_struct(&[0xff, 2, 3], &FIXED).unwrap();
    assert_eq!(partition.fixed, Span { start: 0, end: 3 });
    assert_eq!(partition.directory, Span { start: 3, end: 3 });
    assert_eq!(partition.payload, Span { start: 3, end: 3 });
    assert_eq!(partition.variables, 0);
    assert_eq!(
        partition_struct(&[0; 4], &FIXED),
        Err(fault(ErrorKind::InvalidBoundary, 3))
    );
    assert_eq!(
        struct_variable_field(&[0; 3], &FIXED, 0),
        Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
    );
}

#[test]
fn one_variable_field_has_no_directory_and_can_be_empty() {
    for bytes in [&[0xa5][..], &[0xa5, 0, 0xff][..]] {
        let partition = partition_struct(bytes, &ONE).unwrap();
        assert_eq!(partition.fixed, Span { start: 0, end: 1 });
        assert_eq!(partition.directory, Span { start: 1, end: 1 });
        assert_eq!(
            partition.payload,
            Span {
                start: 1,
                end: bytes.len() as u32
            }
        );
        assert_eq!(struct_variable_field(bytes, &ONE, 0), Ok(partition.payload));
        assert_eq!(
            struct_variable_field(bytes, &ONE, 1),
            Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
        );
    }
    let no_prefix = variable_struct(1);
    assert_eq!(
        struct_variable_field(&[], &no_prefix, 0),
        Ok(Span { start: 0, end: 0 })
    );
    assert_eq!(
        struct_variable_field(b"abc", &no_prefix, 0),
        Ok(Span { start: 0, end: 3 })
    );
    assert_eq!(
        partition_struct(&[], &ONE),
        Err(fault(ErrorKind::Truncated, 0))
    );
}

#[test]
fn fixed_fields_pack_before_interleaved_variable_declarations() {
    // Declaration order: Str, i16, Str, u8, Str, Str.
    // Variable fields: "ab", "", "xyz", ""; only three ends are stored.
    let bytes = [
        0x34, 0x12, 0xa5, 2, 0, 0, 0, 2, 0, 0, 0, 5, 0, 0, 0, b'a', b'b', b'x', b'y', b'z',
    ];
    let partition = partition_struct(&bytes, &MIXED).unwrap();
    assert_eq!(partition.fixed, Span { start: 0, end: 3 });
    assert_eq!(partition.directory, Span { start: 3, end: 15 });
    assert_eq!(partition.payload, Span { start: 15, end: 20 });
    assert_eq!(partition.variables, 4);
    for (i, span) in [(15, 17), (17, 17), (17, 20), (20, 20)]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            struct_variable_field(&bytes, &MIXED, i as u16),
            Ok(Span {
                start: span.0,
                end: span.1
            })
        );
    }
}

#[test]
fn all_small_end_combinations_match_independent_interval_partitioning() {
    let layout = variable_struct(4);
    for len in 0..=5usize {
        for a in 0..=len + 1 {
            for b in 0..=len + 1 {
                for c in 0..=len + 1 {
                    let cuts = [0, a, b, c, len];
                    let mut bytes = Vec::new();
                    for end in [a, b, c] {
                        bytes.extend_from_slice(&(end as u32).to_le_bytes());
                    }
                    bytes.resize(12 + len, 0xff); // Child UTF-8 is intentionally not checked.
                    let intervals = cuts
                        .windows(2)
                        .map(|pair| pair[0]..pair[1])
                        .collect::<Vec<_>>();
                    let valid = intervals
                        .iter()
                        .all(|span| span.start <= span.end && span.end <= len);
                    assert_eq!(
                        partition_struct(&bytes, &layout).is_ok(),
                        valid,
                        "cuts {cuts:?}"
                    );
                    for (i, interval) in intervals.into_iter().enumerate() {
                        let result = struct_variable_field(&bytes, &layout, i as u16);
                        if valid {
                            let span = result.unwrap();
                            assert_eq!(
                                span,
                                Span {
                                    start: (12 + interval.start) as u32,
                                    end: (12 + interval.end) as u32
                                }
                            );
                            assert_eq!(
                                &bytes[span.start as usize..span.end as usize],
                                &bytes[12..][interval]
                            );
                        } else {
                            assert!(result.is_err());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn corrupted_words_report_the_first_directory_fault() {
    let mut bytes = [0u8; 18]; // Three-byte prefix, three words, three payload bytes.
    for (slot, value) in [(0, 4), (1, u32::MAX), (2, u32::MAX), (2, 4)] {
        bytes[3..15].fill(0);
        let at = 3 + slot * 4;
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        assert_eq!(
            partition_struct(&bytes, &MIXED),
            Err(fault(ErrorKind::InvalidBoundary, at as u32))
        );
    }
    bytes[3..7].copy_from_slice(&2u32.to_le_bytes());
    bytes[7..11].copy_from_slice(&1u32.to_le_bytes());
    bytes[11..15].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        partition_struct(&bytes, &MIXED),
        Err(fault(ErrorKind::InvalidBoundary, 7))
    );
    for len in 0..15 {
        let offset = if len < 3 {
            0
        } else {
            3 + 4 * ((len - 3) / 4) as u32
        };
        assert_eq!(
            partition_struct(&bytes[..len], &MIXED),
            Err(fault(ErrorKind::Truncated, offset))
        );
    }
}

#[test]
fn malformed_metadata_is_rejected_before_bytes() {
    let invalid = fault(ErrorKind::InvalidSchema, Error::NO_OFFSET);
    static INVALID: &[Layout] = &[
        Layout::Leaf(Leaf::Raw(0)),
        Layout::Leaf(Leaf::Unsigned(3)),
        Layout::Leaf(Leaf::Signed(32)),
        Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[],
        },
        Layout::Struct {
            fixed_bytes: 1,
            fixed: &[],
            variable: &[],
        },
        Layout::Struct {
            fixed_bytes: 1,
            fixed: &[],
            variable: &[VariableField {
                declaration: 0,
                layout: &STR,
            }],
        },
        Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[VariableField {
                declaration: 0,
                layout: &BYTE,
            }],
        },
        Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[VariableField {
                declaration: 1,
                layout: &STR,
            }],
        },
        Layout::Struct {
            fixed_bytes: 1,
            fixed: &[FixedField {
                declaration: 0,
                offset: 1,
                layout: &BYTE,
            }],
            variable: &[],
        },
        Layout::Struct {
            fixed_bytes: 2,
            fixed: &[FixedField {
                declaration: 0,
                offset: 0,
                layout: &BYTE,
            }],
            variable: &[],
        },
        Layout::Struct {
            fixed_bytes: 1,
            fixed: &[FixedField {
                declaration: 0,
                offset: 0,
                layout: &STR,
            }],
            variable: &[],
        },
        Layout::Struct {
            fixed_bytes: 1,
            fixed: &[FixedField {
                declaration: 0,
                offset: 0,
                layout: &BYTE,
            }],
            variable: &[VariableField {
                declaration: 0,
                layout: &STR,
            }],
        },
        Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[
                VariableField {
                    declaration: 1,
                    layout: &STR,
                },
                VariableField {
                    declaration: 0,
                    layout: &STR,
                },
            ],
        },
        Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: &[
                VariableField {
                    declaration: 0,
                    layout: &STR,
                },
                VariableField {
                    declaration: 0,
                    layout: &STR,
                },
            ],
        },
    ];
    for layout in INVALID {
        assert_eq!(check_layout(layout, MAX_DEPTH), Err(invalid), "{layout:?}");
        assert_eq!(partition_struct(&[], layout), Err(invalid));
        assert_eq!(fixed_region(&[], layout), Err(invalid));
    }
    assert_eq!(partition_struct(&[], &STR), Err(invalid));
    assert_eq!(partition_struct(&[], &BYTE), Err(invalid));
}

#[test]
fn recursive_metadata_checks_widths_depth_and_cycles() {
    for width in [1, 2, 4, 8, 16] {
        for leaf in [Leaf::Unsigned(width), Leaf::Signed(width)] {
            assert_eq!(check_layout(&Layout::Leaf(leaf), 1), Ok(()));
        }
    }
    for domain in [
        KeyDomain::Unsigned,
        KeyDomain::Signed,
        KeyDomain::Bool,
        KeyDomain::Raw,
    ] {
        for width in [0, 1, 2, 3, 4, 8, 16, 32, u32::MAX] {
            let layout = Layout::Map {
                key: KeyLayout { width, domain },
                value: &STR,
            };
            let valid = match domain {
                KeyDomain::Unsigned | KeyDomain::Signed => [1, 2, 4, 8, 16].contains(&width),
                KeyDomain::Bool => width == 1,
                KeyDomain::Raw => width > 0,
            };
            assert_eq!(check_layout(&layout, 2).is_ok(), valid);
        }
    }
    let invalid = fault(ErrorKind::InvalidSchema, Error::NO_OFFSET);
    assert_eq!(check_layout(&CYCLE, MAX_DEPTH), Err(invalid));
    assert_eq!(check_layout(&STR, 0), Err(invalid));
    assert_eq!(check_layout(&STR, MAX_DEPTH + 1), Err(invalid));
    assert_eq!(check_layout(&STR, usize::MAX), Err(invalid));
    let mut child = &STR;
    for depth in 2..=MAX_DEPTH + 1 {
        child = Box::leak(Box::new(Layout::Option { value: child }));
        assert_eq!(check_layout(child, MAX_DEPTH).is_ok(), depth <= MAX_DEPTH);
        if depth <= MAX_DEPTH {
            assert_eq!(check_layout(child, depth - 1), Err(invalid));
        }
    }
    let bad_child = Box::leak(Box::new(Layout::Leaf(Leaf::Raw(0))));
    assert_eq!(
        check_layout(&Layout::List { element: bad_child }, MAX_DEPTH),
        Err(invalid)
    );
    assert_eq!(
        check_layout(&Layout::Option { value: bad_child }, MAX_DEPTH),
        Err(invalid)
    );
    assert_eq!(
        check_layout(
            &Layout::Map {
                key: KeyLayout {
                    width: 1,
                    domain: KeyDomain::Raw
                },
                value: bad_child
            },
            MAX_DEPTH
        ),
        Err(invalid)
    );
}

#[test]
fn maximum_field_count_and_empty_fields_are_supported() {
    let layout = variable_struct(u16::MAX as usize);
    let bytes = vec![0u8; 4 * (u16::MAX as usize - 1)];
    let partition = partition_struct(&bytes, &layout).unwrap();
    assert_eq!(partition.variables, u16::MAX);
    assert_eq!(
        partition.payload,
        Span {
            start: bytes.len() as u32,
            end: bytes.len() as u32
        }
    );
    for index in [0, 1, u16::MAX - 1] {
        assert_eq!(
            struct_variable_field(&bytes, &layout, index),
            Ok(partition.payload)
        );
    }
    assert_eq!(
        struct_variable_field(&bytes, &layout, u16::MAX),
        Err(fault(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET))
    );
    assert_eq!(
        check_layout(&variable_struct(u16::MAX as usize + 1), MAX_DEPTH),
        Err(fault(ErrorKind::InvalidSchema, Error::NO_OFFSET))
    );
}

#[test]
fn wire_overflow_is_checked_before_reading_a_huge_prefix() {
    static HUGE: Layout = Layout::Leaf(Leaf::Raw(u32::MAX));
    static LARGE: Layout = Layout::Struct {
        fixed_bytes: u32::MAX,
        fixed: &[FixedField {
            declaration: 0,
            offset: 0,
            layout: &HUGE,
        }],
        variable: &[
            VariableField {
                declaration: 1,
                layout: &STR,
            },
            VariableField {
                declaration: 2,
                layout: &STR,
            },
        ],
    };
    assert_eq!(check_layout(&LARGE, MAX_DEPTH), Ok(()));
    assert_eq!(
        partition_struct(&[], &LARGE),
        Err(fault(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET))
    );
    static OVERFLOW: Layout = Layout::Struct {
        fixed_bytes: u32::MAX,
        fixed: &[
            FixedField {
                declaration: 0,
                offset: 0,
                layout: &HUGE,
            },
            FixedField {
                declaration: 1,
                offset: u32::MAX,
                layout: &BYTE,
            },
        ],
        variable: &[],
    };
    assert_eq!(
        check_layout(&OVERFLOW, MAX_DEPTH),
        Err(fault(ErrorKind::InvalidSchema, Error::NO_OFFSET))
    );
}
