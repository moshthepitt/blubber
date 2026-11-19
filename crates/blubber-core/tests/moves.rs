#![forbid(unsafe_code)]

//! Original-source semantics, exact move footprints, and complete error frames.

use blubber_core::raw::move_bytes;
use blubber_core::{Error, ErrorKind, Span};

#[test]
fn every_small_move_matches_an_independent_source_snapshot() {
    let before: [u8; 16] = core::array::from_fn(|i| (i as u8).wrapping_mul(17));
    for len in 0..=before.len() {
        for start in 0..=len {
            for end in start..=len {
                for dst in 0..=len - (end - start) {
                    let mut actual = before;
                    let mut expected = before;
                    expected[dst..dst + end - start].copy_from_slice(&before[start..end]);
                    assert_eq!(
                        move_bytes(
                            &mut actual[..len],
                            Span {
                                start: start as u32,
                                end: end as u32,
                            },
                            dst as u32,
                        ),
                        Ok(()),
                        "len {len}, source {start}..{end}, destination {dst}",
                    );
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}

#[test]
fn overlap_and_disjoint_golden_moves_preserve_the_destination_frame() {
    let before = *b"abcdefghij";
    for (start, end, dst, expected) in [
        (0, 8, 2, *b"ababcdefgh"),  // Right overlap.
        (2, 10, 0, *b"cdefghijij"), // Left overlap.
        (0, 2, 8, *b"abcdefghab"),  // Disjoint right.
        (8, 10, 0, *b"ijcdefghij"), // Disjoint left.
        (0, 5, 5, *b"abcdeabcde"),  // Adjacent regions.
        (2, 8, 2, before),          // Identity.
        (0, 10, 0, before),         // Full slice.
        (10, 10, 0, before),        // Empty source at end.
        (0, 0, 10, before),         // Empty destination at end.
    ] {
        let mut actual = before;
        assert_eq!(move_bytes(&mut actual, Span { start, end }, dst), Ok(()));
        assert_eq!(actual, expected);
    }
    assert_eq!(move_bytes(&mut [], Span { start: 0, end: 0 }, 0), Ok(()));
}

#[test]
fn arbitrary_ranges_have_exact_faults_and_never_partially_write() {
    let before = [0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe];
    let positions = [0, 1, 2, 4, 7, 8, 9, u32::MAX - 8, u32::MAX - 1, u32::MAX];
    for len in 0..=before.len() {
        for start in positions {
            for end in positions {
                for dst in positions {
                    // Widened arithmetic keeps this oracle independent of
                    // production's checked subtraction and addition.
                    let count = end as i64 - start as i64;
                    let destination_end = dst as i64 + count;
                    let fault = if count < 0 {
                        Some(Error::new(ErrorKind::InvalidBoundary, start))
                    } else if end as usize > len {
                        Some(Error::new(ErrorKind::Truncated, start))
                    } else if destination_end > u32::MAX as i64 {
                        Some(Error::new(ErrorKind::ArithmeticOverflow, dst))
                    } else if destination_end > len as i64 {
                        Some(Error::new(ErrorKind::Truncated, dst))
                    } else {
                        None
                    };
                    let mut actual = before;
                    let result = move_bytes(&mut actual[..len], Span { start, end }, dst);
                    if let Some(error) = fault {
                        assert_eq!(result, Err(error));
                        assert_eq!(actual, before, "failed move changed supplied storage");
                    } else {
                        assert_eq!(result, Ok(()));
                        let mut expected = before;
                        expected[dst as usize..destination_end as usize]
                            .copy_from_slice(&before[start as usize..end as usize]);
                        assert_eq!(actual, expected);
                    }
                }
            }
        }
    }
}

#[test]
fn empty_and_identity_moves_still_validate_all_bounds() {
    let before = [0xa5; 4];
    for (src, dst, fault) in [
        (Span { start: 5, end: 5 }, 0, (ErrorKind::Truncated, 5)),
        (Span { start: 0, end: 0 }, 5, (ErrorKind::Truncated, 5)),
        (
            Span { start: 0, end: 0 },
            u32::MAX,
            (ErrorKind::Truncated, u32::MAX),
        ),
        (Span { start: 0, end: 5 }, 0, (ErrorKind::Truncated, 0)),
        (
            Span { start: 4, end: 3 },
            4,
            (ErrorKind::InvalidBoundary, 4),
        ),
        (
            Span { start: 0, end: 1 },
            u32::MAX,
            (ErrorKind::ArithmeticOverflow, u32::MAX),
        ),
        (
            Span { start: 0, end: 4 },
            u32::MAX - 3,
            (ErrorKind::ArithmeticOverflow, u32::MAX - 3),
        ),
    ] {
        let mut actual = before;
        assert_eq!(
            move_bytes(&mut actual, src, dst),
            Err(Error::new(fault.0, fault.1))
        );
        assert_eq!(actual, before);
    }
}

#[path = "support/moves.rs"]
mod support;

#[test]
fn chunk_boundaries_and_unaligned_overlap_match_naive_loop() {
    let mut executed = 0;
    support::cases(|start, end, dest| {
        let mut actual = support::initial();
        let mut expected = actual;
        support::naive(&mut expected, start, end, dest);
        move_bytes(
            &mut actual,
            Span {
                start: start as u32,
                end: end as u32,
            },
            dest as u32,
        )
        .unwrap();
        assert_eq!(actual, expected, "{start}..{end} -> {dest}");
        executed += 1;
    });
    assert_eq!(executed, 450);
}

#[test]
fn every_small_range_matches_naive_loop_including_empty_slices() {
    for len in 0..=16 {
        for start in 0..=len {
            for end in start..=len {
                for dest in 0..=len - (end - start) {
                    let mut actual = support::initial();
                    let mut expected = actual;
                    support::naive(&mut expected[..len], start, end, dest);
                    move_bytes(
                        &mut actual[..len],
                        Span {
                            start: start as u32,
                            end: end as u32,
                        },
                        dest as u32,
                    )
                    .unwrap();
                    assert_eq!(actual, expected, "len={len}; {start}..{end} -> {dest}");
                }
            }
        }
    }
}

#[path = "support/word_moves.rs"]
mod word_moves;
