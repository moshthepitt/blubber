#![forbid(unsafe_code)]

//! Snapshot and frame checks for every byte tail and sub-word overlap.

#[test]
fn small_moves_preserve_snapshots_at_every_length_and_distance() {
    use blubber_core::{raw::move_bytes, Span};
    let before: [u8; 40] = core::array::from_fn(|i| (i * 73 + 19) as u8);
    for count in 0..=17 {
        for distance in 1..=17 {
            for right in [false, true] {
                let source = 2 + if right { 0 } else { distance };
                let destination = 2 + if right { distance } else { 0 };
                let mut actual = before;
                let mut expected = before;
                expected[destination..destination + count]
                    .copy_from_slice(&before[source..source + count]);
                assert_eq!(
                    move_bytes(
                        &mut actual,
                        Span {
                            start: source as u32,
                            end: (source + count) as u32,
                        },
                        destination as u32,
                    ),
                    Ok(())
                );
                assert_eq!(
                    actual, expected,
                    "count={count} distance={distance} right={right}"
                );
            }
        }
    }
}

#[test]
fn word_moves_capture_original_source_in_both_directions_for_every_tail() {
    use blubber_core::{raw::move_bytes, Span};
    // Miri keeps the same word/tail overlaps with a complete smaller frame.
    const CAPACITY: usize = if cfg!(miri) { 64 } else { 1088 };
    for tail in 0..8 {
        for words in if cfg!(miri) {
            &[1usize, 2][..]
        } else {
            &[0usize, 1, 2, 127, 128, 129][..]
        } {
            let words = *words;
            let count = words * 8 + tail;
            for alignment in 0..if cfg!(miri) { 2 } else { 8 } {
                for distance in [1, 2, 3, 4, 5, 6, 7, 8, 9, 16] {
                    for right in [false, true] {
                        let start = alignment + if right { 0 } else { distance };
                        let dest = alignment + if right { distance } else { 0 };
                        let before: [u8; CAPACITY] =
                            core::array::from_fn(|i| (i.wrapping_mul(73) ^ (i >> 8)) as u8);
                        let mut actual = before;
                        let mut expected = before;
                        expected[dest..dest + count].copy_from_slice(&before[start..start + count]);
                        assert_eq!(
                            move_bytes(
                                &mut actual,
                                Span {
                                    start: start as u32,
                                    end: (start + count) as u32,
                                },
                                dest as u32
                            ),
                            Ok(())
                        );
                        assert_eq!(actual, expected,
                            "tail={tail}, words={words}, alignment={alignment}, distance={distance}, right={right}");
                    }
                }
            }
        }
    }
}
