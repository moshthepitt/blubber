//! Borrowed encoder contracts and generic exact-loan composition.
//!
//! Theorems start at well-typed projected input satisfying `trusted_input`.
//! Library implementations verify their contracts. Derive projection glue and
//! handwritten true markers are trusted implementer premises; their concrete
//! callbacks are not verified by selecting a route. Primitive leaf/schema
//! bridges live in `encoding_leaf` and `encoding_text`.
//! This module removes no runtime checks.

use vstd::prelude::*;

pub(crate) mod private {
    vstd::prelude::verus! {
        /// Keeps executable encoder implementations in the library.
        pub trait Sealed {}
    }
}

verus! {

/// Library-only source operations, with exact output and immutable source views.
pub trait Encoder: private::Sealed {
    /// Whether trusted projection and true-marker input contracts hold.
    ///
    /// A handwritten marker is an implementer premise, not a verified body.
    spec fn trusted_input(&self) -> bool;

    /// Abstract output determined by the borrowed source.
    spec fn bytes(&self) -> Seq<u8>;

    /// Reports the exact output extent for a source satisfying its input contract.
    fn measure(&self) -> (n: usize)
        requires self.trusted_input(),
        ensures n == self.bytes().len(),
        no_unwind;

    /// Writes an exact destination using only shared source loans.
    ///
    /// # Panics
    /// The source must satisfy its input contract and the destination length
    /// must equal the measured extent. Violating those preconditions may panic.
    fn encode(&self, destination: &mut [u8])
        requires self.trusted_input(), old(destination).len() == self.bytes().len(),
        ensures final(destination)@ == self.bytes(),
        no_unwind;
}

/// Immutable borrowed bytes accepted by the generic encoder drivers.
pub struct Bytes<'a> {
    source: &'a [u8],
}

impl<'a> private::Sealed for Bytes<'a> {}

impl<'a> Encoder for Bytes<'a> {
    open spec fn trusted_input(&self) -> bool { true }
    closed spec fn bytes(&self) -> Seq<u8> { self.source@ }

    #[inline(always)]
    fn measure(&self) -> (n: usize)
        ensures n == self.bytes().len(),
    { self.source.len() }

    #[inline(always)]
    fn encode(&self, destination: &mut [u8])
        ensures final(destination)@ == self.bytes(),
    {
        assert!(destination.len() == self.source.len());
        let mut i = 0;
        while i < self.source.len()
            invariant i <= self.source.len(), destination.len() == self.source.len(),
                forall|j: int| 0 <= j < i ==> destination@[j] == self.source@[j],
            decreases self.source.len() - i,
        { destination[i] = self.source[i]; i += 1; }
        assert(destination@ =~= self.source@);
    }
}

/// Borrows source data without allocating a shadow representation.
pub fn bytes(source: &[u8]) -> (value: Bytes<'_>)
    ensures value.bytes() == source@, value.trusted_input(),
    no_unwind
{ Bytes { source } }

/// Composes arbitrary implementing children, with no concrete child enumeration.
///
/// # Panics
/// The destination must have the sum of the children's measured lengths.
/// Both borrowed sources must satisfy the documented trusted input contract.
pub fn encode_pair<L: Encoder, R: Encoder>(
    left: &L, right: &R, destination: &mut [u8],
)
    requires left.trusted_input(), right.trusted_input(),
        old(destination).len() == left.bytes().len() + right.bytes().len(),
    ensures final(destination)@ == left.bytes() + right.bytes(),
    no_unwind
{
    let width = left.measure();
    let (first, second) = destination.split_at_mut(width);
    left.encode(first);
    right.encode(second);
    proof {
        assert(final(destination)@.subrange(0, left.bytes().len() as int) =~= left.bytes());
        assert(final(destination)@.subrange(left.bytes().len() as int,
            final(destination).len() as int) =~= right.bytes());
        encoder_trait_composition(left, right, final(destination)@);
    }
}

/// Writes an exact loan and preserves every surrounding destination byte.
///
/// # Panics
/// The start and measured source length must fit the destination. The borrowed
/// source must satisfy the documented trusted input contract.
pub fn encode_frame<E: Encoder>(source: &E, destination: &mut [u8], start: usize)
    requires
        source.trusted_input(),
        start <= old(destination).len(),
        source.bytes().len() <= old(destination).len() - start,
    ensures
        final(destination).len() == old(destination).len(),
        final(destination)@.subrange(start as int, start as int + source.bytes().len()) == source.bytes(),
        forall|i: int| 0 <= i < old(destination).len()
            && !(start <= i < start + source.bytes().len())
            ==> final(destination)@[i] == old(destination)@[i],
    no_unwind
{
    let ghost before = destination@;
    let width = source.measure();
    let (_, suffix) = destination.split_at_mut(start);
    let (slot, _) = suffix.split_at_mut(width);
    source.encode(slot);
    proof {
        assert(final(destination)@ =~= before.subrange(0, start as int) + source.bytes()
            + before.subrange(start as int + source.bytes().len(), before.len() as int));
        encoder_source_frame(source, before, final(destination)@, start as int);
    }
}

/// Joins arbitrary child encoder contracts into the complete output.
///
/// The input premises are the trusted projected views and the exact child
/// results established by `Encoder::encode`. No particular child type is chosen.
pub proof fn encoder_trait_composition<L: Encoder, R: Encoder>(
    left: &L, right: &R, after: Seq<u8>,
)
    requires
        left.trusted_input(), right.trusted_input(),
        after.len() == left.bytes().len() + right.bytes().len(),
        after.subrange(0, left.bytes().len() as int) == left.bytes(),
        after.subrange(left.bytes().len() as int, after.len() as int) == right.bytes(),
    ensures after == left.bytes() + right.bytes(),
{
    assert(after =~= left.bytes() + right.bytes());
}

/// Reconstructs the full destination from the exact loan and unchanged regions.
///
/// `encode_frame` obtains this decomposition from safe mutable slice loans.
/// The trusted input premise covers projection glue and true-marker bridges;
/// this theorem makes no claim about an opaque codec or an arbitrary callback.
pub proof fn encoder_source_frame<E: Encoder>(
    source: &E, before: Seq<u8>, after: Seq<u8>, start: int,
)
    requires
        source.trusted_input(),
        0 <= start <= before.len(),
        source.bytes().len() <= before.len() - start,
        after == before.subrange(0, start) + source.bytes()
            + before.subrange(start + source.bytes().len(), before.len() as int),
    ensures
        after.len() == before.len(),
        after.subrange(start, start + source.bytes().len()) == source.bytes(),
        forall|i: int| 0 <= i < before.len()
            && !(start <= i < start + source.bytes().len())
            ==> #[trigger] after[i] == before[i],
{
    assert forall|i: int| 0 <= i < before.len()
        && !(start <= i < start + source.bytes().len())
        implies #[trigger] after[i] == before[i] by { }
    assert(after.subrange(start, start + source.bytes().len()) =~= source.bytes());
}

/// Concrete no-premise check of generic composition, including an empty child.
#[cfg(verus_only)]
pub(crate) fn composition_witness()
    ensures true,
{
    let left = bytes(&[]);
    let right = bytes(&[3, 5]);
    let mut destination = [19; 2];
    encode_pair(&left, &right, &mut destination);
    proof {
        assert(destination@.subrange(0, left.bytes().len() as int) =~= left.bytes());
        assert(destination@.subrange(left.bytes().len() as int,
            destination.len() as int) =~= right.bytes());
        encoder_trait_composition(&left, &right, destination@);
    }
    assert(destination@ == seq![3u8, 5u8]);
}

/// Concrete no-premise check of a nonempty source and two surrounding frames.
#[cfg(verus_only)]
pub(crate) fn frame_witness()
    ensures true,
{
    let source = bytes(&[3, 5]);
    let mut destination = [19; 4];
    encode_frame(&source, &mut destination, 1);
    proof {
        assert(destination@ =~= seq![19u8,19,19,19].subrange(0,1) + source.bytes()
            + seq![19u8,19,19,19].subrange(3,4));
        encoder_source_frame(&source, seq![19u8,19,19,19], destination@, 1);
    }
    assert(destination@ == seq![19u8, 3u8, 5u8, 19u8]);
}

}
