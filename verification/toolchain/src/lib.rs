//! Same-source move fixture for proof, erasure, Miri, and SBF toolchain checks.

#![no_std]
#![forbid(unsafe_code)]
use vstd::prelude::*;

verus! {

/// The prefix is unchanged and the suffix contains the original shifted bytes.
pub open spec fn shifted(before: Seq<u8>, after: Seq<u8>, at: int, delta: int) -> bool {
    &&& after.len() == before.len()
    &&& forall|j: int| 0 <= j < at + delta ==> after[j] == before[j]
    &&& forall|j: int| at + delta <= j < before.len() ==> after[j] == before[j - delta]
}

/// Opens a gap by shifting backward; the last `delta` bytes are spare capacity.
///
/// Callers must satisfy `at <= buf.len()` and `delta <= buf.len() - at`.
/// Verus checks these preconditions; ordinary Rust callers must check them.
pub fn shift_suffix(buf: &mut [u8], at: usize, delta: usize)
    requires
        at <= old(buf)@.len(),
        delta <= old(buf)@.len() - at,
    ensures
        shifted(old(buf)@, final(buf)@, at as int, delta as int),
{
    let ghost before = buf@;
    let n = buf.len();
    let mut i = n - delta;
    while at < i
        invariant
            at <= i <= n - delta,
            delta <= n - at,
            buf@.len() == n,
            before.len() == n,
            forall|j: int| 0 <= j < i + delta ==> buf@[j] == before[j],
            forall|j: int| i + delta <= j < n ==> buf@[j] == before[j - delta],
        decreases i,
    {
        i -= 1;
        let byte = buf[i];
        buf[i + delta] = byte;
    }
}

/// Opens the same gap with the standard overlap-safe slice move.
///
/// Callers must satisfy `at <= buf.len()` and `delta <= buf.len() - at`.
pub fn shift_suffix_memmove(buf: &mut [u8], at: usize, delta: usize)
    requires
        at <= old(buf)@.len(),
        delta <= old(buf)@.len() - at,
    ensures
        shifted(old(buf)@, final(buf)@, at as int, delta as int),
{
    let n = buf.len();
    buf.copy_within(at..n - delta, at + delta);
}

}

#[cfg(test)]
mod tests {
    #[test]
    fn suffix_moves() {
        let before = [0, 1, 2, 3, 4, 5, 6, 7];
        for len in 0..=before.len() {
            for at in 0..=len {
                for delta in 0..=len - at {
                    let mut expected = before;
                    expected[at + delta..len].copy_from_slice(&before[at..len - delta]);
                    let mut loop_bytes = before;
                    let mut copy_bytes = before;
                    super::shift_suffix(&mut loop_bytes[..len], at, delta);
                    super::shift_suffix_memmove(&mut copy_bytes[..len], at, delta);
                    assert_eq!(loop_bytes, expected);
                    assert_eq!(copy_bytes, expected);
                }
            }
        }
    }
}
