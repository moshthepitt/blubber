//! Checked machine sizes and wire-width conversions.

use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// Exact representability and error behavior for a mathematical size.
pub open spec fn sizes_exact(result: Result<usize>, value: int) -> bool {
    match result {
        Ok(size) => size == value && 0 <= value <= usize::MAX,
        Err(error) => !(0 <= value <= usize::MAX)
            && error == (Error::Fault {
                kind: ErrorKind::ArithmeticOverflow, offset: u32::MAX,
            }),
    }
}

/// Adds two machine sizes, rejecting overflow without truncation.
#[inline(always)]
pub fn checked_add(left: usize, right: usize) -> (result: Result<usize>)
    ensures sizes_exact(result, left as int + right),
{
    match left.checked_add(right) {
        Some(value) => Ok(value),
        None => Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    }
}

/// Multiplies two machine sizes, rejecting overflow without truncation.
#[inline(always)]
pub fn checked_mul(left: usize, right: usize) -> (result: Result<usize>)
    ensures sizes_exact(result, left as int * right),
{
    match left.checked_mul(right) {
        Some(value) => Ok(value),
        None => Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    }
}

/// Converts a machine size to a wire word, rejecting values above `u32::MAX`.
#[inline(always)]
pub fn checked_u32(value: usize) -> (result: Result<u32>)
    ensures match result {
        Ok(word) => word == value && value <= u32::MAX,
        Err(error) => value > u32::MAX && error == (Error::Fault {
            kind: ErrorKind::ArithmeticOverflow, offset: u32::MAX,
        }),
    },
{
    if value > u32::MAX as usize {
        Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET))
    } else {
        Ok(value as u32)
    }
}

/// Applies a signed byte delta to a wire offset.
///
/// Accepts every `i64`, including `i64::MIN`; underflow and results above
/// `u32::MAX` return `ArithmeticOverflow` without an offset.
#[inline(always)]
pub fn checked_delta(value: u32, delta: i64) -> (result: Result<u32>)
    ensures match result {
        Ok(word) => word == value as int + delta && 0 <= value as int + delta <= u32::MAX,
        Err(error) => !(0 <= value as int + delta <= u32::MAX)
            && error == (Error::Fault {
                kind: ErrorKind::ArithmeticOverflow, offset: u32::MAX,
            }),
    },
    no_unwind
{
    // Compare against the representable interval before forming the sum.
    // Both endpoints fit i64, including for an arbitrary signed input delta.
    if delta < -(value as i64) || delta > (u32::MAX - value) as i64 {
        Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET))
    } else {
        Ok((value as i64 + delta) as u32)
    }
}

/// Applies a delta already proved to stay within the wire domain.
#[inline(always)]
pub(crate) fn apply_checked_delta(value: u32, delta: i64) -> (result: u32)
    requires 0 <= value as int + delta <= u32::MAX,
    ensures result == value as int + delta,
    no_unwind
{
    (value as i64).wrapping_add(delta) as u32
}

}
