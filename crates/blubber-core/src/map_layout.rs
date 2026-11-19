//! Exact interleaved-entry and stored-terminal-end map byte partitions.

use crate::layout::fixed_width;
#[cfg(verus_only)]
use crate::layout::{descriptor_valid, width_spec};
#[cfg(verus_only)]
use crate::words::le_word;
use crate::LayoutHead;
use crate::{check_layout, read_u32, Error, ErrorKind, Layout, Result, Span, MAX_DEPTH};
use vstd::prelude::*;

verus! {

/// Checked map regions, relative to the supplied exact map slice.
///
/// Public construction establishes no invariant. Gateways recompute the
/// partition before selecting a value. Key domains and ordering are separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapPartition {
    /// Entry count decoded from the four-byte header.
    pub count: u32,
    /// Fixed positive encoded key width.
    pub key_width: u32,
    /// Fixed value width, or `None` for indexed variable values.
    pub value_width: Option<u32>,
    /// Key/end records for variable values; empty for fixed values.
    pub directory: Span,
    /// Interleaved key/value entries, or the indexed values' packed payload.
    pub payload: Span,
}

/// Map key width, independently of descriptor or byte validity.
pub open spec fn key_width(layout: Layout) -> u32 {
    match layout.head() { LayoutHead::Map { key, .. } => key.width, _ => 0u32 }
}

/// Map value width classification, independently of byte parsing.
pub open spec fn value_width(layout: Layout) -> Option<u32> {
    match layout.head() { LayoutHead::Map { value, .. } => width_spec(value), _ => None }
}

/// Mathematical entry stride, including the key and value or stored end.
pub open spec fn entry_stride(layout: Layout) -> int {
    key_width(layout) as int + match value_width(layout) {
        Some(width) => width as int, None => 4int,
    }
}

/// Mathematical payload start; fixed maps keep keys and values interleaved.
pub open spec fn payload_start(bytes: Seq<u8>, layout: Layout) -> int {
    if value_width(layout).is_some() { 4 }
    else { 4 + le_word(bytes, 0) * entry_stride(layout) }
}

/// Declarative map partition, excluding key domains/order and child validity.
///
/// Every indexed entry stores its end, including the terminal one. Equal ends
/// admit empty values; the terminal end must consume the exact payload.
pub open spec fn map_shape(bytes: Seq<u8>, layout: Layout) -> bool {
    match layout.head() {
        LayoutHead::Map { .. } => {
            let count = le_word(bytes, 0);
            let stride = entry_stride(layout);
            &&& 4 <= bytes.len() <= u32::MAX
            &&& match value_width(layout) {
                Some(_) => bytes.len() == 4 + count * stride,
                None => {
                    let p = payload_start(bytes, layout);
                    &&& p <= bytes.len()
                    &&& if count == 0 { bytes.len() == 4 }
                        else {
                            &&& forall|i: int| 0 <= i < count ==> {
                                let end = #[trigger] le_word(bytes, 4 + i * stride + key_width(layout));
                                &&& end <= bytes.len() - p
                                &&& if i == 0 { 0 <= end }
                                    else { le_word(bytes, 4 + (i - 1) * stride + key_width(layout)) <= end }
                            }
                            &&& le_word(bytes, 4 + (count - 1) * stride + key_width(layout)) == bytes.len() - p
                        }
                },
            }
        },
        _ => false,
    }
}

/// Acceptance iff checked metadata and exact map shape, with exact regions.
pub open spec fn partition_iff(
    result: Result<MapPartition>, bytes: Seq<u8>, layout: Layout,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && map_shape(bytes, layout);
    match result {
        Ok(partition) => valid
            && partition.count == le_word(bytes, 0)
            && partition.key_width == key_width(layout)
            && partition.value_width == value_width(layout)
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
        LayoutHead::Map { .. } => {
            let count = le_word(bytes, 0);
            let p = payload_start(bytes, layout);
            &&& 4 <= bytes.len() <= u32::MAX
            &&& if value_width(layout).is_some() { bytes.len() == 4 + count * entry_stride(layout) }
                else { p <= bytes.len() && if count == 0 { bytes.len() == 4 }
                    else { le_word(bytes, 4 + (count - 1) * entry_stride(layout) + key_width(layout)) == bytes.len() - p } }
        },
        _ => false,
    }
}

/// Exact header regions; directory monotonicity remains a separate predicate.
pub open spec fn header_iff(
    result: Result<MapPartition>, bytes: Seq<u8>, layout: Layout,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && header_shape(bytes, layout);
    match result {
        Ok(partition) => valid
            && partition.count == le_word(bytes, 0)
            && partition.key_width == key_width(layout)
            && partition.value_width == value_width(layout)
            && partition.directory == (Span { start: 4, end: payload_start(bytes, layout) as u32 })
            && partition.payload == (Span {
                start: payload_start(bytes, layout) as u32, end: bytes.len() as u32,
            }),
        Err(_) => !valid,
    }
}


/// Checks the count, key/value strides, directory ends and exact map extent.
///
/// Metadata is checked at [`MAX_DEPTH`] first. Slice or computed extents above
/// `u32::MAX` return `ArithmeticOverflow` without an offset. A short count
/// returns `Truncated` at zero; short fixed entries at four; a short indexed
/// directory at its first incomplete key/end record. Trailing fixed entries or
/// count-zero payload bytes return `InvalidBoundary` at the expected end.
/// Descending/out-of-payload ends and an inexact terminal end identify their
/// stored word. Empty maps require only four bytes, irrespective of widths.
/// Key bytes and value encodings are not validated here.
pub fn partition_map(bytes: &[u8], layout: &Layout) -> (result: Result<MapPartition>)
    ensures partition_iff(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    partition_map_mode(bytes, layout, true)
}

/// Checks header extents while leaving unrelated stored ends lazy.
#[inline(always)]
pub(crate) fn partition_map_header(bytes: &[u8], layout: &Layout) -> (result: Result<MapPartition>)
    ensures header_iff(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    partition_map_mode(bytes, layout, false)
}

#[inline(always)]
fn partition_map_mode(bytes: &[u8], layout: &Layout, strict: bool) -> (result: Result<MapPartition>)
    ensures strict ==> partition_iff(result, bytes@, *layout),
        !strict ==> header_iff(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    hide(le_word);
    check_layout(layout, MAX_DEPTH)?;
    let (key,width)=if let Layout::Flat{checked}=layout {
        proof{reveal(crate::layout_head::head_spec);}
        if checked.header(0)!=8{return Err(ErrorKind::InvalidSchema.into());}
        proof{reveal(crate::layout_head::head_spec);}
        (checked.header(6),fixed_width(&Layout::Flat{checked:checked.only_child()}))
    }else{match layout.head() {
        LayoutHead::Map { key, value } => (key.width, fixed_width(&value)),
        _ => return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)),
    }};

    if bytes.len() > u32::MAX as usize {
        return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
    }
    let count = read_u32(bytes, 0)?;
    if count == 0 {
        if bytes.len() != 4 {
            return Err(Error::new(ErrorKind::InvalidBoundary, 4));
        }
        assert(le_word(bytes@, 0) == 0);
        proof {
            reveal(crate::layout_head::head_spec);
            assert(payload_start(bytes@, *layout) == 4);
            assert(map_shape(bytes@, *layout));
        }
        return Ok(MapPartition {
            count, key_width: key, value_width: width,
            directory: Span { start: 4, end: 4 },
            payload: Span { start: 4, end: 4 },
        });
    }
    let tail = width.unwrap_or(4u32);
    let stride = match key.checked_add(tail) {
        Some(stride) => stride,
        None => {
            assert(count as int * entry_stride(*layout) > u32::MAX) by(nonlinear_arith)
                requires count > 0, entry_stride(*layout) > u32::MAX;
            return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
        },
    };
    let size = match count.checked_mul(stride) {
        Some(size) => size,
        None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    let extent = match size.checked_add(4) {
        Some(extent) => extent,
        None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    let payload = match width {
        Some(_) => {
            if bytes.len() < extent as usize {
                return Err(Error::new(ErrorKind::Truncated, 4));
            }
            if bytes.len() != extent as usize {
                return Err(Error::new(ErrorKind::InvalidBoundary, extent));
            }
            4u32
        },
        None => {
            if bytes.len() < extent as usize {
                proof {
                    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(
                        bytes.len() as int - 4, stride as int);
                }
                let at = bytes.len() as u32 - ((bytes.len() as u32 - 4) % stride);
                return Err(Error::new(ErrorKind::Truncated, at));
            }
            if !strict {
                assert(4 + (count as int - 1) * stride + key == extent - 4) by(nonlinear_arith)
                    requires extent == 4 + count as int * stride, stride == key + 4, count > 0;
                let terminal = read_u32(bytes, extent - 4)?;
                if terminal != bytes.len() as u32 - extent {
                    return Err(Error::new(ErrorKind::InvalidBoundary, extent - 4));
                }
                return Ok(MapPartition { count, key_width: key, value_width: width,
                    directory: Span { start: 4, end: extent },
                    payload: Span { start: extent, end: bytes.len() as u32 } });
            }
            let payload_len = bytes.len() as u32 - extent;
            let previous=if key==32 {
                crate::words::check_directory::<32,36>(bytes,extent as usize,Ghost(count as int))? as u32
            }else{
                let mut remaining=&bytes[4..extent as usize];
                let mut previous=0u32;
                let ghost mut i=0int;
                while remaining.len()>=stride as usize
                    invariant strict,descriptor_valid(*layout,MAX_DEPTH as nat),
                        matches!(layout.head(),LayoutHead::Map{..}),width.is_none(),width==value_width(*layout),
                        key==key_width(*layout),stride as int==entry_stride(*layout),stride as int==key+4,
                        count as int==le_word(bytes@,0),count>0,0<=i<=count,
                        extent as int==4+count as int*stride,extent<=bytes.len()<=u32::MAX,
                        payload_len as int==bytes.len()-extent,previous<=payload_len,
                        remaining@==bytes@.subrange(4+i*stride,extent as int),remaining.len()==(count as int-i)*stride,
                        previous as int==if i==0 {0int}else{le_word(bytes@,4+(i-1)*stride+key)},
                        forall|k:int| 0<=k<i ==> {
                            let end=#[trigger] le_word(bytes@,4+k*stride+key);
                            end<=payload_len && (if k==0 {0<=end}else{le_word(bytes@,4+(k-1)*stride+key)<=end})
                        },
                    decreases count-i,
                {
                    proof {
                        assert(i<count) by(nonlinear_arith)
                            requires remaining.len()==(count as int-i)*stride,
                                remaining.len()>=stride,stride>0;
                        assert(4+i*stride+stride<=extent) by(nonlinear_arith)
                            requires 0<=i<count,extent==4+count as int*stride,stride>0;
                        assert(extent-remaining.len()+key+4<=extent) by(nonlinear_arith)
                            requires remaining.len()==(count as int-i)*stride,
                                extent==4+count as int*stride,0<=i<count,stride as int==key+4;
                    }
                    let end=crate::words::directory_word(remaining,key as usize);
                    proof {crate::words::word_subrange(bytes@,4+i*stride,extent as int,key as int);}
                    assert(end==le_word(bytes@,4+i*stride+key));
                    if end<previous || end>payload_len {
                        let at=extent-remaining.len() as u32+key;
                        proof {
                            if map_shape(bytes@,*layout) {
                                assert(payload_start(bytes@,*layout)==extent);
                                assert(le_word(bytes@,4+i*stride+key)<=payload_len);
                                assert(previous<=end);
                            }
                        }
                        return Err(Error::new(ErrorKind::InvalidBoundary,at));
                    }
                    previous=end;
                    proof {
                        assert(4+(i+1)*stride==4+i*stride+stride) by(nonlinear_arith);
                        assert((count as int-i)*stride-stride==(count as int-(i+1))*stride) by(nonlinear_arith);
                        assert(remaining@.subrange(stride as int,remaining.len() as int)
                            =~= bytes@.subrange(4+(i+1)*stride,extent as int));
                    }
                    remaining=&remaining[stride as usize..];
                    proof {i=i+1;}
                }
                assert(i==count) by(nonlinear_arith)
                    requires remaining.len()==(count as int-i)*stride,
                        remaining.len()<stride,stride>0,0<=i<=count;
                previous
            };
            if previous != payload_len {
                return Err(Error::new(ErrorKind::InvalidBoundary, extent - 4));
            }
            extent
        },
    };
    Ok(MapPartition {
        count, key_width: key, value_width: width,
        directory: Span { start: 4, end: payload },
        payload: Span { start: payload, end: bytes.len() as u32 },
    })
}

/// Mathematical value interval, excluding the interleaved key or directory.
pub open spec fn value_span(bytes: Seq<u8>, layout: Layout, index: u32) -> Span {
    let stride = entry_stride(layout);
    match value_width(layout) {
        Some(width) => Span {
            start: (4 + index as int * stride + key_width(layout)) as u32,
            end: (4 + index as int * stride + stride) as u32,
        },
        None => {
            let p = payload_start(bytes, layout);
            Span {
                start: (p + if index == 0 { 0int }
                    else { le_word(bytes, 4 + (index as int - 1) * stride + key_width(layout)) }) as u32,
                end: (p + le_word(bytes, 4 + index as int * stride + key_width(layout))) as u32,
            }
        },
    }
}

/// Exact value selection iff the entire partition and ordinal are valid.
pub open spec fn range_exact(
    result: Result<Span>, bytes: Seq<u8>, layout: Layout, index: u32,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && map_shape(bytes, layout)
        && index < le_word(bytes, 0);
    match result {
        Ok(span) => valid && span == value_span(bytes, layout, index)
            && payload_start(bytes, layout) <= span.start <= span.end <= bytes.len(),
        Err(_) => !valid,
    }
}

/// Returns a value's exact region after checking the entire map partition.
///
/// Invalid ordinals return `IndexOutOfBounds` without an offset after metadata
/// and partition errors. Empty indexed values return empty payload spans.
/// Neither key ordering/domains nor child encodings are checked.
pub fn map_value(bytes: &[u8], layout: &Layout, index: u32) -> (result: Result<Span>)
    ensures range_exact(result, bytes@, *layout, index),
{
    hide(le_word);
    let partition = partition_map(bytes, layout)?;
    if index >= partition.count {
        return Err(Error::new(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET));
    }
    map_value_at(bytes, layout, &partition, index)
}

// Select in constant time from a partition checked once by the caller.
pub(crate) fn map_value_at(
    bytes: &[u8], _layout: &Layout, partition: &MapPartition, index: u32,
) -> (result: Result<Span>)
    requires
        partition_iff(Ok(*partition), bytes@, *_layout),
        index < partition.count,
    ensures range_exact(result, bytes@, *_layout, index), result.is_ok(),
{
    hide(le_word);
    let tail = partition.value_width.unwrap_or(4u32);
    proof {
        let stride = entry_stride(*_layout);
        assert(0 <= index as int * stride
            && 4 + index as int * stride + stride <= bytes.len()
            && stride <= u32::MAX) by(nonlinear_arith)
            requires index < partition.count, partition.count > 0, stride > 0,
                4 + partition.count as int * stride <= bytes.len(), bytes.len() <= u32::MAX;
    }
    let stride = partition.key_width + tail;
    let at = 4 + index * stride + partition.key_width;
    match partition.value_width {
        Some(width) => {
            let span = Span { start: at, end: at + width };
            assert(range_exact(Ok(span), bytes@, *_layout, index));
            Ok(span)
        },
        None => {
            let start = if index == 0 { 0u32 } else {
                proof {
                    assert(at as int - stride == 4 + (index as int - 1) * stride + partition.key_width)
                        by(nonlinear_arith)
                        requires at as int == 4 + index as int * stride + partition.key_width;
                    assert(le_word(bytes@, 4 + (index as int - 1) * stride + partition.key_width)
                        <= bytes.len() - partition.payload.start);
                }
                read_u32(bytes, at - stride)?
            };
            let end = read_u32(bytes, at)?;
            assert(le_word(bytes@, 4 + index as int * stride + partition.key_width)
                <= bytes.len() - partition.payload.start);
            assert(start <= end);
            let span = Span { start: partition.payload.start + start, end: partition.payload.start + end };
            assert(range_exact(Ok(span), bytes@, *_layout, index));
            Ok(span)
        },
    }
}

}
