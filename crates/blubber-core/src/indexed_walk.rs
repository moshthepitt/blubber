//! Retained adjacent ends, equal-size directory frames and relocation.
use super::*;
#[cfg(verus_only)]
use crate::fixed_edit::overwrite_frame;
verus! {
/// Computes a directory coordinate inside a previously admitted extent.
#[doc(hidden)]
#[inline(always)]
pub fn indexed_coordinate(base: usize, index: usize, stride: usize) -> (result: usize)
    requires base + index * stride <= u32::MAX,
    ensures result == base + index * stride,
    no_unwind
{
    proof {
        assert(index * stride <= usize::MAX) by(nonlinear_arith)
            requires base + index * stride <= u32::MAX, u32::MAX <= usize::MAX;
        vstd::arithmetic::div_mod::lemma_small_mod((index * stride) as nat, usize::MAX as nat + 1);
        vstd::arithmetic::div_mod::lemma_small_mod((base + index * stride) as nat, usize::MAX as nat + 1);
    }
    base.wrapping_add(index.wrapping_mul(stride))
}
/// Checks one adjacent end against retained indexed geometry.
/// The list terminal is implicit; a map's stored terminal must equal its extent.
#[doc(hidden)]
#[inline(always)]
pub fn indexed_next_end(previous: usize, current: usize, extent: usize, terminal: bool) -> (result: Result<usize>)
    ensures result.is_ok() == (previous <= current <= extent && (!terminal || current == extent)),
        result.is_ok() ==> result.unwrap() == current,
    no_unwind
{
    if current < previous || current > extent || (terminal && current != extent) {
        Err(Error::new(ErrorKind::InvalidBoundary, 0))
    } else {
        Ok(current)
    }
}


/// Checks the current key and its visited predecessor within a header window.
/// Unvisited directory records are never read by this operation.
#[doc(hidden)]
#[inline(always)]
pub fn indexed_map_key(bytes: &[u8], at: usize, width: usize, stride: usize, first: bool)
    -> (result: Result<&[u8]>)
    ensures result.is_ok() ==> at <= at + width <= bytes.len()
        && result.unwrap()@ == bytes@.subrange(at as int, at + width)
        && (!first ==> stride <= at && width <= stride
            && crate::search::lex_less(bytes@.subrange((at - stride) as int, at - stride + width), result.unwrap()@)),
        at <= bytes.len() && width <= bytes.len() - at && (first || (width <= stride <= at
            && crate::search::lex_less(bytes@.subrange((at - stride) as int, at - stride + width),
                bytes@.subrange(at as int, at + width)))) ==> result.is_ok(),
    no_unwind
{
    if at > bytes.len() || width > bytes.len() - at {
        return Err(Error::new(ErrorKind::Truncated, Error::NO_OFFSET));
    }
    let (_, tail) = bytes.split_at(at);
    let (key, _) = tail.split_at(width);
    proof { assert(key@ =~= bytes@.subrange(at as int, at + width)); }
    if !first {
        if stride > at || width > stride { return Err(Error::new(ErrorKind::InvalidBoundary, Error::NO_OFFSET)); }
        let (_, previous_tail) = bytes.split_at(at - stride);
        let (previous, _) = previous_tail.split_at(width);
        proof { assert(previous@ =~= bytes@.subrange((at - stride) as int, at - stride + width)); }
        let order = if width == 32 {
            crate::search::compare_key_blocks32(previous, key)
        } else { crate::compare_keys(previous, key) };
        match order {
            core::cmp::Ordering::Less => (),
            core::cmp::Ordering::Equal => return Err(Error::new(ErrorKind::DuplicateKey, Error::NO_OFFSET)),
            core::cmp::Ordering::Greater => return Err(Error::new(ErrorKind::UnsortedKeys, Error::NO_OFFSET)),
        }
    }
    Ok(key)
}

/// Reads one directory word within the admitted, unchanged directory extent.
#[doc(hidden)]
#[inline(always)]
pub fn indexed_read_word(bytes: &[u8], at: usize) -> (value: u32)
    requires at + 4 <= bytes.len(),
    ensures value == words::le_word(bytes@, at as int),
    no_unwind
{
    let (_, rest) = bytes.split_at(at);
    let (word, _) = rest.split_at(4);
    word[0] as u32 + word[1] as u32 * 256 + word[2] as u32 * 65536 + word[3] as u32 * 16777216
}

/// Borrows an exact child from previously admitted indexed coordinates.
///
/// # Panics
/// Panics if the caller supplies coordinates outside the admitted storage.
#[doc(hidden)]
#[inline(always)]
pub fn indexed_slot_region(bytes: &mut [u8], start: usize, end: usize) -> (slot: &mut [u8])
    requires start <= end <= old(bytes).len(), end <= u32::MAX,
    ensures slot.len() == end - start,
        slot@ == old(bytes)@.subrange(start as int, end as int),
        final(bytes)@.subrange(start as int, end as int) == final(slot)@,
        crate::fixed_edit::overwrite_frame(old(bytes)@, final(bytes)@,
            Span { start: start as u32, end: end as u32 }),
    no_unwind
{
    let (_, rest) = bytes.split_at_mut(start);
    let (slot, _) = rest.split_at_mut(end - start);
    proof { vstd::slice::mut_ref_slice_len_eq(&slot); }
    slot
}

/// Returns a checked aligned window and the exact child range within it.
/// This borrows bytes only; Rust's safe UTF-8 conversion remains mandatory.
///
/// # Panics
/// Invalid child coordinates may panic. The caller must admit
/// `start <= end <= bytes.len()` before requesting a window.
#[doc(hidden)]
#[inline(always)]
pub fn indexed_string_window(bytes: &[u8], start: usize, end: usize)
    -> (result: Option<(&[u8], usize, usize)>)
    requires start <= end <= bytes.len(),
    ensures result.is_some() == (end - start <= 8 && 16 <= bytes.len() - (start & !7usize)),
        result.is_some() ==> {
            let (window, low, high) = result.unwrap();
            &&& window.len() == 16
            &&& low <= high <= window.len()
            &&& window@ == bytes@.subrange((start & !7usize) as int, (start & !7usize) + 16)
            &&& window@.subrange(low as int, high as int) == bytes@.subrange(start as int, end as int)
        },
    no_unwind
{
    let len = end - start;
    if len > 8 { return None; }
    let aligned = start & !7usize;
    let low = start & 7usize;
    assert((start & 7usize) <= 7usize) by(bit_vector);
    assert((start & !7usize) <= start) by(bit_vector);
    assert((start & !7usize) == start - (start & 7usize)) by(bit_vector);
    let (_, rest) = bytes.split_at(aligned);
    if rest.len() < 16 { return None; }
    let (window, _) = rest.split_at(16);
    Some((window, low, low + len))
}

/// An indexed header admits extents without certifying unvisited leaf domains.
pub(super) open spec fn indexed_header(bytes: Seq<u8>, layout: Layout) -> bool {
    crate::edit_layout::header_valid(bytes, layout) && match layout.head() {
        LayoutHead::List { .. } => crate::list_layout::element_width(layout).is_none(),
        LayoutHead::Map { .. } => crate::map_layout::value_width(layout).is_none(),
        _ => false,
    }
}

pub(super) open spec fn indexed_payload(bytes: Seq<u8>, layout: Layout) -> int {
    match layout.head() {
        LayoutHead::List { .. } => crate::list_layout::payload_start(bytes, layout),
        _ => crate::map_layout::payload_start(bytes, layout),
    }
}

pub(super) open spec fn indexed_previous(bytes: Seq<u8>, layout: Layout, index: u32) -> int {
    if index == 0 { 0 } else { match layout.head() {
        LayoutHead::List { .. } => words::le_word(bytes, 4 + (index - 1) * 4),
        _ => words::le_word(bytes, 4 + (index - 1) * crate::map_layout::entry_stride(layout) + crate::map_layout::key_width(layout)),
    } }
}

pub(super) open spec fn indexed_current(bytes: Seq<u8>, layout: Layout, index: u32) -> int {
    match layout.head() {
        LayoutHead::List { .. } => if index + 1 == words::le_word(bytes, 0) {
            bytes.len() - indexed_payload(bytes, layout)
        } else { words::le_word(bytes, 4 + index * 4) },
        _ => words::le_word(bytes, 4 + index * crate::map_layout::entry_stride(layout) + crate::map_layout::key_width(layout)),
    }
}

/// Header admission alone contains every key/end coordinate used by a visit.
#[verifier::spinoff_prover]
proof fn lazy_map_walk_containment(bytes: Seq<u8>, layout: Layout, index: u32)
    requires indexed_header(bytes, layout), matches!(layout.head(), LayoutHead::Map { .. }),
        index < words::le_word(bytes, 0),
    ensures 4 <= 4 + index * crate::map_layout::entry_stride(layout),
        4 + index * crate::map_layout::entry_stride(layout) + crate::map_layout::key_width(layout) + 4
            <= indexed_payload(bytes, layout) <= bytes.len() <= u32::MAX,
{
    hide(words::le_word);
    let count = words::le_word(bytes, 0);
    let stride = crate::map_layout::entry_stride(layout);
    let width = crate::map_layout::key_width(layout) as int;
    assert(stride == width + 4);
    assert(indexed_payload(bytes, layout) == 4 + count * stride);
    assert(4 + index * stride + width + 4 <= indexed_payload(bytes, layout)) by(nonlinear_arith)
        requires 0 <= index < count, 0 <= width, stride == width + 4, indexed_payload(bytes, layout) == 4 + count * stride;
}

/// Valid old partitions supply the local checks used by a lazy map visit.
#[verifier::spinoff_prover]
proof fn lazy_map_walk_refinement(bytes: Seq<u8>, layout: Layout, index: u32)
    requires indexed_header(bytes, layout), matches!(layout.head(), LayoutHead::Map { .. }),
        crate::map_layout::map_shape(bytes, layout),
        crate::search::keys_ordered(bytes, layout, words::le_word(bytes, 0)),
        index < words::le_word(bytes, 0),
    ensures {
        let width = crate::map_layout::key_width(layout) as int;
        let stride = crate::map_layout::entry_stride(layout);
        let at = 4 + index * stride;
        &&& 4 <= at <= at + width <= indexed_payload(bytes, layout) <= bytes.len() <= u32::MAX
        &&& indexed_previous(bytes, layout, index) <= indexed_current(bytes, layout, index) <= bytes.len() - indexed_payload(bytes, layout)
        &&& index > 0 ==> width <= stride <= at && crate::search::lex_less(
            bytes.subrange(at - stride, at - stride + width), bytes.subrange(at, at + width))
        &&& index + 1 == words::le_word(bytes, 0) ==> indexed_current(bytes, layout, index) == bytes.len() - indexed_payload(bytes, layout)
    },
{
    hide(words::le_word); hide(crate::search::lex_less);
    let count = words::le_word(bytes, 0);
    let stride = crate::map_layout::entry_stride(layout);
    let width = crate::map_layout::key_width(layout) as int;
    assert(stride == width + 4);
    assert(indexed_payload(bytes, layout) == 4 + count * stride);
    assert(0 <= index < count);
    assert(4 + index * stride + width <= indexed_payload(bytes, layout)) by(nonlinear_arith)
        requires 0 <= index < count, 0 <= width, stride == width + 4, indexed_payload(bytes, layout) == 4 + count * stride;    if index > 0 {
        assert(crate::search::lex_less(crate::search::encoded_key(bytes, layout, index as int - 1),
            crate::search::encoded_key(bytes, layout, index as int)));
        assert(4 + (index as int - 1) * stride == 4 + index * stride - stride) by(nonlinear_arith);
    }
}


/// Concrete nonempty visits witness the key and adjacent-end refinement.
#[verifier::spinoff_prover]
proof fn lazy_map_walk_witness()
{
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    let bytes = seq![2u8,0,0,0,2,1,0,0,0,4,2,0,0,0,97,98];
    let layout = witnesses::INDEXED_MAP;
    assert(indexed_header(bytes, layout));
    assert(crate::map_layout::map_shape(bytes, layout));
    assert(crate::search::keys_ordered(bytes, layout, 2)) by {
        assert forall|i: int, j: int| 0 <= i < j < 2 implies crate::search::lex_less(
            #[trigger] crate::search::encoded_key(bytes, layout, i), #[trigger] crate::search::encoded_key(bytes, layout, j)) by {
            assert(i == 0 && j == 1);
            assert(crate::search::encoded_key(bytes, layout, i) =~= seq![2u8]);
            assert(crate::search::encoded_key(bytes, layout, j) =~= seq![4u8]);
            assert(crate::search::same_prefix(seq![2u8], seq![4u8], 0));
        }
    }
    lazy_map_walk_containment(bytes, layout, 0);
    lazy_map_walk_containment(bytes, layout, 1);
    lazy_map_walk_refinement(bytes, layout, 0);
    lazy_map_walk_refinement(bytes, layout, 1);
}

/// Adjacent stored ends and the implicit list terminal equal generic selection.
#[verifier::spinoff_prover]
proof fn adjacent_end_selection(bytes: Seq<u8>, layout: Layout, index: u32)
    requires indexed_header(bytes, layout), index < words::le_word(bytes, 0),
        indexed_previous(bytes, layout, index) <= indexed_current(bytes, layout, index)
            <= bytes.len() - indexed_payload(bytes, layout),
    ensures {
        let span = validation::child_span(bytes, layout, index as int);
        &&& span.start == indexed_payload(bytes, layout) + indexed_previous(bytes, layout, index)
        &&& span.end == indexed_payload(bytes, layout) + indexed_current(bytes, layout, index)
        &&& span.start <= span.end <= bytes.len()
        &&& index > 0 ==> span.start == validation::child_span(bytes, layout, index as int - 1).end
    },
{
    match layout.head() { LayoutHead::List { .. } => {}, _ => {} }
}

/// An exact equal-size overwrite leaves the count, every end and map key intact.
#[verifier::spinoff_prover]
proof fn same_size_directory_frame(before: Seq<u8>, after: Seq<u8>, layout: Layout, index: u32)
    requires indexed_header(before, layout), index < words::le_word(before, 0),
        indexed_previous(before, layout, index) <= indexed_current(before, layout, index)
            <= before.len() - indexed_payload(before, layout),
        overwrite_frame(before, after, validation::child_span(before, layout, index as int)),
    ensures before.len() == after.len(),
        before.subrange(0, indexed_payload(before, layout)) == after.subrange(0, indexed_payload(before, layout)),
        indexed_header(after, layout),
        forall|i: u32| i < words::le_word(before, 0) ==>
            #[trigger] validation::child_span(after, layout, i as int) == validation::child_span(before, layout, i as int),
        matches!(layout.head(), LayoutHead::Map { .. }) ==> crate::search::keys_ordered(after, layout, words::le_word(before, 0))
            == crate::search::keys_ordered(before, layout, words::le_word(before, 0)),
        matches!(layout.head(), LayoutHead::Map { .. }) ==> crate::search::keys_canonical(after, layout, words::le_word(before, 0))
            == crate::search::keys_canonical(before, layout, words::le_word(before, 0)),
{
    adjacent_end_selection(before, layout, index);
    let payload = indexed_payload(before, layout);
    assert(before.subrange(0, payload) =~= after.subrange(0, payload));
    assert forall|at: int| 0 <= at && at + 4 <= payload implies
        #[trigger] words::le_word(after, at) == words::le_word(before, at) by {}
    assert(words::le_word(after, 0) == words::le_word(before, 0));
    if matches!(layout.head(), LayoutHead::Map { .. }) {
        let count = words::le_word(before, 0);
        let stride = crate::map_layout::entry_stride(layout);
        let key = crate::map_layout::key_width(layout);
        if count > 0 {
            assert(4 + (count-1) * stride + key + 4 == payload) by(nonlinear_arith)
                requires stride == key + 4, payload == 4 + count * stride;
            assert(words::le_word(after, 4 + (count-1) * stride + key) == words::le_word(before, 4 + (count-1) * stride + key));
        }
    }
    assert(indexed_header(after, layout));
    assert forall|i: u32| i < words::le_word(before, 0) implies
        #[trigger] validation::child_span(after, layout, i as int) == validation::child_span(before, layout, i as int) by {
        if matches!(layout.head(), LayoutHead::Map { .. }) {
            let stride = crate::map_layout::entry_stride(layout);
            assert(4 + i * stride + crate::map_layout::key_width(layout) + 4 <= payload) by(nonlinear_arith)
                requires i < words::le_word(before,0), stride == crate::map_layout::key_width(layout) + 4,
                    payload == 4 + words::le_word(before,0) * stride;
            if i > 0 { assert(4 + (i-1) * stride + crate::map_layout::key_width(layout) + 4 <= payload) by(nonlinear_arith)
                requires i < words::le_word(before,0), i > 0, stride == crate::map_layout::key_width(layout) + 4,
                    payload == 4 + words::le_word(before,0) * stride; }
        }
    }
    if matches!(layout.head(), LayoutHead::Map { .. }) {
    assert forall|i: int| 0 <= i < words::le_word(before, 0) implies
        #[trigger] crate::search::encoded_key(after, layout, i) == crate::search::encoded_key(before, layout, i) by {
        if matches!(layout.head(), LayoutHead::Map { .. }) {
            let stride = crate::map_layout::entry_stride(layout);
            let key = crate::map_layout::key_width(layout);
            assert(4 + i * stride + key <= payload) by(nonlinear_arith)
                requires i < words::le_word(before,0), stride == key + 4, key > 0,
                    payload == 4 + words::le_word(before,0) * stride;
            assert(after.subrange(4 + i * stride, 4 + i * stride + key) =~= before.subrange(4 + i * stride, 4 + i * stride + key));
        }
    }
    nested::keys_frame(before, after, layout, words::le_word(before, 0));
    }
}

/// Prefix-preserving reacquisition transports the refreshed indexed selection.
#[verifier::spinoff_prover]
proof fn indexed_relocation_transport(before: Seq<u8>, after: Seq<u8>, layout: Layout, used: u32, index: u32)
    requires used <= before.len(), used <= after.len(),
        before.subrange(0, used as int) == after.subrange(0, used as int),
        indexed_header(before.subrange(0, used as int), layout),
        index < words::le_word(before, 0),
        indexed_previous(before.subrange(0, used as int), layout, index)
            <= indexed_current(before.subrange(0, used as int), layout, index)
            <= used - indexed_payload(before.subrange(0, used as int), layout),
    ensures validation::child_span(before.subrange(0, used as int), layout, index as int)
        == validation::child_span(after.subrange(0, used as int), layout, index as int),
        {
            let span = validation::child_span(before.subrange(0, used as int), layout, index as int);
            &&& span.start <= span.end <= used
            &&& before.subrange(span.start as int, span.end as int) == after.subrange(span.start as int, span.end as int)
        },
{
    adjacent_end_selection(before.subrange(0, used as int), layout, index);
    let span = validation::child_span(before.subrange(0, used as int), layout, index as int);
    assert(before.subrange(span.start as int, span.end as int) =~= before.subrange(0, used as int).subrange(span.start as int, span.end as int));
    assert(after.subrange(span.start as int, span.end as int) =~= after.subrange(0, used as int).subrange(span.start as int, span.end as int));
}


/// Full partition admission and its directory frame certify every adjacent pair.
#[verifier::spinoff_prover]
proof fn indexed_partition_admits_next(bytes: Seq<u8>, layout: Layout, index: u32)
    requires indexed_header(bytes,layout), validation::local_valid(bytes,layout,false),
        index < words::le_word(bytes,0),
    ensures indexed_previous(bytes,layout,index) <= indexed_current(bytes,layout,index)
        <= bytes.len()-indexed_payload(bytes,layout),
        matches!(layout.head(),LayoutHead::Map { .. }) && index+1 == words::le_word(bytes,0)
            ==> indexed_current(bytes,layout,index) == bytes.len()-indexed_payload(bytes,layout),
{
    let i = index as int;
    let n = words::le_word(bytes,0);
    if matches!(layout.head(),LayoutHead::List { .. }) {
        let p = crate::list_layout::payload_start(bytes,layout);
        if i>0 {assert(words::le_word(bytes,4+4*(i-1))<=bytes.len()-p);}
        if i+1<n {
            assert(words::le_word(bytes,4+4*i)<=bytes.len()-p);
            if i>0 {assert(words::le_word(bytes,4+4*(i-1))<=words::le_word(bytes,4+4*i));}
        }
    } else {
        let stride = crate::map_layout::entry_stride(layout);
        let key = crate::map_layout::key_width(layout);
        let p = crate::map_layout::payload_start(bytes,layout);
        assert(words::le_word(bytes,4+i*stride+key)<=bytes.len()-p);
        if i>0 {assert(words::le_word(bytes,4+(i-1)*stride+key)<=words::le_word(bytes,4+i*stride+key));}
    }

    assert(indexed_current(bytes,layout,index) <= bytes.len()-indexed_payload(bytes,layout));
    assert(indexed_previous(bytes,layout,index) <= indexed_current(bytes,layout,index));

}

/// Cached coordinates supply exactly the generic selector's span and patch run.
#[verifier::spinoff_prover]
proof fn indexed_admission_matches_generic(bytes: Seq<u8>, layout: Layout, index: u32) -> (s: Selection)
    requires indexed_header(bytes, layout), index < words::le_word(bytes, 0),
        indexed_previous(bytes, layout, index) <= indexed_current(bytes, layout, index)
            <= bytes.len() - indexed_payload(bytes, layout),
    ensures selection_exact(s, bytes, layout, Step::Index(index)),
        s.span.start == indexed_payload(bytes, layout) + indexed_previous(bytes, layout, index),
        s.span.end == indexed_payload(bytes, layout) + indexed_current(bytes, layout, index),
{
    adjacent_end_selection(bytes, layout, index);
    let span = validation::child_span(bytes, layout, index as int);
    let run = expected_run(bytes, layout, index as int);
    if matches!(layout.head(), LayoutHead::Map { .. }) {
        let stride = crate::map_layout::entry_stride(layout);
        let key = crate::map_layout::key_width(layout);
        let count = words::le_word(bytes,0);
        assert(4 + index * stride + key + (count-index-1) * stride + 4 == indexed_payload(bytes,layout)) by(nonlinear_arith)
            requires stride == key + 4, indexed_payload(bytes,layout) == 4 + count * stride;
    } else if index + 1 < words::le_word(bytes,0) {
        assert(4 + 4 * index + (words::le_word(bytes,0)-index-2) * 4 + 4 == indexed_payload(bytes,layout)) by(nonlinear_arith)
            requires indexed_payload(bytes,layout) == 4 + (words::le_word(bytes,0)-1) * 4;
    }
    Selection { span, run, ordinal: index, layout: validation::child_layout(layout,index as int),
        payload_len: payload_extent(bytes,layout) as u32 }
}

/// Equal-size indexed loans compose their directory frame to the complete root.
#[verifier::spinoff_prover]
proof fn indexed_root_preserves(before: Seq<u8>, after: Seq<u8>, plan: Plan, parent: Span, layout: Layout, index: u32)
    requires replacement_checked(before,plan), subtree_plan(plan), plan.delta == 0,
        parent.start <= parent.end <= before.len(),
        indexed_header(before.subrange(parent.start as int,parent.end as int),layout),
        index < words::le_word(before,parent.start as int),
        {
            let node = before.subrange(parent.start as int,parent.end as int);
            let slot = validation::child_span(node,layout,index as int);
            &&& indexed_previous(node,layout,index) <= indexed_current(node,layout,index) <= node.len()-indexed_payload(node,layout)
            &&& plan.region == (Span { start: (parent.start+slot.start) as u32, end: (parent.start+slot.end) as u32 })
        },
        overwrite_frame(before,after,plan.region),
        validation::schema_valid(before.subrange(12,plan.old_used as int),plan.trail.root,MAX_DEPTH as nat,true),
        validation::schema_valid(after.subrange(plan.region.start as int,plan.region.end as int),
            plan.trail.selected,(MAX_DEPTH-plan.trail.depth) as nat,true),
    ensures replacement_result(before,after,after,plan),
        validation::schema_valid(after.subrange(12,plan.new_used as int),plan.trail.root,MAX_DEPTH as nat,true),
        before.subrange(parent.start as int,parent.start+indexed_payload(before.subrange(parent.start as int,parent.end as int),layout))
            == after.subrange(parent.start as int,parent.start+indexed_payload(before.subrange(parent.start as int,parent.end as int),layout)),
{
    hide(validation::schema_valid);
    let node = before.subrange(parent.start as int,parent.end as int);
    let slot = validation::child_span(node,layout,index as int);
    adjacent_end_selection(node,layout,index);
    assert(overwrite_frame(node,after.subrange(parent.start as int,parent.end as int),slot));
    same_size_directory_frame(node,after.subrange(parent.start as int,parent.end as int),layout,index);
    fixed_overwrite_refines(before,after,after,plan);
    subtree_replace_preserves(before,after,after,plan);
    let prefix = indexed_payload(node,layout);
    assert(before.subrange(parent.start as int,parent.start+prefix) =~= after.subrange(parent.start as int,parent.start+prefix));
}

proof fn indexed_root_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,6);
    reveal_with_fuel(crate::layout::descriptor_fields,4);
    reveal_with_fuel(validation::schema_valid,4);
    reveal_with_fuel(validation::children_valid,5);
    reveal(validation::child_valid);
    let root = witnesses::INDEXED_LIST;
    let local = seq![2u8,0,0,0,1,0,0,0,97,98];
    crate::utf8::ascii_valid(seq![97u8]);
    crate::utf8::ascii_valid(seq![98u8]);
    crate::utf8::ascii_valid(seq![99u8]);
    assert(local.subrange(8,9) =~= seq![97u8]);
    assert(local.subrange(9,10) =~= seq![98u8]);
    let before = witnesses::account(local);
    assert(before.subrange(12,22) =~= local);
    let after = before.update(21,99u8);
    assert(after.subrange(21,22) =~= seq![99u8]);
    let selected = indexed_admission_matches_generic(local,root,1);
    let path = InlinePath { steps: [Step::Index(1);MAX_DEPTH], len:1, overflow:false };
    let trail = admission::direct_trail(before,root,path,22,selected);
    let plan = admission::subtree_plan_for(before,path,trail,1,before.len() as u32);
    indexed_root_preserves(before,after,plan,Span {start:12,end:22},root,1);
}

proof fn indexed_list_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 6);
    let layout = Layout::List { element: &Layout::Str };
    let before = seq![2u8,0,0,0,1,0,0,0,97,98];
    let after = before.update(9, 99u8);
    indexed_partition_admits_next(before, layout, 0);
    indexed_admission_matches_generic(before, layout, 0);
    adjacent_end_selection(before, layout, 1);
    same_size_directory_frame(before, after, layout, 1);
    assert(before.subrange(0,10) =~= (before + seq![0u8]).subrange(0,10));
    indexed_relocation_transport(before, before + seq![0u8], layout, 10, 1);
}

proof fn indexed_map_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 6);
    let layout = witnesses::INDEXED_MAP;
    let before = seq![2u8,0,0,0,1,1,0,0,0,3,2,0,0,0,97,98];
    let after = before.update(14, 99u8);
    assert(crate::layout::descriptor_valid(layout, MAX_DEPTH as nat));
    assert(words::le_word(before, 0) == 2);
    assert(words::le_word(before, 10) == 2);
    assert(indexed_header(before, layout));
    indexed_partition_admits_next(before,layout,1);
    adjacent_end_selection(before, layout, 1);
    same_size_directory_frame(before, after, layout, 0);
    assert(before.subrange(0,16) =~= (before + seq![0u8]).subrange(0,16));
    indexed_relocation_transport(before, before + seq![0u8], layout, 16, 1);
}

/// Point admission binds the child schema and exact span to the same selector.
#[verifier::spinoff_prover]
proof fn scoped_loan_identity(bytes: Seq<u8>, layout: Layout, index: u32) -> (s: Selection)
    requires indexed_header(bytes, layout), index < words::le_word(bytes, 0),
        indexed_previous(bytes, layout, index) <= indexed_current(bytes, layout, index)
            <= bytes.len() - indexed_payload(bytes, layout),
    ensures selection_exact(s, bytes, layout, Step::Index(index)),
        s.layout == validation::child_layout(layout, index as int),
        s.span == validation::child_span(bytes, layout, index as int),
        s.span.start <= s.span.end <= bytes.len(),
{
    let s = indexed_admission_matches_generic(bytes, layout, index);
    adjacent_end_selection(bytes, layout, index);
    s
}

/// A completed equal-size edit retains every child coordinate and its schema.
#[verifier::spinoff_prover]
proof fn scoped_span_stable(before: Seq<u8>, after: Seq<u8>, layout: Layout, index: u32)
    requires indexed_header(before, layout), index < words::le_word(before, 0),
        indexed_previous(before, layout, index) <= indexed_current(before, layout, index)
            <= before.len() - indexed_payload(before, layout),
        overwrite_frame(before, after, validation::child_span(before, layout, index as int)),
    ensures indexed_header(after, layout),
        validation::child_span(after, layout, index as int) == validation::child_span(before, layout, index as int),
        after.len() == before.len(),
{
    same_size_directory_frame(before, after, layout, index);
}

/// Dirty coordinates cannot authorize a loan, including after callback release.
/// Safe Rust separately enforces that no borrowed child occurs in callback R.
pub open spec fn scoped_usable(active: bool, dirty: bool, closed: bool, poisoned: bool) -> bool {
    active && !dirty && !closed && !poisoned
}

/// General edits retire the retained witness before any possible relocation.
proof fn scoped_invalidation(active: bool, dirty: bool, closed: bool, poisoned: bool)
    requires dirty || !active || closed || poisoned,
    ensures !scoped_usable(active, dirty, closed, poisoned),
{
}

/// Closed and poison state precede selection errors; callback errors precede refresh errors.
pub open spec fn scoped_error(closed: bool, poisoned: bool, selection: Option<Error>) -> Option<Error> {
    if closed { Some(Error::Fault { kind: ErrorKind::Closed, offset: Error::NO_OFFSET }) }
    else if poisoned { Some(Error::Fault { kind: ErrorKind::Poisoned, offset: Error::NO_OFFSET }) }
    else { selection }
}

/// State errors win even for an absent or invalid child selection.
proof fn scoped_poison_order(closed: bool, poisoned: bool, selection: Option<Error>)
    requires closed || poisoned,
    ensures scoped_error(closed, poisoned, selection) ==
        Some(Error::Fault { kind: if closed { ErrorKind::Closed } else { ErrorKind::Poisoned }, offset: Error::NO_OFFSET }),
{
}

/// Refresh after a backend call selects from reacquired bytes, with no saved address.
#[verifier::spinoff_prover]
proof fn scoped_relocation(before: Seq<u8>, after: Seq<u8>, layout: Layout, used: u32, index: u32)
    requires used <= before.len(), used <= after.len(),
        before.subrange(0, used as int) == after.subrange(0, used as int),
        indexed_header(before.subrange(0, used as int), layout),
        index < words::le_word(before, 0),
        indexed_previous(before.subrange(0, used as int), layout, index)
            <= indexed_current(before.subrange(0, used as int), layout, index)
            <= used - indexed_payload(before.subrange(0, used as int), layout),
    ensures validation::child_span(before.subrange(0, used as int), layout, index as int)
        == validation::child_span(after.subrange(0, used as int), layout, index as int),
        !scoped_usable(true, true, false, false),
{
    indexed_relocation_transport(before, after, layout, used, index);
    scoped_invalidation(true, true, false, false);
}

/// Matches IndexedLoan::with_account: the callback result has precedence.
pub open spec fn scoped_outcome(callback: Result<nat>, refresh: Result<()>) -> Result<nat> {
    match callback {
        Err(error) => Err(error),
        Ok(value) => match refresh { Err(error) => Err(error), Ok(()) => Ok(value) },
    }
}

/// A callback's returned error leaves earlier completed byte edits intact.
/// The error branch of with_account also wins over a subsequent refresh error.
proof fn scoped_partial_progress(before: Seq<u8>, completed: Seq<u8>, after: Seq<u8>, span: Span, error: Error, refresh: Result<()>)
    requires overwrite_frame(before, completed, span), after == completed,
    ensures overwrite_frame(before, after, span),
        after.subrange(span.start as int, span.end as int) == completed.subrange(span.start as int, span.end as int),
        scoped_outcome(Err(error), refresh) == Err(error),
{
}

/// Nonempty point selection, changed payload, retired loan and error witnesses.
proof fn scoped_point_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 6);
    let layout = Layout::List { element: &Layout::Str };
    let before = seq![2u8,0,0,0,1,0,0,0,97,98];
    let after = before.update(9, 99u8);
    let s = scoped_loan_identity(before, layout, 1);
    assert(s.span == (Span { start: 9, end: 10 }));
    assert(s.layout == Layout::Str);
    scoped_span_stable(before, after, layout, 1);
    scoped_invalidation(true, true, false, false);
    scoped_invalidation(false, false, false, false);
    scoped_poison_order(false, true, Some(Error::Fault { kind: ErrorKind::IndexOutOfBounds, offset: Error::NO_OFFSET }));
    scoped_poison_order(true, true, None);
    scoped_partial_progress(before, after, after, s.span, Error::Fault { kind: ErrorKind::Capacity, offset: Error::NO_OFFSET }, Err(Error::Fault { kind: ErrorKind::Poisoned, offset: Error::NO_OFFSET }));
    assert(after[9] != before[9]);
    assert(before.subrange(0,10) =~= (before + seq![0u8]).subrange(0,10));
    scoped_relocation(before, before + seq![0u8], layout, 10, 1);
}

}
