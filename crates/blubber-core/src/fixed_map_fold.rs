//! Fixed-map fold correspondence over exact immutable chunks.
//!
//! Rust trait bodies and callbacks remain outside Verus. Each visited codec
//! contributes its actual result, effects and unwind outcome to `Visit`.
//! The shared `FixedMapState::next_with` is the typed glue for `visit`: key
//! decoding precedes value decoding, then the state advances before the callback.

use crate::{Error, ErrorKind};
use vstd::prelude::*;

verus! {

/// One opaque codec/callback observation in a particular execution history.
pub ghost struct Visit {
    /// Copied key or relative key error.
    pub key: Result<int, Error>,
    /// Decoded value or already absolute value error.
    pub value: Result<int, Error>,
    /// Key decoder unwinds before returning its result.
    pub key_unwind: bool,
    /// Value decoder unwinds before returning its result.
    pub value_unwind: bool,
    /// Callback unwinds after receiving this entry.
    pub callback_unwind: bool,
    /// Actual callback result in this history, without a purity premise.
    pub callback_result: int,
    /// Effects produced by this key decoder call.
    pub key_effects: Seq<int>,
    /// Effects produced by this value decoder call.
    pub value_effects: Seq<int>,
    /// Effects produced by this callback call.
    pub callback_effects: Seq<int>,
}

/// Exact codec inputs and callback arguments, in call order.
pub ghost struct Call {
    /// Zero for key decoding, one for value decoding, two for the callback.
    pub kind: int,
    /// Exact codec input; empty for a callback.
    pub bytes: Seq<u8>,
    /// Absolute codec base; zero for a callback.
    pub base: int,
    /// Copied callback key; zero for codec calls.
    pub key: int,
    /// Decoded callback value; zero for codec calls.
    pub value: int,
    /// Incoming callback accumulator; zero for codec calls.
    pub accumulator: int,
}

/// Observable prefix, including the retained state at an unwind or error.
pub ghost struct Trace {
    /// Accumulator after the last completed callback.
    pub accumulator: int,
    /// Codec and callback call order with exact inputs.
    pub calls: Seq<Call>,
    /// Effects from precisely those calls that occurred.
    pub effects: Seq<int>,
    /// First error delivered to the infallible panic boundary.
    pub error: Option<Error>,
    /// Opaque codec or callback unwind.
    pub unwound: bool,
    /// Bytes left after the last decoder state transition.
    pub remaining: int,
}

/// Production `Error::rebase` contract, including sentinel and program codes.
pub open spec fn rebased(error: Error, base: int) -> Error {
    match error {
        Error::Fault { kind, offset } => if offset == u32::MAX { error }
            else if offset + base < u32::MAX { Error::Fault { kind, offset: (offset + base) as u32 } }
            else { Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: u32::MAX } },
        Error::Program { .. } => error,
    }
}

/// Exact first-error selection: key errors rebase once; value errors pass through.
pub open spec fn selected_error(key: Result<int, Error>, value: Result<int, Error>, base: int)
    -> Option<Error>
{
    if key.is_err() { Some(rebased(key.err().unwrap(), base)) }
    else if value.is_err() { Some(value.err().unwrap()) } else { None }
}

/// Empty trace at a normal exhaustion point.
pub open spec fn empty(accumulator: int, remaining: int) -> Trace {
    Trace { accumulator, calls: Seq::empty(), effects: Seq::empty(), error: None,
        unwound: false, remaining }
}

/// Shared typed decoder's key/value/error/state/callback protocol.
pub open spec fn visit(entry: Seq<u8>, base: int, key_width: int, remaining: int,
    observed: Visit, accumulator: int) -> Trace
{
    let key_calls = seq![Call { kind: 0, bytes: entry.subrange(0, key_width), base, key: 0, value: 0, accumulator: 0 }];
    if observed.key_unwind {
        Trace { calls: key_calls, effects: observed.key_effects, unwound: true,
            ..empty(accumulator, remaining) }
    } else if observed.key.is_err() {
        Trace { calls: key_calls, effects: observed.key_effects,
            error: selected_error(observed.key, observed.value, base), ..empty(accumulator, 0) }
    } else {
        let value_calls = key_calls + seq![Call { kind: 1, bytes: entry.subrange(key_width, entry.len() as int), base: base + key_width, key: 0, value: 0, accumulator: 0 }];
        let value_effects = observed.key_effects + observed.value_effects;
        if observed.value_unwind {
            Trace { calls: value_calls, effects: value_effects, unwound: true,
                ..empty(accumulator, remaining) }
        } else if observed.value.is_err() {
            Trace { calls: value_calls, effects: value_effects,
                error: selected_error(observed.key, observed.value, base), ..empty(accumulator, 0) }
        } else {
            Trace { accumulator: if observed.callback_unwind { accumulator }
                    else { observed.callback_result },
                calls: value_calls + seq![Call { kind: 2, bytes: Seq::empty(), base: 0, key: observed.key.unwrap(), value: observed.value.unwrap(), accumulator }],
                effects: value_effects + observed.callback_effects,
                error: None, unwound: observed.callback_unwind, remaining: remaining - entry.len() }
        }
    }
}

/// Append a completed entry's observations to the continuation's trace.
pub open spec fn followed_by(first: Trace, rest: Trace) -> Trace {
    Trace { calls: first.calls + rest.calls, effects: first.effects + rest.effects, ..rest }
}

/// Ordinary repeated stepping selects from the shrinking borrowed remainder.
pub open spec fn repeated_next(remaining: Seq<u8>, end: int, key_width: int, value_width: int,
    observations: Seq<Visit>, index: int, accumulator: int) -> Trace
    decreases observations.len() - index,
{
    if index < 0 || index >= observations.len() || remaining.len() == 0 { empty(accumulator, remaining.len() as int) }
    else {
        let stride = key_width + value_width;
        let entry = remaining.subrange(0, stride);
        let first = visit(entry, end - remaining.len(), key_width, remaining.len() as int,
            observations[index], accumulator);
        if first.error.is_some() || first.unwound { first }
        else { followed_by(first, repeated_next(remaining.subrange(stride, remaining.len() as int), end,
            key_width, value_width, observations, index + 1, first.accumulator)) }
    }
}

/// Consuming fold enumerates original exact chunks and invokes the same decoder.
pub open spec fn consuming_fold(bytes: Seq<u8>, base: int, key_width: int, value_width: int,
    observations: Seq<Visit>, index: int, accumulator: int) -> Trace
    decreases observations.len() - index,
{
    if index < 0 || index >= observations.len() { empty(accumulator, 0) }
    else {
        let stride = key_width + value_width;
        let at = index * stride;
        let first = visit(bytes.subrange(at, at + stride), base + at, key_width,
            bytes.len() - at, observations[index], accumulator);
        if first.error.is_some() || first.unwound { first }
        else { followed_by(first, consuming_fold(bytes, base, key_width, value_width,
            observations, index + 1, first.accumulator)) }
    }
}

/// Exact stride, key/value partition, tail induction and absolute base recovery.
pub proof fn fixed_fold_stride(bytes: Seq<u8>, base: int, key_width: int, value_width: int,
    count: int, index: int)
    requires 0 < key_width, 0 <= value_width, 0 <= index < count,
        bytes.len() == count * (key_width + value_width),
        0 <= base, base + bytes.len() <= u32::MAX,
    ensures ({
        let stride = key_width + value_width;
        let at = index * stride;
        let remaining = bytes.subrange(at, bytes.len() as int);
        &&& 0 <= at < at + stride <= bytes.len()
        &&& remaining.subrange(0, stride) == bytes.subrange(at, at + stride)
        &&& remaining.subrange(stride, remaining.len() as int) == bytes.subrange(at + stride, bytes.len() as int)
        &&& bytes.subrange(at, at + key_width) + bytes.subrange(at + key_width, at + stride)
            == bytes.subrange(at, at + stride)
        &&& base + bytes.len() - remaining.len() == base + at
        &&& base + at + key_width <= base + at + stride <= u32::MAX
    }),
{
    let stride = key_width + value_width;
    let at = index * stride;
    assert(0 <= at < at + stride <= bytes.len()) by(nonlinear_arith)
        requires at == index * stride, stride == key_width + value_width, key_width > 0, value_width >= 0,
            0 <= index < count, bytes.len() == count * stride;
    let remaining = bytes.subrange(at, bytes.len() as int);
    assert(remaining.subrange(0, stride) =~= bytes.subrange(at, at + stride));
    assert(remaining.subrange(stride, remaining.len() as int) =~= bytes.subrange(at + stride, bytes.len() as int));
    assert(bytes.subrange(at, at + key_width) + bytes.subrange(at + key_width, at + stride)
        =~= bytes.subrange(at, at + stride));
}

/// Whole traces agree for every suffix, codec outcome, effect and callback.
/// This includes decode/callback unwinds and already exhausted iterators.
pub proof fn fixed_fold_equivalent(bytes: Seq<u8>, base: int, key_width: int, value_width: int,
    observations: Seq<Visit>, index: int, accumulator: int)
    requires 0 < key_width, 0 <= value_width, 0 <= index <= observations.len(),
        bytes.len() == observations.len() * (key_width + value_width),
        0 <= base, base + bytes.len() <= u32::MAX,
    ensures consuming_fold(bytes, base, key_width, value_width, observations, index, accumulator)
        == repeated_next(bytes.subrange(index * (key_width + value_width), bytes.len() as int),
            base + bytes.len(), key_width, value_width, observations, index, accumulator),
    decreases observations.len() - index,
{
    let stride = key_width + value_width;
    let at = index * stride;
    reveal_with_fuel(consuming_fold, 2);
    reveal_with_fuel(repeated_next, 2);
    assert(0 <= at <= bytes.len()) by(nonlinear_arith)
        requires at == index * stride, stride > 0, 0 <= index <= observations.len(),
            bytes.len() == observations.len() * stride;
    assert((index + 1) * stride == at + stride) by(nonlinear_arith)
        requires at == index * stride;
    if index < observations.len() {
        fixed_fold_stride(bytes, base, key_width, value_width, observations.len() as int, index);
        let entry = bytes.subrange(at, at + stride);
        let first = visit(entry, base + at, key_width, bytes.len() - at, observations[index], accumulator);
        if first.error.is_none() && !first.unwound {
            fixed_fold_equivalent(bytes, base, key_width, value_width, observations, index + 1, first.accumulator);
        }
    }
}

/// Relative key faults rebase exactly once; absolute value faults remain intact.
pub proof fn fixed_fold_error_rebase(relative: Error, absolute: Error, base: int)
    requires 0 <= base <= u32::MAX,
    ensures selected_error(Err(relative), Err(absolute), base) == Some(rebased(relative, base)),
        selected_error(Ok(0), Err(absolute), base) == Some(absolute),
        selected_error(Ok(0), Ok(0), base).is_none(),
        match relative {
            Error::Fault { kind, offset } => offset < u32::MAX && offset + base < u32::MAX ==>
                selected_error(Err(relative), Ok(0), base) == Some(Error::Fault { kind, offset: (offset + base) as u32 }),
            Error::Program { .. } => selected_error(Err(relative), Ok(0), base) == Some(relative),
        },
{
}

/// Nonempty, singleton and empty witnesses with actual effect sequences.
pub proof fn fold_witness()
{
    let good = Visit { key: Ok(7), value: Ok(9), key_unwind: false, value_unwind: false,
        callback_unwind: false, callback_result: 19, key_effects: seq![1int], value_effects: seq![2int], callback_effects: seq![3int] };
    fixed_fold_equivalent(seq![7u8, 9u8], 91, 1, 1, seq![good], 0, 3);
    fixed_fold_equivalent(seq![], 91, 1, 0, seq![], 0, 3);
    fixed_fold_equivalent(seq![7u8, 9u8], 91, 1, 1, seq![good], 1, 3);
    reveal_with_fuel(consuming_fold, 3);
    reveal(visit);
    assert(consuming_fold(seq![7u8, 9u8], 91, 1, 1, seq![good], 0, 3).effects == seq![1int,2int,3int]);
    let key_error = Visit { key: Err(Error::Fault { kind: ErrorKind::InvalidBool, offset: 0 }), ..good };
    let value_error = Visit { value: Err(Error::Fault { kind: ErrorKind::InvalidBool, offset: 94 }), ..good };
    fixed_fold_equivalent(seq![7u8,9u8,2u8,9u8], 91, 1, 1, seq![good,key_error], 0, 3);
    fixed_fold_equivalent(seq![7u8,9u8,7u8,2u8], 91, 1, 1, seq![good,value_error], 0, 3);
    fixed_fold_equivalent(seq![7u8,9u8], 91, 1, 1, seq![Visit { key_unwind: true, ..good }], 0, 3);
    fixed_fold_equivalent(seq![7u8,9u8], 91, 1, 1, seq![Visit { value_unwind: true, ..good }], 0, 3);
    fixed_fold_equivalent(seq![7u8,9u8], 91, 1, 1, seq![Visit { callback_unwind: true, ..good }], 0, 3);
}

/// Premise-free exact byte partition at a nonzero map entry index.
pub proof fn stride_witness()
{
    fixed_fold_stride(seq![7u8,9u8,8u8,10u8], 91, 1, 1, 2, 1);
    fixed_fold_stride(seq![7u8,8u8], 91, 1, 0, 2, 1);
}

/// Premise-free offset, sentinel, overflow and program-code witnesses.
pub proof fn rebase_witness()
{
    fixed_fold_error_rebase(Error::Fault { kind: ErrorKind::InvalidBool, offset: 0 },
        Error::Fault { kind: ErrorKind::InvalidBool, offset: 96 }, 95);
    fixed_fold_error_rebase(Error::Fault { kind: ErrorKind::InvalidTag, offset: u32::MAX },
        Error::Program { code: 17 }, 95);
    fixed_fold_error_rebase(Error::Fault { kind: ErrorKind::InvalidTag, offset: 1 },
        Error::Program { code: 17 }, u32::MAX as int);
    fixed_fold_error_rebase(Error::Program { code: 17 }, Error::Program { code: 18 }, 95);
}

}
