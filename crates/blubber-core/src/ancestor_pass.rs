//! Forward checked ancestor ends over exact mutable word loans.
use crate::{words, Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// All admitted old ends fit the checked numeric interval.
pub(crate) open spec fn ends_within(bytes: Seq<u8>, lower: u32, upper: u32) -> bool {
    forall|i: int| 0 <= i < bytes.len() / 4 ==> lower <= #[trigger] words::le_word(bytes, 4*i) <= upper
}

/// Every retained word receives precisely the requested delta.
pub(crate) open spec fn shifted(before: Seq<u8>, after: Seq<u8>, delta: i64, count: int) -> bool {
    before.len() == after.len() && forall|i: int| 0 <= i < count ==>
        #[trigger] words::le_word(after, 4*i) == words::le_word(before, 4*i) + delta
}

/// Checks each old end before publishing it through contained scalar loans.
#[inline(always)]
#[verifier::spinoff_prover]
pub(crate) fn patch_words<const GROW: bool>(bytes: &mut [u8], lower: u32, upper: u32, delta: i64, base: u32)
    -> (result: Result<()>)
    requires old(bytes).len() % 4 == 0, base + old(bytes).len() <= u32::MAX,
        lower <= upper, GROW ==> lower == 0, 0 <= lower as int + delta, upper as int + delta <= u32::MAX,
    ensures final(bytes).len() == old(bytes).len(),
        ends_within(old(bytes)@, lower, upper) ==> result.is_ok(),
        result.is_ok() ==> ends_within(old(bytes)@, lower, upper),
        result.is_ok() ==> shifted(old(bytes)@, final(bytes)@, delta, old(bytes).len() as int / 4),
    no_unwind
{
    proof { vstd::slice::mut_ref_slice_len_eq(&bytes); }
    patch_words_pass::<GROW>(bytes, lower, upper, delta, base)
}

/// Checks each old end before publishing it through contained scalar loans.
#[inline(always)]
#[verifier::spinoff_prover]
fn patch_words_pass<const GROW: bool>(bytes: &mut [u8], lower: u32, upper: u32, delta: i64, base: u32)
    -> (result: Result<()>)
    requires old(bytes).len() % 4 == 0, base + old(bytes).len() <= u32::MAX,
        lower <= upper, GROW ==> lower == 0, 0 <= lower as int + delta, upper as int + delta <= u32::MAX,
    ensures ends_within(old(bytes)@, lower, upper) ==> result.is_ok(),
        result.is_ok() ==> ends_within(old(bytes)@, lower, upper),
        result.is_ok() ==> shifted(old(bytes)@, final(bytes)@, delta, old(bytes).len() as int / 4),
    no_unwind
{
    proof { vstd::slice::mut_ref_slice_len_eq(&bytes); }
    let ghost before = bytes@;
    let ghost future = final(bytes)@;
    let length = bytes.len();
    let ghost mut prefix = Seq::<u8>::empty();
    let ghost mut done = 0int;
    {
        let mut rest = &mut *bytes;
        while !rest.is_empty()
            invariant before == old(bytes)@, future.len() == before.len(),
                length == before.len(), length % 4 == 0, base + length <= u32::MAX,
                lower <= upper, GROW ==> lower == 0,
                0 <= lower as int + delta, upper as int + delta <= u32::MAX,
                0 <= done <= length / 4, prefix.len() == 4*done,
                rest.len() == length - 4*done, rest.len() % 4 == 0,
                rest@ == before.subrange(4*done, length as int),
                future == prefix + final(rest)@,
                forall|j: int| 0 <= j < done ==> lower <= #[trigger] words::le_word(before, 4*j) <= upper,
                forall|j: int| 0 <= j < done ==> #[trigger] words::le_word(prefix, 4*j)
                    == words::le_word(before, 4*j) + delta,
            decreases rest.len(),
        {
            let remaining = rest.len();
            let ghost at = length - remaining;
            let (word, tail) = rest.split_at_mut(4);
            proof {
                vstd::slice::mut_ref_slice_len_eq(&word);
                assert(word@ =~= before.subrange(4*done, 4*done+4));
                words::word_subrange(before, 4*done, 4*done+4, 0);
                assert(at == 4*done);
            }
            let end = words::read_contained_word(word, 0);
            assert(end as int == words::le_word(before, 4*done));
            if end > upper {
                return Err(Error::new(ErrorKind::InvalidBoundary, base + (length - remaining) as u32));
            }
            if !GROW && end < lower {
                return Err(Error::new(ErrorKind::ArithmeticOverflow, base + (length - remaining) as u32));
            }
            let value = if GROW {
                proof {
                    assert(0 <= delta <= u32::MAX);
                    assert(0 <= end as int + delta <= u32::MAX);
                }
                end.wrapping_add(delta as u32)
            } else {
                crate::arithmetic::apply_checked_delta(end, delta)
            };
            assert(value as int == end as int + delta);
            write_projected(word, 0, value);
            proof {
                let next = prefix + word@;
                assert forall|j: int| 0 <= j < done+1 implies
                    #[trigger] words::le_word(next, 4*j) == words::le_word(before, 4*j) + delta by {
                    if j < done {
                        assert(4*j+4 <= prefix.len());
                        words::word_subrange(next, 0, prefix.len() as int, 4*j);
                        assert(next.subrange(0, prefix.len() as int) =~= prefix);
                        assert(words::le_word(prefix, 4*j) == words::le_word(before, 4*j) + delta);
                    } else {
                        assert(j == done);
                        words::word_subrange(next, prefix.len() as int, next.len() as int, 0);
                        assert(next.subrange(prefix.len() as int, next.len() as int) =~= word@);
                    }
                }
                prefix = next;
                done = done+1;
                assert(tail@ =~= before.subrange(4*done, length as int));

            }
            rest = tail;
        }
        proof {
            assert(done == length / 4);
            assert(future =~= prefix);
        }
    }
    Ok(())
}

/// Stores scalar projections through one exact four-byte word loan.
#[inline(always)]
fn write_projected(bytes: &mut [u8], at: usize, value: u32)
    requires at as int + 4 <= old(bytes).len(),
    ensures words::encode_exact(final(bytes)@, at as int, value),
        words::le_word(final(bytes)@, at as int) == value,
        words::encode_frame(old(bytes)@, final(bytes)@, at as int),
    no_unwind
{
    proof {
        assert(value as u8 == (value % 256) as u8) by(bit_vector);
        assert((value >> 8) as u8 == (value / 256 % 256) as u8) by(bit_vector);
        assert((value >> 16) as u8 == (value / 65536 % 256) as u8) by(bit_vector);
        assert((value >> 24) as u8 == (value / 16777216) as u8) by(bit_vector);
    }
    let (_, suffix) = bytes.split_at_mut(at);
    let (word, _) = suffix.split_at_mut(4);
    word[0] = value as u8;
    word[1] = (value >> 8) as u8;
    word[2] = (value >> 16) as u8;
    word[3] = (value >> 24) as u8;
}

}
