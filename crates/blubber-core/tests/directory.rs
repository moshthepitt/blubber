//! Independent word and physical-frame oracles for directory delta passes.
#![forbid(unsafe_code)]

use blubber_core::raw::adjust_directory;
use blubber_core::ErrorKind;

#[test]
fn contiguous_and_strided_runs_match_a_word_oracle_and_preserve_keys() {
    for count in [0, 1, 2, 7, 100usize] {
        for first in [0, 1, 3usize] {
            for stride in [4, 5, 36usize] {
                let mut original = vec![0xa5; first + count * stride + 17];
                for i in 0..count {
                    let at = first + i * stride;
                    original[at..at + 4].copy_from_slice(&(10 + 3 * i as u32).to_le_bytes());
                }
                for delta in [-7, 0, 11i64] {
                    let mut expected = original.clone();
                    for i in 0..count {
                        let at = first + i * stride;
                        let old = u32::from_le_bytes(original[at..at + 4].try_into().unwrap());
                        let new = u32::try_from(i128::from(old) + i128::from(delta)).unwrap();
                        expected[at..at + 4].copy_from_slice(&new.to_le_bytes());
                    }
                    let mut actual = original.clone();
                    adjust_directory(
                        &mut actual,
                        first as u32,
                        stride as u32,
                        count as u32,
                        delta,
                        4096,
                    )
                    .unwrap();
                    assert_eq!(
                        actual, expected,
                        "first={first} stride={stride} count={count} delta={delta}"
                    );
                }
            }
        }
    }
}

#[test]
fn late_boundary_and_arithmetic_failures_preserve_every_physical_byte() {
    for (bad, delta, payload, kind) in [
        (4097, 1, 4096, ErrorKind::InvalidBoundary),
        (u32::MAX, 1, u32::MAX, ErrorKind::ArithmeticOverflow),
        (0, -1, u32::MAX, ErrorKind::ArithmeticOverflow),
    ] {
        for stride in [4, 36] {
            let mut bytes = vec![0xa5; 1 + 100 * stride + 17];
            for i in 0..100 {
                bytes[1 + i * stride..5 + i * stride].copy_from_slice(&10u32.to_le_bytes());
            }
            let at = 1 + 99 * stride;
            bytes[at..at + 4].copy_from_slice(&bad.to_le_bytes());
            let before = bytes.clone();
            let error =
                adjust_directory(&mut bytes, 1, stride as u32, 100, delta, payload).unwrap_err();
            assert_eq!(error, blubber_core::Error::new(kind, at as u32));
            assert_eq!(bytes, before);
        }
    }
    for delta in [i64::MIN, i64::MAX] {
        let mut bytes = [0xa5; 24];
        let before = bytes;
        assert!(adjust_directory(&mut bytes, 1, 4, 4, delta, u32::MAX).is_err());
        assert_eq!(bytes, before);
    }
}

#[test]
fn corrupted_range_geometry_rejects_before_writing_and_empty_runs_ignore_coordinates() {
    for (first, stride, count) in [
        (0, 0, 1),
        (0, 3, 2),
        (61, 4, 1),
        (1, 4, 17),
        (u32::MAX, u32::MAX, u32::MAX),
    ] {
        let mut bytes = [0xa5; 64];
        let before = bytes;
        assert!(adjust_directory(&mut bytes, first, stride, count, 1, u32::MAX).is_err());
        assert_eq!(bytes, before);
    }
    assert_eq!(
        adjust_directory(&mut [], u32::MAX, 0, 0, i64::MIN, 0),
        Ok(())
    );
    let mut bytes = [0xa5; 64];
    let before = bytes;
    adjust_directory(&mut bytes, 1, 4, 15, 0, 0).unwrap();
    assert_eq!(bytes, before);
}
