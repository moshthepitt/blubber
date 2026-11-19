//! Full-width directory transport, per-word frames and publication state.
#![forbid(unsafe_code)]

use blubber_core::patch_pass::{checked_end, finish_publication, patch_word};
use blubber_core::{Error, ErrorKind};

#[test]
fn full_width_ends_and_deltas_match_integer_arithmetic() {
    for end in [0, 1, u32::MAX - 1, u32::MAX] {
        for lower in [0, 1, u32::MAX] {
            for upper in [0, u32::MAX - 1, u32::MAX] {
                for delta in [
                    i64::MIN,
                    -4294967296,
                    -4294967295,
                    -1,
                    0,
                    1,
                    4294967295,
                    4294967296,
                    i64::MAX,
                ] {
                    let sum = i128::from(end) + i128::from(delta);
                    let expected = if end < lower || end > upper {
                        Err(Error::new(ErrorKind::InvalidBoundary, 17))
                    } else if !(0..=i128::from(u32::MAX)).contains(&sum) {
                        Err(Error::new(ErrorKind::ArithmeticOverflow, 17))
                    } else {
                        Ok(sum as u32)
                    };
                    assert_eq!(checked_end(end, lower, upper, delta, 17), expected);
                }
            }
        }
    }
}

#[test]
fn later_patch_failure_keeps_completed_words_and_poison() {
    let mut bytes = [1, 0, 0, 0, 255, 255, 255, 255, 12, 0, 0, 0];
    let mut poisoned = false;
    patch_word(&mut bytes, 0, 0, 0, u32::MAX, 2, &mut poisoned).unwrap();
    assert_eq!(&bytes[..4], &3u32.to_le_bytes());
    assert!(poisoned);
    let completed = bytes;
    let result = patch_word(&mut bytes, 4, 4, 0, u32::MAX, 1, &mut poisoned);
    assert_eq!(result, Err(Error::new(ErrorKind::ArithmeticOverflow, 4)));
    assert_eq!(bytes, completed);
    assert!(finish_publication(&mut poisoned, result).is_err());
    assert!(poisoned);
    assert_eq!(&bytes[8..], &12u32.to_le_bytes());
}

#[test]
fn source_and_destination_rejections_preserve_every_byte() {
    let original = [5, 0, 0, 0, 0x51, 0x52, 0x53, 0x54];
    for (source, destination, lower, upper, delta, kind) in [
        (0, 4, 6, 10, 0, ErrorKind::InvalidBoundary),
        (0, 4, 0, 4, 0, ErrorKind::InvalidBoundary),
        (0, 4, 0, 10, i64::MAX, ErrorKind::ArithmeticOverflow),
        (0, 4, 0, 10, i64::MIN, ErrorKind::ArithmeticOverflow),
        (5, 0, 0, 10, 0, ErrorKind::Truncated),
        (u32::MAX, 0, 0, 10, 0, ErrorKind::ArithmeticOverflow),
        (0, 5, 0, 10, 0, ErrorKind::Truncated),
        (0, u32::MAX, 0, 10, 0, ErrorKind::ArithmeticOverflow),
    ] {
        let mut bytes = original;
        let mut poisoned = false;
        let result = patch_word(
            &mut bytes,
            source,
            destination,
            lower,
            upper,
            delta,
            &mut poisoned,
        );
        assert!(matches!(result, Err(Error::Fault { kind: found, .. }) if found == kind));
        assert_eq!(bytes, original);
        assert!(poisoned);
    }
}

#[test]
fn successful_patch_changes_only_the_destination_word() {
    for source in [0, 1, 4] {
        for destination in [0, 1, 4] {
            let mut bytes = [1, 0, 0, 0, 2, 0, 0, 0];
            let original = bytes;
            let end = u32::from_le_bytes(original[source..source + 4].try_into().unwrap());
            let mut poisoned = false;
            patch_word(
                &mut bytes,
                source as u32,
                destination as u32,
                0,
                u32::MAX,
                1,
                &mut poisoned,
            )
            .unwrap();
            let mut expected = original;
            expected[destination..destination + 4].copy_from_slice(&(end + 1).to_le_bytes());
            assert_eq!(bytes, expected);
            assert!(poisoned);
            finish_publication(&mut poisoned, Ok(())).unwrap();
            assert!(!poisoned);
        }
    }
}

#[test]
fn checked_transport_returns_old_end_and_preserves_each_failed_word() {
    for end in [0u32, 1, u32::MAX - 1, u32::MAX] {
        for delta in [i64::MIN, -4294967295, -1, 0, 1, 4294967295, i64::MAX] {
            let mut bytes = [0x51; 12];
            bytes[..4].copy_from_slice(&end.to_le_bytes());
            let before = bytes;
            let mut poisoned = false;
            let result = blubber_core::patch_pass::transport(
                &mut bytes,
                0,
                4,
                0,
                u32::MAX,
                delta,
                &mut poisoned,
            );
            let sum = i128::from(end) + i128::from(delta);
            if (0..=i128::from(u32::MAX)).contains(&sum) {
                assert_eq!(result, Ok(end));
                assert_eq!(&bytes[4..8], &(sum as u32).to_le_bytes());
            } else {
                assert_eq!(result, Err(Error::new(ErrorKind::ArithmeticOverflow, 0)));
                assert_eq!(bytes, before);
            }
            assert!(poisoned);
            assert_eq!(&bytes[..4], &before[..4]);
            assert_eq!(&bytes[8..], &before[8..]);
        }
    }
}

#[test]
fn contained_transport_checks_full_width_results_and_word_frames() {
    for end in [0u32, 1, u32::MAX - 1, u32::MAX] {
        for delta in [-4294967295i64, -1, 0, 1, 4294967295] {
            for (lower, upper) in [(0, u32::MAX), (1, u32::MAX - 1)] {
                let mut bytes = [0x51; 12];
                bytes[..4].copy_from_slice(&end.to_le_bytes());
                let before = bytes;
                let mut poisoned = false;
                let result = blubber_core::patch_pass::transport_contained(
                    &mut bytes,
                    0,
                    4,
                    lower,
                    upper,
                    delta,
                    &mut poisoned,
                );
                let sum = i128::from(end) + i128::from(delta);
                if end < lower || end > upper {
                    assert_eq!(result, Err(Error::new(ErrorKind::InvalidBoundary, 0)));
                    assert_eq!(bytes, before);
                } else if !(0..=i128::from(u32::MAX)).contains(&sum) {
                    assert_eq!(result, Err(Error::new(ErrorKind::ArithmeticOverflow, 0)));
                    assert_eq!(bytes, before);
                } else {
                    assert_eq!(result, Ok(end));
                    assert_eq!(&bytes[4..8], &(sum as u32).to_le_bytes());
                }
                assert!(poisoned);
                assert_eq!(&bytes[..4], &before[..4]);
                assert_eq!(&bytes[8..], &before[8..]);
            }
        }
    }
}

#[test]
fn first_middle_and_last_patch_rejections_keep_completed_prefix_and_shrink_floor() {
    for bad in 0..3 {
        for (bad_end, lower, upper, delta, kind) in [
            (u32::MAX, 0, 12, 1i64, ErrorKind::InvalidBoundary),
            (0, 1, 12, -1, ErrorKind::InvalidBoundary),
            (u32::MAX, 0, u32::MAX, 1, ErrorKind::ArithmeticOverflow),
            (0, 0, u32::MAX, -1, ErrorKind::ArithmeticOverflow),
        ] {
            let mut bytes = [0xa5; 16];
            for i in 0..3 {
                let end = if i == bad { bad_end } else { 4 + 4 * i as u32 };
                bytes[4 * i..4 * i + 4].copy_from_slice(&end.to_le_bytes());
            }
            let before = bytes;
            let mut poisoned = false;
            for i in 0..=bad {
                let result = patch_word(
                    &mut bytes,
                    4 * i as u32,
                    4 * i as u32,
                    lower,
                    upper,
                    delta,
                    &mut poisoned,
                );
                if i == bad {
                    assert_eq!(result, Err(Error::new(kind, 4 * i as u32)));
                    assert!(finish_publication(&mut poisoned, result).is_err());
                } else {
                    assert_eq!(result, Ok(()));
                }
            }
            assert!(poisoned);
            for i in 0..bad {
                let expected = 4 + 4 * i as i64 + delta;
                assert_eq!(&bytes[4 * i..4 * i + 4], &expected.to_le_bytes()[..4]);
            }
            assert_eq!(&bytes[4 * bad..], &before[4 * bad..]);
        }
    }
}
