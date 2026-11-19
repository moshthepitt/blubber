//! Immutable variable-map selection with retained directory and payload coordinates.

use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// Borrows the key at an admitted key/end directory coordinate.
///
/// # Panics
/// Coordinates outside the admitted directory may panic.
#[doc(hidden)]
#[inline(always)]
pub fn key(bytes: &[u8], _payload: usize, at: usize, width: usize) -> (key: &[u8])
    requires _payload <= bytes.len() <= u32::MAX, 4 <= at,
        width > 0, at + width + 4 <= _payload,
    ensures key@ == bytes@.subrange(at as int, (at + width) as int), key.len() == width,
    no_unwind
{
    let (_, rest) = bytes.split_at(at);
    let (key, _) = rest.split_at(width);
    key
}

/// Borrows a value after its preceding end has already been checked.
/// The selected end, including the stored terminal, is checked on every step.
///
/// # Panics
/// Missing directory admission or an out-of-payload prior end may panic.
#[doc(hidden)]
#[inline(always)]
pub fn region(bytes: &[u8], payload: usize, at: usize, last: bool, prior: usize)
    -> (result: Result<(&[u8], u32)>)
    requires payload <= bytes.len() <= u32::MAX, 4 <= at, at + 4 <= payload,
        prior <= bytes.len() - payload,
    ensures result.is_ok() == (prior <= crate::words::le_word(bytes@, at as int) <= bytes.len() - payload
            && (!last || crate::words::le_word(bytes@, at as int) == bytes.len() - payload)),
        result.is_ok() ==> {
            let (chunk, start) = result.unwrap();
            &&& start == payload + prior
            &&& chunk@ == bytes@.subrange(payload + prior,
                payload + crate::words::le_word(bytes@, at as int))
            &&& chunk.len() == crate::words::le_word(bytes@, at as int) - prior
        },
        result.is_err() ==> result.unwrap_err() == (Error::Fault { kind: ErrorKind::InvalidBoundary, offset: at as u32 }),
    no_unwind
{
    let end = crate::raw::indexed_read_word(bytes, at) as usize;
    if end < prior || end > bytes.len() - payload || (last && end != bytes.len() - payload) {
        return Err(Error::new(ErrorKind::InvalidBoundary, at as u32));
    }
    let start = crate::raw::indexed_coordinate(payload, prior, 1);
    let (_, rest) = bytes.split_at(start);
    let (chunk, _) = rest.split_at(end - prior);
    Ok((chunk, start as u32))
}

/// Header admission without certifying key domains or unvisited ends.
pub open spec fn admitted(bytes: Seq<u8>, payload: int, count: int, width: int) -> bool {
    &&& 0 < width <= u32::MAX
    &&& 0 <= count <= u32::MAX
    &&& payload == 4 + count * (width + 4)
    &&& payload <= bytes.len() <= u32::MAX
    &&& count == crate::words::le_word(bytes, 0)
    &&& count == 0 ==> bytes.len() == 4
}

/// The key's physical directory coordinate.
pub open spec fn coordinate(index: int, width: int) -> int { 4 + index * (width + 4) }

/// The preceding stored end, or the implicit initial zero.
pub open spec fn previous(bytes: Seq<u8>, index: int, width: int) -> int {
    if index == 0 { 0 } else { crate::words::le_word(bytes, coordinate(index-1,width)+width) as int }
}

/// The selected stored end, including the terminal word.
pub open spec fn current(bytes: Seq<u8>, index: int, width: int) -> int {
    crate::words::le_word(bytes, coordinate(index,width)+width) as int
}

/// Independent point selection, including the preceding-word error.
pub open spec fn point(bytes: Seq<u8>, payload: int, count: int, index: int, width: int) -> Result<crate::Span> {
    let low = previous(bytes,index,width);
    let high = current(bytes,index,width);
    if low > bytes.len()-payload {
        Err(Error::Fault {kind:ErrorKind::InvalidBoundary,
            offset:(coordinate(if index == 0 {0} else {index-1},width)+width) as u32})
    } else if high < low || high > bytes.len()-payload || (index+1 == count && high != bytes.len()-payload) {
        Err(Error::Fault {kind:ErrorKind::InvalidBoundary,offset:(coordinate(index,width)+width) as u32})
    } else { Ok(crate::Span {start:(payload+low) as u32,end:(payload+high) as u32}) }
}

/// Streamed selection from a retained, checked prior end.
pub open spec fn selected(bytes: Seq<u8>, payload: int, count: int, index: int, width: int, prior: int) -> Result<crate::Span> {
    let high = current(bytes,index,width);
    if high < prior || high > bytes.len()-payload || (index+1 == count && high != bytes.len()-payload) {
        Err(Error::Fault {kind:ErrorKind::InvalidBoundary,offset:(coordinate(index,width)+width) as u32})
    } else { Ok(crate::Span {start:(payload+prior) as u32,end:(payload+high) as u32}) }
}

/// Retained coordinates select the original key and exact point-read value.
pub proof fn map_directory_payload_correspondence(bytes: Seq<u8>, payload: int, count: int,
    index: int, width: int, at: int, prior: int)
    requires admitted(bytes,payload,count,width), 0 <= index < count,
        at == coordinate(index,width), prior == previous(bytes,index,width),
        0 <= prior <= bytes.len()-payload,
    ensures 4 <= at, at+width+4 <= payload,
        selected(bytes,payload,count,index,width,prior) == point(bytes,payload,count,index,width),
        bytes.subrange(at,at+width) == bytes.subrange(coordinate(index,width),coordinate(index,width)+width),
        selected(bytes,payload,count,index,width,prior).is_ok() ==> {
            let span = selected(bytes,payload,count,index,width,prior).unwrap();
            &&& payload <= span.start <= span.end <= bytes.len()
            &&& bytes.subrange(span.start as int,span.end as int)
                == bytes.subrange(payload+prior,payload+current(bytes,index,width))
        },
{
    assert(index * (width+4) + width+4 <= count * (width+4)) by(nonlinear_arith)
        requires 0 <= index < count, width > 0;
}

/// Key faults precede selection; value faults follow successful selection.
/// Every error fuses the state, and successful steps retain the selected end.
pub open spec fn step(bytes: Seq<u8>, payload: int, count: int, index: int, width: int, prior: int,
    key_error: Option<Error>, value_error: Option<Error>) -> (int,int,Option<Result<crate::Span>>)
{
    if index == count { (count,prior,None) }
    else {
        let region = selected(bytes,payload,count,index,width,prior);
        let result = if key_error.is_some() { Err(key_error.unwrap()) }
            else if region.is_err() { region }
            else if value_error.is_some() { Err(value_error.unwrap()) } else { region };
        if result.is_err() { (count,prior,Some(result)) }
        else { (index+1,current(bytes,index,width),Some(result)) }
    }
}

/// Progress is bounded by the admitted count; a fault is yielded only once.
pub proof fn map_bounded_progress(bytes: Seq<u8>, payload: int, count: int, index: int,
    width: int, prior: int, key_error: Option<Error>, value_error: Option<Error>)
    requires admitted(bytes,payload,count,width), 0 <= index <= count,
        0 <= prior <= bytes.len()-payload,
    ensures {
        let next = step(bytes,payload,count,index,width,prior,key_error,value_error);
        &&& index <= next.0 <= count
        &&& 0 <= next.1 <= bytes.len()-payload
        &&& index == count ==> next.2.is_none()
        &&& index < count ==> next.0 > index
        &&& next.2.is_some() && next.2.unwrap().is_err() ==>
            step(bytes,payload,count,next.0,width,next.1,key_error,value_error).2.is_none()
        &&& next.2.is_some() && next.2.unwrap().is_ok() ==> {
            &&& next.0 == index+1
            &&& coordinate(index,width)+width+4 == coordinate(index+1,width)
            &&& index+1 < count ==> next.1 == previous(bytes,index+1,width)
            &&& index+1 == count ==> next.1 == bytes.len()-payload
        }
    },
{
    assert(coordinate(index,width)+width+4 == coordinate(index+1,width)) by(nonlinear_arith);
}

/// Directory faults identify the same absolute word as point selection.
pub proof fn map_error_offsets(bytes: Seq<u8>, payload: int, count: int, index: int,
    width: int, prior: int, base: int, key_error: Option<Error>, value_error: Option<Error>)
    requires admitted(bytes,payload,count,width), 0 <= index < count,
        prior == previous(bytes,index,width), 0 <= prior <= bytes.len()-payload,
        0 <= base, base+bytes.len() <= u32::MAX,
    ensures selected(bytes,payload,count,index,width,prior) == point(bytes,payload,count,index,width),
        selected(bytes,payload,count,index,width,prior).is_err() ==> {
            &&& selected(bytes,payload,count,index,width,prior).unwrap_err()
                == Error::Fault {kind:ErrorKind::InvalidBoundary, offset:(coordinate(index,width)+width) as u32}
            &&& base <= base+coordinate(index,width)+width < base+bytes.len()
        },
        key_error.is_some() ==> step(bytes,payload,count,index,width,prior,key_error,value_error).2 == Some(Err(key_error.unwrap())),
        key_error.is_none() && selected(bytes,payload,count,index,width,prior).is_err() ==>
            step(bytes,payload,count,index,width,prior,key_error,value_error).2 == Some(point(bytes,payload,count,index,width)),
{
    map_directory_payload_correspondence(bytes,payload,count,index,width,coordinate(index,width),prior);
}

/// Immutable source equality retains keys, payload bytes, bases and clone results.
/// Rust's shared lifetime also keeps the allocation and its descriptor alive.
pub proof fn map_immutable_identity(before: Seq<u8>, after: Seq<u8>, payload: int, count: int,
    index: int, width: int, prior: int, base: int, clone_index: int, clone_prior: int,
    key_error: Option<Error>, value_error: Option<Error>)
    requires before == after, admitted(before,payload,count,width), 0 <= index < count,
        0 <= prior <= before.len()-payload, 0 <= base, base+before.len() <= u32::MAX,
        index == clone_index, prior == clone_prior,
    ensures step(before,payload,count,index,width,prior,key_error,value_error)
            == step(after,payload,count,clone_index,width,clone_prior,key_error,value_error),
        before.subrange(coordinate(index,width),coordinate(index,width)+width)
            == after.subrange(coordinate(index,width),coordinate(index,width)+width),
        selected(before,payload,count,index,width,prior).is_ok() ==> {
            let span = selected(before,payload,count,index,width,prior).unwrap();
            &&& before.subrange(span.start as int,span.end as int) == after.subrange(span.start as int,span.end as int)
            &&& 0 <= base+span.start <= base+span.end <= u32::MAX
        },
{
}

#[cfg(verus_only)]
fn correspondence_witness()
    ensures true,
{
    let ghost bytes = seq![2u8,0,0,0,1,0,0,0,0,2,1,0,0,0,97];
    let ghost bad = bytes.update(5,2u8);
    proof {
        map_directory_payload_correspondence(bytes,14,2,0,1,4,0);
        map_directory_payload_correspondence(bytes,14,2,1,1,9,0);
        map_directory_payload_correspondence(bad,14,2,0,1,4,0);
        assert(point(bad,14,2,0,1).unwrap_err() == Error::Fault {kind:ErrorKind::InvalidBoundary,offset:5});
        assert(point(bytes,14,2,0,1).unwrap() == crate::Span {start:14,end:14});
        assert(point(bytes,14,2,1,1).unwrap() == crate::Span {start:14,end:15});
    }
}

#[cfg(verus_only)]
fn progress_witness()
    ensures true,
{
    let ghost bytes = seq![2u8,0,0,0,1,0,0,0,0,2,1,0,0,0,97];
    let ghost bad_key = bytes.update(4,2u8);
    let ghost bad_text = bytes.update(14,255u8);
    let ghost key_fault = Error::Fault {kind:ErrorKind::InvalidBool,offset:4};
    let ghost value_fault = Error::Fault {kind:ErrorKind::InvalidUtf8,offset:14};
    proof {
        map_bounded_progress(bytes,14,2,0,1,0,None,None);
        map_bounded_progress(bytes,14,2,1,1,0,None,None);
        map_bounded_progress(bad_key,14,2,0,1,0,Some(key_fault),None);
        map_bounded_progress(bad_text,14,2,1,1,0,None,Some(value_fault));
        map_bounded_progress(bytes,14,2,2,1,1,None,None);
        assert(step(bytes,14,2,0,1,0,None,None).0 == 1);
        assert(step(bad_key,14,2,0,1,0,Some(key_fault),None).0 == 2);
        assert(step(bad_text,14,2,1,1,0,None,Some(value_fault)).0 == 2);
    }
}

#[cfg(verus_only)]
fn offsets_witness()
    ensures true,
{
    let ghost bytes = seq![2u8,0,0,0,1,0,0,0,0,2,1,0,0,0,97];
    let ghost bad = bytes.update(10,0u8);
    let ghost fault = Error::Fault {kind:ErrorKind::InvalidBool,offset:50};
    proof {
        map_error_offsets(bytes,14,2,0,1,0,41,None,None);
        map_error_offsets(bad,14,2,1,1,0,41,None,None);
        map_error_offsets(bad,14,2,1,1,0,41,Some(fault),None);
        assert(point(bad,14,2,1,1).unwrap_err() == Error::Fault {kind:ErrorKind::InvalidBoundary,offset:10});
        assert(step(bad,14,2,1,1,0,Some(fault),None).2 == Some(Err(fault)));
    }
}

#[cfg(verus_only)]
fn identity_witness()
    ensures true,
{
    let ghost bytes = seq![2u8,0,0,0,1,0,0,0,0,2,1,0,0,0,97];
    proof {
        map_immutable_identity(bytes,bytes,14,2,0,1,0,41,0,0,None,None);
        map_immutable_identity(bytes,bytes,14,2,1,1,0,41,1,0,None,None);
        assert(bytes.subrange(14,15) == seq![97u8]);
    }
}

}
