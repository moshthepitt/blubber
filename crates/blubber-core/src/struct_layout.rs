//! Exact fixed regions and nonrecursive struct byte partitions.

use crate::layout::fixed_width;
#[cfg(verus_only)]
use crate::layout::{descriptor_valid, width_spec};
#[cfg(verus_only)]
use crate::words::le_word;
use crate::LayoutHead;
use crate::{check_layout, read_u32, Error, ErrorKind, Layout, Result, Span, MAX_DEPTH};
use vstd::prelude::*;

verus! {

/// Checked regions of a struct encoding, relative to its supplied slice.
///
/// Public fields support inspection. Direct construction establishes no invariant;
/// the byte gateways compute and check their own partitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructPartition {
    /// Packed fixed prefix, beginning at zero.
    pub fixed: Span,
    /// Stored ends, with the last end elided.
    pub directory: Span,
    /// Concatenated variable fields, including empty ones.
    pub payload: Span,
    /// Number of variable fields; zero and one both use no stored ends.
    pub variables: u16,
}

/// Mathematical directory size, including the zero/one-field cases.
pub open spec fn directory_size(count: int) -> int {
    if count > 0 { 4 * (count - 1) } else { 0 }
}

/// The payload's mathematical start, independent of the parser.
pub open spec fn payload_start(layout: Layout) -> int {
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed: _, variable } =>
            fixed_bytes as int + directory_size(variable.len() as int),
        _ => 0,
    }
}

/// Declarative struct shape: exact fixed extent and monotone payload boundaries.
///
/// This predicate does not validate leaf domains or any child's encoding.
pub open spec fn struct_shape(bytes: Seq<u8>, layout: Layout) -> bool {
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed: _, variable } => {
            let p = payload_start(layout);
            &&& bytes.len() <= u32::MAX
            &&& p <= bytes.len()
            &&& if variable.len() == 0 { bytes.len() == fixed_bytes }
                else {
                    forall|i: int| 0 <= i < variable.len() - 1 ==> {
                        let end = #[trigger] le_word(bytes, fixed_bytes as int + 4 * i);
                        &&& end <= bytes.len() - p
                        &&& if i == 0 { 0 <= end }
                            else { le_word(bytes, fixed_bytes as int + 4 * (i - 1)) <= end }
                    }
                }
        },
        _ => false,
    }
}

/// Acceptance and rejection are equivalent to descriptor and structural validity.
pub open spec fn partition_iff(
    result: Result<StructPartition>, bytes: Seq<u8>, layout: Layout,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && struct_shape(bytes, layout);
    match result {
        Ok(partition) => valid && match layout.head() {
            LayoutHead::Struct { fixed_bytes, fixed: _, variable } =>
                partition.fixed == (Span { start: 0, end: fixed_bytes })
                && partition.directory == (Span { start: fixed_bytes, end: payload_start(layout) as u32 })
                && partition.payload == (Span { start: payload_start(layout) as u32, end: bytes.len() as u32 })
                && partition.variables == variable.len(),
            _ => false,
        },
        Err(_) => !valid,
    }
}

/// Header geometry without certifying untouched directory words.
pub open spec fn header_shape(bytes: Seq<u8>, layout: Layout) -> bool {
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, variable, .. } => bytes.len() <= u32::MAX
            && payload_start(layout) <= bytes.len()
            && (variable.len() == 0 ==> bytes.len() == fixed_bytes),
        _ => false,
    }
}

/// Exact header regions; directory monotonicity remains a separate predicate.
pub open spec fn header_iff(
    result: Result<StructPartition>, bytes: Seq<u8>, layout: Layout,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && header_shape(bytes, layout);
    match result {
        Ok(partition) => valid && match layout.head() {
            LayoutHead::Struct { fixed_bytes, fixed: _, variable } =>
                partition.fixed == (Span { start: 0, end: fixed_bytes })
                && partition.directory == (Span { start: fixed_bytes, end: payload_start(layout) as u32 })
                && partition.payload == (Span { start: payload_start(layout) as u32, end: bytes.len() as u32 })
                && partition.variables == variable.len(),
            _ => false,
        },
        Err(_) => !valid,
    }
}


/// Checks a fixed-kind descriptor and its exact region length.
///
/// Metadata uses [`MAX_DEPTH`]. Variable-kind layouts return `InvalidSchema`.
/// A short slice returns `Truncated` at zero; trailing bytes return
/// `InvalidBoundary` at the fixed width. Leaf bit domains are checked separately.
pub fn fixed_region(bytes: &[u8], layout: &Layout) -> (result: Result<Span>)
    ensures match result {
        Ok(span) => descriptor_valid(*layout, MAX_DEPTH as nat)
            && width_spec(*layout) == Some(bytes@.len() as u32)
            && bytes@.len() <= u32::MAX
            && span == (Span { start: 0, end: bytes@.len() as u32 }),
        Err(_) => !(descriptor_valid(*layout, MAX_DEPTH as nat)
            && width_spec(*layout).is_some()
            && width_spec(*layout).unwrap() == bytes@.len()),
    },
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    check_layout(layout, MAX_DEPTH)?;
    let width = match fixed_width(layout) {
        Some(width) => width,
        None => return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)),
    };
    if bytes.len() < width as usize {
        return Err(Error::new(ErrorKind::Truncated, 0));
    }
    if bytes.len() != width as usize {
        return Err(Error::new(ErrorKind::InvalidBoundary, width));
    }
    Ok(Span { start: 0, end: width })
}

/// Checks a struct's exact fixed prefix, directory, and variable payload.
///
/// Checks descriptor metadata first, then wire-length/header overflow, fixed
/// prefix truncation, directory truncation, and stored ends in wire order.
/// Ends may be equal. Zero variable fields require an exact fixed-only region;
/// one variable field consumes the entire remaining region without a word.
/// Child encodings and scalar bit domains are left to schema validation.
pub fn partition_struct(bytes: &[u8], layout: &Layout) -> (result: Result<StructPartition>)
    ensures partition_iff(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    partition_struct_mode(bytes, layout, true)
}

/// Checks header extents while leaving unrelated stored ends lazy.
#[inline(always)]
pub(crate) fn partition_struct_header(bytes: &[u8], layout: &Layout) -> (result: Result<StructPartition>)
    ensures header_iff(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    partition_struct_mode(bytes, layout, false)
}

#[inline(always)]
fn partition_struct_mode(bytes: &[u8], layout: &Layout, strict: bool) -> (result: Result<StructPartition>)
    ensures strict ==> partition_iff(result, bytes@, *layout),
        !strict ==> header_iff(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    hide(le_word);
    check_layout(layout, MAX_DEPTH)?;
    let (fixed_bytes, count) = if let Layout::Flat{checked}=layout {
        proof{reveal(crate::layout_head::head_spec);}
        if checked.header(0)!=9{return Err(ErrorKind::InvalidSchema.into());}
        proof{reveal(crate::layout_head::head_spec);}
        (checked.header(1),checked.header(4) as usize)
    }else{match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed: _, variable } => (fixed_bytes, variable.len()),
        _ => return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)),
    }};

    if bytes.len() > u32::MAX as usize {
        return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
    }
    let words = if count == 0 { 0usize } else { count - 1 };
    // Descriptor validation bounds the field count; multiplication is exact.
    assert(count <= 65535);
    let directory_bytes = (words * 4) as u32;
    let payload = match fixed_bytes.checked_add(directory_bytes) {
        Some(payload) => payload,
        None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    if bytes.len() < fixed_bytes as usize {
        return Err(Error::new(ErrorKind::Truncated, 0));
    }
    if bytes.len() < payload as usize {
        let complete_words = (bytes.len() - fixed_bytes as usize) / 4;
        let at = fixed_bytes + (complete_words * 4) as u32;
        return Err(Error::new(ErrorKind::Truncated, at));
    }
    if count == 0 && bytes.len() != fixed_bytes as usize {
        return Err(Error::new(ErrorKind::InvalidBoundary, fixed_bytes));
    }
    if !strict {
                return Ok(StructPartition { fixed: Span { start: 0, end: fixed_bytes },
            directory: Span { start: fixed_bytes, end: payload },
            payload: Span { start: payload, end: bytes.len() as u32 }, variables: count as u16 });
            }
    let payload_len = bytes.len() as u32 - payload;
    let mut i = 0usize;
    let mut previous = 0u32;
    while i < words
        invariant strict,
            match layout.head() {
                LayoutHead::Struct { fixed_bytes: f, fixed: _, variable } =>
                    fixed_bytes == f && count == variable.len(),
                _ => false,
            },
            count <= 65535,
            words == if count == 0 { 0int } else { count as int - 1 },
            i <= words,
            payload as int == fixed_bytes as int + 4 * words,
            payload <= bytes.len() <= u32::MAX,
            payload_len as int == bytes.len() - payload,
            previous <= payload_len,
            previous as int == if i == 0 { 0int }
                else { le_word(bytes@, fixed_bytes as int + 4 * (i - 1)) },
            forall|k: int| 0 <= k < i ==> {
                let end = #[trigger] le_word(bytes@, fixed_bytes as int + 4 * k);
                &&& end <= payload_len
                &&& if k == 0 { 0 <= end }
                    else { le_word(bytes@, fixed_bytes as int + 4 * (k - 1)) <= end }
            },
        decreases words - i,
    {
        let at = fixed_bytes + (i * 4) as u32;
        let end = read_u32(bytes, at)?;
        if end < previous || end > payload_len {
            proof {
                if struct_shape(bytes@, *layout) {
                    assert(payload_start(*layout) == payload);
                    assert(le_word(bytes@, fixed_bytes as int + 4 * i) <= payload_len);
                    assert(previous <= end);
                }
            }
            return Err(Error::new(ErrorKind::InvalidBoundary, at));
        }
        previous = end;
        i += 1;
    }
    Ok(StructPartition {
        fixed: Span { start: 0, end: fixed_bytes },
        directory: Span { start: fixed_bytes, end: payload },
        payload: Span { start: payload, end: bytes.len() as u32 },
        variables: count as u16,
    })
}

/// Exact parent-delimited region of a variable field, in payload order.
pub open spec fn variable_span(bytes: Seq<u8>, layout: Layout, index: u16) -> Span {
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed: _, variable } => {
            let p = payload_start(layout);
            Span {
                start: (p + if index == 0 { 0int }
                    else { le_word(bytes, fixed_bytes as int + 4 * (index as int - 1)) }) as u32,
                end: if index as int + 1 == variable.len() { bytes.len() as u32 }
                    else { (p + le_word(bytes, fixed_bytes as int + 4 * index)) as u32 },
            }
        },
        _ => Span { start: 0, end: 0 },
    }
}

/// Exact field endpoints and containment in the immediate struct payload.
pub open spec fn range_exact(
    result: Result<Span>, bytes: Seq<u8>, layout: Layout, index: u16,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && struct_shape(bytes, layout)
        && match layout.head() {
            LayoutHead::Struct { fixed_bytes: _, fixed: _, variable } => index < variable.len(),
            _ => false,
        };
    match result {
        Ok(span) => valid && span == variable_span(bytes, layout, index)
            && payload_start(layout) <= span.start <= span.end <= bytes.len(),
        Err(_) => !valid,
    }
}

/// Returns one variable field's exact span after checking the full partition.
///
/// `index` is the zero-based variable-field ordinal, independent of fixed fields
/// and declaration ordinals. An invalid index returns `IndexOutOfBounds` without
/// an offset. Empty fields and the implicit first/last boundaries are supported.
pub fn struct_variable_field(bytes: &[u8], layout: &Layout, index: u16) -> (result: Result<Span>)
    ensures range_exact(result, bytes@, *layout, index),
{
    let partition = partition_struct(bytes, layout)?;
    if index >= partition.variables {
        return Err(Error::new(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET));
    }
    struct_variable_field_at(bytes, layout, &partition, index)
}

// Select in constant time from a partition checked once by the caller.
pub(crate) fn struct_variable_field_at(
    bytes: &[u8], _layout: &Layout, partition: &StructPartition, index: u16,
) -> (result: Result<Span>)
    requires
        partition_iff(Ok(*partition), bytes@, *_layout),
        index < partition.variables,
    ensures range_exact(result, bytes@, *_layout, index), result.is_ok(),
{
    hide(le_word);
    let start = if index == 0 { 0u32 } else {
        read_u32(bytes, partition.directory.start + 4 * (index as u32 - 1))?
    };
    let end = if index as u32 + 1 == partition.variables as u32 {
        partition.payload.end - partition.payload.start
    } else {
        read_u32(bytes, partition.directory.start + 4 * index as u32)?
    };
    Ok(Span { start: partition.payload.start + start, end: partition.payload.start + end })
}

}
