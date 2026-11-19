//! Exact fixed-stride and elided-end list byte partitions.

use crate::layout::fixed_width;
#[cfg(verus_only)]
use crate::layout::{descriptor_valid, width_spec};
#[cfg(verus_only)]
use crate::struct_layout::directory_size;
#[cfg(verus_only)]
use crate::words::le_word;
use crate::LayoutHead;
use crate::{check_layout, read_u32, Error, ErrorKind, Layout, Result, Span, MAX_DEPTH};
use vstd::prelude::*;

verus! {

/// Checked regions of a list encoding, relative to its supplied slice.
///
/// Direct construction establishes no invariant. The byte gateways always
/// compute and check their own partitions before selecting an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListPartition {
    /// Element count decoded from the four-byte header.
    pub count: u32,
    /// Fixed element width, or `None` for indexed variable elements.
    pub element_width: Option<u32>,
    /// Stored ends; empty for fixed elements and counts zero or one.
    pub directory: Span,
    /// Exact element payload, after the count and any stored ends.
    pub payload: Span,
}

/// Width classification of a list's element, independently of byte parsing.
pub open spec fn element_width(layout: Layout) -> Option<u32> {
    match layout.head() {
        LayoutHead::List { element } => width_spec(element),
        _ => None,
    }
}

/// Mathematical start of a list's payload, including zero/one-count elision.
pub open spec fn payload_start(bytes: Seq<u8>, layout: Layout) -> int {
    if element_width(layout).is_some() { 4 }
    else { 4 + directory_size(le_word(bytes, 0)) }
}

/// Declarative list shape, excluding recursive child-byte validity.
///
/// Fixed elements occupy exactly count times their width. Variable elements
/// partition the payload with monotone stored ends and implicit first/last ends.
pub open spec fn list_shape(bytes: Seq<u8>, layout: Layout) -> bool {
    match layout.head() {
        LayoutHead::List { element } => {
            &&& 4 <= bytes.len() <= u32::MAX
            &&& match width_spec(element) {
                Some(width) => bytes.len() == 4 + le_word(bytes, 0) * width,
                None => {
                    let count = le_word(bytes, 0);
                    let p = payload_start(bytes, layout);
                    &&& p <= bytes.len()
                    &&& if count == 0 { bytes.len() == 4 }
                        else {
                            forall|i: int| 0 <= i < count - 1 ==> {
                                let end = #[trigger] le_word(bytes, 4 + 4 * i);
                                &&& end <= bytes.len() - p
                                &&& if i == 0 { 0 <= end }
                                    else { le_word(bytes, 4 + 4 * (i - 1)) <= end }
                            }
                        }
                },
            }
        },
        _ => false,
    }
}

/// Acceptance iff descriptor and exact structural validity, with exact regions.
pub open spec fn partition_iff(
    result: Result<ListPartition>, bytes: Seq<u8>, layout: Layout,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && list_shape(bytes, layout);
    match result {
        Ok(partition) => valid
            && partition.count == le_word(bytes, 0)
            && partition.element_width == element_width(layout)
            && partition.directory == (Span { start: 4, end: payload_start(bytes, layout) as u32 })
            && partition.payload == (Span {
                start: payload_start(bytes, layout) as u32, end: bytes.len() as u32,
            }),
        Err(_) => !valid,
    }
}

/// Header geometry without certifying untouched directory words.
pub open spec fn header_shape(bytes: Seq<u8>, layout: Layout) -> bool {
    match layout.head() {
        LayoutHead::List { element } => 4 <= bytes.len() <= u32::MAX && match width_spec(element) {
            Some(width) => bytes.len() == 4 + le_word(bytes, 0) * width,
            None => payload_start(bytes, layout) <= bytes.len()
                && (le_word(bytes, 0) == 0 ==> bytes.len() == 4),
        },
        _ => false,
    }
}

/// Exact header regions; directory monotonicity remains a separate predicate.
pub open spec fn header_iff(
    result: Result<ListPartition>, bytes: Seq<u8>, layout: Layout,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && header_shape(bytes, layout);
    match result {
        Ok(partition) => valid
            && partition.count == le_word(bytes, 0)
            && partition.element_width == element_width(layout)
            && partition.directory == (Span { start: 4, end: payload_start(bytes, layout) as u32 })
            && partition.payload == (Span {
                start: payload_start(bytes, layout) as u32, end: bytes.len() as u32,
            }),
        Err(_) => !valid,
    }
}


/// Checks a list's count, exact extents, and any stored element ends.
///
/// Checks metadata at [`MAX_DEPTH`] before bytes. Slice or computed extents
/// above `u32::MAX` return `ArithmeticOverflow` without an offset. A short count
/// returns `Truncated` at zero; a short fixed payload at four; a short directory
/// at its first incomplete word. Trailing fixed or count-zero payload bytes
/// return `InvalidBoundary` at the expected end. The first descending or
/// out-of-payload end returns `InvalidBoundary` at its word. Child encodings
/// are not checked. Equal ends and empty variable elements are valid.
pub fn partition_list(bytes: &[u8], layout: &Layout) -> (result: Result<ListPartition>)
    ensures partition_iff(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    partition_list_mode(bytes, layout, true)
}

/// Checks header extents while leaving unrelated stored ends lazy.
#[inline(always)]
pub(crate) fn partition_list_header(bytes: &[u8], layout: &Layout) -> (result: Result<ListPartition>)
    ensures header_iff(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    partition_list_mode(bytes, layout, false)
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn partition_list_mode(bytes: &[u8], layout: &Layout, strict: bool) -> (result: Result<ListPartition>)
    ensures strict ==> partition_iff(result, bytes@, *layout),
        !strict ==> header_iff(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    hide(le_word);
    check_layout(layout, MAX_DEPTH)?;
    let width=if let Layout::Flat{checked}=layout {
        proof{reveal(crate::layout_head::head_spec);}
        if checked.header(0)!=7{return Err(ErrorKind::InvalidSchema.into());}
        proof{reveal(crate::layout_head::head_spec);}
        fixed_width(&Layout::Flat{checked:checked.only_child()})
    }else{match layout.head() {
        LayoutHead::List { element } => fixed_width(&element),
        _ => return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)),
    }};

    if bytes.len() > u32::MAX as usize {
        return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
    }
    let count = read_u32(bytes, 0)?;
    let payload = match width {
        Some(width) => {
            let size = match count.checked_mul(width) {
                Some(size) => size,
                None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
            };
            let end = match size.checked_add(4) {
                Some(end) => end,
                None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
            };
            if bytes.len() < end as usize {
                return Err(Error::new(ErrorKind::Truncated, 4));
            }
            if bytes.len() != end as usize {
                return Err(Error::new(ErrorKind::InvalidBoundary, end));
            }
            4u32
        },
        None => {
            let words = if count == 0 { 0u32 } else { count - 1 };
            let size = match words.checked_mul(4) {
                Some(size) => size,
                None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
            };
            let payload = match size.checked_add(4) {
                Some(payload) => payload,
                None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
            };
            if bytes.len() < payload as usize {
                let at = 4 + ((bytes.len() as u32 - 4) / 4) * 4;
                return Err(Error::new(ErrorKind::Truncated, at));
            }
            if count == 0 && bytes.len() != 4 {
                return Err(Error::new(ErrorKind::InvalidBoundary, 4));
            }
            if !strict {
                return Ok(ListPartition { count, element_width: width,
                    directory: Span { start: 4, end: payload },
                    payload: Span { start: payload, end: bytes.len() as u32 } });
            }
            assert(payload_start(bytes@,*layout)==payload);
            proof {
                reveal(crate::words::directory_sorted);
                reveal(list_shape);
                assert(matches!(layout.head(),LayoutHead::List{..}));
                assert(element_width(*layout).is_none());
                if crate::words::directory_sorted(bytes@,payload as int,0,4,words as int) {
                    assert forall|i:int| 0<=i<count as int-1 implies {
                        let end=#[trigger] le_word(bytes@,4+4*i);
                        end<=bytes.len()-payload && (if i==0 {0<=end}else{le_word(bytes@,4+4*(i-1))<=end})
                    } by {
                        assert(i<words);
                        assert(i*4==4*i) by(nonlinear_arith);
                        assert((i-1)*4==4*(i-1)) by(nonlinear_arith);
                        assert(le_word(bytes@,4+i*4+0)<=bytes.len()-payload);
                    }
                    assert(list_shape(bytes@,*layout));
                }
                if list_shape(bytes@,*layout) {
                    assert forall|i:int| 0<=i<words implies {
                        let end=#[trigger] le_word(bytes@,4+i*4+0);
                        end<=bytes.len()-payload && (if i==0 {0<=end}else{le_word(bytes@,4+(i-1)*4+0)<=end})
                    } by {
                        assert(count>0 && i<count as int-1);
                        assert(i*4==4*i) by(nonlinear_arith);
                        assert((i-1)*4==4*(i-1)) by(nonlinear_arith);
                        assert(le_word(bytes@,4+4*i)<=bytes.len()-payload);
                    }
                    assert(crate::words::directory_sorted(bytes@,payload as int,0,4,words as int));
                }
            }
            assert(crate::words::directory_sorted(bytes@,payload as int,0,4,words as int)
                ==list_shape(bytes@,*layout));
            crate::words::check_directory::<0,4>(bytes,payload as usize,Ghost(words as int))?;
            payload
        },
    };
    Ok(ListPartition {
        count,
        element_width: width,
        directory: Span { start: 4, end: payload },
        payload: Span { start: payload, end: bytes.len() as u32 },
    })
}

/// Mathematical element interval, using fixed stride or implicit endpoints.
pub open spec fn element_span(bytes: Seq<u8>, layout: Layout, index: u32) -> Span {
    match element_width(layout) {
        Some(width) => Span {
            start: (4 + index as int * width) as u32,
            end: (4 + (index as int + 1) * width) as u32,
        },
        None => {
            let p = payload_start(bytes, layout);
            Span {
                start: (p + if index == 0 { 0int }
                    else { le_word(bytes, 4 + 4 * (index as int - 1)) }) as u32,
                end: if index as int + 1 == le_word(bytes, 0) { bytes.len() as u32 }
                    else { (p + le_word(bytes, 4 + 4 * index)) as u32 },
            }
        },
    }
}

/// Exact element selection iff in bounds, with immediate payload containment.
pub open spec fn range_exact(
    result: Result<Span>, bytes: Seq<u8>, layout: Layout, index: u32,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && list_shape(bytes, layout)
        && index < le_word(bytes, 0);
    match result {
        Ok(span) => valid && span == element_span(bytes, layout, index)
            && payload_start(bytes, layout) <= span.start <= span.end <= bytes.len(),
        Err(_) => !valid,
    }
}

/// Returns an element's exact region after checking the entire list partition.
///
/// An invalid index returns `IndexOutOfBounds` without an offset, after any
/// metadata or partition error. The first/last indexed boundaries are implicit;
/// empty variable elements return an empty span within the list payload.
pub fn list_element(bytes: &[u8], layout: &Layout, index: u32) -> (result: Result<Span>)
    ensures range_exact(result, bytes@, *layout, index),
{
    let partition = partition_list(bytes, layout)?;
    if index >= partition.count {
        return Err(Error::new(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET));
    }
    list_element_at(bytes, layout, &partition, index)
}

// Select in constant time from a partition checked once by the caller.
pub(crate) fn list_element_at(
    bytes: &[u8], _layout: &Layout, partition: &ListPartition, index: u32,
) -> (result: Result<Span>)
    requires
        partition_iff(Ok(*partition), bytes@, *_layout),
        index < partition.count,
    ensures range_exact(result, bytes@, *_layout, index), result.is_ok(),
{
    hide(le_word);
    match partition.element_width {
        Some(width) => {
            proof {
                assert(bytes.len() == 4 + partition.count as int * width);
                assert(0 <= index as int * width
                    && 4 + index as int * width + width <= bytes.len()) by(nonlinear_arith)
                    requires 0 <= width, index < partition.count,
                        bytes.len() == 4 + partition.count as int * width;
                assert((index as int + 1) * width == index as int * width + width)
                    by(nonlinear_arith);
            }
            let start = 4 + index * width;
            let span = Span { start, end: start + width };
            assert(range_exact(Ok(span), bytes@, *_layout, index));
            Ok(span)
        },
        None => {
            let start = if index == 0 { 0u32 } else {
                read_u32(bytes, 4 + 4 * (index - 1))?
            };
            let end = if index + 1 == partition.count {
                partition.payload.end - partition.payload.start
            } else {
                read_u32(bytes, 4 + 4 * index)?
            };
            let span = Span { start: partition.payload.start + start, end: partition.payload.start + end };
            assert(range_exact(Ok(span), bytes@, *_layout, index));
            Ok(span)
        },
    }
}

}
