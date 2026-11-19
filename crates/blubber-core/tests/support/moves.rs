//! Shared host/SBF geometries and an independent directional byte oracle.

/// Visits overlap, adjacency, disjoint, identity, empty and unaligned moves.
pub fn cases(mut visit: impl FnMut(usize, usize, usize)) {
    for distance in [1usize, 31, 32, 63, 64, 127, 128, 1023, 1024, 1025] {
        for count in [
            0,
            1,
            distance - 1,
            distance,
            distance + 1,
            257,
            1023,
            1024,
            1025,
            2049,
            3073,
        ] {
            for start in [1, 3] {
                visit(start, start + count, start + distance);
                visit(start + distance, start + distance + count, start);
            }
        }
    }
    for (start, end, dest) in [
        (0, 0, 0),
        (6144, 6144, 6144),
        (0, 0, 6144),
        (6144, 6144, 0),
        (0, 6144, 0),
        (3, 260, 3),
        (0, 1024, 5120),
        (5120, 6144, 0),
        (0, 5120, 1024),
        (1024, 6144, 0),
    ] {
        visit(start, end, dest);
    }
}

/// Copies one byte at a time away from source bytes that are still unread.
pub fn naive(bytes: &mut [u8], start: usize, end: usize, dest: usize) {
    if dest <= start {
        for offset in 0..end - start {
            bytes[dest + offset] = bytes[start + offset];
        }
    } else {
        for offset in (0..end - start).rev() {
            bytes[dest + offset] = bytes[start + offset];
        }
    }
}

/// Produces position-dependent bytes without a one-byte repeating pattern.
pub fn initial() -> [u8; 6144] {
    core::array::from_fn(|i| (i.wrapping_mul(73) ^ (i >> 8)) as u8)
}
