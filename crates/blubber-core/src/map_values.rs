//! Fixed-map value selection, independent of key domains and ordering.

use vstd::prelude::*;

verus! {

/// Exact fixed geometry after the typed width checks.
pub open spec fn admitted(bytes: Seq<u8>, key: int, value: int) -> bool {
    &&& 0 < key <= u32::MAX
    &&& 0 <= value <= u32::MAX
    &&& 4 <= bytes.len() <= u32::MAX
    &&& bytes.len() == 4 + crate::words::le_word(bytes, 0) * (key + value)
}

/// Start of one value in its original map slice.
pub open spec fn offset(index: int, key: int, value: int) -> int {
    4 + index * (key + value) + key
}

/// Borrows only the admitted entry's value without computing an error offset.
///
/// # Panics
/// The caller must supply an exact entry within a representable map region.
#[doc(hidden)]
#[inline(always)]
pub fn region(entry: &[u8], key: usize) -> (result: &[u8])
    requires key <= entry.len(),
    ensures result@ == entry@.subrange(key as int, entry.len() as int),
        result.len() == entry.len() - key,
    no_unwind
{
    let (_, value) = entry.split_at(key);
    value
}

/// Exact slot coordinates, absolute offsets and cursor advance stay in bounds.
pub proof fn values_offsets(bytes: Seq<u8>, key: int, value: int, base: int, index: int)
    requires admitted(bytes, key, value), 0 <= base, base + bytes.len() <= u32::MAX,
        0 <= index < crate::words::le_word(bytes, 0),
    ensures 4 <= offset(index, key, value) <= offset(index, key, value) + value <= bytes.len(),
        base + offset(index, key, value) + value <= u32::MAX,
        base + bytes.len() - (crate::words::le_word(bytes,0) - index - 1) * (key+value) - value
            == base + offset(index,key,value),
        4 + (index + 1) * (key + value) <= bytes.len(),
        bytes.subrange(4 + index * (key + value), 4 + (index + 1) * (key + value))
            .subrange(key, key + value)
            == bytes.subrange(offset(index, key, value), offset(index, key, value) + value),
{
    let count = crate::words::le_word(bytes, 0);
    assert(4 <= 4 + index * (key + value)
        && 4 + index * (key + value) + key + value == 4 + (index + 1) * (key + value)
        && 4 + (index + 1) * (key + value) <= bytes.len()) by(nonlinear_arith)
        requires 0 <= index < count, key > 0, value >= 0,
            bytes.len() == 4 + count * (key + value);
    assert(base + bytes.len() - (count-index-1)*(key+value)-value == base + offset(index,key,value))
        by(nonlinear_arith) requires bytes.len() == 4+count*(key+value);
    assert(bytes.subrange(4 + index * (key + value), 4 + (index + 1) * (key + value))
        .subrange(key, key + value)
        =~= bytes.subrange(offset(index, key, value), offset(index, key, value) + value));
}

/// Header admission agrees with the fixed partition; changing key bytes leaves
/// admission and every selected value unchanged when header/value bytes agree.
pub proof fn values_admission_frame(left: Seq<u8>, right: Seq<u8>, layout: crate::Layout,
    key: int, value: int, index: int)
    requires crate::map_layout::map_shape(left, layout),
        crate::map_layout::key_width(layout) == key, key > 0,
        0 <= value <= u32::MAX,
        crate::map_layout::value_width(layout) == Some(value as u32),
        right.len() == left.len(), right.take(4) == left.take(4),
        0 <= index < crate::words::le_word(left, 0),
        right.subrange(offset(index,key,value), offset(index,key,value)+value)
            == left.subrange(offset(index,key,value), offset(index,key,value)+value),
    ensures admitted(left,key,value), admitted(right,key,value),
        right.subrange(offset(index,key,value), offset(index,key,value)+value)
            == left.subrange(offset(index,key,value), offset(index,key,value)+value),
{
    assert(admitted(left,key,value));
    assert(4 <= left.len());
    assert forall|i: int| 0 <= i < 4 implies right[i] == left[i] by {
        vstd::seq::lemma_seq_subrange_index(left,0,4,i);
        vstd::seq::lemma_seq_subrange_index(right,0,4,i);
        assert(right.take(4)[i] == left.take(4)[i]);
    }
    assert(crate::words::le_word(right,0) == crate::words::le_word(left,0));
}

/// Each result, including an error, consumes precisely one slot. Exhaustion is
/// stable, and clone positions have identical remaining counts and coordinates.
pub proof fn values_progress(bytes: Seq<u8>, key: int, value: int, index: int, cloned: int)
    requires admitted(bytes,key,value), 0 <= index <= crate::words::le_word(bytes,0), cloned == index,
    ensures 0 <= crate::words::le_word(bytes,0) - index,
        index < crate::words::le_word(bytes,0) ==>
            crate::words::le_word(bytes,0) - (index+1) == crate::words::le_word(bytes,0) - index - 1,
        index == crate::words::le_word(bytes,0) ==> 4 + index * (key+value) == bytes.len(),
        offset(cloned,key,value) == offset(index,key,value),
        crate::words::le_word(bytes,0) - cloned == crate::words::le_word(bytes,0) - index,
{ }

/// Nonempty and zero-width-value offset witnesses at a nonzero account base.
pub proof fn offsets_witness()
{
    values_offsets(seq![2u8,0,0,0,255,1,254,0],1,1,31,0);
    values_offsets(seq![2u8,0,0,0,255,1,254,0],1,1,31,1);
    values_offsets(seq![1u8,0,0,0,255],1,0,u32::MAX-5,0);
}

/// Invalid key bytes can differ while count and value selection agree.
pub proof fn admission_witness()
{
    reveal(crate::Layout::head_spec);
    reveal(crate::layout_head::head_spec);
    let layout = crate::Layout::Map { key: crate::KeyLayout { width: 1, domain: crate::KeyDomain::Bool },
        value: &crate::Layout::Leaf(crate::Leaf::Bool) };
    let left = seq![2u8,0,0,0,255,1,254,0];
    let right = seq![2u8,0,0,0,7,1,9,0];
    assert(left.take(4) =~= right.take(4));
    assert(left.subrange(5,6) =~= right.subrange(5,6));
    assert(left.subrange(7,8) =~= right.subrange(7,8));
    values_admission_frame(left,right,layout,1,1,0);
    values_admission_frame(left,right,layout,1,1,1);
}

/// Empty, first, partial and exhausted iterator states are reachable.
pub proof fn progress_witness()
{
    values_progress(seq![0u8,0,0,0],1,1,0,0);
    values_progress(seq![1u8,0,0,0,255,1],1,1,0,0);
    values_progress(seq![1u8,0,0,0,255,1],1,1,1,1);
}

}
