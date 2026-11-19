//! Checked equal-width overwrites for generated packed-prefix fields.

use crate::{checked_delta, checked_u32, read_u32, Error, ErrorKind, Result};
#[cfg(verus_only)]
use crate::{words, Span};
use vstd::prelude::*;

verus! {

/// Exact overwrite range and complete-buffer frame, including spare bytes.
pub open spec fn overwrite_frame(before: Seq<u8>, after: Seq<u8>, span: Span) -> bool {
    before.len() == after.len() && forall|i: int| 0 <= i < before.len()
        && !(span.start <= i < span.end) ==> #[trigger] after[i] == before[i]
}

/// Borrows an equal-width destination after checking the account envelope.
///
/// `offset` is relative to the root at byte twelve. Callers derive it from
/// certified fixed-field metadata or a checked collection-element span. `root_width` is present only for a
/// fixed-only root, whose logical extent must match exactly. This gateway checks
/// containment independently of that metadata and changes no byte itself.
#[doc(hidden)]
#[inline(always)]
pub fn fixed_destination(bytes: &mut [u8], discriminator: [u8; 8], minimum: u32,
    root_width: Option<u32>, offset: u32, width: u32) -> (result: Result<&mut [u8]>)
    ensures result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> {
            let start = 12 + offset as int;
            let end = start + width;
            &&& 12 <= start <= end <= words::le_word(old(bytes)@, 8) <= old(bytes).len() <= u32::MAX
            &&& result.unwrap()@ == old(bytes)@.subrange(start, end)
            &&& result.unwrap().len() == width
            &&& final(bytes).len() == old(bytes).len()
            &&& final(bytes)@.subrange(start, end) == final(result.unwrap())@
            &&& overwrite_frame(old(bytes)@, final(bytes)@, Span { start: start as u32, end: end as u32 })
        },
{
    if discriminator == [0u8; 8] { return Err(ErrorKind::InvalidSchema.into()); }
    checked_u32(bytes.len())?;
    if bytes.len() < 12 { return Err(Error::new(ErrorKind::Truncated, 0)); }
    if bytes[0] != discriminator[0] || bytes[1] != discriminator[1]
        || bytes[2] != discriminator[2] || bytes[3] != discriminator[3]
        || bytes[4] != discriminator[4] || bytes[5] != discriminator[5]
        || bytes[6] != discriminator[6] || bytes[7] != discriminator[7] {
        return Err(Error::new(ErrorKind::Discriminator, 0));
    }
    let minimum_used = checked_delta(12, minimum as i64)?;
    let used = read_u32(bytes, 8)?;
    if used < minimum_used || used as usize > bytes.len() { return Err(Error::new(ErrorKind::UsedLength, 8)); }
    if let Some(root_width) = root_width {
        let exact = checked_delta(12, root_width as i64)?;
        if used != exact { return Err(Error::new(ErrorKind::InvalidBoundary, exact)); }
    }
    let start = checked_delta(12, offset as i64)?;
    let end = checked_delta(start, width as i64)?;
    if end > used { return Err(Error::new(ErrorKind::InvalidBoundary, start)); }
    let (prefix, suffix) = bytes.split_at_mut(start as usize);
    let (destination, tail) = suffix.split_at_mut(width as usize);
    proof { vstd::slice::mut_ref_slice_len_eq(&destination); }
    let _ = prefix;
    let _ = tail;
    Ok(destination)
}

}
