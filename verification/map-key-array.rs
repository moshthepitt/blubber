//! Width-independent raw-array key model; executable instantiations use Kani.

#![forbid(unsafe_code)]

use vstd::prelude::*;

verus! {

/// Raw arrays encode as their input byte sequence for every positive wire width.
pub open spec fn raw_encoding(value: Seq<u8>) -> Seq<u8> { value }

/// Lexicographic order for equal-width raw arrays, independent of a machine N.
pub open spec fn raw_less(left: Seq<u8>, right: Seq<u8>) -> bool {
    exists|i: int| 0 <= i < left.len() && i < right.len()
        && left[i] < right[i]
        && forall|j: int| 0 <= j < i ==> left[j] == right[j]
}

/// Identity encoding preserves order, equality, width and decode at arbitrary N.
pub proof fn raw_key_identity(left: Seq<u8>, right: Seq<u8>)
    requires 0 < left.len() <= u32::MAX, left.len() == right.len(),
    ensures
        raw_encoding(left).len() == left.len(),
        raw_encoding(left) == raw_encoding(right) <==> left == right,
        raw_less(raw_encoding(left), raw_encoding(right)) <==> raw_less(left, right),
        raw_encoding(raw_encoding(left)) == left,
{
}

}
