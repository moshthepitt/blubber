#![forbid(unsafe_code)]

//! UTF-8 acceptance and fault offsets compared with Rust's independent checker.

use blubber_core::{validate_utf8, Error, ErrorKind};

fn agrees_with_core(bytes: &[u8]) {
    let expected = core::str::from_utf8(bytes)
        .map(|_| ())
        .map_err(|error| Error::new(ErrorKind::InvalidUtf8, error.valid_up_to() as u32));
    assert_eq!(validate_utf8(bytes), expected, "bytes: {bytes:02x?}");
}

#[test]
fn exhaustive_strings_through_two_bytes_match_core() {
    agrees_with_core(&[]);
    for first in 0..=u8::MAX {
        agrees_with_core(&[first]);
        for second in 0..=u8::MAX {
            agrees_with_core(&[first, second]);
        }
    }
}

#[test]
fn every_unicode_scalar_has_an_accepted_canonical_encoding() {
    let mut buffer = [0; 4];
    let mut count = 0;
    for value in 0..=0x10ffff {
        if let Some(scalar) = char::from_u32(value) {
            let bytes = scalar.encode_utf8(&mut buffer).as_bytes();
            agrees_with_core(bytes);
            assert_eq!(validate_utf8(bytes), Ok(()));
            count += 1;
        }
    }
    assert_eq!(count, 1_112_064);
}

#[test]
fn exhaustive_leading_pairs_with_extreme_continuations_match_core() {
    // All leading/second-byte combinations, with both ends of each remaining
    // continuation domain. This includes invalid leaders and restricted seconds.
    for first in 0..=u8::MAX {
        for second in 0..=u8::MAX {
            for third in [0x80, 0xbf] {
                agrees_with_core(&[first, second, third]);
                for fourth in [0x80, 0xbf] {
                    agrees_with_core(&[first, second, third, fourth]);
                }
            }
        }
    }
}

#[test]
fn every_byte_in_each_multibyte_position_matches_core() {
    let examples: &[&[u8]] = &[
        &[0xc2, 0x80],
        &[0xdf, 0xbf],
        &[0xe0, 0xa0, 0x80],
        &[0xe1, 0x80, 0xbf],
        &[0xed, 0x9f, 0xbf],
        &[0xef, 0xbf, 0xbf],
        &[0xf0, 0x90, 0x80, 0x80],
        &[0xf1, 0x80, 0xbf, 0x80],
        &[0xf4, 0x8f, 0xbf, 0xbf],
    ];
    let prefix = "\0a¢€🦀".as_bytes();
    for original in examples {
        for position in 0..original.len() {
            for byte in 0..=u8::MAX {
                let mut candidate = original.to_vec();
                candidate[position] = byte;
                agrees_with_core(&candidate);
                let mut surrounded = prefix.to_vec();
                surrounded.extend_from_slice(&candidate);
                surrounded.extend_from_slice("zé".as_bytes());
                agrees_with_core(&surrounded);
            }
        }
    }
}

#[test]
fn overlong_surrogate_and_out_of_range_forms_are_rejected_at_the_leader() {
    let invalid: &[&[u8]] = &[
        &[0x80],
        &[0xbf],
        &[0xc0, 0x80],
        &[0xc1, 0xbf],
        &[0xe0, 0x80, 0x80],
        &[0xe0, 0x9f, 0xbf],
        &[0xed, 0xa0, 0x80],
        &[0xed, 0xbf, 0xbf],
        &[0xed, 0xa0, 0x80, 0xed, 0xb0, 0x80], // CESU-8 surrogate pair.
        &[0xf0, 0x80, 0x80, 0x80],
        &[0xf0, 0x8f, 0xbf, 0xbf],
        &[0xf4, 0x90, 0x80, 0x80],
        &[0xf5, 0x80, 0x80, 0x80],
        &[0xf7, 0xbf, 0xbf, 0xbf],
        &[0xf8, 0x88, 0x80, 0x80, 0x80],
        &[0xfc, 0x84, 0x80, 0x80, 0x80, 0x80],
        &[0xfe],
        &[0xff],
        &[0xe2, 0x28, 0xa1],
        &[0xf0, 0x90, 0x80, 0x7f],
    ];
    for bytes in invalid {
        agrees_with_core(bytes);
        assert_eq!(
            validate_utf8(bytes),
            Err(Error::new(ErrorKind::InvalidUtf8, 0))
        );
        for prefix in ["", "hello", "é€🦀\0"] {
            let mut prefixed = prefix.as_bytes().to_vec();
            prefixed.extend_from_slice(bytes);
            prefixed.extend_from_slice(&[0xff, b'a']);
            let snapshot = prefixed.clone();
            agrees_with_core(&prefixed);
            assert_eq!(
                validate_utf8(&prefixed),
                Err(Error::new(ErrorKind::InvalidUtf8, prefix.len() as u32))
            );
            assert_eq!(prefixed, snapshot);
        }
    }
}

#[test]
fn every_truncation_of_valid_multibyte_scalars_reports_the_leader() {
    for scalar in [
        '\u{80}',
        '\u{7ff}',
        '\u{800}',
        '\u{d7ff}',
        '\u{e000}',
        '\u{ffff}',
        '\u{10000}',
        '\u{10ffff}',
    ] {
        let mut buffer = [0; 4];
        let bytes = scalar.encode_utf8(&mut buffer).as_bytes();
        for end in 1..bytes.len() {
            for prefix in ["", "abc", "¢€🦀"] {
                let mut candidate = prefix.as_bytes().to_vec();
                candidate.extend_from_slice(&bytes[..end]);
                agrees_with_core(&candidate);
                assert_eq!(
                    validate_utf8(&candidate),
                    Err(Error::new(ErrorKind::InvalidUtf8, prefix.len() as u32))
                );
            }
        }
    }
}

#[test]
fn unicode_boundaries_nul_bom_and_noncharacters_are_valid() {
    let text = "\0\u{7f}\u{80}\u{7ff}\u{800}\u{d7ff}\u{e000}\u{feff}\u{fdd0}\u{fffe}\u{ffff}\u{10000}\u{10ffff}";
    assert_eq!(validate_utf8(text.as_bytes()), Ok(()));
    // UTF-8 validity does not impose Unicode normalization.
    assert_eq!(validate_utf8("é e\u{301}".as_bytes()), Ok(()));
    for end in 0..=text.len() {
        agrees_with_core(&text.as_bytes()[..end]);
    }
}

#[test]
fn long_strings_and_unaligned_subslices_keep_exact_offsets() {
    let mut data = vec![b'x'; 16_385];
    data.extend_from_slice("¢€🦀".repeat(1024).as_bytes());
    assert_eq!(validate_utf8(&data), Ok(()));
    let valid_len = data.len();
    data.extend_from_slice(&[0xf4, 0x90, 0x80, 0x80]);
    for start in 0..16 {
        agrees_with_core(&data[start..]);
        assert_eq!(
            validate_utf8(&data[start..]),
            Err(Error::new(
                ErrorKind::InvalidUtf8,
                (valid_len - start) as u32
            ))
        );
    }
}

#[test]
fn deterministic_corrupted_sequences_match_core() {
    let mut state = 0x7c93_85a1_4b2d_06efu64;
    let mut buffer = [0; 64];
    for length in 0..=buffer.len() {
        for _ in 0..1024 {
            for byte in &mut buffer[..length] {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = state as u8;
            }
            agrees_with_core(&buffer[..length]);
        }
    }
}
