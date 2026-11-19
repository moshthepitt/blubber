//! UTF-8 byte validation against canonical Unicode scalar encodings.

use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// Canonical encoding of one scalar, specified by byte prefixes and its value.
///
/// The weighted sums remove byte prefixes from [RFC 3629, section 3]'s table.
/// Minimal values exclude overlong forms; U+D800..=U+DFFF are not scalars.
///
/// [RFC 3629, section 3]: https://www.rfc-editor.org/rfc/rfc3629.html#section-3
pub open spec fn scalar_encoding(bytes: Seq<u8>, start: int, end: int) -> bool {
    0 <= start < end <= bytes.len() && if end - start == 1 { bytes[start] <= 0x7f }
        else if end - start == 2 {
            let a = bytes[start] as int;
            let b = bytes[start + 1] as int;
            let value = 64 * (a - 0xc0) + (b - 0x80);
            0xc0 <= a <= 0xdf && 0x80 <= b <= 0xbf && 0x80 <= value <= 0x7ff
        } else if end - start == 3 {
            let a = bytes[start] as int;
            let b = bytes[start + 1] as int;
            let c = bytes[start + 2] as int;
            let value = 4096 * (a - 0xe0) + 64 * (b - 0x80) + (c - 0x80);
            0xe0 <= a <= 0xef && 0x80 <= b <= 0xbf && 0x80 <= c <= 0xbf
                && 0x800 <= value <= 0xffff && !(0xd800 <= value <= 0xdfff)
        } else if end - start == 4 {
            let a = bytes[start] as int;
            let b = bytes[start + 1] as int;
            let c = bytes[start + 2] as int;
            let d = bytes[start + 3] as int;
            let value = 262144 * (a - 0xf0) + 4096 * (b - 0x80)
                + 64 * (c - 0x80) + (d - 0x80);
            0xf0 <= a <= 0xf7 && 0x80 <= b <= 0xbf && 0x80 <= c <= 0xbf
                && 0x80 <= d <= 0xbf && 0x10000 <= value <= 0x10ffff
        } else { false }
}

/// Width indicated by a leading byte; scalar validity separately rejects it.
pub open spec fn scalar_width(first: u8) -> int {
    if first <= 0x7f { 1 } else if first <= 0xdf { 2 }
    else if first <= 0xef { 3 } else { 4 }
}

/// The complete suffix consists of canonical scalar encodings.
pub open spec fn utf8_valid(bytes: Seq<u8>, at: int) -> bool
    decreases bytes.len() - at,
{
    if !(0 <= at <= bytes.len()) { false }
    else if at == bytes.len() { true }
    else {
        let end = at + scalar_width(bytes[at]);
        end <= bytes.len() && scalar_encoding(bytes, at, end) && utf8_valid(bytes, end)
    }
}

/// A prefix has a partition into canonical scalars, independent of the scan.
pub open spec fn scalar_prefix(bytes: Seq<u8>, end: int) -> bool
    decreases end,
{
    0 <= end <= bytes.len() && (end == 0 || exists|start: int| #[trigger]
        scalar_encoding(bytes, start, end) && 0 <= start < end && scalar_prefix(bytes, start))
}

/// Acceptance iff the entire wire-sized slice is UTF-8, with exact fault framing.
pub open spec fn utf8_iff(result: Result<()>, bytes: Seq<u8>) -> bool {
    result.is_ok() == (bytes.len() <= u32::MAX && utf8_valid(bytes, 0))
        && match result {
            Ok(()) => true,
            Err(error) => if bytes.len() > u32::MAX {
                error == (Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: u32::MAX })
            } else {
                match error {
                    Error::Fault { kind: ErrorKind::InvalidUtf8, offset } =>
                        offset < bytes.len() && scalar_prefix(bytes, offset as int)
                        && !scalar_encoding(bytes, offset as int,
                            offset as int + scalar_width(bytes[offset as int])),
                    _ => false,
                }
            },
        }
}

// LLVM lowers the explicit byte assembly to an unaligned word load on SBF.
// The same safe body is proved; there is no platform UTF-8 assumption.
#[inline(always)]
fn ascii8(bytes:&[u8],at:usize)->(result:bool)
    requires at+8<=bytes.len(),
    ensures result == (forall|i:int| at<=i<at+8 ==> #[trigger] bytes@[i]<=0x7f),
{
    let chunk=&bytes[at..at+8];
    let word=crate::words::key_word64(chunk);
    let ascii=word & 0x8080808080808080u64 == 0;
    proof {
        vstd::bytes::lemma_auto_spec_u64_to_from_le_bytes();
        vstd::bytes::spec_u64_to_le_bytes_to_open(word);
        assert(vstd::bytes::spec_u64_to_le_bytes_open(word)==chunk@);
        assert((word & 0x8080808080808080u64 == 0) == (
            (word & 0xff) as u8<=0x7f && ((word>>8)&0xff) as u8<=0x7f
            && ((word>>16)&0xff) as u8<=0x7f && ((word>>24)&0xff) as u8<=0x7f
            && ((word>>32)&0xff) as u8<=0x7f && ((word>>40)&0xff) as u8<=0x7f
            && ((word>>48)&0xff) as u8<=0x7f && ((word>>56)&0xff) as u8<=0x7f
        )) by(bit_vector);
        if ascii {
            assert forall|i:int| at<=i<at+8 implies #[trigger] bytes@[i]<=0x7f by {
                assert(chunk@[i-at]<=0x7f);
            }
        }
    }
    ascii
}

/// Certifies ASCII payloads in word blocks; a false result uses the full decoder.
#[inline(never)]
pub(crate) fn all_ascii(bytes:&[u8])->(result:bool)
    ensures result ==> forall|i:int| 0<=i<bytes.len() ==> #[trigger] bytes@[i]<=0x7f,
{
    let mut remaining=bytes;
    let ghost mut at=0int;
    while remaining.len()>=32
        invariant 0<=at<=bytes.len(),remaining@==bytes@.subrange(at,bytes.len() as int),
            remaining.len()==bytes.len()-at,
            forall|i:int| 0<=i<at ==> #[trigger] bytes@[i]<=0x7f,
        decreases remaining.len(),
    {
        let block=&remaining[..32];
        let a=ascii8(block,0);
        if !a {return false;}
        let b=ascii8(block,8);
        if !b {return false;}
        let c=ascii8(block,16);
        if !c {return false;}
        let d=ascii8(block,24);
        if !d {return false;}
        proof {
            assert forall|i:int| at<=i<at+32 implies #[trigger] bytes@[i]<=0x7f by {
                assert(block@[i-at]==bytes@[i]);
            }
        }
        remaining=&remaining[32..];
        proof {at=at+32;}
    }
    while !remaining.is_empty()
        invariant 0<=at<=bytes.len(),remaining@==bytes@.subrange(at,bytes.len() as int),
            remaining.len()==bytes.len()-at,
            forall|i:int| 0<=i<at ==> #[trigger] bytes@[i]<=0x7f,
        decreases remaining.len(),
    {
        if remaining[0]>0x7f {return false;}
        remaining=&remaining[1..];
        proof {at=at+1;}
    }
    true
}

/// Every ASCII byte is a complete one-byte UTF-8 scalar.
pub(crate) proof fn ascii_valid(bytes:Seq<u8>)
    requires forall|i:int| 0<=i<bytes.len() ==> #[trigger] bytes[i]<=0x7f,
    ensures utf8_valid(bytes,0),
{
    ascii_run(bytes,0,bytes.len() as int);
}

proof fn ascii_run(bytes:Seq<u8>,at:int,end:int)
    requires 0<=at<=end<=bytes.len(), scalar_prefix(bytes,at),
        forall|i:int| at<=i<end ==> #[trigger] bytes[i]<=0x7f,
    ensures scalar_prefix(bytes,end), utf8_valid(bytes,at)==utf8_valid(bytes,end),
    decreases end-at,
{
    if at<end {
        reveal_with_fuel(utf8_valid,2);
        assert(scalar_encoding(bytes,at,at+1));
        assert(scalar_prefix(bytes,at+1));
        ascii_run(bytes,at+1,end);
    }
}

/// Checks an entire parent-delimited string without constructing a string reference.
///
/// Accepts canonical UTF-8, including empty strings, NUL, BOM and noncharacters.
/// Rejects overlong forms, surrogates, values above U+10FFFF, stray continuation
/// bytes, and incomplete scalars. `InvalidUtf8` identifies the leading byte of
/// the first invalid sequence, as `core::str::from_utf8`'s `valid_up_to()` does.
/// A length above `u32::MAX` returns `ArithmeticOverflow` with `NO_OFFSET` first.
/// Uses constant space, no allocation or unsafe code, and at most one byte scan.
#[inline(always)]
pub fn validate_utf8(bytes: &[u8]) -> (result: Result<()>)
    ensures utf8_iff(result, bytes@),
{
    if bytes.len()==8 && ascii8(bytes,0) {
        proof {ascii_run(bytes@,0,8);}
        return Ok(());
    }
    validate_utf8_inner(bytes)
}

fn validate_utf8_inner(bytes: &[u8]) -> (result: Result<()>)
    ensures utf8_iff(result, bytes@),
{
    if bytes.len() > u32::MAX as usize {
        return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
    }
    let mut at = 0usize;
    while at < bytes.len()
        invariant
            at <= bytes.len() <= u32::MAX,
            utf8_valid(bytes@, 0) == utf8_valid(bytes@, at as int),
            scalar_prefix(bytes@, at as int),
        decreases bytes.len() - at,
    {
        if bytes.len()-at>=8 && ascii8(bytes,at) {
            proof { ascii_run(bytes@,at as int,at+8); }
            at+=8;
            continue;
        }
        let first = bytes[at];
        let (width, lower, upper): (usize, u8, u8) = match first {
            0x00..=0x7f => (1, 0x80, 0xbf),
            0xc2..=0xdf => (2, 0x80, 0xbf),
            0xe0 => (3, 0xa0, 0xbf),
            0xe1..=0xec | 0xee..=0xef => (3, 0x80, 0xbf),
            0xed => (3, 0x80, 0x9f),
            0xf0 => (4, 0x90, 0xbf),
            0xf1..=0xf3 => (4, 0x80, 0xbf),
            0xf4 => (4, 0x80, 0x8f),
            _ => return Err(Error::new(ErrorKind::InvalidUtf8, at as u32)),
        };
        // Subtract before adding: even a maximum wire-sized slice cannot wrap.
        if bytes.len() - at < width {
            return Err(Error::new(ErrorKind::InvalidUtf8, at as u32));
        }
        if width > 1 && (bytes[at + 1] < lower || bytes[at + 1] > upper) {
            return Err(Error::new(ErrorKind::InvalidUtf8, at as u32));
        }
        if width > 2 && (bytes[at + 2] < 0x80 || bytes[at + 2] > 0xbf) {
            return Err(Error::new(ErrorKind::InvalidUtf8, at as u32));
        }
        if width > 3 && (bytes[at + 3] < 0x80 || bytes[at + 3] > 0xbf) {
            return Err(Error::new(ErrorKind::InvalidUtf8, at as u32));
        }
        proof {
            assert(scalar_encoding(bytes@, at as int, at as int + width));
            assert(scalar_prefix(bytes@, at as int + width)) by {
                reveal_with_fuel(scalar_prefix, 2);
                assert(exists|start: int| 0 <= start < at + width
                    && scalar_encoding(bytes@, start, at as int + width)
                    && scalar_prefix(bytes@, start));
            }
        }
        at += width;
    }
    Ok(())
}

}
