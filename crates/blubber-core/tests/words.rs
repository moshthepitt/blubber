#![forbid(unsafe_code)]

//! Wire words, full-width arithmetic, spans, and stable error behavior.

use blubber_core::raw::write_u32;
use blubber_core::{
    checked_add, checked_delta, checked_mul, checked_span, checked_u32, read_u32, Error, ErrorKind,
    Span,
};

const OVERFLOW: Error = Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET);

#[test]
fn machine_sizes_match_wide_arithmetic() {
    let values = [
        0,
        1,
        2,
        4,
        255,
        65536,
        u32::MAX as usize,
        usize::MAX / 2,
        usize::MAX / 2 + 1,
        usize::MAX - 1,
        usize::MAX,
    ];
    for left in values {
        for right in values {
            let sum = left as u128 + right as u128;
            let product = left as u128 * right as u128;
            assert_eq!(
                checked_add(left, right),
                usize::try_from(sum).map_err(|_| OVERFLOW),
                "add {left} {right}"
            );
            assert_eq!(
                checked_mul(left, right),
                usize::try_from(product).map_err(|_| OVERFLOW),
                "mul {left} {right}"
            );
        }
        assert_eq!(checked_u32(left), u32::try_from(left).map_err(|_| OVERFLOW));
    }
    // These cases catch accidental wire-width arithmetic and truncated products.
    assert_eq!(checked_mul(usize::MAX, 0), Ok(0));
    assert_eq!(checked_mul(usize::MAX / 2 + 1, 2), Err(OVERFLOW));
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(checked_u32(u32::MAX as usize + 1), Err(OVERFLOW));
        assert_eq!(checked_add(u32::MAX as usize, 1), Ok(1usize << 32));
        assert_eq!(checked_mul(1usize << 32, 1usize << 32), Err(OVERFLOW));
    }
}

#[test]
fn signed_deltas_cover_the_entire_i64_domain() {
    let values = [0, 1, 2, u32::MAX / 2, u32::MAX - 1, u32::MAX];
    let deltas = [
        i64::MIN,
        i64::MIN + 1,
        -(u32::MAX as i64) - 1,
        -(u32::MAX as i64),
        -2,
        -1,
        0,
        1,
        2,
        u32::MAX as i64,
        u32::MAX as i64 + 1,
        i64::MAX - 1,
        i64::MAX,
    ];
    for value in values {
        for delta in deltas {
            assert_eq!(
                checked_delta(value, delta),
                u32::try_from(value as i128 + delta as i128).map_err(|_| OVERFLOW),
                "value {value} delta {delta}"
            );
        }
    }
}

#[test]
fn spans_match_mathematical_containment_and_fault_precedence() {
    let values = [0, 1, 2, 3, 4, 16, u32::MAX - 1, u32::MAX];
    for start in values {
        for len in values {
            for limit in values {
                let end = start as u64 + len as u64;
                let expected = if end > u32::MAX as u64 {
                    Err(Error::new(ErrorKind::ArithmeticOverflow, start))
                } else if end > limit as u64 {
                    Err(Error::new(ErrorKind::InvalidBoundary, start))
                } else {
                    Ok(Span {
                        start,
                        end: end as u32,
                    })
                };
                assert_eq!(checked_span(start, len, limit), expected);
            }
        }
    }
    assert_eq!(checked_span(0, 0, 0), Ok(Span { start: 0, end: 0 }));
    assert_eq!(
        checked_span(u32::MAX, 0, u32::MAX),
        Ok(Span {
            start: u32::MAX,
            end: u32::MAX
        })
    );
}

#[test]
fn little_endian_golden_words_are_unaligned_and_framed() {
    for value in [0, 1, 0x1234_5678, 0x8000_0000, u32::MAX] {
        for at in 0..=9 {
            let mut bytes = [0xa5; 16];
            let mut expected = bytes;
            expected[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert_eq!(write_u32(&mut bytes, at as u32, value), Ok(()));
            assert_eq!(bytes, expected);
            assert_eq!(read_u32(&bytes, at as u32), Ok(value));
        }
    }
}

#[test]
fn every_byte_value_in_every_word_position_matches_standard_endianness() {
    for position in 0..4 {
        for byte in 0..=255 {
            let mut encoded = [0x5a; 4];
            encoded[position] = byte;
            let expected = u32::from_le_bytes(encoded);
            assert_eq!(read_u32(&encoded, 0), Ok(expected));
            let mut destination = [0; 4];
            assert_eq!(write_u32(&mut destination, 0, expected), Ok(()));
            assert_eq!(destination, encoded);
        }
    }
}

#[test]
fn arbitrary_short_ranges_never_panic_or_partially_write() {
    let before = [0x98, 0x76, 0x54, 0x32, 0x10, 0xfe, 0xdc, 0xba];
    for len in 0..=before.len() {
        for at in (0..=12).chain([u32::MAX - 4, u32::MAX - 3, u32::MAX - 1, u32::MAX]) {
            let end = at as u64 + 4;
            let mut bytes = before;
            let result = write_u32(&mut bytes[..len], at, 0x1234_5678);
            let read = read_u32(&before[..len], at);
            if end <= len as u64 {
                assert_eq!(result, Ok(()));
                let index = at as usize;
                let expected = u32::from_le_bytes(before[index..index + 4].try_into().unwrap());
                assert_eq!(read, Ok(expected));
                let mut expected = before;
                expected[index..index + 4].copy_from_slice(&0x1234_5678u32.to_le_bytes());
                assert_eq!(bytes, expected);
            } else {
                let kind = if end > u32::MAX as u64 {
                    ErrorKind::ArithmeticOverflow
                } else {
                    ErrorKind::Truncated
                };
                assert_eq!(result, Err(Error::new(kind, at)));
                assert_eq!(read, Err(Error::new(kind, at)));
                assert_eq!(bytes, before, "error must frame the complete allocation");
            }
        }
    }
}

// Explicit pairs catch accidental changes to either discriminants or codes.
const KINDS: [(ErrorKind, u16); 30] = [
    (ErrorKind::Truncated, 1),
    (ErrorKind::Discriminator, 2),
    (ErrorKind::UsedLength, 3),
    (ErrorKind::ArithmeticOverflow, 4),
    (ErrorKind::InvalidBoundary, 5),
    (ErrorKind::InvalidBool, 6),
    (ErrorKind::InvalidUtf8, 7),
    (ErrorKind::InvalidTag, 8),
    (ErrorKind::UnsortedKeys, 9),
    (ErrorKind::DuplicateKey, 10),
    (ErrorKind::IndexOutOfBounds, 11),
    (ErrorKind::MissingKey, 12),
    (ErrorKind::MissingValue, 13),
    (ErrorKind::Utf8Boundary, 14),
    (ErrorKind::Capacity, 15),
    (ErrorKind::Allocation, 16),
    (ErrorKind::DepthExceeded, 17),
    (ErrorKind::WrongOwner, 18),
    (ErrorKind::NotWritable, 19),
    (ErrorKind::BorrowConflict, 20),
    (ErrorKind::GrowthLimit, 21),
    (ErrorKind::RentShortfall, 22),
    (ErrorKind::Unauthorized, 23),
    (ErrorKind::InvalidPayer, 24),
    (ErrorKind::DuplicateAccount, 25),
    (ErrorKind::AlreadyInitialized, 26),
    (ErrorKind::Closed, 27),
    (ErrorKind::Poisoned, 28),
    (ErrorKind::InvalidPlan, 29),
    (ErrorKind::InvalidSchema, 30),
];

#[test]
fn errors_preserve_codes_and_rebase_offsets_once() {
    for (kind, discriminant) in KINDS {
        assert_eq!(kind as u16, discriminant);
        let relative = Error::new(kind, 3);
        assert_eq!(relative.custom_code(), Some(0xb100 + discriminant as u32));
        assert_eq!(relative.rebase(12), Error::new(kind, 15));
        assert_eq!(relative.rebase(0), relative);
        assert_eq!(
            relative.rebase(u32::MAX - 4),
            Error::new(kind, u32::MAX - 1)
        );
        assert_eq!(relative.rebase(u32::MAX - 3), OVERFLOW);
        assert_eq!(relative.rebase(u32::MAX), OVERFLOW);
        let unlocated: Error = kind.into();
        assert_eq!(unlocated, Error::new(kind, Error::NO_OFFSET));
        assert_eq!(unlocated.rebase(u32::MAX), unlocated);
    }
    for code in [0, 1, u32::MAX as u64, 1u64 << 32, u64::MAX] {
        let error = Error::Program { code };
        assert_eq!(error.custom_code(), None);
        assert_eq!(error.rebase(u32::MAX), error);
    }
}

#[test]
fn errors_format_only_on_request() {
    assert_eq!(Error::from(ErrorKind::Truncated).to_string(), "Truncated");
    assert_eq!(
        Error::new(ErrorKind::InvalidTag, 12).to_string(),
        "InvalidTag at byte 12"
    );
    assert_eq!(
        Error::Program { code: u64::MAX }.to_string(),
        "program error 18446744073709551615"
    );
    fn accepts_core_error(_: &dyn core::error::Error) {}
    accepts_core_error(&OVERFLOW);
}
