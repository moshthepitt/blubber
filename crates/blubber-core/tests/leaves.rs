#![forbid(unsafe_code)]

//! Exact scalar domains, fixed composition, and option tag/extent partitions.

use blubber_core::{
    partition_option, validate_fixed, Error, ErrorKind, FixedField, Layout, Leaf, Span,
    VariableField, MAX_DEPTH,
};

static BOOL: Layout = Layout::Leaf(Leaf::Bool);
static WORD: Layout = Layout::Leaf(Leaf::Signed(2));
static RAW: Layout = Layout::Leaf(Leaf::Raw(3));
static STR: Layout = Layout::Str;
static INNER: Layout = Layout::Struct {
    fixed_bytes: 3,
    fixed: &[
        FixedField {
            declaration: 0,
            offset: 0,
            layout: &WORD,
        },
        FixedField {
            declaration: 1,
            offset: 2,
            layout: &BOOL,
        },
    ],
    variable: &[],
};
static OUTER: Layout = Layout::Struct {
    fixed_bytes: 8,
    fixed: &[
        FixedField {
            declaration: 0,
            offset: 0,
            layout: &RAW,
        },
        FixedField {
            declaration: 1,
            offset: 3,
            layout: &BOOL,
        },
        FixedField {
            declaration: 2,
            offset: 4,
            layout: &INNER,
        },
        FixedField {
            declaration: 3,
            offset: 7,
            layout: &BOOL,
        },
    ],
    variable: &[],
};
static CYCLE: Layout = Layout::Option { value: &CYCLE };

fn fault(kind: ErrorKind, offset: u32) -> Error {
    Error::new(kind, offset)
}

#[test]
fn bool_accepts_exactly_two_of_all_256_encodings() {
    for byte in 0..=u8::MAX {
        let expected = if byte <= 1 {
            Ok(())
        } else {
            Err(fault(ErrorKind::InvalidBool, 0))
        };
        assert_eq!(validate_fixed(&[byte], &BOOL), expected);
    }
    assert_eq!(
        validate_fixed(&[], &BOOL),
        Err(fault(ErrorKind::Truncated, 0))
    );
    // Exact extent is checked before the bool bit domain.
    assert_eq!(
        validate_fixed(&[2, 0], &BOOL),
        Err(fault(ErrorKind::InvalidBoundary, 1))
    );
}

#[test]
fn integer_widths_accept_every_byte_at_every_position() {
    for width in [1, 2, 4, 8, 16] {
        for leaf in [Leaf::Unsigned(width), Leaf::Signed(width)] {
            let layout = Layout::Leaf(leaf);
            for position in 0..width as usize {
                for byte in 0..=u8::MAX {
                    let mut bytes = vec![0xff; width as usize];
                    bytes[position] = byte;
                    assert_eq!(validate_fixed(&bytes, &layout), Ok(()));
                }
            }
            for len in 0..width as usize {
                assert_eq!(
                    validate_fixed(&vec![0; len], &layout),
                    Err(fault(ErrorKind::Truncated, 0))
                );
            }
            assert_eq!(
                validate_fixed(&vec![0; width as usize + 1], &layout),
                Err(fault(ErrorKind::InvalidBoundary, width as u32))
            );
        }
    }
}

#[test]
fn raw_arrays_and_pubkeys_have_no_bit_domain_restrictions() {
    for width in [1, 3, 16, 32, 257] {
        let layout = Layout::Leaf(Leaf::Raw(width));
        for byte in 0..=u8::MAX {
            assert_eq!(validate_fixed(&vec![byte; width as usize], &layout), Ok(()));
        }
    }
    assert_eq!(
        validate_fixed(&[], &Layout::Leaf(Leaf::Raw(u32::MAX))),
        Err(fault(ErrorKind::Truncated, 0))
    );
}

#[test]
fn nested_fixed_fields_rebase_the_first_invalid_bool_in_wire_order() {
    for a in [0, 1, 2, 255] {
        for b in [0, 1, 2, 255] {
            for c in [0, 1, 2, 255] {
                // Raw and integer bytes deliberately resemble invalid bools.
                let bytes = [255, 2, 128, a, 254, 253, b, c];
                let first = [(3, a), (6, b), (7, c)]
                    .into_iter()
                    .find(|(_, byte)| *byte > 1);
                let expected = first.map_or(Ok(()), |(offset, _)| {
                    Err(fault(ErrorKind::InvalidBool, offset))
                });
                assert_eq!(validate_fixed(&bytes, &OUTER), expected);
            }
        }
    }
    assert_eq!(
        validate_fixed(&[2; 7], &OUTER),
        Err(fault(ErrorKind::Truncated, 0))
    );
    assert_eq!(
        validate_fixed(&[2; 9], &OUTER),
        Err(fault(ErrorKind::InvalidBoundary, 8))
    );
}

#[test]
fn option_tags_none_extent_and_empty_some_are_distinct() {
    let layout = Layout::Option { value: &STR };
    assert_eq!(
        partition_option(&[], &layout),
        Err(fault(ErrorKind::Truncated, 0))
    );
    for tag in 0..=u8::MAX {
        let expected = match tag {
            0 => Ok(None),
            1 => Ok(Some(Span { start: 1, end: 1 })),
            _ => Err(fault(ErrorKind::InvalidTag, 0)),
        };
        assert_eq!(partition_option(&[tag], &layout), expected);
        for len in 2..=6 {
            let mut bytes = vec![0xff; len];
            bytes[0] = tag;
            let expected = match tag {
                0 => Err(fault(ErrorKind::InvalidBoundary, 1)),
                1 => Ok(Some(Span {
                    start: 1,
                    end: len as u32,
                })),
                _ => Err(fault(ErrorKind::InvalidTag, 0)),
            };
            assert_eq!(partition_option(&bytes, &layout), expected);
        }
    }
}

#[test]
fn some_fixed_payload_requires_its_exact_width() {
    for value in [&BOOL, &WORD, &RAW, &INNER, &OUTER] {
        let width = match value {
            Layout::Leaf(Leaf::Bool) => 1,
            Layout::Leaf(Leaf::Signed(n)) => *n as usize,
            Layout::Leaf(Leaf::Raw(n)) => *n as usize,
            Layout::Struct { fixed_bytes, .. } => *fixed_bytes as usize,
            _ => unreachable!(),
        };
        let layout = Layout::Option { value };
        assert_eq!(partition_option(&[0], &layout), Ok(None));
        for len in 0..=width + 2 {
            let mut bytes = vec![0xff; 1 + len];
            bytes[0] = 1;
            let expected = match len.cmp(&width) {
                core::cmp::Ordering::Less => Err(fault(ErrorKind::Truncated, 1)),
                core::cmp::Ordering::Equal => Ok(Some(Span {
                    start: 1,
                    end: bytes.len() as u32,
                })),
                core::cmp::Ordering::Greater => {
                    Err(fault(ErrorKind::InvalidBoundary, 1 + width as u32))
                }
            };
            assert_eq!(partition_option(&bytes, &layout), expected);
        }
    }
    static HUGE: Layout = Layout::Leaf(Leaf::Raw(u32::MAX));
    let layout = Layout::Option { value: &HUGE };
    assert_eq!(partition_option(&[0], &layout), Ok(None));
    assert_eq!(
        partition_option(&[1], &layout),
        Err(fault(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET))
    );
    assert_eq!(
        partition_option(&[2], &layout),
        Err(fault(ErrorKind::InvalidTag, 0))
    );
}

#[test]
fn option_partition_and_fixed_domains_compose_with_one_rebase() {
    let layout = Layout::Option { value: &OUTER };
    let mut bytes = [1, 255, 255, 255, 0, 255, 255, 1, 0];
    for (position, offset) in [(4, 4), (7, 7), (8, 8)] {
        bytes[position] = 2;
        let span = partition_option(&bytes, &layout).unwrap().unwrap();
        let result = validate_fixed(&bytes[span.start as usize..span.end as usize], &OUTER)
            .map_err(|error| error.rebase(span.start));
        assert_eq!(result, Err(fault(ErrorKind::InvalidBool, offset)));
        bytes[position] = 0;
    }
    let layout = Layout::Option { value: &BOOL };
    for byte in 0..=u8::MAX {
        let bytes = [1, byte];
        let span = partition_option(&bytes, &layout).unwrap().unwrap();
        assert_eq!(span, Span { start: 1, end: 2 });
        assert_eq!(validate_fixed(&bytes[1..], &BOOL).is_ok(), byte <= 1);
    }
}

#[test]
fn variable_some_payload_is_parent_delimited_without_early_child_validation() {
    static OPTIONS: Layout = Layout::Option { value: &BOOL };
    static LIST: Layout = Layout::List { element: &BOOL };
    for value in [&OPTIONS, &LIST, &STR] {
        let layout = Layout::Option { value };
        for bytes in [&[1][..], &[1, 2, 255][..]] {
            assert_eq!(
                partition_option(bytes, &layout),
                Ok(Some(Span {
                    start: 1,
                    end: bytes.len() as u32,
                }))
            );
        }
    }
}

#[test]
fn dishonest_metadata_and_wrong_gateway_kinds_are_rejected_before_bytes() {
    static BAD: Layout = Layout::Leaf(Leaf::Raw(0));
    let invalid = fault(ErrorKind::InvalidSchema, Error::NO_OFFSET);
    static MALFORMED: &[Layout] = &[
        BAD,
        Layout::Leaf(Leaf::Unsigned(3)),
        Layout::Leaf(Leaf::Signed(0)),
        Layout::Option { value: &BAD },
        CYCLE,
        Layout::Struct {
            fixed_bytes: 1,
            fixed: &[FixedField {
                declaration: 0,
                offset: 1,
                layout: &BOOL,
            }],
            variable: &[],
        },
        Layout::Struct {
            fixed_bytes: 1,
            fixed: &[FixedField {
                declaration: 0,
                offset: 0,
                layout: &BOOL,
            }],
            variable: &[VariableField {
                declaration: 0,
                layout: &STR,
            }],
        },
    ];
    for layout in MALFORMED {
        assert_eq!(validate_fixed(&[255], layout), Err(invalid));
        assert_eq!(partition_option(&[255], layout), Err(invalid));
        assert_eq!(partition_option(&[], layout), Err(invalid));
    }
    for layout in [&BOOL, &OUTER, &STR] {
        assert_eq!(partition_option(&[0], layout), Err(invalid));
    }
    for layout in [&STR, &Layout::Option { value: &BOOL }] {
        assert_eq!(validate_fixed(&[0], layout), Err(invalid));
    }
}

#[test]
fn maximum_depth_fixed_composition_is_bounded_and_rebases_all_levels() {
    let mut layout = &BOOL;
    for depth in 2..=MAX_DEPTH + 1 {
        let fields = Box::leak(Box::new([FixedField {
            declaration: 0,
            offset: 0,
            layout,
        }]));
        layout = Box::leak(Box::new(Layout::Struct {
            fixed_bytes: 1,
            fixed: fields,
            variable: &[],
        }));
        if depth <= MAX_DEPTH {
            assert_eq!(validate_fixed(&[1], layout), Ok(()));
            assert_eq!(
                validate_fixed(&[255], layout),
                Err(fault(ErrorKind::InvalidBool, 0))
            );
        } else {
            assert_eq!(
                validate_fixed(&[1], layout),
                Err(fault(ErrorKind::InvalidSchema, Error::NO_OFFSET))
            );
        }
    }
}
