//! Primitive byte views and exact encoders. SDK projection is supplied by callers.

use crate::encoding::{private, Encoder};
#[cfg(verus_only)]
use crate::{Layout, Leaf, MAX_DEPTH};
use vstd::prelude::*;

verus! {

/// Borrowed fixed bytes with an erased leaf descriptor.
///
/// Integer and SDK adapters project their canonical bytes once. Full-domain
/// Kani harnesses check those fixed-size projections and Rust ordering. The
/// generic encoder and validator correspondence below do not call an SDK.
pub struct Array<'a, const N: usize> {
    source: &'a [u8; N],
    #[cfg(verus_only)]
    ghost _leaf: Leaf,
}

impl<'a, const N: usize> private::Sealed for Array<'a, N> {}

impl<'a, const N: usize> Array<'a, N> {
    /// Erased descriptor chosen by the primitive projection adapter.
    pub closed spec fn kind(&self) -> Leaf { self._leaf }
    /// Complete projected value before the exact copy.
    pub closed spec fn projected(&self) -> Seq<u8> { self.source@ }
}

impl<'a, const N: usize> Encoder for Array<'a, N> {
    closed spec fn trusted_input(&self) -> bool {
        0 < N <= u32::MAX
            && crate::layout::descriptor_valid(Layout::Leaf(self._leaf), MAX_DEPTH as nat)
            && crate::layout::width_spec(Layout::Leaf(self._leaf)) == Some(N as u32)
            && (self._leaf == Leaf::Bool ==> self.source@[0] <= 1)
    }
    closed spec fn bytes(&self) -> Seq<u8> { self.source@ }

    #[inline(always)]
    fn measure(&self) -> (n: usize)
        ensures n == self.bytes().len(),
    { N }

    #[inline(always)]
    fn encode(&self, destination: &mut [u8])
        ensures final(destination)@ == self.bytes(),
    {
        encode_array(self.source, destination);
        proof { encoder_leaf_valid(self, destination@, MAX_DEPTH as nat); }
    }
}

/// Borrows a positive raw width admitted by the public array codec.
///
/// The width must be positive and at most `u32::MAX`.
#[inline(always)]
pub fn raw<const N: usize>(source: &[u8; N]) -> (value: Array<'_, N>)
    requires 0 < N <= u32::MAX,
    ensures value.trusted_input(), value.bytes() == source@, value.kind() == Leaf::Raw(N as u32),
    no_unwind
{ proof { reveal(crate::layout_head::head_spec); } Array { source, #[cfg(verus_only)] _leaf: Leaf::Raw(N as u32) } }

/// Borrows an unsigned integer projection at an existing wire width.
///
/// The width must be 1, 2, 4, 8 or 16 bytes.
#[inline(always)]
pub fn unsigned<const N: usize>(source: &[u8; N]) -> (value: Array<'_, N>)
    requires N == 1 || N == 2 || N == 4 || N == 8 || N == 16,
    ensures value.trusted_input(), value.bytes() == source@, value.kind() == Leaf::Unsigned(N as u8),
    no_unwind
{ proof { reveal(crate::layout_head::head_spec); } Array { source, #[cfg(verus_only)] _leaf: Leaf::Unsigned(N as u8) } }

/// Borrows a signed integer projection at an existing wire width.
///
/// The width must be 1, 2, 4, 8 or 16 bytes.
#[inline(always)]
pub fn signed<const N: usize>(source: &[u8; N]) -> (value: Array<'_, N>)
    requires N == 1 || N == 2 || N == 4 || N == 8 || N == 16,
    ensures value.trusted_input(), value.bytes() == source@, value.kind() == Leaf::Signed(N as u8),
    no_unwind
{ proof { reveal(crate::layout_head::head_spec); } Array { source, #[cfg(verus_only)] _leaf: Leaf::Signed(N as u8) } }

/// Borrows the one-byte projection produced by `bool_bytes`.
#[inline(always)]
pub fn boolean(source: &[u8; 1]) -> (value: Array<'_, 1>)
    requires source@[0] <= 1,
    ensures value.trusted_input(), value.bytes() == source@, value.kind() == Leaf::Bool,
    no_unwind
{ proof { reveal(crate::layout_head::head_spec); } Array { source, #[cfg(verus_only)] _leaf: Leaf::Bool } }

/// Produces the canonical value/key byte for either Rust boolean.
#[inline(always)]
pub fn bool_bytes(source: bool) -> (result: [u8; 1])
    ensures result@[0] == (if source { 1u8 } else { 0u8 }), result@[0] <= 1,
    no_unwind
{ [if source { 1 } else { 0 }] }

/// Copies a fixed array to its exact preflighted destination, at every width.
///
/// # Panics
/// The destination length must equal `N`.
#[inline(always)]
pub fn encode_array<const N: usize>(source: &[u8; N], destination: &mut [u8])
    requires old(destination).len() == N,
    ensures final(destination)@ == source@,
    no_unwind
{
    assert!(destination.len() == N);
    let mut i = 0;
    while i < N
        invariant i <= N, destination.len() == N,
            forall|j: int| 0 <= j < i ==> destination@[j] == source@[j],
        decreases N - i,
    {
        destination[i] = source[i];
        i += 1;
    }
    assert(destination@ =~= source@);
}

/// Returns a raw value/key projection without changing any byte.
#[inline(always)]
pub fn array_bytes<const N: usize>(source: [u8; N]) -> (result: [u8; N])
    ensures result@ == source@,
    no_unwind
{ source }

/// Exact primitive writes satisfy the independent leaf schema at every depth.
pub proof fn encoder_leaf_valid<const N: usize>(source: &Array<'_, N>, after: Seq<u8>, budget: nat)
    requires source.trusted_input(), after == source.bytes(), 0 < budget <= MAX_DEPTH,
    ensures crate::validation::schema_valid(after, Layout::Leaf(source.kind()), budget, true),
{
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::validation::children_valid, 2);
}

/// Canonical projected keys are complete, injective and in their leaf domain.
///
/// Numeric big-endian/sign projections and SDK/Rust-order bridges have
/// fixed-domain Kani support. Here the projected byte view is the input value.
pub proof fn encoder_key_canonical<const N: usize>(
    left: &Array<'_, N>, right: &Array<'_, N>, a: Seq<u8>, b: Seq<u8>,
)
    requires left.trusted_input(), right.trusted_input(),
        left.kind() == right.kind(), a == left.bytes(), b == right.bytes(),
    ensures a.len() == N, b.len() == N,
        (a == b) == (left.projected() == right.projected()),
        left.kind() == Leaf::Bool ==> a[0] <= 1 && b[0] <= 1,
{
}

/// A first differing projected byte determines canonical order at any width.
pub proof fn encoder_key_order(a: Seq<u8>, b: Seq<u8>, at: int, result: core::cmp::Ordering)
    requires a.len() == b.len(), 0 <= at <= a.len(),
        forall|i: int| 0 <= i < at ==> a[i] == b[i],
        if at == a.len() { result == core::cmp::Ordering::Equal }
        else { (result == core::cmp::Ordering::Less && a[at] < b[at])
            || (result == core::cmp::Ordering::Greater && a[at] > b[at]) },
    ensures crate::search::lex_total(result, a, b),
{
    if at == a.len() {
        assert(a =~= b);
        crate::search::lex_irreflexive(a, b);
    } else if a[at] < b[at] {
        assert(crate::search::same_prefix(a, b, at));
        assert(crate::search::lex_witness(a, b, at));
        assert(crate::search::lex_less(a, b));
        crate::search::lex_antisymmetric(a, b);
    } else {
        assert(crate::search::same_prefix(b, a, at));
        assert(crate::search::lex_witness(b, a, at));
        assert(crate::search::lex_less(b, a));
        crate::search::lex_antisymmetric(a, b);
    }
}

/// Compares canonical fixed bytes with no callbacks or allocation.
#[inline(always)]
pub fn compare_array<const N: usize>(left: &[u8; N], right: &[u8; N]) -> (result: core::cmp::Ordering)
    ensures crate::search::lex_total(result, left@, right@),
    no_unwind
{
    let mut i = 0;
    while i < N
        invariant i <= N, forall|j: int| 0 <= j < i ==> left@[j] == right@[j],
        decreases N - i,
    {
        if left[i] < right[i] {
            proof { encoder_key_order(left@, right@, i as int, core::cmp::Ordering::Less); }
            return core::cmp::Ordering::Less;
        }
        if left[i] > right[i] {
            proof { encoder_key_order(left@, right@, i as int, core::cmp::Ordering::Greater); }
            return core::cmp::Ordering::Greater;
        }
        i += 1;
    }
    proof { encoder_key_order(left@, right@, i as int, core::cmp::Ordering::Equal); }
    core::cmp::Ordering::Equal
}

/// Repeated measurement of the same immutable primitive view has one extent.
pub proof fn encoder_primitive_measure_pure(bytes: Seq<u8>, first: usize, second: usize)
    requires first == bytes.len(), second == bytes.len(),
    ensures first == second,
{
}

/// Runs repeated primitive measurement under the verified borrowed contract.
#[inline(always)]
pub fn measure_twice<E: Encoder>(source: &E) -> (result: usize)
    requires source.trusted_input(),
    ensures result == source.bytes().len(),
    no_unwind
{
    let _first = source.measure();
    let second = source.measure();
    proof { encoder_primitive_measure_pure(source.bytes(), _first, second); }
    second
}

/// Concrete leaf-validity witness through the actual array encoder.
#[cfg(verus_only)]
pub(crate) fn leaf_witness()
    ensures true,
{
    let source = [255u8, 3]; let value = unsigned(&source);
    let mut output = [19u8; 2];
    value.encode(&mut output);
    proof { encoder_leaf_valid(&value, output@, 1); }
    assert(output@ == seq![255u8, 3]);
}

/// Concrete key-domain and injectivity witness with both Boolean values.
#[cfg(verus_only)]
pub(crate) fn canonical_witness()
    ensures true,
{
    let a = bool_bytes(false); let b = bool_bytes(true);
    let left = boolean(&a); let right = boolean(&b);
    proof { encoder_key_canonical(&left, &right, left.bytes(), right.bytes()); }
    assert(left.bytes() != right.bytes());
}

/// Concrete first-difference witness for the unbounded ordering theorem.
#[cfg(verus_only)]
pub(crate) fn order_witness()
    ensures true,
{
    let a = [3u8, 5]; let b = [3u8, 7];
    let order = compare_array(&a, &b);
    proof { encoder_key_order(a@, b@, 1, core::cmp::Ordering::Less); }
    assert(order == core::cmp::Ordering::Less);
}

/// Concrete pure measurement witness, including an empty string.
#[cfg(verus_only)]
pub(crate) fn measure_witness()
    ensures true,
{
    proof { reveal_strlit(""); }
    let source = crate::encoding_text::text(""); let _first = source.measure(); let second = measure_twice(&source);
    proof { encoder_primitive_measure_pure(source.bytes(), _first, second); }
    assert(second == 0);
}

}
