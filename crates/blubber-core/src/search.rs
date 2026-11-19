//! Byte-key ordering, exact key regions, and bounded map lookup.

#[cfg(verus_only)]
use crate::map_layout::{entry_stride, key_width, map_shape, partition_iff, value_span};
#[cfg(verus_only)]
use crate::words::le_word;
use crate::LayoutHead;
use crate::{
    checked_span, partition_map, Error, ErrorKind, KeyDomain, Layout, MapPartition, Result, Span,
};
#[cfg(verus_only)]
use crate::{layout::descriptor_valid, MAX_DEPTH};
use core::cmp::Ordering;
use vstd::prelude::*;

verus! {

/// A lookup's matching ordinal or insertion position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Search {
    /// Matching entry, or insertion position in `0..=count` on ordered maps.
    pub index: u32,
    /// Whether the selected key equals the supplied encoded key.
    pub found: bool,
}

/// Two byte sequences agree on a bounded prefix.
pub open spec fn same_prefix(a: Seq<u8>, b: Seq<u8>, n: int) -> bool {
    0 <= n <= a.len() && n <= b.len()
        && forall|i: int| 0 <= i < n ==> #[trigger] a[i] == b[i]
}

/// Strict lexicographic order, including the proper-prefix case.
pub open spec fn lex_less(a: Seq<u8>, b: Seq<u8>) -> bool {
    (a.len() < b.len() && same_prefix(a, b, a.len() as int))
        || exists|i: int| 0 <= i < a.len() && i < b.len()
            && same_prefix(a, b, i) && a[i] < b[i]
}

/// The comparator returns exactly the total byte-sequence order.
pub open spec fn lex_total(result: Ordering, a: Seq<u8>, b: Seq<u8>) -> bool {
    (result == Ordering::Less) == lex_less(a, b)
        && (result == Ordering::Greater) == lex_less(b, a)
        && (result == Ordering::Equal) == (a == b)
}

/// Equality is reported iff the complete byte sequences are identical.
pub open spec fn lex_equal_iff(result: Ordering, a: Seq<u8>, b: Seq<u8>) -> bool {
    (result == Ordering::Equal) == (a == b)
}

/// A less-than witness is the first difference or the shorter sequence's end.
pub open spec fn lex_witness(a: Seq<u8>, b: Seq<u8>, i: int) -> bool {
    same_prefix(a, b, i) && i < b.len()
        && (i == a.len() || (i < a.len() && a[i] < b[i]))
}

proof fn less_witness(a: Seq<u8>, b: Seq<u8>) -> (i: int)
    requires lex_less(a, b),
    ensures lex_witness(a, b, i),
{
    if a.len() < b.len() && same_prefix(a, b, a.len() as int) {
        a.len() as int
    } else {
        choose|i: int| 0 <= i < a.len() && i < b.len()
            && same_prefix(a, b, i) && a[i] < b[i]
    }
}

/// Reversing the operands reverses their strict order.
pub proof fn lex_antisymmetric(a: Seq<u8>, b: Seq<u8>)
    ensures !(lex_less(a, b) && lex_less(b, a)),
{
    if lex_less(a, b) && lex_less(b, a) {
        let i = less_witness(a, b);
        let j = less_witness(b, a);
        if i < j { assert(b[i] == a[i]); }
        else if j < i { assert(a[j] == b[j]); }
    }
}

/// Byte-sequence lexicographic order is transitive for arbitrary lengths.
pub proof fn lex_transitive(a: Seq<u8>, b: Seq<u8>, c: Seq<u8>)
    requires lex_less(a, b), lex_less(b, c),
    ensures lex_less(a, c),
{
    let i = less_witness(a, b);
    let j = less_witness(b, c);
    let k = if i <= j { i } else { j };
    assert(same_prefix(a, c, k)) by {
        assert forall|p: int| 0 <= p < k implies #[trigger] a[p] == c[p] by {
            assert(a[p] == b[p]);
            assert(b[p] == c[p]);
        }
    }
    if i < j { assert(b[i] == c[i]); }
    else if j < i { assert(a[j] == b[j]); }
    assert(lex_witness(a, c, k));
    if k < a.len() {
        assert(exists|p: int| 0 <= p < a.len() && p < c.len()
            && same_prefix(a, c, p) && a[p] < c[p]);
    }
}

/// Equal byte sequences admit neither strict direction.
pub proof fn lex_irreflexive(a: Seq<u8>, b: Seq<u8>)
    requires a == b,
    ensures !lex_less(a, b), !lex_less(b, a),
{
    if lex_less(a, b) {
        let i = less_witness(a, b);
    }
}

// The executable scan supplies the first differing position or a sequence end.
proof fn compare_at(a: Seq<u8>, b: Seq<u8>, i: int, result: Ordering)
    requires
        same_prefix(a, b, i),
        i == a.len() || i == b.len() || a[i] != b[i],
        result == if i == a.len() {
            if i == b.len() { Ordering::Equal } else { Ordering::Less }
        } else if i == b.len() { Ordering::Greater }
        else if a[i] < b[i] { Ordering::Less } else { Ordering::Greater },
    ensures lex_total(result, a, b),
{
    if result == Ordering::Equal {
        assert(a =~= b);
        lex_irreflexive(a, b);
    } else {
        if result == Ordering::Less {
            assert(lex_witness(a, b, i));
            if i < a.len() {
                assert(exists|p: int| 0 <= p < a.len() && p < b.len()
                    && same_prefix(a, b, p) && a[p] < b[p]);
            }
        } else {
            assert(same_prefix(b, a, i));
            assert(lex_witness(b, a, i));
            if i < b.len() {
                assert(exists|p: int| 0 <= p < b.len() && p < a.len()
                    && same_prefix(b, a, p) && b[p] < a[p]);
            }
        }
        lex_antisymmetric(a, b);
        if a == b { lex_irreflexive(a, b); }
    }
}

// Compare a complete eight-byte block using safe standard integer decoding.
// Byte order and its lexicographic meaning remain production proof obligations.
#[inline(always)]
fn decoded_block_order(raw_a:u64,raw_b:u64,Ghost(left):Ghost<Seq<u8>>,Ghost(right):Ghost<Seq<u8>>)
    ->(result:Ordering)
    requires left.len()==8,right.len()==8,
        raw_a==vstd::bytes::spec_u64_from_le_bytes(left),raw_b==vstd::bytes::spec_u64_from_le_bytes(right),
    ensures lex_total(result,left,right),
    no_unwind
{
    let a=((raw_a & 0x00000000000000ffu64)<<56) | ((raw_a & 0x000000000000ff00u64)<<40) | ((raw_a & 0x0000000000ff0000u64)<<24) | ((raw_a & 0x00000000ff000000u64)<<8) | ((raw_a & 0x000000ff00000000u64)>>8) | ((raw_a & 0x0000ff0000000000u64)>>24) | ((raw_a & 0x00ff000000000000u64)>>40) | ((raw_a & 0xff00000000000000u64)>>56);
    let b=((raw_b & 0x00000000000000ffu64)<<56) | ((raw_b & 0x000000000000ff00u64)<<40) | ((raw_b & 0x0000000000ff0000u64)<<24) | ((raw_b & 0x00000000ff000000u64)<<8) | ((raw_b & 0x000000ff00000000u64)>>8) | ((raw_b & 0x0000ff0000000000u64)>>24) | ((raw_b & 0x00ff000000000000u64)>>40) | ((raw_b & 0xff00000000000000u64)>>56);
    let result=if a<b {Ordering::Less}else if a>b {Ordering::Greater}else{Ordering::Equal};
    proof {
        vstd::bytes::lemma_auto_spec_u64_to_from_le_bytes();
        vstd::bytes::spec_u64_to_le_bytes_to_open(raw_a);
        vstd::bytes::spec_u64_to_le_bytes_to_open(raw_b);
        assert(vstd::bytes::spec_u64_to_le_bytes_open(raw_a)==left);
        assert(vstd::bytes::spec_u64_to_le_bytes_open(raw_b)==right);
        let a0=left[0];
        let a1=left[1];
        let a2=left[2];
        let a3=left[3];
        let a4=left[4];
        let a5=left[5];
        let a6=left[6];
        let a7=left[7];
        assert(raw_a==(((a0 as u64)<<0) | ((a1 as u64)<<8) | ((a2 as u64)<<16) | ((a3 as u64)<<24) | ((a4 as u64)<<32) | ((a5 as u64)<<40) | ((a6 as u64)<<48) | ((a7 as u64)<<56))) by(bit_vector)
            requires ((raw_a & 0xff) as u8)==a0, (((raw_a>>8) & 0xff) as u8)==a1, (((raw_a>>16) & 0xff) as u8)==a2, (((raw_a>>24) & 0xff) as u8)==a3, (((raw_a>>32) & 0xff) as u8)==a4, (((raw_a>>40) & 0xff) as u8)==a5, (((raw_a>>48) & 0xff) as u8)==a6, (((raw_a>>56) & 0xff) as u8)==a7;
        assert(a==(((a0 as u64)<<56) | ((a1 as u64)<<48) | ((a2 as u64)<<40) | ((a3 as u64)<<32) | ((a4 as u64)<<24) | ((a5 as u64)<<16) | ((a6 as u64)<<8) | ((a7 as u64)<<0))) by(bit_vector)
            requires raw_a==(((a0 as u64)<<0) | ((a1 as u64)<<8) | ((a2 as u64)<<16) | ((a3 as u64)<<24) | ((a4 as u64)<<32) | ((a5 as u64)<<40) | ((a6 as u64)<<48) | ((a7 as u64)<<56)), a==(((raw_a & 0x00000000000000ffu64)<<56) | ((raw_a & 0x000000000000ff00u64)<<40) | ((raw_a & 0x0000000000ff0000u64)<<24) | ((raw_a & 0x00000000ff000000u64)<<8) | ((raw_a & 0x000000ff00000000u64)>>8) | ((raw_a & 0x0000ff0000000000u64)>>24) | ((raw_a & 0x00ff000000000000u64)>>40) | ((raw_a & 0xff00000000000000u64)>>56));
        let b0=right[0];
        let b1=right[1];
        let b2=right[2];
        let b3=right[3];
        let b4=right[4];
        let b5=right[5];
        let b6=right[6];
        let b7=right[7];
        assert(raw_b==(((b0 as u64)<<0) | ((b1 as u64)<<8) | ((b2 as u64)<<16) | ((b3 as u64)<<24) | ((b4 as u64)<<32) | ((b5 as u64)<<40) | ((b6 as u64)<<48) | ((b7 as u64)<<56))) by(bit_vector)
            requires ((raw_b & 0xff) as u8)==b0, (((raw_b>>8) & 0xff) as u8)==b1, (((raw_b>>16) & 0xff) as u8)==b2, (((raw_b>>24) & 0xff) as u8)==b3, (((raw_b>>32) & 0xff) as u8)==b4, (((raw_b>>40) & 0xff) as u8)==b5, (((raw_b>>48) & 0xff) as u8)==b6, (((raw_b>>56) & 0xff) as u8)==b7;
        assert(b==(((b0 as u64)<<56) | ((b1 as u64)<<48) | ((b2 as u64)<<40) | ((b3 as u64)<<32) | ((b4 as u64)<<24) | ((b5 as u64)<<16) | ((b6 as u64)<<8) | ((b7 as u64)<<0))) by(bit_vector)
            requires raw_b==(((b0 as u64)<<0) | ((b1 as u64)<<8) | ((b2 as u64)<<16) | ((b3 as u64)<<24) | ((b4 as u64)<<32) | ((b5 as u64)<<40) | ((b6 as u64)<<48) | ((b7 as u64)<<56)), b==(((raw_b & 0x00000000000000ffu64)<<56) | ((raw_b & 0x000000000000ff00u64)<<40) | ((raw_b & 0x0000000000ff0000u64)<<24) | ((raw_b & 0x00000000ff000000u64)<<8) | ((raw_b & 0x000000ff00000000u64)>>8) | ((raw_b & 0x0000ff0000000000u64)>>24) | ((raw_b & 0x00ff000000000000u64)>>40) | ((raw_b & 0xff00000000000000u64)>>56));
        assert(((((a0 as u64)<<56) | ((a1 as u64)<<48) | ((a2 as u64)<<40) | ((a3 as u64)<<32) | ((a4 as u64)<<24) | ((a5 as u64)<<16) | ((a6 as u64)<<8) | ((a7 as u64)<<0))<(((b0 as u64)<<56) | ((b1 as u64)<<48) | ((b2 as u64)<<40) | ((b3 as u64)<<32) | ((b4 as u64)<<24) | ((b5 as u64)<<16) | ((b6 as u64)<<8) | ((b7 as u64)<<0)))==(if a0!=b0 {a0<b0} else {if a1!=b1 {a1<b1} else {if a2!=b2 {a2<b2} else {if a3!=b3 {a3<b3} else {if a4!=b4 {a4<b4} else {if a5!=b5 {a5<b5} else {if a6!=b6 {a6<b6} else {a7<b7}}}}}}})) by(bit_vector);
        assert(((((a0 as u64)<<56) | ((a1 as u64)<<48) | ((a2 as u64)<<40) | ((a3 as u64)<<32) | ((a4 as u64)<<24) | ((a5 as u64)<<16) | ((a6 as u64)<<8) | ((a7 as u64)<<0))==(((b0 as u64)<<56) | ((b1 as u64)<<48) | ((b2 as u64)<<40) | ((b3 as u64)<<32) | ((b4 as u64)<<24) | ((b5 as u64)<<16) | ((b6 as u64)<<8) | ((b7 as u64)<<0)))==(a0==b0 && a1==b1 && a2==b2 && a3==b3 && a4==b4 && a5==b5 && a6==b6 && a7==b7)) by(bit_vector);
        let k=if a0!=b0 {0int} else {if a1!=b1 {1int} else {if a2!=b2 {2int} else {if a3!=b3 {3int} else {if a4!=b4 {4int} else {if a5!=b5 {5int} else {if a6!=b6 {6int} else {if a7!=b7 {7int} else {8int}}}}}}}};
        assert(same_prefix(left,right,k)) by {
            assert forall|i:int| 0<=i<k implies left[i]==right[i] by { }
        }
        compare_at(left,right,k,result);
    }
    result
}
proof fn block_order_at(left:Seq<u8>,right:Seq<u8>,at:int,order:Ordering)
    requires at+8<=left.len(),at+8<=right.len(),same_prefix(left,right,at),
        lex_total(order,left.subrange(at,at+8),right.subrange(at,at+8)),
        order!=Ordering::Equal,
    ensures lex_total(order,left,right),
{
    let a=left.subrange(at,at+8); let b=right.subrange(at,at+8);
    let k=if order==Ordering::Less {less_witness(a,b)}else{less_witness(b,a)};
    assert(same_prefix(left,right,at+k)) by {
        assert forall|i:int| 0<=i<at+k implies left[i]==right[i] by {
            if i>=at {assert(a[i-at]==b[i-at]);}
        }
    }
    compare_at(left,right,at+k,order);
}

/// Compares arbitrary byte slices lexicographically with a bounded scalar loop.
///
/// Equal prefixes sort by length. Map gateways separately enforce fixed widths.
/// No allocation, user callback, alignment assumption or platform memcmp is used.
pub fn compare_keys(left: &[u8], right: &[u8]) -> (result: Ordering)
    ensures lex_total(result, left@, right@), lex_equal_iff(result, left@, right@),
    no_unwind
{
    let mut at = 0usize;
    while at < left.len() && at < right.len()
        invariant same_prefix(left@, right@, at as int),
        decreases left.len() - at,
    {
        if left[at] < right[at] {
            proof { compare_at(left@, right@, at as int, Ordering::Less); }
            return Ordering::Less;
        }
        if left[at] > right[at] {
            proof { compare_at(left@, right@, at as int, Ordering::Greater); }
            return Ordering::Greater;
        }
        at += 1;
    }
    let result = if left.len() < right.len() { Ordering::Less }
        else if left.len() > right.len() { Ordering::Greater }
        else { Ordering::Equal };
    proof { compare_at(left@, right@, at as int, result); }
    result
}

// Local probes use words only at the admitted 32-byte width. Other widths
// retain the scalar comparator, including its proper-prefix behavior.
#[inline(always)]
fn compare_local_keys(left: &[u8], right: &[u8]) -> (result: Ordering)
    ensures lex_total(result, left@, right@), lex_equal_iff(result, left@, right@),
{
    if left.len() != 32 || right.len() != 32 {
        return compare_keys(left, right);
    }
    let mut at = 0usize;
    while at < 32
        invariant at <= 32, at % 8 == 0, left.len() == 32, right.len() == 32,
            same_prefix(left@, right@, at as int),
        decreases 32 - at,
    {
        let a = crate::words::key_word64(&left[at..at + 8]);
        let b = crate::words::key_word64(&right[at..at + 8]);
        let ghost x = left@.subrange(at as int, at + 8);
        let ghost y = right@.subrange(at as int, at + 8);
        if a != b {
            let order = decoded_block_order(a, b, Ghost(x), Ghost(y));
            proof { block_order_at(left@, right@, at as int, order); }
            return order;
        }
        proof {
            vstd::bytes::lemma_auto_spec_u64_to_from_le_bytes();
            assert(vstd::bytes::spec_u64_to_le_bytes(a) == x);
            assert(vstd::bytes::spec_u64_to_le_bytes(b) == y);
            assert(x == y);
            assert(same_prefix(left@, right@, at + 8)) by {
                assert forall|i: int| 0 <= i < at + 8 implies left@[i] == right@[i] by {
                    if i >= at { assert(x[i - at] == y[i - at]); }
                }
            }
        }
        at += 8;
    }
    proof { compare_at(left@, right@, 32, Ordering::Equal); }
    Ordering::Equal
}

/// Calls the production local comparator for bounded model checks.
#[cfg(any(verus_only, kani))]
#[doc(hidden)]
pub fn __local_key_order(left: &[u8], right: &[u8]) -> (result: Ordering)
    requires left.len() == right.len(), left.len() > 0,
    ensures lex_total(result, left@, right@), lex_equal_iff(result, left@, right@),
{
    compare_local_keys(left, right)
}

/// A word probe and a scalar probe have the same unique total order.
#[verifier::spinoff_prover]
pub proof fn local_order_totality(left: Seq<u8>, right: Seq<u8>, scalar: Ordering, word: Ordering)
    requires lex_total(scalar, left, right), lex_total(word, left, right),
    ensures scalar == word,
{
    match scalar {
        Ordering::Equal => {},
        Ordering::Less => {},
        Ordering::Greater => {},
    }
}

/// Replacing one comparison preserves complete branch results, including faults.
#[verifier::spinoff_prover]
pub proof fn local_exact_errors(left: Seq<u8>, right: Seq<u8>, scalar: Ordering, word: Ordering,
    equal: Result<Search>, less: Result<Search>, greater: Result<Search>)
    requires lex_total(scalar, left, right), lex_total(word, left, right),
    ensures (match scalar { Ordering::Equal => equal, Ordering::Less => less, Ordering::Greater => greater })
        == (match word { Ordering::Equal => equal, Ordering::Less => less, Ordering::Greater => greater }),
{
    local_order_totality(left, right, scalar, word);
}

/// Domain-checked insertion neighbors admit exactly the existing local split.
#[verifier::spinoff_prover]
pub proof fn local_neighbor_admission(bytes: Seq<u8>, layout: Layout, query: Seq<u8>, count: u32,
    index: u32, left: Ordering, right: Ordering)
    requires count == le_word(bytes, 0), index <= count,
        query.len() == key_width(layout), !bool_keys(layout) || query[0] <= 1,
        index > 0 ==> lex_total(left, encoded_key(bytes, layout, index as int - 1), query)
            && left == Ordering::Less && (!bool_keys(layout) || encoded_key(bytes, layout, index as int - 1)[0] <= 1),
        index < count ==> lex_total(right, encoded_key(bytes, layout, index as int), query)
            && right == Ordering::Greater && (!bool_keys(layout) || encoded_key(bytes, layout, index as int)[0] <= 1),
    ensures search_found(Search { index, found: false }, bytes, layout, query),
        search_partition(Search { index, found: false }, bytes, layout, query),
{
    endpoint_partition(bytes, layout, query, count, index);
}

/// Every admitted probe and both following intervals stay within the wire count.
#[verifier::spinoff_prover]
pub proof fn local_probe_progress(lo: u32, hi: u32, count: u32)
    requires lo < hi <= count,
    ensures lo <= lo + (hi - lo) / 2 < hi <= count,
        lo + (hi - lo) / 2 + 1 <= hi,
        search_budget(((hi - lo) / 2) as nat) + 1 <= search_budget((hi - lo) as nat),
        search_budget((hi - (lo + (hi - lo) / 2) - 1) as nat) + 1 <= search_budget((hi - lo) as nat),
{
    search_progress((hi - lo) as nat, ((hi - lo) / 2) as nat);
    search_progress((hi - lo) as nat, (hi - (lo + (hi - lo) / 2) - 1) as nat);
}

/// Header geometry contains the complete key before either comparison route.
#[verifier::spinoff_prover]
pub proof fn local_probe_bounds(bytes: Seq<u8>, layout: Layout, partition: MapPartition, index: u32)
    requires crate::map_layout::header_iff(Ok(partition), bytes, layout), index < partition.count,
    ensures 4 <= key_span(layout, index as int).start < key_span(layout, index as int).end <= bytes.len(),
        key_span(layout, index as int).end - key_span(layout, index as int).start == partition.key_width,
{
    let stride = entry_stride(layout);
    assert(0 <= index as int * stride
        && 4 + index as int * stride + stride <= bytes.len()
        && stride <= u32::MAX) by(nonlinear_arith)
        requires index < partition.count, stride > 0,
            4 + partition.count as int * stride <= bytes.len(), bytes.len() <= u32::MAX;
}

/// Re-admitting an unchanged valid header returns its unique existing partition.
#[verifier::spinoff_prover]
pub proof fn local_header_reuse(bytes: Seq<u8>, layout: Layout, before: MapPartition,
    after: Result<MapPartition>)
    requires crate::map_layout::header_iff(Ok(before), bytes, layout),
        crate::map_layout::header_iff(after, bytes, layout),
    ensures after == Ok(before),
{
    match after {
        Ok(partition) => { assert(partition == before); },
        Err(_) => {},
    }
}

/// Concrete word, scalar, fault and interval cases keep the order claims inhabited.
#[verifier::spinoff_prover]
pub proof fn local_order_witness()
{
    let left = Seq::new(32, |i: int| if i == 17 { 1u8 } else { 0u8 });
    let right = Seq::new(32, |i: int| if i == 17 { 2u8 } else { 0u8 });
    assert(same_prefix(left, right, 17));
    compare_at(left, right, 17, Ordering::Less);
    compare_at(right, left, 17, Ordering::Greater);
    compare_at(left, left, 32, Ordering::Equal);
    local_order_totality(left, right, Ordering::Less, Ordering::Less);
    local_order_totality(right, left, Ordering::Greater, Ordering::Greater);
    local_order_totality(left, left, Ordering::Equal, Ordering::Equal);
    let equal = Ok(Search { index: 1, found: true });
    let less = Err(Error::Fault { kind: ErrorKind::InvalidBool, offset: 37 });
    let greater = Err(Error::Fault { kind: ErrorKind::InvalidPlan, offset: Error::NO_OFFSET });
    local_exact_errors(left, right, Ordering::Less, Ordering::Less, equal, less, greater);
    local_exact_errors(right, left, Ordering::Greater, Ordering::Greater, equal, less, greater);
    local_exact_errors(left, left, Ordering::Equal, Ordering::Equal, equal, less, greater);
    local_probe_progress(0, 1, 1);
    local_probe_progress(1, 1000, 1000);
    local_probe_progress(0, u32::MAX, u32::MAX);
}

/// A nonempty raw map admits interior neighbors and bounded keys.
#[verifier::spinoff_prover]
pub proof fn local_map_witness()
{
    reveal(crate::Layout::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    let layout = Layout::Map { key: crate::KeyLayout { width: 1, domain: KeyDomain::Raw },
        value: &Layout::Leaf(crate::Leaf::Unsigned(1)) };
    let bytes = seq![2u8, 0, 0, 0, 1, 8, 3, 9];
    let query = seq![2u8];
    let partition = MapPartition { count: 2, key_width: 1, value_width: Some(1),
        directory: Span { start: 4, end: 4 }, payload: Span { start: 4, end: 8 } };
    reveal(crate::layout_head::head_spec);
    assert(le_word(bytes, 0) == 2);
    assert(key_width(layout) == 1);
    assert(crate::map_layout::value_width(layout) == Some(1u32));
    assert(descriptor_valid(layout, MAX_DEPTH as nat));
    assert(crate::map_layout::header_iff(Ok(partition), bytes, layout));
    local_header_reuse(bytes, layout, partition, Ok(partition));
    assert(encoded_key(bytes, layout, 0) =~= seq![1u8]);
    assert(encoded_key(bytes, layout, 1) =~= seq![3u8]);
    compare_at(seq![1u8], query, 0, Ordering::Less);
    compare_at(seq![3u8], query, 0, Ordering::Greater);
    local_neighbor_admission(bytes, layout, query, 2, 1, Ordering::Less, Ordering::Greater);
    local_probe_bounds(bytes, layout, partition, 0);
    local_probe_bounds(bytes, layout, partition, 1);
}

/// Mathematical key interval in either interleaved layout.
pub open spec fn key_span(layout: Layout, index: int) -> Span {
    Span {
        start: (4 + index * entry_stride(layout)) as u32,
        end: (4 + index * entry_stride(layout) + key_width(layout)) as u32,
    }
}

/// Encoded key selected directly from the physical map sequence.
pub open spec fn encoded_key(bytes: Seq<u8>, layout: Layout, index: int) -> Seq<u8> {
    let span = key_span(layout, index);
    bytes.subrange(span.start as int, span.end as int)
}

/// Exact checked key interval and containment, independent of key order.
pub open spec fn key_range_exact(
    result: Result<Span>, bytes: Seq<u8>, layout: Layout, index: u32,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && map_shape(bytes, layout)
        && index < le_word(bytes, 0);
    match result {
        Ok(span) => valid && span == key_span(layout, index as int)
            && 4 <= span.start < span.end <= bytes.len(),
        Err(_) => !valid,
    }
}

// A checked partition is reused for each probe; no directory rescan per key.
fn key_at(bytes: &[u8], _layout: &Layout, partition: &MapPartition, index: u32)
    -> (result: Result<Span>)
    requires partition_iff(Ok(*partition), bytes@, *_layout),
    ensures key_range_exact(result, bytes@, *_layout, index),
{
    if index >= partition.count {
        return Err(Error::new(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET));
    }
    let tail = partition.value_width.unwrap_or(4u32);
    proof {
        let stride = entry_stride(*_layout);
        assert(0 <= index as int * stride
            && 4 + index as int * stride + stride <= bytes.len()
            && stride <= u32::MAX) by(nonlinear_arith)
            requires index < partition.count, stride > 0,
                4 + partition.count as int * stride <= bytes.len(), bytes.len() <= u32::MAX;
    }
    let stride = partition.key_width + tail;
    let at = 4 + index * stride;
    checked_span(at, partition.key_width, bytes.len() as u32)
}

/// Returns an encoded key's exact span after checking the entire map partition.
///
/// Partition/metadata faults precede `IndexOutOfBounds` with `NO_OFFSET`.
/// Key domains, ordering and value encodings are checked separately.
pub fn map_key(bytes: &[u8], layout: &Layout, index: u32) -> (result: Result<Span>)
    ensures key_range_exact(result, bytes@, *layout, index),
{
    let partition = partition_map(bytes, layout)?;
    key_at(bytes, layout, &partition, index)
}

/// Whether a map uses the one constrained encoded-key domain.
pub open spec fn bool_keys(layout: Layout) -> bool {
    match layout.head() { LayoutHead::Map { key, .. } => matches!(key.domain, KeyDomain::Bool), _ => false }
}

/// Canonical encoded keys; every integer/raw pattern is canonical.
pub open spec fn keys_canonical(bytes: Seq<u8>, layout: Layout, count: int) -> bool {
    forall|i: int| 0 <= i < count ==> !bool_keys(layout)
        || (#[trigger] encoded_key(bytes, layout, i))[0] <= 1
}

/// Pairwise strict physical order implies uniqueness, without decoding keys.
pub open spec fn keys_ordered(bytes: Seq<u8>, layout: Layout, count: int) -> bool {
    forall|i: int, j: int| 0 <= i < j < count ==>
        lex_less(#[trigger] encoded_key(bytes, layout, i), #[trigger] encoded_key(bytes, layout, j))
}

/// Acceptance iff the map partition, canonical keys and strict order are valid.
pub open spec fn keys_valid_iff(result: Result<()>, bytes: Seq<u8>, layout: Layout) -> bool {
    result.is_ok() == (descriptor_valid(layout, MAX_DEPTH as nat) && map_shape(bytes, layout)
        && keys_canonical(bytes, layout, le_word(bytes, 0))
        && keys_ordered(bytes, layout, le_word(bytes, 0)))
}

/// A key fault identifies the first invalid domain or adjacent order pair.
pub open spec fn keys_fault(result: Result<()>, bytes: Seq<u8>, layout: Layout) -> bool {
    descriptor_valid(layout, MAX_DEPTH as nat) && map_shape(bytes, layout) ==> match result {
        Ok(()) => true,
        Err(error) => exists|i: int| 0 <= i < le_word(bytes, 0)
            && keys_canonical(bytes, layout, i) && keys_ordered(bytes, layout, i)
            && if bool_keys(layout) && encoded_key(bytes, layout, i)[0] > 1 {
                error == (Error::Fault { kind: ErrorKind::InvalidBool, offset: key_span(layout, i).start })
            } else {
                i > 0 && if encoded_key(bytes, layout, i - 1) == encoded_key(bytes, layout, i) {
                    error == (Error::Fault { kind: ErrorKind::DuplicateKey, offset: key_span(layout, i).start })
                } else {
                    lex_less(encoded_key(bytes, layout, i), encoded_key(bytes, layout, i - 1))
                        && error == (Error::Fault { kind: ErrorKind::UnsortedKeys, offset: key_span(layout, i).start })
                }
            },
    }
}

/// Checks canonical keys and strict order after the exact map partition.
///
/// Scans in physical order. An invalid bool returns `InvalidBool` at its key
/// start before comparing that key with its predecessor. Equal adjacent keys
/// return `DuplicateKey`; descending keys return `UnsortedKeys`, both at the
/// later key's start. Values' bit domains and child structure are not checked.
#[verifier::spinoff_prover]
// control: map-key-scan
pub fn validate_map_keys(bytes: &[u8], layout: &Layout) -> (result: Result<()>)
    ensures keys_valid_iff(result, bytes@, *layout), keys_fault(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    let partition=partition_map(bytes,layout)?;
    validate_partition_keys(bytes,layout,&partition)
}

#[inline(always)]
#[verifier::spinoff_prover]
// control: partition-key-scan
pub(crate) fn validate_partition_keys(bytes:&[u8],layout:&Layout,partition:&MapPartition)->(result:Result<()>)
    requires partition_iff(Ok(*partition),bytes@,*layout),
    ensures keys_valid_iff(result,bytes@,*layout),keys_fault(result,bytes@,*layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(),bytes@.len()),
{
    if partition.key_width == 32 {
        assert(!bool_keys(*layout));
        return validate_keys32(bytes, partition, Ghost(*layout));
    }
    validate_generic_partition_keys(bytes, layout, partition)
}

// Keep structural admission outside Kani's scan-state cohort.
#[cfg(any(verus_only, kani))]
#[doc(hidden)]
/// Calls the production key scan with an already admitted 32-byte partition.
pub fn __strict_map_partition(bytes:&[u8],layout:&Layout,partition:&MapPartition)->(result:Result<()>)
    requires partition_iff(Ok(*partition),bytes@,*layout),partition.key_width==32,
    ensures keys_valid_iff(result,bytes@,*layout),keys_fault(result,bytes@,*layout),
{
    validate_partition_keys(bytes,layout,partition)
}

// Keep the generic and 32-byte validation loops in separate compiler contexts.
#[inline(never)]
#[verifier::spinoff_prover]
fn validate_generic_partition_keys(bytes:&[u8],layout:&Layout,partition:&MapPartition)->(result:Result<()>)
    requires partition_iff(Ok(*partition),bytes@,*layout), partition.key_width != 32,
    ensures keys_valid_iff(result,bytes@,*layout),keys_fault(result,bytes@,*layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(),bytes@.len()),
{
    hide(lex_less);
    let is_bool = if let Layout::Flat { checked } = layout {
        proof { reveal(crate::layout_head::head_spec); }
        checked.header(7) == 3
    } else { match layout.head() {
        LayoutHead::Map { key, .. } => matches!(key.domain, KeyDomain::Bool),
        _ => return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)),
    }};
    if partition.count == 0 { return Ok(()); }
    let first = key_at(bytes, layout, partition, 0)?;
    assert(encoded_key(bytes@, *layout, 0) == bytes@.subrange(first.start as int, first.end as int));
    if is_bool && bytes[first.start as usize] > 1 {
        proof {
            assert(is_bool == bool_keys(*layout));
            assert(first == key_span(*layout, 0));
            assert(keys_canonical(bytes@, *layout, 0));
            assert(keys_ordered(bytes@, *layout, 0));
            assert(encoded_key(bytes@, *layout, 0)[0] > 1);
            assert(exists|j: int| 0 <= j < le_word(bytes@, 0)
                && keys_canonical(bytes@, *layout, j)
                && keys_ordered(bytes@, *layout, j)
                && bool_keys(*layout) && encoded_key(bytes@, *layout, j)[0] > 1
                && first.start == key_span(*layout, j).start) by {
                assert(0 < le_word(bytes@, 0));
            }
        }
        return Err(Error::new(ErrorKind::InvalidBool, first.start));
    }
    let mut i = 1usize;
    let mut previous = first;
    while i < partition.count as usize
        invariant
            partition_iff(Ok(*partition), bytes@, *layout),
            1 <= i <= partition.count,
            is_bool == bool_keys(*layout),
            keys_canonical(bytes@, *layout, i as int),
            keys_ordered(bytes@, *layout, i as int),
            i > 0 ==> previous == key_span(*layout, i as int - 1)
                && 4 <= previous.start < previous.end <= bytes.len(),
        decreases partition.count - i,
    {
        let span = key_at(bytes, layout, partition, i as u32)?;
        assert(encoded_key(bytes@, *layout, i as int)
            == bytes@.subrange(span.start as int, span.end as int));
        if is_bool && bytes[span.start as usize] > 1 {
            return Err(Error::new(ErrorKind::InvalidBool, span.start));
        }
        let order = compare_keys(&bytes[previous.start as usize..previous.end as usize],
            &bytes[span.start as usize..span.end as usize]);
        assert(lex_total(order, encoded_key(bytes@, *layout, i as int - 1),
            encoded_key(bytes@, *layout, i as int)));
        match order {
            Ordering::Equal => {
                proof {
                    lex_irreflexive(encoded_key(bytes@, *layout, i as int - 1),
                        encoded_key(bytes@, *layout, i as int));
                    if keys_ordered(bytes@, *layout, partition.count as int) {
                        assert(lex_less(encoded_key(bytes@, *layout, i as int - 1),
                            encoded_key(bytes@, *layout, i as int)));
                    }
                }
                return Err(Error::new(ErrorKind::DuplicateKey, span.start));
            },
            Ordering::Greater => {
                proof {
                    if keys_ordered(bytes@, *layout, partition.count as int) {
                        assert(lex_less(encoded_key(bytes@, *layout, i as int - 1),
                            encoded_key(bytes@, *layout, i as int)));
                        lex_antisymmetric(encoded_key(bytes@, *layout, i as int - 1),
                            encoded_key(bytes@, *layout, i as int));
                    }
                }
                return Err(Error::new(ErrorKind::UnsortedKeys, span.start));
            },
            Ordering::Less => {},
        }
        proof {
            assert forall|a: int, b: int| 0 <= a < b < i + 1 implies
                lex_less(#[trigger] encoded_key(bytes@, *layout, a),
                    #[trigger] encoded_key(bytes@, *layout, b)) by {
                if b == i as int && a < i as int - 1 {
                    lex_transitive(encoded_key(bytes@, *layout, a),
                        encoded_key(bytes@, *layout, i as int - 1),
                        encoded_key(bytes@, *layout, i as int));
                }
            }
        }

        previous = span;
        i += 1;
    }
    Ok(())
}


#[inline(always)]
pub(crate) fn compare_key_blocks32(left:&[u8],right:&[u8])
    ->(result:Ordering)
    requires left.len()==32,right.len()==32,
    ensures lex_total(result,left@,right@),
    no_unwind
{
    let ghost left_bytes=left@;
    let ghost right_bytes=right@;
    proof {vstd::bytes::lemma_auto_spec_u64_to_from_le_bytes();}
    let ghost x0=left_bytes.subrange(0,8);
    let ghost y0=right_bytes.subrange(0,8);
    assert(x0=~=left@.subrange(0,8));
    assert(y0=~=right@.subrange(0,8));
    let (_, block_tail) = left.split_at(0);
    let (block, _) = block_tail.split_at(8);
    proof { assert(block@ =~= left@.subrange(0, 8)); }
    let a0=crate::words::key_word64(block);
    let (_, block_tail) = right.split_at(0);
    let (block, _) = block_tail.split_at(8);
    proof { assert(block@ =~= right@.subrange(0, 8)); }
    let b0=crate::words::key_word64(block);
    if a0!=b0 {
        let order=decoded_block_order(a0,b0,Ghost(x0),Ghost(y0));
        proof {block_order_at(left_bytes,right_bytes,0,order);}
        return order;
    }
    proof {
        assert(vstd::bytes::spec_u64_to_le_bytes(a0)==x0);
        assert(vstd::bytes::spec_u64_to_le_bytes(b0)==y0);
        assert(x0==y0);
        assert(same_prefix(left_bytes,right_bytes,8)) by {
            assert forall|k:int| 0<=k<8 implies left_bytes[k]==right_bytes[k] by {
                if k>=0 {assert(x0[k-0]==y0[k-0]);}
            }
        }
    }
    let ghost x1=left_bytes.subrange(8,16);
    let ghost y1=right_bytes.subrange(8,16);
    assert(x1=~=left@.subrange(8,16));
    assert(y1=~=right@.subrange(8,16));
    let (_, block_tail) = left.split_at(8);
    let (block, _) = block_tail.split_at(8);
    proof { assert(block@ =~= left@.subrange(8, 16)); }
    let a1=crate::words::key_word64(block);
    let (_, block_tail) = right.split_at(8);
    let (block, _) = block_tail.split_at(8);
    proof { assert(block@ =~= right@.subrange(8, 16)); }
    let b1=crate::words::key_word64(block);
    if a1!=b1 {
        let order=decoded_block_order(a1,b1,Ghost(x1),Ghost(y1));
        proof {block_order_at(left_bytes,right_bytes,8,order);}
        return order;
    }
    proof {
        assert(vstd::bytes::spec_u64_to_le_bytes(a1)==x1);
        assert(vstd::bytes::spec_u64_to_le_bytes(b1)==y1);
        assert(x1==y1);
        assert(same_prefix(left_bytes,right_bytes,16)) by {
            assert forall|k:int| 0<=k<16 implies left_bytes[k]==right_bytes[k] by {
                if k>=8 {assert(x1[k-8]==y1[k-8]);}
            }
        }
    }
    let ghost x2=left_bytes.subrange(16,24);
    let ghost y2=right_bytes.subrange(16,24);
    assert(x2=~=left@.subrange(16,24));
    assert(y2=~=right@.subrange(16,24));
    let (_, block_tail) = left.split_at(16);
    let (block, _) = block_tail.split_at(8);
    proof { assert(block@ =~= left@.subrange(16, 24)); }
    let a2=crate::words::key_word64(block);
    let (_, block_tail) = right.split_at(16);
    let (block, _) = block_tail.split_at(8);
    proof { assert(block@ =~= right@.subrange(16, 24)); }
    let b2=crate::words::key_word64(block);
    if a2!=b2 {
        let order=decoded_block_order(a2,b2,Ghost(x2),Ghost(y2));
        proof {block_order_at(left_bytes,right_bytes,16,order);}
        return order;
    }
    proof {
        assert(vstd::bytes::spec_u64_to_le_bytes(a2)==x2);
        assert(vstd::bytes::spec_u64_to_le_bytes(b2)==y2);
        assert(x2==y2);
        assert(same_prefix(left_bytes,right_bytes,24)) by {
            assert forall|k:int| 0<=k<24 implies left_bytes[k]==right_bytes[k] by {
                if k>=16 {assert(x2[k-16]==y2[k-16]);}
            }
        }
    }
    let ghost x3=left_bytes.subrange(24,32);
    let ghost y3=right_bytes.subrange(24,32);
    assert(x3=~=left@.subrange(24,32));
    assert(y3=~=right@.subrange(24,32));
    let (_, block_tail) = left.split_at(24);
    let (block, _) = block_tail.split_at(8);
    proof { assert(block@ =~= left@.subrange(24, 32)); }
    let a3=crate::words::key_word64(block);
    let (_, block_tail) = right.split_at(24);
    let (block, _) = block_tail.split_at(8);
    proof { assert(block@ =~= right@.subrange(24, 32)); }
    let b3=crate::words::key_word64(block);
    if a3!=b3 {
        let order=decoded_block_order(a3,b3,Ghost(x3),Ghost(y3));
        proof {block_order_at(left_bytes,right_bytes,24,order);}
        return order;
    }
    proof {
        assert(vstd::bytes::spec_u64_to_le_bytes(a3)==x3);
        assert(vstd::bytes::spec_u64_to_le_bytes(b3)==y3);
        assert(x3==y3);
        assert(same_prefix(left_bytes,right_bytes,32)) by {
            assert forall|k:int| 0<=k<32 implies left_bytes[k]==right_bytes[k] by {
                if k>=24 {assert(x3[k-24]==y3[k-24]);}
            }
        }
    }
    proof {compare_at(left_bytes,right_bytes,32,Ordering::Equal);}
    Ordering::Equal
}

/// Extends strict physical order from an admitted prefix and its next pair.
pub proof fn strict32_extend(bytes:Seq<u8>,layout:Layout,i:int)
    requires i>0,keys_ordered(bytes,layout,i),
        lex_less(encoded_key(bytes,layout,i-1),encoded_key(bytes,layout,i)),
    ensures keys_ordered(bytes,layout,i+1),
{
    assert forall|a:int,b:int| 0<=a<b<i+1 implies
        lex_less(#[trigger] encoded_key(bytes,layout,a),#[trigger] encoded_key(bytes,layout,b)) by {
        if b==i && a<i-1 {
            lex_transitive(encoded_key(bytes,layout,a),encoded_key(bytes,layout,i-1),encoded_key(bytes,layout,i));
        }
    }
}

/// A non-increasing adjacent pair reports its later key as the first fault.
pub proof fn strict32_first_fault(bytes:Seq<u8>,layout:Layout,i:int,order:Ordering)
    requires descriptor_valid(layout,MAX_DEPTH as nat),map_shape(bytes,layout),
        !bool_keys(layout),0<i<le_word(bytes,0),keys_ordered(bytes,layout,i),
        lex_total(order,encoded_key(bytes,layout,i-1),encoded_key(bytes,layout,i)),
        order!=Ordering::Less,
    ensures keys_valid_iff(Err(Error::Fault{kind:if order==Ordering::Equal {ErrorKind::DuplicateKey}
            else {ErrorKind::UnsortedKeys},offset:key_span(layout,i).start}),bytes,layout),
        keys_fault(Err(Error::Fault{kind:if order==Ordering::Equal {ErrorKind::DuplicateKey}
            else {ErrorKind::UnsortedKeys},offset:key_span(layout,i).start}),bytes,layout),
{
    if order==Ordering::Equal {
        lex_irreflexive(encoded_key(bytes,layout,i-1),encoded_key(bytes,layout,i));
    } else {
        lex_antisymmetric(encoded_key(bytes,layout,i-1),encoded_key(bytes,layout,i));
    }
    if keys_ordered(bytes,layout,le_word(bytes,0)) {
        assert(lex_less(encoded_key(bytes,layout,i-1),encoded_key(bytes,layout,i)));
    }
}

/// Partition admission bounds every scan addition; empty maps need no stride.
pub proof fn strict32_coordinates(bytes:Seq<u8>,layout:Layout,partition:MapPartition,i:int)
    requires partition_iff(Ok(partition),bytes,layout),partition.key_width==32,
        0<=i<=partition.count,
    ensures partition.count>0 ==> 32+partition.value_width.unwrap_or(4)<=usize::MAX,
        partition.count>0 ==> 4+i*entry_stride(layout)<=bytes.len()<=usize::MAX,
        i<partition.count ==> 4+i*entry_stride(layout)+32<=bytes.len(),
{
    let stride=entry_stride(layout);
    if partition.count>0 {
        assert(stride<=usize::MAX && 4+i*stride<=bytes.len()) by(nonlinear_arith)
            requires 0<=i<=partition.count,4+partition.count as int*stride<=bytes.len(),
                bytes.len()<=usize::MAX,stride>=32,partition.count>0;
    }
    if i<partition.count {
        assert(4+i*stride+32<=bytes.len()) by(nonlinear_arith)
            requires 0<=i<partition.count,4+partition.count as int*stride<=bytes.len(),stride>=32;
    }
}

/// Concrete prefix, duplicate and descending witnesses include a 31-byte prefix.
pub proof fn strict32_order_witness()
    ensures true,
{
    reveal(crate::Layout::head_spec);
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,4);
    let layout=Layout::Map{key:crate::KeyLayout{width:32,domain:KeyDomain::Raw},
        value:&Layout::Leaf(crate::Leaf::Unsigned(1))};
    let bytes=Seq::new(103,|i:int| if i==0 {3u8} else if i==68 {1u8} else if i==101 {2u8} else {0u8});
    assert(encoded_key(bytes,layout,0)=~=Seq::new(32,|i:int|0u8));
    assert(encoded_key(bytes,layout,1)=~=Seq::new(32,|i:int|if i==31 {1u8}else{0u8}));
    assert(encoded_key(bytes,layout,2)=~=Seq::new(32,|i:int|if i==31 {2u8}else{0u8}));
    assert(same_prefix(encoded_key(bytes,layout,0),encoded_key(bytes,layout,1),31));
    strict32_extend(bytes,layout,1);
    assert(same_prefix(encoded_key(bytes,layout,1),encoded_key(bytes,layout,2),31));
    strict32_extend(bytes,layout,2);
    let dup=bytes.update(101,1);
    assert(le_word(dup,0)==3);
    assert(map_shape(dup,layout));
    assert(encoded_key(dup,layout,1)=~=encoded_key(dup,layout,2));
    lex_irreflexive(encoded_key(dup,layout,1),encoded_key(dup,layout,2));
    assert(encoded_key(dup,layout,0)=~=encoded_key(bytes,layout,0));
    assert(encoded_key(dup,layout,1)=~=encoded_key(bytes,layout,1));
    assert(keys_ordered(dup,layout,2)) by {
        assert forall|a:int,b:int| 0<=a<b<2 implies
            lex_less(#[trigger] encoded_key(dup,layout,a),#[trigger] encoded_key(dup,layout,b)) by {
            assert(a==0 && b==1);
        }
    }
    strict32_first_fault(dup,layout,2,Ordering::Equal);
    let descending=bytes.update(101,0);
    assert(le_word(descending,0)==3);
    assert(map_shape(descending,layout));
    assert(encoded_key(descending,layout,2)=~=encoded_key(descending,layout,0));
    assert(encoded_key(descending,layout,1)=~=encoded_key(bytes,layout,1));
    lex_antisymmetric(encoded_key(descending,layout,1),encoded_key(descending,layout,2));
    assert(encoded_key(descending,layout,0)=~=encoded_key(bytes,layout,0));
    assert(encoded_key(descending,layout,1)=~=encoded_key(bytes,layout,1));
    assert(keys_ordered(descending,layout,2)) by {
        assert forall|a:int,b:int| 0<=a<b<2 implies
            lex_less(#[trigger] encoded_key(descending,layout,a),#[trigger] encoded_key(descending,layout,b)) by {
            assert(a==0 && b==1);
        }
    }
    strict32_first_fault(descending,layout,2,Ordering::Greater);
}

/// Empty, singleton, first/last pair and terminal-coordinate witnesses.
#[verifier::spinoff_prover]
pub proof fn strict32_bounds_witness()
    ensures true,
{
    reveal(crate::Layout::head_spec);
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,4);
    let layout=Layout::Map{key:crate::KeyLayout{width:32,domain:KeyDomain::Raw},
        value:&Layout::Leaf(crate::Leaf::Unsigned(1))};
    let empty=seq![0u8,0,0,0];
    let p0=MapPartition{count:0,key_width:32,value_width:Some(1),directory:Span{start:4,end:4},payload:Span{start:4,end:4}};
    strict32_coordinates(empty,layout,p0,0);
    let one=Seq::new(37,|i:int|if i==0 {1u8}else{0u8});
    let p1=MapPartition{count:1,key_width:32,value_width:Some(1),directory:Span{start:4,end:4},payload:Span{start:4,end:37}};
    assert(le_word(one,0)==1);
    assert(partition_iff(Ok(p1),one,layout));
    strict32_coordinates(one,layout,p1,0);
    strict32_coordinates(one,layout,p1,1);
    let two=Seq::new(70,|i:int|if i==0 {2u8}else{0u8});
    let p2=MapPartition{count:2,key_width:32,value_width:Some(1),directory:Span{start:4,end:4},payload:Span{start:4,end:70}};
    strict32_coordinates(two,layout,p2,0);
    strict32_coordinates(two,layout,p2,1);
    strict32_coordinates(two,layout,p2,2);
}

#[inline(never)]
#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
fn validate_keys32(bytes:&[u8],partition:&MapPartition,Ghost(layout):Ghost<Layout>)->(result:Result<()>)
    requires partition_iff(Ok(*partition),bytes@,layout),partition.key_width==32,!bool_keys(layout),
    ensures keys_valid_iff(result,bytes@,layout),keys_fault(result,bytes@,layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(),bytes@.len()),
{
    hide(lex_less);
    proof {strict32_coordinates(bytes@,layout,*partition,0);}
    let n=partition.count as usize;
    if n<=1 {return Ok(());}
    let tail=partition.value_width.unwrap_or(4) as usize;
    proof {
        assert(4+n*(32+tail)<=bytes.len());
        assert(32+tail<=usize::MAX) by(nonlinear_arith)
            requires n>0,4+n*(32+tail)<=bytes.len(),bytes.len()<=usize::MAX;
    }
    let stride=crate::words::add_bounded(32,tail);
    proof {
        assert(36 <= bytes.len()) by(nonlinear_arith)
            requires n > 0, 4+n*stride <= bytes.len(), stride >= 32;
        assert(4+stride <= bytes.len()) by(nonlinear_arith)
            requires n > 0, 4+n*stride <= bytes.len(), stride >= 32;
    }
    assert(encoded_key(bytes@,layout,0) == bytes@.subrange(4,36));
    let mut at = crate::words::add_bounded(4, stride);
    let ghost mut i = 1int;
    let limit=if partition.value_width.is_some() {bytes.len() as u32} else {partition.directory.end};
    let limit=limit as usize;
    assert(limit==4+n*stride);
    let mut previous_at = 4usize;
    while at<limit
        invariant partition_iff(Ok(*partition),bytes@,layout),partition.key_width==32,!bool_keys(layout),
            n==partition.count,0<n<=u32::MAX,1<=i<=n,
            stride==entry_stride(layout),stride==32+tail,tail==partition.value_width.unwrap_or(4),
            at==4+i*stride,
            keys_ordered(bytes@,layout,i as int),
            previous_at==4+(i-1)*stride,
            4<=previous_at,previous_at+32<=bytes.len(),
            limit==4+n*stride,at<=limit<=bytes.len(),
        decreases limit-at,
    {
        proof {
            assert(i<n) by(nonlinear_arith)
                requires at==4+i*stride,limit==4+n*stride,at<limit,stride>0;
            assert(at+32<=bytes.len()) by(nonlinear_arith)
                requires at==4+i*stride,i<n,stride>=32,4+n*stride<=bytes.len();
        }
        let end=crate::words::add_bounded(at,32);
        let ghost span=Span{start:at as u32,end:end as u32};
        assert(span==key_span(layout,i as int));
        let previous_end=crate::words::add_bounded(previous_at,32);
        assert(encoded_key(bytes@,layout,i as int)==bytes@.subrange(at as int,end as int));
        assert(encoded_key(bytes@,layout,i as int-1)
            ==bytes@.subrange(previous_at as int,previous_end as int));
        assert(bytes@.subrange(previous_at as int,bytes.len() as int).subrange(0,32)
            =~=encoded_key(bytes@,layout,i as int-1));
        assert(bytes@.subrange(at as int,bytes.len() as int).subrange(0,32)
            =~=encoded_key(bytes@,layout,i as int));
        let order=compare_key_blocks32(&bytes[previous_at..previous_end],&bytes[at..end]);
        match order {
            Ordering::Equal=>{
                proof {strict32_first_fault(bytes@,layout,i as int,Ordering::Equal);}
                return Err(Error::new(ErrorKind::DuplicateKey,at as u32));
            },
            Ordering::Greater=>{
                proof {strict32_first_fault(bytes@,layout,i as int,Ordering::Greater);}
                return Err(Error::new(ErrorKind::UnsortedKeys,at as u32));
            },
            Ordering::Less=>{},
        }
        proof {strict32_extend(bytes@,layout,i as int);}
        previous_at=at;
        let ghost old_i=i;
        proof {i=i+1;}
        proof {
            assert(at+stride==4+i*stride) by(nonlinear_arith)
                requires at==4+old_i*stride,i==old_i+1;
            assert(at+stride<=limit) by(nonlinear_arith)
                requires at+stride==4+i*stride,i<=n,limit==4+n*stride,stride>0;
        }
        at=crate::words::add_bounded(at,stride);
    }

    assert(i==n) by(nonlinear_arith)
        requires at==4+i*stride,limit==4+n*stride,at>=limit,i<=n,stride>0;

    Ok(())
}

/// Physical keys and exact values denote a finite mathematical map.
///
/// This relation excludes recursive value validity, which composes in schema
/// validation. Domain equality prevents extra logical entries or missing keys.
pub open spec fn rep_map(bytes: Seq<u8>, layout: Layout, model: Map<Seq<u8>, Seq<u8>>) -> bool {
    let count = le_word(bytes, 0);
    let keys = Seq::new(count as nat, |i: int| encoded_key(bytes, layout, i));
    descriptor_valid(layout, MAX_DEPTH as nat) && map_shape(bytes, layout)
        && keys_canonical(bytes, layout, count) && keys_ordered(bytes, layout, count)
        && model.dom() == keys.to_set()
        && forall|i: int| 0 <= i < count ==> {
            let span = value_span(bytes, layout, i as u32);
            #[trigger] model[encoded_key(bytes, layout, i)] == bytes.subrange(span.start as int, span.end as int)
        }
}

/// Constructs the finite map model from independent physical intervals.
pub open spec fn map_model(bytes: Seq<u8>, layout: Layout) -> Map<Seq<u8>, Seq<u8>> {
    let keys = Seq::new(le_word(bytes, 0) as nat, |i: int| encoded_key(bytes, layout, i));
    Map::new(keys.to_set(), |key: Seq<u8>| {
        let i = choose|i: int| 0 <= i < keys.len() && keys[i] == key;
        let span = value_span(bytes, layout, i as u32);
        bytes.subrange(span.start as int, span.end as int)
    })
}

/// Canonical ordered bytes denote their exact finite key/value map.
pub proof fn map_model_exact(bytes: Seq<u8>, layout: Layout)
    requires
        descriptor_valid(layout, MAX_DEPTH as nat), map_shape(bytes, layout),
        keys_canonical(bytes, layout, le_word(bytes, 0)),
        keys_ordered(bytes, layout, le_word(bytes, 0)),
    ensures rep_map(bytes, layout, map_model(bytes, layout)),
{
    let count = le_word(bytes, 0);
    let keys = Seq::new(count as nat, |i: int| encoded_key(bytes, layout, i));
    assert forall|i: int| 0 <= i < count implies {
        let span = value_span(bytes, layout, i as u32);
        #[trigger] map_model(bytes, layout)[encoded_key(bytes, layout, i)]
            == bytes.subrange(span.start as int, span.end as int)
    } by {
        let key = encoded_key(bytes, layout, i);
        assert(keys[i] == key);
        assert(keys.to_set().contains(key));
        let j = choose|j: int| 0 <= j < keys.len() && keys[j] == key;
        if j < i { lex_irreflexive(encoded_key(bytes, layout, j), key); }
        else if i < j { lex_irreflexive(key, encoded_key(bytes, layout, j)); }
        assert(j == i);
    }
}

/// All probes are bounded; a reported match always identifies equal bytes.
pub open spec fn search_found(result: Search, bytes: Seq<u8>, layout: Layout, key: Seq<u8>) -> bool {
    result.index <= le_word(bytes, 0)
        && (result.found ==> result.index < le_word(bytes, 0)
            && encoded_key(bytes, layout, result.index as int) == key)
}

/// Ordered inputs yield the unique insertion split and exact presence result.
pub open spec fn search_partition(result: Search, bytes: Seq<u8>, layout: Layout, key: Seq<u8>) -> bool {
    keys_ordered(bytes, layout, le_word(bytes, 0)) ==> {
        &&& forall|i: int| 0 <= i < result.index ==>
            lex_less(#[trigger] encoded_key(bytes, layout, i), key)
        &&& forall|i: int| result.index as int + (if result.found { 1int } else { 0int }) <= i < le_word(bytes, 0) ==>
            lex_less(key, #[trigger] encoded_key(bytes, layout, i))
        &&& result.found == (exists|i: int| 0 <= i < le_word(bytes, 0)
            && #[trigger] encoded_key(bytes, layout, i) == key)
    }
}

/// Lookup presence and selected value refine the independent finite map model.
pub proof fn search_refines_map(
    bytes: Seq<u8>, layout: Layout, key: Seq<u8>, result: Search,
    model: Map<Seq<u8>, Seq<u8>>,
)
    requires rep_map(bytes, layout, model), search_found(result, bytes, layout, key),
        search_partition(result, bytes, layout, key),
    ensures result.found == model.dom().contains(key),
        result.found ==> {
            let span = value_span(bytes, layout, result.index);
            model[key] == bytes.subrange(span.start as int, span.end as int)
        },
{
    hide(lex_less);
    let keys = Seq::new(le_word(bytes, 0) as nat, |i: int| encoded_key(bytes, layout, i));
    if result.found {
        assert(keys[result.index as int] == key);
        assert(keys.to_set().contains(key));
    } else if model.dom().contains(key) {
        assert(keys.contains(key));
        let i = choose|i: int| 0 <= i < keys.len() && keys[i] == key;
        assert(encoded_key(bytes, layout, i) == key);
    }
}

/// Worst-case probe budget: zero for empty, one plus the half-range budget.
pub open spec fn search_budget(count: nat) -> nat
    decreases count,
{
    if count == 0 { 0 } else { 1 + search_budget(count / 2) }
}

proof fn budget_monotone(a: nat, b: nat)
    requires a <= b,
    ensures search_budget(a) <= search_budget(b),
    decreases b,
{
    if a > 0 { budget_monotone(a / 2, b / 2); }
}

// The recursive budget is logarithmic and never exceeds 32 for a wire count.
proof fn budget_bound(count: nat, bits: nat)
    requires count < vstd::arithmetic::power2::pow2(bits),
    ensures search_budget(count) <= bits,
    decreases bits,
{
    if count > 0 {
        if bits == 0 {
            vstd::arithmetic::power2::lemma2_to64();
        } else {
            vstd::arithmetic::power2::lemma_pow2_unfold(bits);
            budget_bound(count / 2, (bits - 1) as nat);
        }
    }
}

/// Each nonterminal probe halves the remaining range and consumes one budget.
pub proof fn search_progress(old: nat, new: nat)
    requires old > 0, new <= old / 2,
    ensures new < old, search_budget(new) + 1 <= search_budget(old),
{
    budget_monotone(new, old / 2);
}

/// Rechecks a retained entry ordinal without repeating binary search.
///
/// The ordinal is a hint, never a freshness witness. Local header checking
/// remains in force; a stale hint returns `InvalidPlan`. Ordered inputs yield
/// the same presence/split guarantees as binary search.
#[verifier::spinoff_prover]
pub(crate) fn search_selected(bytes: &[u8], layout: &Layout, key: &[u8], hint: Search)
    -> (result: Result<Search>)
    ensures result.is_ok() ==> descriptor_valid(*layout, MAX_DEPTH as nat) && crate::map_layout::header_shape(bytes@, *layout)
        && key.len() == key_width(*layout) && (!bool_keys(*layout) || key@[0] <= 1)
        && search_found(result.unwrap(), bytes@, *layout, key@)
        && search_partition(result.unwrap(), bytes@, *layout, key@),
{
    hide(lex_less);
    let partition = crate::map_layout::partition_map_header(bytes, layout)?;
    if key.len() != partition.key_width as usize { return Err(ErrorKind::InvalidBoundary.into()); }
    let is_bool = match layout.head() {
        LayoutHead::Map { key, .. } => matches!(key.domain, KeyDomain::Bool),
        _ => return Err(ErrorKind::InvalidSchema.into()),
    };
    if is_bool && key[0] > 1 { return Err(ErrorKind::InvalidBool.into()); }
    if hint.index > partition.count || (hint.found && hint.index == partition.count) {
        return Err(ErrorKind::InvalidPlan.into());
    }
    if hint.found {
        let stored = probe_key(bytes, layout, &partition, hint.index)?;
        let order = compare_local_keys(stored, key);
        assert(lex_total(order, encoded_key(bytes@, *layout, hint.index as int), key@));
        match order { Ordering::Equal => {}, _ => return Err(ErrorKind::InvalidPlan.into()) }
    } else {
        if hint.index > 0 {
            let stored = probe_key(bytes, layout, &partition, hint.index - 1)?;
            let order = compare_local_keys(stored, key);
            assert(lex_total(order, encoded_key(bytes@, *layout, hint.index as int - 1), key@));
            match order { Ordering::Less => {}, _ => return Err(ErrorKind::InvalidPlan.into()) }
        }
        if hint.index < partition.count {
            let stored = probe_key(bytes, layout, &partition, hint.index)?;
            let order = compare_local_keys(stored, key);
            assert(lex_total(order, encoded_key(bytes@, *layout, hint.index as int), key@));
            match order { Ordering::Greater => {}, _ => return Err(ErrorKind::InvalidPlan.into()) }
        }
    }
    assert(!hint.found && hint.index < partition.count ==>
        lex_less(key@, encoded_key(bytes@, *layout, hint.index as int)));
    proof {
        if keys_ordered(bytes@, *layout, partition.count as int) {
            assert forall|i: int| 0 <= i < hint.index implies
                lex_less(#[trigger] encoded_key(bytes@, *layout, i), key@) by {
                if !hint.found && i < hint.index as int - 1 {
                    lex_transitive(encoded_key(bytes@, *layout, i),
                        encoded_key(bytes@, *layout, hint.index as int - 1), key@);
                }
            }
            assert forall|i: int| hint.index as int + (if hint.found { 1int } else { 0int }) <= i < partition.count implies
                lex_less(key@, #[trigger] encoded_key(bytes@, *layout, i)) by {
                if !hint.found && hint.index < i {
                    lex_transitive(key@, encoded_key(bytes@, *layout, hint.index as int),
                        encoded_key(bytes@, *layout, i));
                }
            }
            if !hint.found {
                assert forall|i: int| 0 <= i < partition.count implies
                    #[trigger] encoded_key(bytes@, *layout, i) != key@ by {
                    lex_irreflexive(key@, key@);
                }
            }
        }
    }
    Ok(hint)
}

// Scalar geometry avoids a partition copy on endpoint returns.
#[allow(clippy::too_many_arguments, reason = "Scalar fields avoid materializing unused partition fields on early returns.")]
#[inline(never)]
#[verifier::spinoff_prover]
fn search_selected_partition(bytes: &[u8], layout: &Layout, key: &[u8], hint: Search,
    count: u32, width: u32, value_width: Option<u32>, Ghost(admitted): Ghost<MapPartition>)
    -> (result: Result<Search>)
    requires crate::map_layout::header_iff(Ok(admitted), bytes@, *layout),
        count == admitted.count, width == admitted.key_width, value_width == admitted.value_width,
        count > 0, key.len() == width, !bool_keys(*layout) || key@[0] <= 1,
    ensures result.is_ok() ==> search_found(result.unwrap(), bytes@, *layout, key@)
        && search_partition(result.unwrap(), bytes@, *layout, key@),
{
    hide(lex_less);
    let payload = if value_width.is_some() { 4u32 } else {
        proof {
            let stride = entry_stride(*layout);
            assert(stride == width + 4);
            assert(4 + count as int * stride <= bytes.len());
            assert(width + 4 <= u32::MAX && count as int * (width + 4) + 4 <= u32::MAX)
                by(nonlinear_arith)
                requires count > 0, width > 0, stride == width + 4,
                    4 + count as int * stride <= bytes.len(), bytes.len() <= u32::MAX;
        }
        4 + count * (width + 4)
    };
    let partition = MapPartition { count, key_width: width, value_width,
        directory: Span { start: 4, end: payload },
        payload: Span { start: payload, end: bytes.len() as u32 } };
    assert(partition == admitted);
    if hint.index > partition.count || (hint.found && hint.index == partition.count) {
        return Err(ErrorKind::InvalidPlan.into());
    }
    if hint.found {
        let stored = probe_key(bytes, layout, &partition, hint.index)?;
        let order = compare_local_keys(stored, key);
        assert(lex_total(order, encoded_key(bytes@, *layout, hint.index as int), key@));
        match order { Ordering::Equal => {}, _ => return Err(ErrorKind::InvalidPlan.into()) }
    } else {
        if hint.index > 0 {
            let stored = probe_key(bytes, layout, &partition, hint.index - 1)?;
            let order = compare_local_keys(stored, key);
            assert(lex_total(order, encoded_key(bytes@, *layout, hint.index as int - 1), key@));
            match order { Ordering::Less => {}, _ => return Err(ErrorKind::InvalidPlan.into()) }
        }
        if hint.index < partition.count {
            let stored = probe_key(bytes, layout, &partition, hint.index)?;
            let order = compare_local_keys(stored, key);
            assert(lex_total(order, encoded_key(bytes@, *layout, hint.index as int), key@));
            match order { Ordering::Greater => {}, _ => return Err(ErrorKind::InvalidPlan.into()) }
        }
    }
    assert(!hint.found && hint.index < partition.count ==>
        lex_less(key@, encoded_key(bytes@, *layout, hint.index as int)));
    proof {
        if keys_ordered(bytes@, *layout, partition.count as int) {
            assert forall|i: int| 0 <= i < hint.index implies
                lex_less(#[trigger] encoded_key(bytes@, *layout, i), key@) by {
                if !hint.found && i < hint.index as int - 1 {
                    lex_transitive(encoded_key(bytes@, *layout, i),
                        encoded_key(bytes@, *layout, hint.index as int - 1), key@);
                }
            }
            assert forall|i: int| hint.index as int + (if hint.found { 1int } else { 0int }) <= i < partition.count implies
                lex_less(key@, #[trigger] encoded_key(bytes@, *layout, i)) by {
                if !hint.found && hint.index < i {
                    lex_transitive(key@, encoded_key(bytes@, *layout, hint.index as int),
                        encoded_key(bytes@, *layout, i));
                }
            }
            if !hint.found {
                assert forall|i: int| 0 <= i < partition.count implies
                    #[trigger] encoded_key(bytes@, *layout, i) != key@ by {
                    lex_irreflexive(key@, key@);
                }
            }
        }
    }
    Ok(hint)
}



/// Looks up an encoded key with at most `1 + floor(log2(count))` probes.
///
/// Checks the entire partition first, then requires exactly the descriptor's key
/// width (`InvalidBoundary`, `NO_OFFSET`) and a canonical bool query (`InvalidBool`,
/// `NO_OFFSET`). Searches only key bytes; stored key domains/order and child
/// values are not validated. On strictly ordered maps an absent result gives the
/// unique insertion position. Unsorted maps still have bounded accesses and
/// logarithmic probes, but absence/insertion results have no ordering guarantee.
/// Indexed partition validation scans directory ends once before lookup.
pub fn search_map(bytes: &[u8], layout: &Layout, key: &[u8]) -> (result: Result<Search>)
    ensures match result {
        Ok(search) => descriptor_valid(*layout, MAX_DEPTH as nat) && map_shape(bytes@, *layout)
            && key.len() == key_width(*layout)
            && (!bool_keys(*layout) || key@[0] <= 1)
            && search_found(search, bytes@, *layout, key@)
            && search_partition(search, bytes@, *layout, key@),
        Err(_) => !(descriptor_valid(*layout, MAX_DEPTH as nat) && map_shape(bytes@, *layout)
            && key.len() == key_width(*layout) && (!bool_keys(*layout) || key@[0] <= 1)),
    },
{
    hide(lex_less);
    let partition = partition_map(bytes, layout)?;
    if key.len() != partition.key_width as usize {
        return Err(Error::new(ErrorKind::InvalidBoundary, Error::NO_OFFSET));
    }
    let is_bool = if let Layout::Flat { checked } = layout {
        proof { reveal(crate::layout_head::head_spec); }
        checked.header(7) == 3
    } else { match layout.head() {
        LayoutHead::Map { key, .. } => matches!(key.domain, KeyDomain::Bool),
        _ => return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)),
    }};
    if is_bool && key[0] > 1 {
        return Err(Error::new(ErrorKind::InvalidBool, Error::NO_OFFSET));
    }
    let mut lo = 0u32;
    let mut hi = partition.count;
    let ghost mut probes = 0nat;
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        budget_bound(partition.count as nat, 32);
    }
    while lo < hi
        invariant
            partition_iff(Ok(partition), bytes@, *layout),
            key.len() == key_width(*layout),
            !bool_keys(*layout) || key@[0] <= 1,
            lo <= hi <= partition.count,
            search_budget(partition.count as nat) <= 32,
            probes + search_budget((hi - lo) as nat) <= search_budget(partition.count as nat),
            keys_ordered(bytes@, *layout, partition.count as int) ==> {
                &&& forall|i: int| 0 <= i < lo ==>
                    lex_less(#[trigger] encoded_key(bytes@, *layout, i), key@)
                &&& forall|i: int| hi <= i < partition.count ==>
                    lex_less(key@, #[trigger] encoded_key(bytes@, *layout, i))
            },
        decreases hi - lo,
    {
        let mid = lo + (hi - lo) / 2;
        let span = key_at(bytes, layout, &partition, mid)?;
        let order = compare_keys(&bytes[span.start as usize..span.end as usize], key);
        assert(lex_total(order, encoded_key(bytes@, *layout, mid as int), key@));
        proof { probes = probes + 1; }
        match order {
            Ordering::Equal => {
                proof {
                    assert(probes <= search_budget(partition.count as nat) <= 32);
                    if keys_ordered(bytes@, *layout, partition.count as int) {
                        assert forall|i: int| 0 <= i < mid implies
                            lex_less(#[trigger] encoded_key(bytes@, *layout, i), key@) by {}
                        assert forall|i: int| mid < i < partition.count implies
                            lex_less(key@, #[trigger] encoded_key(bytes@, *layout, i)) by {}
                    }
                }
                return Ok(Search { index: mid, found: true });
            },
            Ordering::Less => {
                proof {
                    if keys_ordered(bytes@, *layout, partition.count as int) {
                        assert forall|i: int| 0 <= i < mid + 1 implies
                            lex_less(#[trigger] encoded_key(bytes@, *layout, i), key@) by {
                            if i < mid {
                                lex_transitive(encoded_key(bytes@, *layout, i),
                                    encoded_key(bytes@, *layout, mid as int), key@);
                            }
                        }
                    }
                    search_progress((hi - lo) as nat, (hi - mid - 1) as nat);
                }
                lo = mid + 1;
            },
            Ordering::Greater => {
                proof {
                    if keys_ordered(bytes@, *layout, partition.count as int) {
                        assert forall|i: int| mid <= i < partition.count implies
                            lex_less(key@, #[trigger] encoded_key(bytes@, *layout, i)) by {
                            if i > mid {
                                lex_transitive(key@, encoded_key(bytes@, *layout, mid as int),
                                    encoded_key(bytes@, *layout, i));
                            }
                        }
                    }
                    search_progress((hi - lo) as nat, (mid - lo) as nat);
                }
                hi = mid;
            },
        }
    }
    proof {
        assert(probes <= search_budget(partition.count as nat) <= 32);
        if keys_ordered(bytes@, *layout, partition.count as int) {
            assert forall|i: int| 0 <= i < partition.count implies
                #[trigger] encoded_key(bytes@, *layout, i) != key@ by {
                lex_irreflexive(key@, key@);
            }
        }
    }
    Ok(Search { index: lo, found: false })
}

#[inline(always)]
fn key_at_local(bytes: &[u8], _layout: &Layout, partition: &MapPartition, index: u32)
    -> (result: Result<Span>)
    requires crate::map_layout::header_iff(Ok(*partition), bytes@, *_layout),
    ensures result.is_ok() ==> result.unwrap() == key_span(*_layout, index as int)
        && 4 <= result.unwrap().start < result.unwrap().end <= bytes.len(),

{
    if index >= partition.count {
        return Err(Error::new(ErrorKind::IndexOutOfBounds, Error::NO_OFFSET));
    }
    let tail = partition.value_width.unwrap_or(4u32);
    proof {
        let stride = entry_stride(*_layout);
        assert(0 <= index as int * stride
            && 4 + index as int * stride + stride <= bytes.len()
            && stride <= u32::MAX) by(nonlinear_arith)
            requires index < partition.count, stride > 0,
                4 + partition.count as int * stride <= bytes.len(), bytes.len() <= u32::MAX;
    }
    let stride = partition.key_width + tail;
    let at = 4 + index * stride;
    checked_span(at, partition.key_width, bytes.len() as u32)
}

// Every touched key is contained and its constrained domain is checked.
#[inline(always)]
fn probe_key<'a>(bytes: &'a [u8], layout: &Layout, partition: &MapPartition, index: u32)
    -> (result: Result<&'a [u8]>)
    requires crate::map_layout::header_iff(Ok(*partition), bytes@, *layout), index < partition.count,
    ensures result.is_ok() ==> result.unwrap()@ == encoded_key(bytes@, *layout, index as int)
        && (!bool_keys(*layout) || result.unwrap()@[0] <= 1),
{
    let span = key_at_local(bytes, layout, partition, index)?;
    let key = &bytes[span.start as usize..span.end as usize];
    let is_bool = if let Layout::Flat { checked } = layout {
        proof { reveal(crate::layout_head::head_spec); }
        checked.header(7) == 3
    } else { match layout.head() { LayoutHead::Map { key, .. } => matches!(key.domain, KeyDomain::Bool), _ => false } };
    if is_bool && key[0] > 1 { return Err(Error::new(ErrorKind::InvalidBool, span.start)); }
    Ok(key)
}

proof fn endpoint_partition(bytes: Seq<u8>, layout: Layout, key: Seq<u8>, count: u32, index: u32)
    requires count == le_word(bytes, 0), index <= count,
        index > 0 ==> lex_less(encoded_key(bytes, layout, index as int - 1), key),
        index < count ==> lex_less(key, encoded_key(bytes, layout, index as int)),
    ensures search_found(Search { index, found: false }, bytes, layout, key),
        search_partition(Search { index, found: false }, bytes, layout, key),
{
    if keys_ordered(bytes, layout, count as int) {
        assert forall|i: int| 0 <= i < count implies
            if i < index { lex_less(#[trigger] encoded_key(bytes, layout, i), key) }
            else { lex_less(key, #[trigger] encoded_key(bytes, layout, i)) } by {
            if i < index as int - 1 {
                lex_transitive(encoded_key(bytes, layout, i), encoded_key(bytes, layout, index as int - 1), key);
            } else if index < i {
                lex_transitive(key, encoded_key(bytes, layout, index as int), encoded_key(bytes, layout, i));
            }
        }
        assert forall|i: int| 0 <= i < count implies #[trigger] encoded_key(bytes, layout, i) != key by {
            lex_irreflexive(key, key);
        }
    }
}

proof fn endpoint_match(bytes: Seq<u8>, layout: Layout, key: Seq<u8>, count: u32, index: u32)
    requires count == le_word(bytes, 0), index < count,
        encoded_key(bytes, layout, index as int) == key,
    ensures search_found(Search { index, found: true }, bytes, layout, key),
        search_partition(Search { index, found: true }, bytes, layout, key),
{
    if keys_ordered(bytes, layout, count as int) {
        assert forall|i: int| 0 <= i < index implies
            lex_less(#[trigger] encoded_key(bytes, layout, i), key) by {}
        assert forall|i: int| index < i < count implies
            lex_less(key, #[trigger] encoded_key(bytes, layout, i)) by {}
    }
}

#[verifier::spinoff_prover]
fn search_local_raw(bytes: &[u8], layout: &Layout, key: &[u8]) -> (result: Result<Search>)
    ensures result.is_ok() ==> descriptor_valid(*layout, MAX_DEPTH as nat)
        && crate::map_layout::header_shape(bytes@, *layout) && key.len() == key_width(*layout)
        && (!bool_keys(*layout) || key@[0] <= 1)
        && search_found(result.unwrap(), bytes@, *layout, key@)
        && search_partition(result.unwrap(), bytes@, *layout, key@),

{
    hide(lex_less);
    let partition = match crate::map_layout::partition_map_header(bytes, layout) {
        Ok(partition) => partition,
        Err(error) => return Err(crate::edit_layout::count_error(error)),
    };
    if key.len() != partition.key_width as usize {
        return Err(Error::new(ErrorKind::InvalidBoundary, Error::NO_OFFSET));
    }
    let is_bool = if let Layout::Flat { checked } = layout {
        proof { reveal(crate::layout_head::head_spec); }
        checked.header(7) == 3
    } else { match layout.head() {
        LayoutHead::Map { key, .. } => matches!(key.domain, KeyDomain::Bool),
        _ => return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)),
    }};
    if is_bool && key[0] > 1 {
        return Err(Error::new(ErrorKind::InvalidBool, Error::NO_OFFSET));
    }
    if partition.count == 0 {
        proof { endpoint_partition(bytes@, *layout, key@, 0, 0); }
        return Ok(Search { index: 0, found: false });
    }
    let mut lo = 0u32;
    let mut hi = partition.count;
    if partition.count > 0 {
        let last = probe_key(bytes, layout, &partition, partition.count - 1)?;
        let last_order = compare_local_keys(last, key);
        assert(lex_total(last_order, encoded_key(bytes@, *layout, partition.count as int - 1), key@));
        match last_order {
            Ordering::Less => {
                proof { endpoint_partition(bytes@, *layout, key@, partition.count, partition.count); }
                return Ok(Search { index: partition.count, found: false });
            },
            Ordering::Equal => {
                proof { endpoint_match(bytes@, *layout, key@, partition.count, (partition.count - 1) as u32); }
                return Ok(Search { index: partition.count - 1, found: true });
            },
            Ordering::Greater => (),
        }
        let first = probe_key(bytes, layout, &partition, 0)?;
        let first_order = compare_local_keys(first, key);
        assert(lex_total(first_order, encoded_key(bytes@, *layout, 0), key@));
        match first_order {
            Ordering::Greater => {
                proof { endpoint_partition(bytes@, *layout, key@, partition.count, 0); }
                return Ok(Search { index: 0, found: false });
            },
            Ordering::Equal => {
                proof { endpoint_match(bytes@, *layout, key@, partition.count, 0); }
                return Ok(Search { index: 0, found: true });
            },
            Ordering::Less => (),
        }
        // The endpoints are already checked. Search only their open interval.
        if partition.count >= 2 {
            lo = 1;
            hi = partition.count - 1;
            if partition.count == 2 {
                // Both insertion neighbors have already been checked.
                proof { endpoint_partition(bytes@, *layout, key@, partition.count, 1); }
                return Ok(Search { index: 1, found: false });
            }
        }
    }
    let ghost mut probes = 0nat;
    proof {
        vstd::arithmetic::power2::lemma2_to64();
        budget_bound(partition.count as nat, 32);
        budget_monotone((hi - lo) as nat, partition.count as nat);
    }
    while lo < hi
        invariant
            crate::map_layout::header_iff(Ok(partition), bytes@, *layout),
            key.len() == key_width(*layout),
            !bool_keys(*layout) || key@[0] <= 1,
            lo <= hi <= partition.count, partition.count > 0,
            search_budget(partition.count as nat) <= 32,
            probes + search_budget((hi - lo) as nat) <= search_budget(partition.count as nat),
            keys_ordered(bytes@, *layout, partition.count as int) ==> {
                &&& forall|i: int| 0 <= i < lo ==>
                    lex_less(#[trigger] encoded_key(bytes@, *layout, i), key@)
                &&& forall|i: int| hi <= i < partition.count ==>
                    lex_less(key@, #[trigger] encoded_key(bytes@, *layout, i))
            },
        decreases hi - lo,
    {
        let mid = lo + (hi - lo) / 2;
        let stored = probe_key(bytes, layout, &partition, mid)?;
        let order = compare_local_keys(stored, key);
        assert(lex_total(order, encoded_key(bytes@, *layout, mid as int), key@));
        proof { probes = probes + 1; }
        match order {
            Ordering::Equal => {
                proof {
                    assert(probes <= search_budget(partition.count as nat) <= 32);
                    if keys_ordered(bytes@, *layout, partition.count as int) {
                        assert forall|i: int| 0 <= i < mid implies
                            lex_less(#[trigger] encoded_key(bytes@, *layout, i), key@) by {}
                        assert forall|i: int| mid < i < partition.count implies
                            lex_less(key@, #[trigger] encoded_key(bytes@, *layout, i)) by {}
                    }
                }
                return Ok(Search { index: mid, found: true });
            },
            Ordering::Less => {
                proof {
                    if keys_ordered(bytes@, *layout, partition.count as int) {
                        assert forall|i: int| 0 <= i < mid + 1 implies
                            lex_less(#[trigger] encoded_key(bytes@, *layout, i), key@) by {
                            if i < mid {
                                lex_transitive(encoded_key(bytes@, *layout, i),
                                    encoded_key(bytes@, *layout, mid as int), key@);
                            }
                        }
                    }
                    search_progress((hi - lo) as nat, (hi - mid - 1) as nat);
                }
                lo = mid + 1;
            },
            Ordering::Greater => {
                proof {
                    if keys_ordered(bytes@, *layout, partition.count as int) {
                        assert forall|i: int| mid <= i < partition.count implies
                            lex_less(key@, #[trigger] encoded_key(bytes@, *layout, i)) by {
                            if i > mid {
                                lex_transitive(key@, encoded_key(bytes@, *layout, mid as int),
                                    encoded_key(bytes@, *layout, i));
                            }
                        }
                    }
                    search_progress((hi - lo) as nat, (mid - lo) as nat);
                }
                hi = mid;
            },
        }
    }
    proof {
        assert(probes <= search_budget(partition.count as nat) <= 32);
        if keys_ordered(bytes@, *layout, partition.count as int) {
            assert forall|i: int| 0 <= i < partition.count implies
                #[trigger] encoded_key(bytes@, *layout, i) != key@ by {
                lex_irreflexive(key@, key@);
            }
        }
    }
    search_selected_partition(bytes, layout, key, Search { index: lo, found: false },
        partition.count, partition.key_width, partition.value_width, Ghost(partition))
}

/// Checks only the endpoint needed for a fixed tail insertion.
#[inline(always)]
pub(crate) fn search_append(bytes: &[u8], layout: &Layout, key: &[u8]) -> (result: Result<bool>)
    ensures result.is_ok() ==> descriptor_valid(*layout, MAX_DEPTH as nat)
        && crate::map_layout::header_shape(bytes@, *layout) && key.len() == key_width(*layout)
        && (!bool_keys(*layout) || key@[0] <= 1),
        result == Ok(true) ==> search_found(Search { index: le_word(bytes@, 0) as u32, found: false }, bytes@, *layout, key@)
            && search_partition(Search { index: le_word(bytes@, 0) as u32, found: false }, bytes@, *layout, key@),
{
    let partition = match crate::map_layout::partition_map_header(bytes, layout) {
        Ok(partition) => partition,
        Err(error) => return Err(crate::edit_layout::count_error(error)),
    };
    if key.len() != partition.key_width as usize { return Err(ErrorKind::InvalidBoundary.into()); }
    let is_bool = if let Layout::Flat { checked } = layout {
        proof { reveal(crate::layout_head::head_spec); }
        checked.header(7) == 3
    } else { match layout.head() { LayoutHead::Map { key, .. } => matches!(key.domain, KeyDomain::Bool), _ => false } };
    if is_bool && key[0] > 1 { return Err(ErrorKind::InvalidBool.into()); }
    if partition.count == 0 { return Ok(true); }
    let last = probe_key(bytes, layout, &partition, partition.count - 1)?;
    let order = compare_local_keys(last, key);
    if matches!(order, Ordering::Less) {
        proof { endpoint_partition(bytes@, *layout, key@, partition.count, partition.count); }
        Ok(true)
    } else { Ok(false) }
}

/// Bounded local lookup, validating touched keys and absent insertion neighbors.
pub fn search_map_local(bytes: &[u8], layout: &Layout, key: &[u8]) -> (result: Result<Search>)
    ensures result.is_ok() ==> descriptor_valid(*layout, MAX_DEPTH as nat)
        && crate::map_layout::header_shape(bytes@, *layout) && key.len() == key_width(*layout)
        && (!bool_keys(*layout) || key@[0] <= 1)
        && search_found(result.unwrap(), bytes@, *layout, key@)
        && search_partition(result.unwrap(), bytes@, *layout, key@),
{
    search_local_raw(bytes, layout, key)
}

}
