//! Correspondence between Rust string bytes and the independent wire relation.

use vstd::prelude::*;
use vstd::utf8::*;

verus! {

/// One standard UTF-8 scalar satisfies the independent weighted-byte relation.
pub proof fn scalar_bridge(bytes: Seq<u8>)
    requires bytes.len() > 0, valid_utf8(bytes),
    ensures
        length_of_first_scalar(bytes) == crate::utf8::scalar_width(bytes[0]),
        crate::utf8::scalar_width(bytes[0]) <= bytes.len(),
        crate::utf8::scalar_encoding(bytes, 0, crate::utf8::scalar_width(bytes[0])),
{
    reveal_with_fuel(valid_utf8, 2);
    let a = bytes[0];
    if a <= 0x7f {
    } else if a <= 0xdf {
        let b = bytes[1];
        assert(0xc0 <= a <= 0xdf && 0x80 <= b <= 0xbf);
        assert(codepoint_width_2(a,b) == 64u32 * (a as u32 - 0xc0) + (b as u32 - 0x80))
            by(bit_vector)
            requires 0xc0u8 <= a <= 0xdfu8, 0x80u8 <= b <= 0xbfu8;
    } else if a <= 0xef {
        let b = bytes[1]; let c = bytes[2];
        assert(0xe0 <= a <= 0xef && 0x80 <= b <= 0xbf && 0x80 <= c <= 0xbf);
        assert(codepoint_width_3(a,b,c) == 4096u32 * (a as u32 - 0xe0)
            + 64u32 * (b as u32 - 0x80) + (c as u32 - 0x80))
            by(bit_vector)
            requires 0xe0u8 <= a <= 0xefu8, 0x80u8 <= b <= 0xbfu8, 0x80u8 <= c <= 0xbfu8;
    } else {
        let b = bytes[1]; let c = bytes[2]; let d = bytes[3];
        assert(0xf0 <= a <= 0xf7 && 0x80 <= b <= 0xbf && 0x80 <= c <= 0xbf && 0x80 <= d <= 0xbf);
        assert(codepoint_width_4(a,b,c,d) == 262144u32 * (a as u32 - 0xf0)
            + 4096u32 * (b as u32 - 0x80) + 64u32 * (c as u32 - 0x80) + (d as u32 - 0x80))
            by(bit_vector)
            requires 0xf0u8 <= a <= 0xf7u8, 0x80u8 <= b <= 0xbfu8,
                0x80u8 <= c <= 0xbfu8, 0x80u8 <= d <= 0xbfu8;
    }
}

/// Induct over every scalar in an arbitrary suffix of a Rust byte string.
pub proof fn string_bridge(bytes: Seq<u8>, at: int)
    requires 0 <= at <= bytes.len(), valid_utf8(bytes.subrange(at, bytes.len() as int)),
    ensures crate::utf8::utf8_valid(bytes, at),
    decreases bytes.len() - at,
{
    if at < bytes.len() {
        let suffix = bytes.subrange(at, bytes.len() as int);
        scalar_bridge(suffix);
        let end = at + crate::utf8::scalar_width(bytes[at]);
        assert(crate::utf8::scalar_encoding(bytes, at, end));
        assert(pop_first_scalar(suffix) =~= bytes.subrange(end, bytes.len() as int));
        string_bridge(bytes, end);
    }
    reveal_with_fuel(crate::utf8::utf8_valid, 2);
}

}
