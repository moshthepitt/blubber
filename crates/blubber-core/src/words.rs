//! Checked byte spans and unaligned little-endian words.

use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// A half-open wire region, `[start, end)`.
///
/// Public fields allow descriptor construction. Consumers must check ordering
/// and containment; constructing a `Span` alone establishes no invariant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// First byte of the region.
    pub start: u32,
    /// Exclusive end of the region.
    pub end: u32,
}

/// Exact span acceptance, including empty regions at the parent end.
pub open spec fn span_iff(result: Result<Span>, start: u32, len: u32, limit: u32) -> bool {
    match result {
        Ok(span) => span.start == start && span.end == start as int + len
            && start as int + len <= limit,
        Err(error) => if start as int + len > u32::MAX {
            error == (Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: start })
        } else {
            start as int + len > limit
                && error == (Error::Fault { kind: ErrorKind::InvalidBoundary, offset: start })
        },
    }
}

/// Checks `start + len` for wire overflow, then for containment in `limit`.
///
/// Returns `ArithmeticOverflow` before `InvalidBoundary`; both identify
/// `start`. Equal boundaries and an empty span at `limit` are valid.
pub fn checked_span(start: u32, len: u32, limit: u32) -> (result: Result<Span>)
    ensures span_iff(result, start, len, limit),
{
    match start.checked_add(len) {
        None => Err(Error::new(ErrorKind::ArithmeticOverflow, start)),
        Some(end) => {
            if end > limit {
                Err(Error::new(ErrorKind::InvalidBoundary, start))
            } else {
                Ok(Span { start, end })
            }
        },
    }
}

/// Mathematical little-endian value of four bytes at `at`.
pub open spec fn le_word(bytes: Seq<u8>, at: int) -> int
    recommends 0 <= at, at + 4 <= bytes.len(),
{
    bytes[at] as int + bytes[at + 1] as int * 256
        + bytes[at + 2] as int * 65536 + bytes[at + 3] as int * 16777216
}

/// Restricting a slice preserves each contained directory word.
pub(crate) proof fn word_subrange(bytes:Seq<u8>,start:int,end:int,at:int)
    requires 0<=start<=end<=bytes.len(),0<=at,at+4<=end-start,
    ensures le_word(bytes.subrange(start,end),at)==le_word(bytes,start+at),
{
}

/// A word must fit both the wire address space and the actual slice.
pub open spec fn word_fits(len: int, at: u32) -> bool {
    at as int + 4 <= u32::MAX && at as int + 4 <= len
}

/// The word gateway's ordered fault classification.
pub open spec fn word_error(at: u32) -> Error {
    Error::Fault {
        kind: if at as int + 4 > u32::MAX { ErrorKind::ArithmeticOverflow }
            else { ErrorKind::Truncated },
        offset: at,
    }
}

/// A successful read decodes exactly the mathematical LE value.
pub open spec fn decode_exact(result: Result<u32>, bytes: Seq<u8>, at: u32) -> bool {
    match result {
        Ok(value) => word_fits(bytes.len() as int, at) && value == le_word(bytes, at as int),
        Err(_) => true,
    }
}

/// Rejection occurs exactly outside the word's domain, with the specified fault.
pub open spec fn decode_rejects(result: Result<u32>, bytes: Seq<u8>, at: u32) -> bool {
    match result {
        Ok(_) => word_fits(bytes.len() as int, at),
        Err(error) => !word_fits(bytes.len() as int, at) && error == word_error(at),
    }
}

/// Reads an unaligned four-byte little-endian word.
///
/// An overflowing wire end returns `ArithmeticOverflow`; a short slice returns
/// `Truncated`. Errors identify the start of the word. No alignment is required.
#[inline(always)]
pub fn read_u32(bytes: &[u8], at: u32) -> (result: Result<u32>)
    ensures
        decode_exact(result, bytes@, at),
        decode_rejects(result, bytes@, at),
    no_unwind
{
    let end = match at.checked_add(4) {
        Some(end) => end,
        None => return Err(Error::new(ErrorKind::ArithmeticOverflow, at)),
    };
    if end as usize > bytes.len() {
        return Err(Error::new(ErrorKind::Truncated, at));
    }
    let index = at as usize;
    // Explicit byte arithmetic keeps the endian contract inside the proof.
    Ok(bytes[index] as u32 + bytes[index + 1] as u32 * 256
        + bytes[index + 2] as u32 * 65536 + bytes[index + 3] as u32 * 16777216)
}

/// Reads an exact, prechecked directory word through a bounded four-byte slice.
#[inline(always)]
pub(crate) fn directory_word(bytes:&[u8],at:usize)->(value:u32)
    requires at+4<=bytes.len(),
    ensures value==le_word(bytes@,at as int),
{
    let remaining=&bytes[at..];
    let word=&remaining[..4];
    word[0] as u32 + word[1] as u32*256
        +word[2] as u32*65536+word[3] as u32*16777216
}

/// Monotone stored ends contained in the payload, independent of child bytes.
pub(crate) open spec fn directory_sorted(bytes:Seq<u8>,payload:int,key:int,stride:int,count:int)->bool {
    forall|i:int| 0<=i<count ==> {
        let end=#[trigger] le_word(bytes,4+i*stride+key);
        end<=bytes.len()-payload && (if i==0 {0<=end}else{le_word(bytes,4+(i-1)*stride+key)<=end})
    }
}

/// Checks a prebounded directory with compile-time key width and stride.
#[inline(never)]
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
pub(crate) fn check_directory<const KEY:usize,const STRIDE:usize>(bytes:&[u8],payload:usize,
    Ghost(count):Ghost<int>)->(result:Result<usize>)
    requires KEY+4==STRIDE,0<=count,payload==4+count*STRIDE,payload<=bytes.len()<=u32::MAX,
    ensures result.is_ok()==directory_sorted(bytes@,payload as int,KEY as int,STRIDE as int,count),
        result.is_ok() ==> result.unwrap()==if count==0 {0int}else{le_word(bytes@,4+(count-1)*STRIDE+KEY)},
        result.is_ok() ==> result.unwrap()<=bytes.len()-payload,
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(),bytes@.len()),
{
    hide(le_word);
    let limit=bytes.len()-payload;
    let mut remaining=&bytes[4..payload];
    let mut previous=0usize;
    let ghost mut i=0int;
    while !remaining.is_empty()
        invariant KEY+4==STRIDE,0<=i<=count,0<=count,payload==4+count*STRIDE,
            payload<=bytes.len()<=u32::MAX,limit==bytes.len()-payload,previous<=limit,
            remaining@==bytes@.subrange(4+i*STRIDE,payload as int),remaining.len()==(count-i)*STRIDE,
            previous==if i==0 {0int}else{le_word(bytes@,4+(i-1)*STRIDE+KEY)},
            directory_sorted(bytes@,payload as int,KEY as int,STRIDE as int,i),
        decreases count-i,
    {
        proof {
            assert(i<count && remaining.len()>=STRIDE) by(nonlinear_arith)
                requires remaining.len()>0,remaining.len()==(count-i)*STRIDE,STRIDE>0,0<=i<=count;
            assert(4+i*STRIDE+STRIDE<=payload) by(nonlinear_arith)
                requires 0<=i<count,payload==4+count*STRIDE,STRIDE>0;
            word_subrange(bytes@,4+i*STRIDE,payload as int,KEY as int);
        }
        let end=directory_word(remaining,KEY) as usize;
        if end<previous || end>limit {
            let at=payload-remaining.len()+KEY;
            proof {
                assert(at==4+i*STRIDE+KEY);
                assert(at+4<=payload);
                assert(!directory_sorted(bytes@,payload as int,KEY as int,STRIDE as int,count));
            }
            return Err(Error::new(ErrorKind::InvalidBoundary,at as u32));
        }
        previous=end;
        proof {
            assert(4+(i+1)*STRIDE==4+i*STRIDE+STRIDE) by(nonlinear_arith);
            assert((count-i)*STRIDE-STRIDE==(count-(i+1))*STRIDE) by(nonlinear_arith);
            assert(remaining@.subrange(STRIDE as int,remaining.len() as int)
                =~=bytes@.subrange(4+(i+1)*STRIDE,payload as int));
            assert(directory_sorted(bytes@,payload as int,KEY as int,STRIDE as int,i+1));
        }
        remaining=&remaining[STRIDE..];
        proof {i=i+1;}
    }
    assert(i==count) by(nonlinear_arith)
        requires remaining.len()==(count-i)*STRIDE,remaining.len()==0,STRIDE>0,0<=i<=count;
    Ok(previous)
}

/// Decodes an admitted key block with safe byte reads.
#[inline(always)]
pub(crate) fn key_word64(bytes:&[u8])->(value:u64)
    requires bytes.len()==8,
    ensures value==vstd::bytes::spec_u64_from_le_bytes(bytes@),
    no_unwind
{
    let value=(bytes[0] as u64) | ((bytes[1] as u64)<<8) | ((bytes[2] as u64)<<16) | ((bytes[3] as u64)<<24) | ((bytes[4] as u64)<<32) | ((bytes[5] as u64)<<40) | ((bytes[6] as u64)<<48) | ((bytes[7] as u64)<<56);
    proof {
        let a0=bytes@[0];
        let a1=bytes@[1];
        let a2=bytes@[2];
        let a3=bytes@[3];
        let a4=bytes@[4];
        let a5=bytes@[5];
        let a6=bytes@[6];
        let a7=bytes@[7];
        assert((value&0xff) as u8==a0 && ((value>>8)&0xff) as u8==a1 && ((value>>16)&0xff) as u8==a2 && ((value>>24)&0xff) as u8==a3 && ((value>>32)&0xff) as u8==a4 && ((value>>40)&0xff) as u8==a5 && ((value>>48)&0xff) as u8==a6 && ((value>>56)&0xff) as u8==a7) by(bit_vector)
            requires value==((a0 as u64) | ((a1 as u64)<<8) | ((a2 as u64)<<16) | ((a3 as u64)<<24) | ((a4 as u64)<<32) | ((a5 as u64)<<40) | ((a6 as u64)<<48) | ((a7 as u64)<<56));
        vstd::bytes::lemma_auto_spec_u64_to_from_le_bytes();
        vstd::bytes::spec_u64_to_le_bytes_to_open(value);
        assert(vstd::bytes::spec_u64_to_le_bytes_open(value) =~= bytes@);
        assert(vstd::bytes::spec_u64_to_le_bytes(value)==bytes@);
    }
    value
}

/// Reads a header word through two bounded four-byte loans.
#[inline(always)]
pub(crate) fn header_word64(bytes:&[u8])->(value:u64)
    requires bytes.len()==8,
    ensures value==vstd::bytes::spec_u64_from_le_bytes(bytes@),
{
    let low=directory_word(bytes,0);
    let high=directory_word(bytes,4);
    let value=(low as u64) | ((high as u64)<<32);
    proof {
        let a0=bytes@[0];
        let a1=bytes@[1];
        let a2=bytes@[2];
        let a3=bytes@[3];
        let a4=bytes@[4];
        let a5=bytes@[5];
        let a6=bytes@[6];
        let a7=bytes@[7];
        assert(low==(((a0 as u32)<<0) | ((a1 as u32)<<8) | ((a2 as u32)<<16) | ((a3 as u32)<<24))) by(bit_vector) requires low==1*a0 as int + 256*a1 as int + 65536*a2 as int + 16777216*a3 as int;
        assert(high==(((a4 as u32)<<0) | ((a5 as u32)<<8) | ((a6 as u32)<<16) | ((a7 as u32)<<24))) by(bit_vector) requires high==1*a4 as int + 256*a5 as int + 65536*a6 as int + 16777216*a7 as int;
        assert((value&0xff) as u8==a0 && ((value>>8)&0xff) as u8==a1 && ((value>>16)&0xff) as u8==a2 && ((value>>24)&0xff) as u8==a3 && ((value>>32)&0xff) as u8==a4 && ((value>>40)&0xff) as u8==a5 && ((value>>48)&0xff) as u8==a6 && ((value>>56)&0xff) as u8==a7) by(bit_vector)
            requires value==(low as u64)|((high as u64)<<32),low==(((a0 as u32)<<0) | ((a1 as u32)<<8) | ((a2 as u32)<<16) | ((a3 as u32)<<24)),high==(((a4 as u32)<<0) | ((a5 as u32)<<8) | ((a6 as u32)<<16) | ((a7 as u32)<<24));
        vstd::bytes::lemma_auto_spec_u64_to_from_le_bytes();
        vstd::bytes::spec_u64_to_le_bytes_to_open(value);
        assert(vstd::bytes::spec_u64_to_le_bytes_open(value) =~= bytes@);
        assert(vstd::bytes::spec_u64_to_le_bytes(value)==bytes@);
    }
    value
}

/// Adds prechecked coordinates without repeating an overflow branch.
#[inline(always)]
pub(crate) fn add_bounded(a:usize,b:usize)->(result:usize)
    requires a+b<=usize::MAX,
    ensures result==a+b,
    no_unwind
{
    proof {vstd::arithmetic::div_mod::lemma_small_mod(a as nat+b as nat,usize::MAX as nat+1);}
    a.wrapping_add(b)
}

/// Subtracts ordered coordinates without repeating an underflow branch.
#[inline(always)]
pub(crate) fn sub_bounded(a:usize,b:usize)->(result:usize)
    requires b<=a,
    ensures result==a-b,
    no_unwind
{
    proof {vstd::arithmetic::div_mod::lemma_small_mod((a-b) as nat,usize::MAX as nat+1);}
    a.wrapping_sub(b)
}

/// Exact encoded bytes in the destination word.
pub open spec fn encode_exact(bytes: Seq<u8>, at: int, value: u32) -> bool {
    &&& bytes[at] == value as int % 256
    &&& bytes[at + 1] == value as int / 256 % 256
    &&& bytes[at + 2] == value as int / 65536 % 256
    &&& bytes[at + 3] == value as int / 16777216
}

/// Decoding and re-encoding a contained word preserves its four bytes.
#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
pub proof fn word_roundtrip(bytes: Seq<u8>, at: int)
    requires 0 <= at, at + 4 <= bytes.len(),
    ensures encode_exact(bytes, at, le_word(bytes, at) as u32),
{ }

/// Bytes outside the destination word retain their original values.
pub open spec fn encode_frame(before: Seq<u8>, after: Seq<u8>, at: int) -> bool {
    &&& after.len() == before.len()
    &&& forall|i: int| 0 <= i < before.len() && !(at <= i < at + 4)
        ==> after[i] == before[i]
}

/// Every failed write preserves the complete supplied buffer.
pub open spec fn encode_no_write_on_error(
    result: Result<()>, before: Seq<u8>, after: Seq<u8>, at: u32,
) -> bool {
    match result {
        Ok(()) => word_fits(before.len() as int, at),
        Err(error) => !word_fits(before.len() as int, at)
            && error == word_error(at) && after == before,
    }
}

/// Writes an unaligned four-byte little-endian word after checking all bounds.
///
/// Returns the same faults as [`read_u32`]. Every error leaves the entire
/// supplied slice unchanged; success changes only `[at, at + 4)`.
#[inline(always)]
pub fn write_u32(bytes: &mut [u8], at: u32, value: u32) -> (result: Result<()>)
    ensures
        match result {
            Ok(()) => word_fits(old(bytes)@.len() as int, at)
                && encode_exact(final(bytes)@, at as int, value)
                && le_word(final(bytes)@, at as int) == value
                && encode_frame(old(bytes)@, final(bytes)@, at as int),
            Err(_) => true,
        },
        encode_no_write_on_error(result, old(bytes)@, final(bytes)@, at),
    no_unwind
{
    let end = match at.checked_add(4) {
        Some(end) => end,
        None => return Err(Error::new(ErrorKind::ArithmeticOverflow, at)),
    };
    if end as usize > bytes.len() {
        return Err(Error::new(ErrorKind::Truncated, at));
    }
    let index = at as usize;
    bytes[index] = (value % 256) as u8;
    bytes[index + 1] = (value / 256 % 256) as u8;
    bytes[index + 2] = (value / 65536 % 256) as u8;
    bytes[index + 3] = (value / 16777216) as u8;
    Ok(())
}

/// Decodes a word whose complete range was admitted by its caller.
#[inline(always)]
pub(crate) fn read_contained_word(bytes: &[u8], at: usize) -> (result: u32)
    requires at as int + 4 <= bytes.len(),
    ensures result == le_word(bytes@, at as int),
    no_unwind
{
    let (_, suffix) = bytes.split_at(at);
    let (word, _) = suffix.split_at(4);
    word[0] as u32 + word[1] as u32 * 256
        + word[2] as u32 * 65536 + word[3] as u32 * 16777216
}

/// Encodes one admitted word and preserves every other physical byte.
#[inline(always)]
pub(crate) fn write_contained_word(bytes: &mut [u8], at: usize, value: u32)
    requires at as int + 4 <= old(bytes).len(),
    ensures encode_exact(final(bytes)@, at as int, value),
        le_word(final(bytes)@, at as int) == value,
        encode_frame(old(bytes)@, final(bytes)@, at as int),
    no_unwind
{
    let (_, suffix) = bytes.split_at_mut(at);
    let (word, _) = suffix.split_at_mut(4);
    word[0] = (value % 256) as u8;
    word[1] = (value / 256 % 256) as u8;
    word[2] = (value / 65536 % 256) as u8;
    word[3] = (value / 16777216) as u8;
}

}
