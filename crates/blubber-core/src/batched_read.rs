//! Batch string slicing, ordinary fallback and immutable traversal equivalence.

use crate::utf8::{scalar_encoding, scalar_width, utf8_iff, utf8_valid};
use crate::{Error, Result, Span};
use vstd::prelude::*;

verus! {

/// A character boundary in a completely valid UTF-8 payload.
pub open spec fn boundary(bytes: Seq<u8>, at: int) -> bool {
    0 <= at <= bytes.len() && (at == bytes.len() || at == 0
        || bytes[at] < 0x80 || bytes[at] >= 0xc0)
}

/// A finite scalar partition of a selected range.
pub open spec fn segment(bytes: Seq<u8>, low: int, high: int) -> bool
    decreases high-low,
{
    0 <= low <= high <= bytes.len() && (low == high || {
        let end = low + scalar_width(bytes[low]);
        end <= high && scalar_encoding(bytes,low,end) && segment(bytes,end,high)
    })
}

proof fn scalar_interior(bytes: Seq<u8>, low: int, high: int, at: int)
    requires scalar_encoding(bytes,low,high), low < at < high,
    ensures 0x80 <= bytes[at] < 0xc0,
{
}

proof fn suffix_boundary(bytes: Seq<u8>, at: int, end: int)
    requires 0 <= at <= end <= bytes.len(), utf8_valid(bytes,at), boundary(bytes,end),
    ensures utf8_valid(bytes,end),
    decreases end-at,
{
    if at < end {
        reveal_with_fuel(utf8_valid,2);
        let next = at + scalar_width(bytes[at]);
        if end < next { scalar_interior(bytes,at,next,end); }
        assert(next <= end);
        suffix_boundary(bytes,next,end);
    }
}

proof fn prefix_segment(bytes: Seq<u8>, low: int, high: int)
    requires 0 <= low <= high <= bytes.len(), utf8_valid(bytes,low), boundary(bytes,high),
    ensures segment(bytes,low,high),
    decreases high-low,
{
    if low < high {
        reveal_with_fuel(utf8_valid,2);
        let next = low + scalar_width(bytes[low]);
        if high < next { scalar_interior(bytes,low,next,high); }
        assert(next <= high);
        prefix_segment(bytes,next,high);
    }
}

proof fn segment_slice(bytes: Seq<u8>, low: int, at: int, high: int)
    requires 0 <= low <= at <= high <= bytes.len(), segment(bytes,at,high),
    ensures utf8_valid(bytes.subrange(low,high),at-low),
    decreases high-at,
{
    let slice = bytes.subrange(low,high);
    if at < high {
        reveal_with_fuel(segment,2);
        let next = at + scalar_width(bytes[at]);
        assert(scalar_encoding(slice,at-low,next-low));
        segment_slice(bytes,low,next,high);
        reveal_with_fuel(utf8_valid,2);
    }
}

/// Safe character-boundary slicing preserves the canonical scalar predicate.
pub proof fn batch_slice_valid(bytes: Seq<u8>, low: int, high: int)
    requires utf8_valid(bytes,0), 0 <= low <= high <= bytes.len(),
        boundary(bytes,low), boundary(bytes,high),
    ensures utf8_valid(bytes.subrange(low,high),0),
{
    suffix_boundary(bytes,0,low);
    prefix_segment(bytes,low,high);
    segment_slice(bytes,low,low,high);
}

/// Rebase a fault with the production sentinel and overflow rules.
pub open spec fn rebased(error: Error, base: u32) -> Error {
    match error {
        Error::Fault { kind, offset } => if offset == u32::MAX { error }
            else if offset as int + base < u32::MAX { Error::Fault {kind,offset:(offset+base) as u32} }
            else { Error::Fault {kind:crate::ErrorKind::ArithmeticOverflow,offset:u32::MAX} },
        Error::Program {..} => error,
    }
}

/// Ordinary conversion over an exact selected region.
pub open spec fn converted(bytes: Seq<u8>, ordinary: Result<()>, base: u32) -> Result<Span> {
    match ordinary {
        Ok(()) => Ok(Span {start:base,end:(base+bytes.len()) as u32}),
        Err(error) => Err(rebased(error,base)),
    }
}

/// A batch succeeds only at two character boundaries; all other cases retry.
pub open spec fn batched(bytes: Seq<u8>, low: int, high: int,
    ordinary: Result<()>, base: u32) -> Result<Span>
{
    if utf8_valid(bytes,0) && boundary(bytes,low) && boundary(bytes,high) {
        Ok(Span { start:base, end:(base+high-low) as u32 })
    } else { converted(bytes.subrange(low,high),ordinary,base) }
}

/// Accepted items, invalid payloads, split characters and first faults agree.
pub proof fn batch_ordinary_equivalent(bytes: Seq<u8>, low: int, high: int,
    ordinary: Result<()>, base: u32)
    requires 0 <= low <= high <= bytes.len() <= u32::MAX,
        base+high-low <= u32::MAX, utf8_iff(ordinary,bytes.subrange(low,high)),
    ensures batched(bytes,low,high,ordinary,base)
        == converted(bytes.subrange(low,high),ordinary,base),
        ordinary.is_err() ==> batched(bytes,low,high,ordinary,base).unwrap_err()
            == rebased(ordinary.unwrap_err(),base),
{
    if utf8_valid(bytes,0) && boundary(bytes,low) && boundary(bytes,high) {
        batch_slice_valid(bytes,low,high);
    }
}

/// Physical traversal preserves accepted prefixes and key/selection precedence.
pub open spec fn traversal(items: Seq<Result<Span>>, index: int) -> Seq<Result<Span>>
    decreases items.len()-index,
{
    if index >= items.len() { Seq::empty() }
    else if items[index].is_err() { seq![items[index]] }
    else { seq![items[index]] + traversal(items,index+1) }
}

/// Ordered key and directory faults precede any text conversion.
pub open spec fn item(bytes: Seq<u8>, selection: Result<Span>, codec: Result<()>, base: u32, batch: bool) -> Result<Span> {
    match selection {
        Err(error) => Err(error),
        Ok(span) => if batch { batched(bytes,span.start as int,span.end as int,codec,(base+span.start) as u32) }
            else { converted(bytes.subrange(span.start as int,span.end as int),codec,(base+span.start) as u32) },
    }
}

/// Per-item equivalence composes through any prefix and the first error.
/// Selection errors include the unchanged key-before-end checks for maps.
pub proof fn batch_prefix_equivalent(bytes: Seq<u8>, selected: Seq<Result<Span>>, codecs: Seq<Result<()>>, base: u32, index: int)
    requires bytes.len() <= u32::MAX, base+bytes.len() <= u32::MAX,
        selected.len() == codecs.len(), 0 <= index <= selected.len(),
        forall|i:int| 0 <= i < selected.len() && #[trigger] selected[i].is_ok() ==> {
            let span=selected[i].unwrap();
            &&& span.start <= span.end <= bytes.len()
            &&& utf8_iff(codecs[i],bytes.subrange(span.start as int,span.end as int))
        },
    ensures traversal(Seq::new(selected.len(),|i:int| item(bytes,selected[i],codecs[i],base,true)),index)
        == traversal(Seq::new(selected.len(),|i:int| item(bytes,selected[i],codecs[i],base,false)),index),
{
    let batch=Seq::new(selected.len(),|i:int| item(bytes,selected[i],codecs[i],base,true));
    let ordinary=Seq::new(selected.len(),|i:int| item(bytes,selected[i],codecs[i],base,false));
    assert forall|i:int| 0 <= i < selected.len() implies batch[i] == ordinary[i] by {
        if selected[i].is_ok() {
            let span=selected[i].unwrap();
            batch_ordinary_equivalent(bytes,span.start as int,span.end as int,codecs[i],(base+span.start) as u32);
        }
    }
    assert(batch =~= ordinary);
}

/// One iterator transition; exhaustion and errors are sticky.
pub open spec fn advance(count: int, index: int, value: Result<Span>) -> (int,Option<Result<Span>>) {
    if index == count { (count,None) }
    else { (if value.is_err() {count} else {index+1},Some(value)) }
}

/// Every successful item advances; an error consumes all remaining states.
pub proof fn batch_bounded_progress(count: int, index: int, value: Result<Span>)
    requires 0 <= index <= count <= u32::MAX,
    ensures index <= advance(count,index,value).0 <= count,
        index < count ==> index < advance(count,index,value).0,
        index == count ==> advance(count,index,value).1.is_none(),
        index < count && value.is_err() ==> {
            let next=advance(count,index,value);
            &&& next.0 == count
            &&& advance(count,next.0,value).1.is_none()
        },
        index < count && value.is_ok() ==> advance(count,index,value).0 == index+1,
{
}

/// The cached payload and selected root range have identical bytes and bases.
/// Shared-frame identity also preserves every cache decision and clone result.
pub proof fn batch_immutable_identity(before: Seq<u8>, after: Seq<u8>, payload: int,
    low: int, high: int, ordinary: Result<()>, base: u32)
    requires before == after, 0 <= payload <= before.len() <= u32::MAX,
        0 <= low <= high <= before.len()-payload, base+before.len() <= u32::MAX,
        utf8_iff(ordinary,before.subrange(payload+low,payload+high)),
    ensures after.subrange(payload,after.len() as int).subrange(low,high)
            == before.subrange(payload+low,payload+high),
        before.subrange(payload+low,payload+high) == after.subrange(payload+low,payload+high),
        0 <= base+payload+low <= base+payload+high <= u32::MAX,
        batched(after.subrange(payload,after.len() as int),low,high,ordinary,(base+payload+low) as u32)
            == converted(before.subrange(payload+low,payload+high),ordinary,(base+payload+low) as u32),
        batched(before.subrange(payload,before.len() as int),low,high,ordinary,(base+payload+low) as u32)
            == batched(after.subrange(payload,after.len() as int),low,high,ordinary,(base+payload+low) as u32),
{
    let data=before.subrange(payload,before.len() as int);
    assert(data.subrange(low,high) =~= before.subrange(payload+low,payload+high));
    batch_ordinary_equivalent(data,low,high,ordinary,(base+payload+low) as u32);
}

#[cfg(verus_only)]
fn slice_witness()
    ensures true,
{
    let ghost bytes=seq![195u8,169,97];
    proof {
        reveal_with_fuel(utf8_valid,5);
        batch_slice_valid(bytes,0,2);
        batch_slice_valid(bytes,2,3);
        batch_slice_valid(bytes,2,2);
        assert(utf8_valid(bytes.subrange(0,2),0));
        assert(!boundary(bytes,1));
    }
}

#[cfg(verus_only)]
fn ordinary_witness()
    ensures true,
{
    let ghost valid=seq![195u8,169,97];
    let ghost invalid=seq![97u8,255];
    let ghost split=Err(Error::Fault {kind:crate::ErrorKind::InvalidUtf8,offset:0});
    let ghost later=Err(Error::Fault {kind:crate::ErrorKind::InvalidUtf8,offset:1});
    proof {
        reveal_with_fuel(utf8_valid,5);
        reveal_with_fuel(crate::utf8::scalar_prefix,4);
        batch_ordinary_equivalent(valid,0,2,Ok(()),29);
        batch_ordinary_equivalent(valid,0,1,split,29);
        batch_ordinary_equivalent(valid,1,2,split,30);
        batch_ordinary_equivalent(invalid,0,1,Ok(()),29);
        assert(invalid.subrange(0,2) == invalid);
        assert(crate::utf8::scalar_encoding(invalid,0,1));
        assert(crate::utf8::scalar_prefix(invalid,0));
        assert(crate::utf8::scalar_prefix(invalid,1));
        assert(utf8_iff(later,invalid));
        batch_ordinary_equivalent(invalid,0,2,later,29);
        assert(batched(valid,0,2,Ok(()),29).is_ok());
        assert(batched(valid,0,1,split,29).unwrap_err() == Error::Fault {kind:crate::ErrorKind::InvalidUtf8,offset:29});
        assert(batched(invalid,0,2,later,29).unwrap_err() == Error::Fault {kind:crate::ErrorKind::InvalidUtf8,offset:30});
    }
}

#[cfg(verus_only)]
fn prefix_witness()
    ensures true,
{
    let ghost bytes=seq![97u8,255];
    let ghost selected=seq![Ok(Span {start:0,end:1}),Ok(Span {start:1,end:2}),Err(Error::Fault {kind:crate::ErrorKind::InvalidBoundary,offset:8})];
    let ghost codecs=seq![Ok(()),Err(Error::Fault {kind:crate::ErrorKind::InvalidUtf8,offset:0}),Ok(())];
    proof {
        reveal_with_fuel(utf8_valid,4);
        assert forall|i:int| 0 <= i < selected.len() && #[trigger] selected[i].is_ok() implies {
            let span=selected[i].unwrap();
            &&& span.start <= span.end <= bytes.len()
            &&& utf8_iff(codecs[i],bytes.subrange(span.start as int,span.end as int))
        } by { assert(i == 0 || i == 1); }
        batch_prefix_equivalent(bytes,selected,codecs,29,0);
        batch_prefix_equivalent(bytes,selected,codecs,29,1);
        batch_prefix_equivalent(bytes,Seq::empty(),Seq::empty(),29,0);
        let ghost items=Seq::new(selected.len(),|i:int| item(bytes,selected[i],codecs[i],29,true));
        reveal_with_fuel(traversal,4);
        assert(traversal(items,0).len()==2);
        assert(traversal(items,0)[1] == Err(Error::Fault {kind:crate::ErrorKind::InvalidUtf8,offset:30}));
    }
}

#[cfg(verus_only)]
fn progress_witness()
    ensures true,
{
    proof {
        batch_bounded_progress(0,0,Ok(Span {start:0,end:0}));
        batch_bounded_progress(3,0,Ok(Span {start:0,end:1}));
        batch_bounded_progress(3,1,Err(Error::Fault {kind:crate::ErrorKind::InvalidBoundary,offset:4}));
        batch_bounded_progress(3,3,Err(Error::Fault {kind:crate::ErrorKind::InvalidBoundary,offset:4}));
    }
}

#[cfg(verus_only)]
fn identity_witness()
    ensures true,
{
    let ghost bytes=seq![255u8,195,169];
    proof {
        reveal_with_fuel(utf8_valid,4);
        batch_immutable_identity(bytes,bytes,1,0,2,Ok(()),(u32::MAX-3) as u32);
        assert(bytes.subrange(1,3)==seq![195u8,169]);
        assert(batched(bytes.subrange(1,3),0,2,Ok(()),(u32::MAX-2) as u32).unwrap().end==u32::MAX);
        assert(!utf8_valid(bytes,0));
    }
}

}
