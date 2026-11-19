//! Checked aggregate string-list extent, independent of item boundaries.

use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// Indexed-list payload start, including the zero/one-count elision.
pub open spec fn start(bytes: Seq<u8>) -> int {
    4 + (if crate::words::le_word(bytes,0) == 0 { 0 }
        else { crate::words::le_word(bytes,0) - 1 }) * 4
}

/// Exact ordered header faults; item ends are deliberately outside admission.
pub open spec fn extent(bytes: Seq<u8>) -> Result<u32> {
    if bytes.len() < 4 { Err(Error::Fault {kind:ErrorKind::Truncated,offset:0}) }
    else if start(bytes) > u32::MAX {
        Err(Error::Fault {kind:ErrorKind::ArithmeticOverflow,offset:u32::MAX})
    } else if start(bytes) > bytes.len() {
        Err(Error::Fault {kind:ErrorKind::Truncated,offset:(4+(bytes.len()-4)/4*4) as u32})
    } else if crate::words::le_word(bytes,0) == 0 && bytes.len() != 4 {
        Err(Error::Fault {kind:ErrorKind::InvalidBoundary,offset:4})
    } else { Ok(start(bytes) as u32) }
}

/// Checks the complete header extent and borrows its aggregate payload.
///
/// Stored item ends are not read. The caller supplies a representable region
/// and checks that its actual element schema is string before conversion.
///
/// # Panics
/// The caller must bound the supplied region to the wire-size domain.
#[doc(hidden)]
#[inline(always)]
#[allow(clippy::question_mark)] // Verus has no no_unwind contract for FromResidual.
pub fn payload(bytes: &[u8]) -> (result: Result<(&[u8], u32)>)
    requires bytes.len() <= u32::MAX,
    ensures result.is_ok() == extent(bytes@).is_ok(),
        result.is_ok() ==> {
            let (text, at) = result.unwrap();
            &&& at == extent(bytes@).unwrap()
            &&& at <= bytes.len()
            &&& text@ == bytes@.subrange(at as int,bytes.len() as int)
        },
        result.is_err() ==> result.unwrap_err() == extent(bytes@).unwrap_err(),
    no_unwind
{
    let count = match crate::read_u32(bytes,0) { Ok(count) => count, Err(error) => return Err(error) };
    let wide = if count == 0 { 4u64 } else { count as u64 * 4 };
    if wide > u32::MAX as u64 {
        return Err(Error::new(ErrorKind::ArithmeticOverflow,Error::NO_OFFSET));
    }
    let at = wide as u32;
    if at as usize > bytes.len() {
        return Err(Error::new(ErrorKind::Truncated,4+(bytes.len() as u32-4)/4*4));
    }
    if count == 0 && bytes.len() != 4 {
        return Err(Error::new(ErrorKind::InvalidBoundary,4));
    }
    let (_, text) = bytes.split_at(at as usize);
    Ok((text,at))
}


/// Byte checksum, defined independently of the item partition.
pub open spec fn checksum(bytes: Seq<u8>) -> int
    decreases bytes.len(),
{
    if bytes.len() == 0 { 0 } else { bytes[0] as int + checksum(bytes.skip(1)) }
}

proof fn checksum_bound(bytes: Seq<u8>)
    ensures 0 <= checksum(bytes) <= 255 * bytes.len(),
    decreases bytes.len(),
{
    if bytes.len() > 0 { checksum_bound(bytes.skip(1)); }
}

proof fn checksum_join(left: Seq<u8>, right: Seq<u8>)
    ensures checksum(left+right) == checksum(left)+checksum(right),
    decreases left.len(),
{
    if left.len() > 0 {
        checksum_join(left.skip(1),right);
        assert((left+right).skip(1) =~= left.skip(1)+right);
    }
}

proof fn utf8_join_at(left: Seq<u8>, right: Seq<u8>, at: int)
    requires 0 <= at <= left.len(), crate::utf8::utf8_valid(left,at),
        crate::utf8::utf8_valid(right,0),
    ensures crate::utf8::utf8_valid(left+right,at),
    decreases left.len()-at,
{
    reveal_with_fuel(crate::utf8::utf8_valid,2);
    if at == left.len() {
        assert((left+right).subrange(left.len() as int,(left+right).len() as int) =~= right);
        utf8_shift(right,left+right,0,left.len() as int);
    } else {
        let end=at+crate::utf8::scalar_width(left[at]);
        assert(crate::utf8::scalar_encoding(left+right,at,end));
        utf8_join_at(left,right,end);
    }
}

proof fn utf8_shift(source: Seq<u8>, target: Seq<u8>, at: int, base: int)
    requires 0 <= at <= source.len(), 0 <= base,
        target.len() == base+source.len(), target.subrange(base,target.len() as int) == source,
        crate::utf8::utf8_valid(source,at),
    ensures crate::utf8::utf8_valid(target,base+at),
    decreases source.len()-at,
{
    reveal_with_fuel(crate::utf8::utf8_valid,2);
    if at < source.len() {
        let end=at+crate::utf8::scalar_width(source[at]);
        assert(crate::utf8::scalar_encoding(target,base+at,base+end));
        utf8_shift(source,target,end,base);
    }
}

/// Independent sum of ordinary item checksums, in directory order.
pub open spec fn item_checksums(bytes: Seq<u8>, layout: crate::Layout, index: int) -> int
    decreases crate::words::le_word(bytes,0)-index,
{
    if index >= crate::words::le_word(bytes,0) { 0 }
    else {
        let span=crate::validation::child_span(bytes,layout,index);
        checksum(bytes.subrange(span.start as int,span.end as int))
            + item_checksums(bytes,layout,index+1)
    }
}

proof fn valid_suffix(bytes: Seq<u8>, layout: crate::Layout, budget: nat, index: int, low: int)
    requires matches!(layout.head(), crate::LayoutHead::List {element} if matches!(element.head(),crate::LayoutHead::Str)),
        crate::list_layout::list_shape(bytes,layout),
        0 < budget, 0 <= index <= crate::words::le_word(bytes,0),
        crate::validation::children_valid(bytes,layout,index,budget,true),
        low == (if index == 0 { start(bytes) }
            else { crate::validation::child_span(bytes,layout,index-1).end as int }),
    ensures 0 <= low <= bytes.len(),
        crate::utf8::utf8_valid(bytes.subrange(low,bytes.len() as int),0),
        checksum(bytes.subrange(low,bytes.len() as int)) == item_checksums(bytes,layout,index),
    decreases crate::words::le_word(bytes,0)-index,
{
    let count=crate::words::le_word(bytes,0);
    if index < count {
        crate::validation::children_split(bytes,layout,index,budget,true);
        crate::validation::child_definition(bytes,layout,index,budget,true);
        let span=crate::validation::child_span(bytes,layout,index);
        assert(span.start == low);
        assert(0 <= low <= span.end <= bytes.len());
        let first=bytes.subrange(low,span.end as int);
        reveal_with_fuel(crate::validation::schema_valid,2);
        assert(crate::utf8::utf8_valid(first,0));
        valid_suffix(bytes,layout,budget,index+1,span.end as int);
        let rest=bytes.subrange(span.end as int,bytes.len() as int);
        assert(first+rest =~= bytes.subrange(low,bytes.len() as int));
        utf8_join_at(first,rest,0);
        checksum_join(first,rest);
    } else {
        assert(low == bytes.len());
        reveal_with_fuel(crate::utf8::utf8_valid,2);
    }
}

/// A valid string-list root has the same text bytes and checksum as its items.
pub proof fn concat_valid_root(bytes: Seq<u8>, layout: crate::Layout, budget: nat)
    requires crate::validation::schema_valid(bytes,layout,budget,true),
        1 < budget <= crate::MAX_DEPTH,
        matches!(layout.head(), crate::LayoutHead::List {element} if matches!(element.head(),crate::LayoutHead::Str)),
    ensures extent(bytes).is_ok(),
        crate::utf8::utf8_valid(bytes.subrange(start(bytes),bytes.len() as int),0),
        checksum(bytes.subrange(start(bytes),bytes.len() as int)) == item_checksums(bytes,layout,0),
        0 <= checksum(bytes.subrange(start(bytes),bytes.len() as int))
            <= 255 * (bytes.len()-start(bytes)) <= u64::MAX,
{
    reveal_with_fuel(crate::validation::schema_valid,2);
    valid_suffix(bytes,layout,(budget-1) as nat,0,start(bytes));
    checksum_bound(bytes.subrange(start(bytes),bytes.len() as int));
}

/// Aggregate conversion after extent admission, with the ordinary rebasing rule.
pub open spec fn aggregate(bytes: Seq<u8>, text: Result<()>, base: u32) -> Result<crate::Span> {
    match extent(bytes) {
        Err(error) => Err(crate::batched_read::rebased(error,base)),
        Ok(at) => match text {
            Ok(()) => Ok(crate::Span {start:(base+at) as u32,end:(base+bytes.len()) as u32}),
            Err(error) => Err(crate::batched_read::rebased(error,(base+at) as u32)),
        },
    }
}

/// Arbitrary bounded bytes have exact faults and a bounded SDK conversion range.
/// The SDK supplies the UTF-8 iff result; no item-end validity is required.
/// Its linear conversion stays outside Verus.
pub proof fn concat_bounded_admission(bytes: Seq<u8>, text: Result<()>, base: u32)
    requires bytes.len() <= u32::MAX, base+bytes.len() <= u32::MAX,
        extent(bytes).is_ok() ==> crate::utf8::utf8_iff(text,bytes.subrange(start(bytes),bytes.len() as int)),
    ensures aggregate(bytes,text,base).is_ok() == (extent(bytes).is_ok()
            && crate::utf8::utf8_valid(bytes.subrange(start(bytes),bytes.len() as int),0)),
        extent(bytes).is_err() ==> aggregate(bytes,text,base).unwrap_err()
            == crate::batched_read::rebased(extent(bytes).unwrap_err(),base),
        extent(bytes).is_ok() && text.is_err() ==> aggregate(bytes,text,base).unwrap_err()
            == crate::batched_read::rebased(text.unwrap_err(),(base+start(bytes)) as u32),
        extent(bytes).is_ok() && bytes.len() == 4 ==> start(bytes) == 4
            && bytes.subrange(start(bytes),bytes.len() as int) == Seq::<u8>::empty(),
        extent(bytes).is_ok() ==> 0 <= bytes.len()-start(bytes) <= bytes.len(),
        extent(bytes).is_ok() ==> 4+bytes.len()-start(bytes) <= 4+bytes.len(),
        aggregate(bytes,text,base).is_ok() ==> {
            let span=aggregate(bytes,text,base).unwrap();
            &&& base <= span.start <= span.end <= u32::MAX
            &&& span.end-span.start == bytes.len()-start(bytes)
        },
{
}

#[cfg(verus_only)]
fn valid_root_witness()
    ensures true,
{
    let ghost bytes=seq![2u8,0,0,0,1,0,0,0,97,195,169];
    let ghost empty=seq![0u8,0,0,0];
    let ghost layout=crate::Layout::List {element:&crate::Layout::Str};
    proof {
        reveal_with_fuel(crate::layout::descriptor_valid,4);
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::validation::schema_valid,4);
        reveal_with_fuel(crate::validation::children_valid,4);
        reveal_with_fuel(crate::validation::child_valid,4);
        reveal_with_fuel(crate::utf8::utf8_valid,4);
        concat_valid_root(bytes,layout,2);
        concat_valid_root(empty,layout,2);
        reveal_with_fuel(checksum,5);
        assert(checksum(bytes.subrange(8,11)) == 461);
        assert(item_checksums(empty,layout,0) == 0);
    }
}

#[cfg(verus_only)]
fn admission_witness()
    ensures true,
{
    let ghost split=seq![2u8,0,0,0,1,0,0,0,195,169];
    let ghost invalid=seq![1u8,0,0,0,97,255];
    let ghost fault=Err(Error::Fault {kind:ErrorKind::InvalidUtf8,offset:1});
    proof {
        reveal_with_fuel(crate::utf8::utf8_valid,4);
        concat_bounded_admission(Seq::empty(),Ok(()),12);
        concat_bounded_admission(seq![0u8,0,0,0],Ok(()),12);
        concat_bounded_admission(seq![255u8,255,255,255],Ok(()),12);
        concat_bounded_admission(split,Ok(()),12);
        assert(aggregate(split,Ok(()),12).is_ok());
        assert(crate::utf8::scalar_encoding(seq![97u8,255],0,1));
        assert(crate::utf8::scalar_prefix(seq![97u8,255],0));
        assert(crate::utf8::scalar_prefix(seq![97u8,255],1));
        assert(invalid.subrange(4,6) == seq![97u8,255]);
        concat_bounded_admission(invalid,fault,12);
        assert(aggregate(invalid,fault,12) == Err(Error::Fault {kind:ErrorKind::InvalidUtf8,offset:17}));
    }
}

}
