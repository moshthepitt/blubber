//! Immutable variable-list traversal with retained checked ends.

use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// An admitted variable-list directory, without any leaf-domain claim.
pub open spec fn admitted(bytes: Seq<u8>, payload: int, count: int) -> bool {
    &&& 0 <= count <= u32::MAX
    &&& payload == 4 + (if count > 0 { count - 1 } else { 0 }) * 4
    &&& payload <= bytes.len() <= u32::MAX
    &&& count == crate::words::le_word(bytes, 0)
    &&& count == 0 ==> bytes.len() == 4
}

/// The preceding stored end, or the implicit initial zero.
pub open spec fn previous(bytes: Seq<u8>, index: int) -> int {
    if index == 0 { 0 } else { crate::words::le_word(bytes, 4 + (index - 1) * 4) as int }
}

/// The selected stored end, or the implicit list terminal.
pub open spec fn current(bytes: Seq<u8>, payload: int, index: int, count: int) -> int {
    if index + 1 == count { bytes.len() - payload }
    else { crate::words::le_word(bytes, 4 + index * 4) as int }
}

/// Borrows the next exact region using an already checked preceding end.
/// Errors select the same current directory word as ordinary point selection.
/// The shared source and its complete directory extent remain unchanged.
///
/// # Panics
/// Missing directory admission or out-of-range coordinates may panic.
#[doc(hidden)]
#[inline(always)]
pub fn region(bytes: &[u8], payload: usize, index: usize, count: usize, prior: usize)
    -> (result: Result<(&[u8], u32)>)
    requires admitted(bytes@, payload as int, count as int), index < count,
        prior <= bytes.len() - payload,
    ensures result.is_ok() == (prior <= current(bytes@, payload as int, index as int, count as int) <= bytes.len() - payload),
        result.is_ok() ==> {
            let (chunk, start) = result.unwrap();
            &&& start == payload + prior
            &&& chunk@ == bytes@.subrange(payload + prior,
                payload + current(bytes@, payload as int, index as int, count as int))
            &&& chunk.len() == current(bytes@, payload as int, index as int, count as int) - prior
        },
        result.is_err() ==> result.unwrap_err() == (Error::Fault { kind: ErrorKind::InvalidBoundary, offset: (4 + index * 4) as u32 }),
    no_unwind
{
    let at = crate::raw::indexed_coordinate(4,index,4);
    let end = if index + 1 == count { bytes.len() - payload }
        else { crate::raw::indexed_read_word(bytes, at) as usize };
    if end < prior || end > bytes.len() - payload {
        return Err(Error::new(ErrorKind::InvalidBoundary, at as u32));
    }
    let start = crate::raw::indexed_coordinate(payload,prior,1);
    let (_, rest) = bytes.split_at(start);
    let (chunk, _) = rest.split_at(end - prior);
    Ok((chunk, start as u32))
}


/// Independent point-read selection, including the preceding-end error.
pub open spec fn point(bytes: Seq<u8>, payload: int, index: int, count: int) -> Result<crate::Span> {
    let low = previous(bytes,index);
    let high = current(bytes,payload,index,count);
    if low > bytes.len() - payload {
        Err(Error::Fault { kind: ErrorKind::InvalidBoundary, offset: (if index == 0 { 4 } else { 4 + (index-1)*4 }) as u32 })
    } else if high < low || high > bytes.len() - payload {
        Err(Error::Fault {kind:ErrorKind::InvalidBoundary,offset:(4+index*4) as u32})
    } else {
        Ok(crate::Span { start:(payload+low) as u32, end:(payload+high) as u32 })
    }
}

/// Streamed selection with the checked preceding end retained by the iterator.
pub open spec fn selected(bytes: Seq<u8>, payload: int, index: int, count: int, prior: int) -> Result<crate::Span> {
    let high = current(bytes,payload,index,count);
    if high < prior || high > bytes.len() - payload {
        Err(Error::Fault { kind: ErrorKind::InvalidBoundary, offset:(4+index*4) as u32 })
    } else { Ok(crate::Span { start:(payload+prior) as u32, end:(payload+high) as u32 }) }
}

/// Successful selections and exact first-error offsets equal point reads.
pub proof fn list_selected_error_equivalent(bytes: Seq<u8>, payload: int, index: int, count: int, prior: int)
    requires admitted(bytes,payload,count), 0 <= index < count,
        0 <= prior <= bytes.len()-payload, prior == previous(bytes,index),
    ensures selected(bytes,payload,index,count,prior) == point(bytes,payload,index,count),
        point(bytes,payload,index,count).is_ok() ==> {
            let span = point(bytes,payload,index,count).unwrap();
            &&& payload <= span.start <= span.end <= bytes.len()
            &&& bytes.subrange(span.start as int,span.end as int)
                == bytes.subrange(payload+prior,payload+current(bytes,payload,index,count))
        },
{
}

/// The initial end and every successful transition satisfy the next invariant.
pub proof fn list_end_induction(bytes: Seq<u8>, payload: int, index: int, count: int, prior: int)
    requires admitted(bytes,payload,count), 0 <= index < count,
        0 <= prior <= bytes.len()-payload, prior == previous(bytes,index),
        selected(bytes,payload,index,count,prior).is_ok(),
    ensures previous(bytes,0) == 0,
        0 <= current(bytes,payload,index,count) <= bytes.len()-payload,
        index+1 < count ==> current(bytes,payload,index,count) == previous(bytes,index+1),
        index+1 == count ==> current(bytes,payload,index,count) == bytes.len()-payload,
{
}

/// Iterator progress and fusion. An error consumes all remaining results.
pub open spec fn step(bytes: Seq<u8>, payload: int, count: int, index: int, prior: int, codec: Option<Error>)
    -> (int,int,Option<Result<crate::Span>>)
{
    if index == count { (count,prior,None) }
    else {
        let selected = selected(bytes,payload,index,count,prior);
        let result = if selected.is_ok() && codec.is_some() { Err(codec.unwrap()) } else { selected };
        if result.is_err() { (count,prior,Some(result)) }
        else { (index+1,current(bytes,payload,index,count),Some(result)) }
    }
}

/// A cloned state repeats the same result; errors and exhaustion remain fused.
pub proof fn list_clone_fusion(bytes: Seq<u8>, payload: int, count: int, index: int, prior: int,
    clone_index: int, clone_prior: int, codec: Option<Error>)
    requires admitted(bytes,payload,count), 0 <= index <= count,
        0 <= prior <= bytes.len()-payload, index == clone_index, prior == clone_prior,
    ensures step(bytes,payload,count,index,prior,codec) == step(bytes,payload,count,clone_index,clone_prior,codec),
        index == count ==> step(bytes,payload,count,index,prior,codec).2.is_none(),
        index < count && (selected(bytes,payload,index,count,prior).is_err() || codec.is_some()) ==> {
            let next = step(bytes,payload,count,index,prior,codec);
            &&& next.0 == count
            &&& step(bytes,payload,count,next.0,next.1,codec).2.is_none()
        },
        index < count && selected(bytes,payload,index,count,prior).is_ok() && codec.is_none() ==>
            step(bytes,payload,count,index,prior,codec).0 == index+1,
{
}

/// Route zero uses compact fixed traversal; route two uses ordinary selection.
pub open spec fn route(actual: Option<u32>, expected: Option<u32>) -> int {
    if actual.is_some() && actual == expected { 0 }
    else if actual.is_some() { 2 } else { 1 }
}

/// Immutable bytes preserve selected regions, bases and foreign-layout routing.
/// Rust's shared lifetime additionally preserves the underlying allocation.
pub proof fn list_immutable_identity(before: Seq<u8>, after: Seq<u8>, payload: int,
    count: int, index: int, prior: int, base: int, actual: Option<u32>, expected: Option<u32>)
    requires before == after, admitted(before,payload,count), 0 <= index < count,
        0 <= prior <= before.len()-payload, 0 <= base, base+before.len() <= u32::MAX,
    ensures selected(before,payload,index,count,prior) == selected(after,payload,index,count,prior),
        selected(before,payload,index,count,prior).is_ok() ==> {
            let span = selected(before,payload,index,count,prior).unwrap();
            &&& before.subrange(span.start as int,span.end as int) == after.subrange(span.start as int,span.end as int)
            &&& 0 <= base+span.start <= base+span.end <= u32::MAX
        },
        actual.is_some() && actual != expected ==> route(actual,expected) == 2,
        actual.is_some() && actual == expected ==> route(actual,expected) == 0,
        actual.is_none() ==> route(actual,expected) == 1,
{
}

#[cfg(verus_only)]
fn selected_witness()
    ensures true,
{
    let ghost good = seq![2u8,0,0,0,1,0,0,0,97,98];
    let ghost bad = good.update(4,3u8);
    proof {
        list_selected_error_equivalent(good,8,0,2,0);
        list_selected_error_equivalent(good,8,1,2,1);
        list_selected_error_equivalent(bad,8,0,2,0);
        assert(point(good,8,0,2).is_ok());
        assert(point(bad,8,0,2).unwrap_err() == Error::Fault { kind:ErrorKind::InvalidBoundary, offset:4 });
    }
}

#[cfg(verus_only)]
fn induction_witness()
    ensures true,
{
    let ghost bytes = seq![2u8,0,0,0,0,0,0,0,97];
    proof {
        list_end_induction(bytes,8,0,2,0);
        list_end_induction(bytes,8,1,2,0);
        assert(current(bytes,8,0,2)==0);
        assert(current(bytes,8,1,2)==1);
    }
}

#[cfg(verus_only)]
fn clone_witness()
    ensures true,
{
    let ghost bytes = seq![2u8,0,0,0,3,0,0,0,97];
    proof {
        list_clone_fusion(bytes,8,2,0,0,0,0,None);
        list_clone_fusion(bytes,8,2,2,0,2,0,None);
        assert(step(bytes,8,2,0,0,None).2.unwrap().is_err());
        assert(step(bytes,8,2,2,0,None).2.is_none());
        let valid = seq![1u8,0,0,0,255];
        let codec = Some(Error::Fault {kind:ErrorKind::InvalidUtf8,offset:4});
        list_clone_fusion(valid,4,1,0,0,0,0,codec);
        assert(step(valid,4,1,0,0,codec).2.unwrap().is_err());
        list_clone_fusion(seq![0u8,0,0,0],4,0,0,0,0,0,None);
    }
}

#[cfg(verus_only)]
fn identity_witness()
    ensures true,
{
    let ghost bytes = seq![1u8,0,0,0,195,169];
    proof {
        list_immutable_identity(bytes,bytes,4,1,0,0,12,None,None);
        list_immutable_identity(bytes,bytes,4,1,0,0,12,Some(1u32),None);
        list_immutable_identity(bytes,bytes,4,1,0,0,12,Some(1u32),Some(2u32));
        list_immutable_identity(bytes,bytes,4,1,0,0,12,Some(1u32),Some(1u32));
        assert(route(Some(1u32),None)==2);
        assert(selected(bytes,4,0,1,0).unwrap().end==6);
    }
}

}
