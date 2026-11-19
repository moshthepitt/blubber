//! Checked directory transport and sticky publication state.

use crate::{checked_delta, read_u32, Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// Numeric containment of a directory boundary word; independent of its bytes.
pub open spec fn directory_slot(base: int, payload: int, boundary: int) -> bool {
    base <= base + 4 * boundary && base + 4 * boundary + 4 <= payload
}

/// Exact range and arithmetic result for a retained source boundary.
pub open spec fn end_result(result: Result<u32>, end: u32, lower: u32, upper: u32, delta: i64, at: u32) -> bool {
    if end < lower || end > upper {
        result == Err(Error::Fault { kind: ErrorKind::InvalidBoundary, offset: at })
    } else if 0 <= end as int + delta <= u32::MAX {
        result == Ok((end as int + delta) as u32)
    } else {
        result == Err(Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: at })
    }
}

/// Checks the old boundary before applying a full-width signed delta.
///
/// Range errors precede arithmetic errors. Both identify the source word.
/// No source-validity premise is needed, including for `i64::MIN`.
#[inline(always)]
pub fn checked_end(end: u32, lower: u32, upper: u32, delta: i64, at: u32) -> (result: Result<u32>)
    ensures end_result(result, end, lower, upper, delta, at),
    no_unwind
{
    if end < lower || end > upper {
        return Err(Error::new(ErrorKind::InvalidBoundary, at));
    }
    match checked_delta(end, delta) {
        Ok(value) => Ok(value),
        Err(_) => Err(Error::new(ErrorKind::ArithmeticOverflow, at)),
    }
}

/// Checks one retained end and its shifted result before publishing that word.
///
/// The caller admits movement and caller-input errors before entering a patch
/// pass. Every return leaves poison armed. Failure preserves the current word
/// and all other bytes; earlier successful calls may already have written.
/// Sources and destinations may coincide. No byte or coordinate premise is
/// required for containment, arithmetic, or termination.
// Explicit returns keep the no-unwind proof independent of residual conversion.
#[allow(clippy::question_mark)]
#[inline(always)]
pub fn patch_word(bytes: &mut [u8], source: u32, destination: u32,
    lower: u32, upper: u32, delta: i64, poisoned: &mut bool) -> (result: Result<()>)
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> crate::words::word_fits(old(bytes).len() as int, source)
            && crate::words::word_fits(old(bytes).len() as int, destination)
            && end_result(Ok(crate::words::le_word(final(bytes)@, destination as int) as u32),
                crate::words::le_word(old(bytes)@, source as int) as u32, lower, upper, delta, source)
            && crate::words::encode_frame(old(bytes)@, final(bytes)@, destination as int),
    no_unwind
{
    *poisoned = true;
    let end = match read_u32(bytes, source) { Ok(value) => value, Err(error) => return Err(error) };
    let value = match checked_end(end, lower, upper, delta, source) { Ok(value) => value, Err(error) => return Err(error) };
    crate::raw::write_u32(bytes, destination, value)
}

/// Transports a word and returns its old end for the next order check.
///
/// The source is read once. Poison stays armed on every return, and a rejected
/// word preserves the complete slice supplied to this call.
#[inline(always)]
#[allow(clippy::question_mark)]
pub fn transport(bytes: &mut [u8], source: u32, destination: u32,
    lower: u32, upper: u32, delta: i64, poisoned: &mut bool) -> (result: Result<u32>)
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        result.is_ok() == (crate::words::word_fits(old(bytes).len() as int, source)
            && crate::words::word_fits(old(bytes).len() as int, destination)
            && lower <= crate::words::le_word(old(bytes)@, source as int) <= upper
            && 0 <= crate::words::le_word(old(bytes)@, source as int) + delta <= u32::MAX),
        result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> crate::words::word_fits(old(bytes).len() as int, source)
            && crate::words::word_fits(old(bytes).len() as int, destination)
            && result.unwrap() == crate::words::le_word(old(bytes)@, source as int)
            && lower <= result.unwrap() <= upper
            && 0 <= result.unwrap() as int + delta <= u32::MAX
            && crate::words::le_word(final(bytes)@, destination as int) == result.unwrap() as int + delta
            && crate::words::encode_frame(old(bytes)@, final(bytes)@, destination as int),
    no_unwind
{
    *poisoned = true;
    let end = match read_u32(bytes, source) { Ok(value) => value, Err(error) => return Err(error) };
    let value = match checked_end(end, lower, upper, delta, source) { Ok(value) => value, Err(error) => return Err(error) };
    match crate::raw::write_u32(bytes, destination, value) {
        Ok(()) => Ok(end), Err(error) => Err(error),
    }
}

/// Transports a word inside the already admitted directory movement window.
/// Numeric containment is independent of every source word's value.
#[inline(always)]
#[allow(clippy::question_mark)]
pub fn transport_contained(bytes: &mut [u8], source: usize, destination: usize,
    lower: u32, upper: u32, delta: i64, poisoned: &mut bool) -> (result: Result<u32>)
    requires source + 4 <= old(bytes).len() <= u32::MAX, destination + 4 <= old(bytes).len(),
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        result.is_ok() == (lower <= crate::words::le_word(old(bytes)@, source as int) <= upper
            && 0 <= crate::words::le_word(old(bytes)@, source as int) + delta <= u32::MAX),
        result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> result.unwrap() == crate::words::le_word(old(bytes)@, source as int)
            && lower <= result.unwrap() <= upper
            && crate::words::le_word(final(bytes)@, destination as int) == result.unwrap() as int + delta
            && crate::words::encode_frame(old(bytes)@, final(bytes)@, destination as int),
    no_unwind
{
    *poisoned = true;
    let end = crate::words::read_contained_word(bytes, source);
    if end < lower || end > upper { return Err(Error::new(ErrorKind::InvalidBoundary, source as u32)); }
    let value = match (end as i64).checked_add(delta) {
        Some(value) => value,
        None => return Err(Error::new(ErrorKind::ArithmeticOverflow, source as u32)),
    };
    if value < 0 || value > u32::MAX as i64 { return Err(Error::new(ErrorKind::ArithmeticOverflow, source as u32)); }
    crate::words::write_contained_word(bytes, destination, value as u32);
    Ok(end)
}


/// Ghost-only success condition for the old ends consumed by a directory loan.
pub(crate) open spec fn directory_safe(bytes: Seq<u8>, key: usize, stride: usize,
    count: usize, lower: u32, upper: u32, delta: i64) -> bool {
    forall|i: int| 0 <= i < count ==> {
        let end = #[trigger] crate::words::le_word(bytes, i * stride + key);
        &&& lower <= end <= upper
        &&& i > 0 ==> crate::words::le_word(bytes, (i - 1) * stride + key) <= end
        &&& 0 <= end + delta <= u32::MAX
    }
}

/// Walks one exact directory loan, checking and publishing each end once.
#[verifier::spinoff_prover]
#[inline(always)]
#[allow(clippy::too_many_arguments, clippy::question_mark)]
pub(crate) fn transport_directory<const KEY: usize, const STRIDE: usize>(bytes: &mut [u8], key: usize, stride: usize, _count: usize,
    lower: u32, upper: u32, delta: i64, base: u32, poisoned: &mut bool) -> (result: Result<()>)
    requires key + 4 == stride, old(bytes).len() == _count * stride,
        KEY == 0 || key == KEY, STRIDE == 0 || stride == STRIDE,
        base + old(bytes).len() <= u32::MAX,
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        directory_safe(old(bytes)@, key, stride, _count, lower, upper, delta) ==> result.is_ok(),
        result.is_ok() ==> {
            &&& forall|i: int| 0 <= i < _count ==> #[trigger] crate::words::le_word(final(bytes)@, i * stride + key)
                == crate::words::le_word(old(bytes)@, i * stride + key) + delta
            &&& forall|i: int, k: int| 0 <= i < _count && 0 <= k < key ==>
                #[trigger] final(bytes)@[i * stride + k] == old(bytes)@[i * stride + k]
        },
    no_unwind
{
    proof { vstd::slice::mut_ref_slice_len_eq(&bytes); }
    transport_directory_pass::<KEY, STRIDE>(bytes, key, stride, _count, lower, upper, delta, base, poisoned)
}

#[verifier::spinoff_prover]
#[inline(always)]
#[allow(clippy::too_many_arguments, clippy::question_mark)]
fn transport_directory_pass<const KEY: usize, const STRIDE: usize>(bytes: &mut [u8], key: usize, stride: usize, _count: usize,
    lower: u32, upper: u32, delta: i64, base: u32, poisoned: &mut bool) -> (result: Result<()>)
    requires key + 4 == stride, old(bytes).len() == _count * stride,
        KEY == 0 || key == KEY, STRIDE == 0 || stride == STRIDE,
        base + old(bytes).len() <= u32::MAX,
    ensures *final(poisoned),
        directory_safe(old(bytes)@, key, stride, _count, lower, upper, delta) ==> result.is_ok(),
        result.is_ok() ==> {
            &&& final(bytes).len() == old(bytes).len()
            &&& forall|i: int| 0 <= i < _count ==> #[trigger] crate::words::le_word(final(bytes)@, i * stride + key)
                == crate::words::le_word(old(bytes)@, i * stride + key) + delta
            &&& forall|i: int, k: int| 0 <= i < _count && 0 <= k < key ==>
                #[trigger] final(bytes)@[i * stride + k] == old(bytes)@[i * stride + k]
        },
    no_unwind
{
    hide(crate::words::le_word);
    proof { vstd::slice::mut_ref_slice_len_eq(&bytes); }
    let patch_key = if KEY == 0 { key } else { KEY };
    let patch_stride = if STRIDE == 0 { stride } else { STRIDE };
    *poisoned = true;
    let ghost before = bytes@;
    let ghost future = final(bytes)@;
    proof { assert(future.len() == before.len()); }
    let ghost mut prefix = Seq::<u8>::empty();
    let ghost mut done = 0int;
    let mut previous = lower;
    let mut offset = base;
    {
        let mut rest = &mut *bytes;
        while !rest.is_empty()
            invariant before == old(bytes)@, patch_key == key, patch_stride == stride, future.len() == before.len(), key + 4 == stride, 0 <= done <= _count,
                before.len() == old(bytes).len() == _count * stride, base + before.len() <= u32::MAX,
                prefix.len() + rest.len() == before.len(), prefix.len() == done * stride,
                rest.len() == (_count - done) * stride, *poisoned,
                rest@ == before.subrange(prefix.len() as int, before.len() as int),
                future == prefix + final(rest)@,
                previous == if done == 0 { lower as int } else { crate::words::le_word(before, (done - 1) * stride + key) },
                offset == base + done * stride,
                forall|i: int| 0 <= i < done ==> #[trigger] crate::words::le_word(prefix, i * stride + key)
                    == crate::words::le_word(before, i * stride + key) + delta,
                forall|i: int, k: int| 0 <= i < done && 0 <= k < key ==>
                    #[trigger] prefix[i * stride + k] == before[i * stride + k],
            decreases rest.len(),
        {
            proof {
                assert(done < _count && stride <= rest.len()) by(nonlinear_arith)
                    requires rest.len() > 0, rest.len() == (_count - done) * stride, stride > 0, done <= _count;
            }
            let ghost old_offset = prefix.len();
            let ghost length = rest.len();
            let (entry, tail) = rest.split_at_mut(patch_stride);
            proof {
                vstd::slice::mut_ref_slice_len_eq(&entry);
                assert(entry@ =~= before.subrange(old_offset as int, old_offset + stride));
                crate::words::word_subrange(before, old_offset as int, old_offset + stride, key as int);
                if directory_safe(before, key, stride, _count, lower, upper, delta) {
                    assert(previous <= crate::words::le_word(entry@, key as int) <= upper);
                    assert(0 <= crate::words::le_word(entry@, key as int) + delta <= u32::MAX);
                }
            }
            previous = match transport_contained(entry, patch_key, patch_key, previous, upper, delta, poisoned) {
                Ok(value) => value, Err(error) => return Err(error.rebase(offset)),
            };
            proof {
                let next = prefix + entry@;
                assert((done + 1) * stride == prefix.len() + stride) by(nonlinear_arith)
                    requires prefix.len() == done * stride;
                assert(tail.len() == (_count - (done + 1)) * stride) by(nonlinear_arith)
                    requires tail.len() == length - stride, length == (_count - done) * stride;
                crate::replace_plan::directory_suffix_induction(before, prefix, entry@, key as int, stride as int, delta as int, done);
                prefix = next;
                done = done + 1;
                assert(tail@ =~= before.subrange(prefix.len() as int, before.len() as int));
            }
            offset += patch_stride as u32;
            rest = tail;
        }
        proof {
            assert(done == _count) by(nonlinear_arith)
                requires rest.len() == (_count - done) * stride, rest.len() == 0, stride > 0;
            assert(future =~= prefix);
        }
    }
    Ok(())
}

/// Publication clears poison only after the complete operation succeeds.
///
/// Invoke this after local words, ancestor patches and used length are all
/// published. An error, including a detached preparation error, stays sticky.
#[inline(always)]
pub fn finish_publication(poisoned: &mut bool, publication: Result<()>) -> (result: Result<()>)
    ensures result == publication, *final(poisoned) == publication.is_err(),
    no_unwind
{
    *poisoned = publication.is_err();
    publication
}

/// Writes the complete empty-list encoding into an exact four-byte loan.
///
/// A wrong loan width is rejected before any write.
#[inline(always)]
pub fn write_empty_list(bytes: &mut [u8]) -> (result: Result<()>)
    ensures result.is_ok() ==> final(bytes)@ == seq![0u8, 0, 0, 0],
        result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() == (old(bytes).len() == 4),
    no_unwind
{
    if bytes.len() != 4 { return Err(Error::new(ErrorKind::InvalidBoundary, Error::NO_OFFSET)); }
    crate::raw::write_u32(bytes, 0, 0)
}

/// Discarding all indexed values produces a valid list without reading old ends.
pub(crate) proof fn cleared_list_valid(layout: crate::Layout)
    requires crate::layout::descriptor_valid(layout, crate::MAX_DEPTH as nat),
        matches!(layout.head(), crate::LayoutHead::List { .. }),
    ensures crate::validation::schema_valid(seq![0u8,0,0,0], layout, crate::MAX_DEPTH as nat, true),
{
    reveal_with_fuel(crate::validation::schema_valid, 1);
    reveal_with_fuel(crate::validation::children_valid, 1);
}

/// Checked transport either rejects or returns the exact representable sum.
proof fn transport_totality(result: Result<u32>, end: u32, lower: u32, upper: u32, delta: i64, at: u32)
    requires end_result(result, end, lower, upper, delta, at),
    ensures result.is_ok() == (lower <= end <= upper && 0 <= end as int + delta <= u32::MAX),
        result.is_ok() ==> result.unwrap() == end as int + delta,
        result.is_err() ==> result.unwrap_err() == (Error::Fault {
            kind: if end < lower || end > upper { ErrorKind::InvalidBoundary } else { ErrorKind::ArithmeticOverflow },
            offset: at }),
{
}

/// A later rejected word retains the completed prefix of a patch pass.
proof fn later_failure_frame(before: Seq<u8>, completed: Seq<u8>, after: Seq<u8>, at: u32)
    requires crate::words::encode_frame(before, completed, at as int), after == completed,
    ensures crate::words::encode_frame(before, after, at as int),
        after.subrange(at as int, at as int + 4) == completed.subrange(at as int, at as int + 4),
{
}

/// A returned publication error cannot clear poison.
proof fn sticky_publication(publication: Result<()>, poisoned: bool)
    requires poisoned == publication.is_err(),
    ensures publication.is_err() ==> poisoned,
        !poisoned ==> publication.is_ok(),
{
}

/// Concrete success, full-width overflow, range rejection and late-failure frames.
proof fn patch_witness()
{
    let arithmetic = Err(Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: 4 });
    let range = Err(Error::Fault { kind: ErrorKind::InvalidBoundary, offset: 0 });
    transport_totality(Ok(3u32), 1, 0, u32::MAX, 2, 0);
    transport_totality(arithmetic, u32::MAX, 0, u32::MAX, 1, 4);
    transport_totality(range, 0, 1, 10, i64::MIN, 0);
    transport_totality(Err(Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: 0 }),
        0, 0, u32::MAX, i64::MIN, 0);
    let before = seq![1u8,0,0,0,255,255,255,255];
    let completed = seq![3u8,0,0,0,255,255,255,255];
    later_failure_frame(before, completed, completed, 0);
    assert(completed[0] != before[0]);
    sticky_publication(Err(Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: 4 }), true);
    sticky_publication(Ok(()), false);
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    cleared_list_valid(crate::Layout::List { element: &crate::Layout::Str });
}

}
