//! Exact fixed-list payloads and streamed disjoint slot loans.

use super::*;
#[cfg(verus_only)]
use crate::fixed_edit::overwrite_frame;

verus! {

/// Exact shape admitted by the tail gateway, without any leaf-domain claim.
pub open spec fn walk_admitted(bytes: Seq<u8>, discriminator: [u8; 8], minimum: u32, offset: u32, width: u32) -> bool {
    &&& prepared::account_admitted(bytes, discriminator, minimum)
    &&& 0 < width
    &&& 12 + offset + 4 <= words::le_word(bytes, 8)
    &&& words::le_word(bytes, 8) == 12 + offset + 4 + words::le_word(bytes, 12 + offset) * width
}

/// Borrows a fixed-list payload at a certified root-relative tail offset.
///
/// Checks the current account envelope and exact count/stride extent. The
/// caller establishes selector/offset correspondence from schema metadata.
/// Leaf domains remain lazy; every byte outside the payload is framed.
#[doc(hidden)]
#[inline(always)]
pub fn fixed_list_payload(bytes: &mut [u8], discriminator: [u8; 8], minimum: u32,
    offset: u32, width: u32) -> (result: Result<(&mut [u8], u32, u32)>)
    ensures result.is_ok() == walk_admitted(old(bytes)@, discriminator, minimum, offset, width),
        result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> {
            let (payload, used, count) = result.unwrap();
            &&& 0 < width
            &&& 12 + offset + 4 <= used <= old(bytes).len() <= u32::MAX
            &&& used == words::le_word(old(bytes)@, 8)
            &&& count == words::le_word(old(bytes)@, 12 + offset)
            &&& payload.len() == count * width
            &&& payload@ == old(bytes)@.subrange(12 + offset + 4, used as int)
            &&& final(bytes).len() == old(bytes).len()
            &&& overwrite_frame(old(bytes)@, final(bytes)@, Span { start: (12 + offset + 4) as u32, end: used })
            &&& final(bytes)@.subrange(12 + offset + 4, used as int) == final(payload)@
        },
{
    let used = tail_edit::account_used(bytes, discriminator, minimum)?;
    if width == 0 { return Err(ErrorKind::InvalidSchema.into()); }
    let start = checked_delta(12, offset as i64)?;
    let base = checked_delta(start, 4)?;
    if base > used { return Err(Error::new(ErrorKind::Truncated, start)); }
    let count = read_u32(bytes, start)?;
    let len = checked_mul(count as usize, width as usize)?;
    let word_len = checked_u32(len)?;
    let end = checked_delta(base, word_len as i64)?;
    if end > used { return Err(Error::new(ErrorKind::Truncated, base)); }
    if end < used { return Err(Error::new(ErrorKind::InvalidBoundary, end)); }
    let (_, rest) = bytes.split_at_mut(base as usize);
    let (payload, _) = rest.split_at_mut(len);
    proof { vstd::slice::mut_ref_slice_len_eq(&payload); }
    Ok((payload, used, count))
}

/// Splits one exact fixed slot from the unvisited suffix.
///
/// The caller admits the stride/count once and calls this only while a slot
/// remains. The two loans partition the original bytes; neither can resize.
///
/// # Panics
/// Panics if `width` exceeds the remaining byte length.
#[doc(hidden)]
#[inline(always)]
pub fn split_fixed_slot(bytes: &mut [u8], width: usize) -> (result: (&mut [u8], &mut [u8]))
    requires width <= old(bytes).len(),
    ensures result.0.len() == width, result.1.len() == old(bytes).len() - width,
        result.0@ == old(bytes)@.subrange(0, width as int),
        result.1@ == old(bytes)@.subrange(width as int, old(bytes).len() as int),
        final(bytes)@ == final(result.0)@ + final(result.1)@,
    no_unwind
{
    bytes.split_at_mut(width)
}

/// Lends a contained fixed field without resolving an account or edit trail.
/// The caller obtains offset/width from the inherited field certificate.
#[doc(hidden)]
#[inline(always)]
pub fn fixed_slot_region(bytes: &mut [u8], offset: u32, width: u32) -> (result: Result<&mut [u8]>)
    ensures result.is_ok() == (offset + width <= old(bytes).len() && offset + width <= u32::MAX),
        result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> {
            &&& result.unwrap().len() == width
            &&& result.unwrap()@ == old(bytes)@.subrange(offset as int, offset + width)
            &&& final(bytes)@.subrange(offset as int, offset + width) == final(result.unwrap())@
            &&& overwrite_frame(old(bytes)@, final(bytes)@,
                Span { start: offset, end: (offset + width) as u32 })
        },
    no_unwind
{
    let end = match offset.checked_add(width) {
        Some(end) => end,
        None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    if end as usize > bytes.len() {
        return Err(Error::new(ErrorKind::InvalidBoundary, Error::NO_OFFSET));
    }
    let (_, rest) = bytes.split_at_mut(offset as usize);
    let (slot, _) = rest.split_at_mut(width as usize);
    proof { vstd::slice::mut_ref_slice_len_eq(&slot); }
    Ok(slot)
}

/// Returns the next exact borrowed chunk, or permanent exhaustion.
/// The caller admits a positive stride and complete payload partition once.
#[doc(hidden)]
#[inline(always)]
pub fn split_fixed_chunk(bytes: &[u8], width: usize) -> (result: Option<(&[u8], &[u8])>)
    requires width > 0, bytes.len() % width == 0,
    ensures result.is_some() == (bytes.len() > 0),
        result.is_some() ==> {
            let (chunk, tail) = result.unwrap();
            &&& chunk.len() == width
            &&& tail.len() == bytes.len() - width
            &&& chunk@ == bytes@.subrange(0, width as int)
            &&& tail@ == bytes@.subrange(width as int, bytes.len() as int)
            &&& chunk@ + tail@ == bytes@
        },
    no_unwind
{
    if bytes.len() < width {
        proof { vstd::arithmetic::div_mod::lemma_small_mod(bytes.len() as nat, width as nat); }
        None
    } else {
        Some(bytes.split_at(width))
    }
}

/// Recovers a chunk's absolute start from its admitted end and remaining bytes.
/// The immutable walk's containment proof establishes the subtraction order.
#[doc(hidden)]
#[inline(always)]
pub fn fixed_chunk_base(end: u32, remaining: usize) -> (result: u32)
    requires remaining <= end,
    ensures result == end - remaining,
    no_unwind
{
    proof { vstd::arithmetic::div_mod::lemma_small_mod((end - remaining) as nat, u32::MAX as nat + 1); }
    end.wrapping_sub(remaining as u32)
}

/// Recovers an immutable region's end without repeating its admitted bound check.
#[doc(hidden)]
#[inline(always)]
pub fn fixed_chunk_end(base: u32, len: usize) -> (result: u32)
    requires base + len <= u32::MAX,
    ensures result == base + len,
    no_unwind
{
    proof { vstd::arithmetic::div_mod::lemma_small_mod((base + len) as nat, u32::MAX as nat + 1); }
    base.wrapping_add(len as u32)
}

/// Exact fixed-list shape, without visiting any element domain.
pub open spec fn read_chunks_admitted(bytes: Seq<u8>, width: u32) -> bool {
    0 < width && 4 <= bytes.len() <= u32::MAX
        && bytes.len() == 4 + words::le_word(bytes, 0) * width
}

/// Checks the count/product/extent once and borrows the complete fixed payload.
/// Metadata classification belongs to the caller; element domains remain lazy.
#[doc(hidden)]
#[inline(always)]
#[allow(clippy::question_mark)]
pub fn fixed_read_payload(bytes: &[u8], width: u32) -> (result: Result<&[u8]>)
    requires bytes.len() <= u32::MAX,
    ensures result.is_ok() == read_chunks_admitted(bytes@, width),
        result.is_ok() ==> result.unwrap()@ == bytes@.subrange(4, bytes.len() as int),
    no_unwind
{
    if width == 0 { return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)); }
    // Pinned vstd's Result::from_residual has no no_unwind contract.
    let count = match read_u32(bytes, 0) { Ok(count) => count, Err(error) => return Err(error) };
    let payload = match count.checked_mul(width) {
        Some(len) => len,
        None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    let end = match payload.checked_add(4) {
        Some(end) => end,
        None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    if end as usize > bytes.len() { return Err(Error::new(ErrorKind::Truncated, 4)); }
    if (end as usize) < bytes.len() { return Err(Error::new(ErrorKind::InvalidBoundary, end)); }
    Ok(bytes.split_at(4).1)
}

/// A sole variable field has the same start/layout as the generic selector.
#[verifier::spinoff_prover]
proof fn walk_tail_selector(bytes: Seq<u8>, layout: Layout, declaration: u16) -> (s: Selection)
    requires crate::layout::descriptor_valid(layout, MAX_DEPTH as nat),
        match layout.head() { LayoutHead::Struct { fixed_bytes, variable, .. } =>
            variable.len() == 1 && variable.view()[0].declaration == declaration
                && fixed_bytes <= bytes.len() <= u32::MAX, _ => false },
    ensures selection_exact(s, bytes, layout, Step::Field(declaration)),
        s.span.end == bytes.len(), s.run == empty_run(),
        match layout.head() { LayoutHead::Struct { fixed_bytes, fixed, variable } =>
            s.span.start == fixed_bytes && s.ordinal == fixed.len()
                && s.layout == variable.view()[0].layout, _ => false },
{
    let (prefix, ordinal, child) = match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } => (fixed_bytes, fixed.len(), variable.view()[0].layout),
        _ => { assert(false); arbitrary() },
    };
    assert(crate::struct_layout::header_shape(bytes, layout));
    assert(crate::edit_layout::header_valid(bytes, layout));
    assert(validation::child_span(bytes, layout, ordinal as int) == (Span { start: prefix, end: bytes.len() as u32 }));
    assert(selects(layout, Step::Field(declaration), ordinal as int));
    Selection { payload_len: (bytes.len() - prefix) as u32,
        span: Span { start: prefix, end: bytes.len() as u32 }, layout: child,
        ordinal: ordinal as u32, run: empty_run() }
}

/// The metadata-only route built by consecutive sole-variable projections.
spec fn tail_route(parents: Seq<Layout>, offsets: Seq<u32>, declarations: Seq<u16>) -> bool {
    &&& parents.len() == offsets.len() == declarations.len() + 1
    &&& offsets[0] == 0
    &&& forall|i: int| 0 <= i < parents.len() ==> #[trigger] crate::layout::descriptor_valid(parents[i], MAX_DEPTH as nat)
    &&& forall|i: int| 0 <= i < declarations.len() ==> #[trigger] offsets[i] <= offsets[i + 1]
    &&& forall|i: int| #![trigger parents[i], offsets[i]] 0 <= i < declarations.len() ==> {
        match parents[i].head() {
            LayoutHead::Struct { fixed_bytes, variable, .. } => variable.len() == 1
                && variable.view()[0].declaration == declarations[i]
                && variable.view()[0].layout == parents[i + 1]
                && offsets[i + 1] == offsets[i] + fixed_bytes,
            _ => false,
        }
    }
}

proof fn tail_offset_bound(offsets: Seq<u32>, i: int, last: int)
    requires 0 <= i <= last < offsets.len(),
        forall|j: int| 0 <= j < last ==> #[trigger] offsets[j] <= offsets[j + 1],
    ensures offsets[i] <= offsets[last],
    decreases last - i,
{
    if i < last { tail_offset_bound(offsets, i + 1, last); }
}

/// Every prefix of a cached tail route denotes the generic local selection.
#[verifier::spinoff_prover]
proof fn walk_tail_path(bytes: Seq<u8>, parents: Seq<Layout>, offsets: Seq<u32>, declarations: Seq<u16>)
    requires tail_route(parents, offsets, declarations),
        offsets[parents.len() - 1] <= bytes.len() <= u32::MAX,
    ensures forall|i: int| #![trigger parents[i], offsets[i]] 0 <= i < declarations.len() ==> {
        let local = bytes.subrange(offsets[i] as int, bytes.len() as int);
        let s = Selection { payload_len: (bytes.len() - offsets[i + 1]) as u32,
            span: Span { start: (offsets[i + 1] - offsets[i]) as u32, end: (bytes.len() - offsets[i]) as u32 },
            layout: parents[i + 1], ordinal: match parents[i].head() {
                LayoutHead::Struct { fixed, .. } => fixed.len() as u32, _ => 0 }, run: empty_run() };
        &&& selection_exact(s, local, parents[i], Step::Field(declarations[i]))
        &&& offsets[i] + s.span.start == offsets[i + 1]
        &&& offsets[i] + s.span.end == bytes.len()
    },
{
    reveal(tail_route);
    assert forall|i: int| #![trigger parents[i], offsets[i]] 0 <= i < parents.len() implies offsets[i] <= bytes.len() by {
        tail_offset_bound(offsets, i, (parents.len() - 1) as int);
    }
    assert forall|i: int| #![trigger parents[i], offsets[i]] 0 <= i < declarations.len() implies {
        let local = bytes.subrange(offsets[i] as int, bytes.len() as int);
        let s = Selection { payload_len: (bytes.len() - offsets[i + 1]) as u32,
            span: Span { start: (offsets[i + 1] - offsets[i]) as u32, end: (bytes.len() - offsets[i]) as u32 },
            layout: parents[i + 1], ordinal: match parents[i].head() {
                LayoutHead::Struct { fixed, .. } => fixed.len() as u32, _ => 0 }, run: empty_run() };
        &&& selection_exact(s, local, parents[i], Step::Field(declarations[i]))
        &&& offsets[i] + s.span.start == offsets[i + 1]
        &&& offsets[i] + s.span.end == bytes.len()
    } by {
        let local = bytes.subrange(offsets[i] as int, bytes.len() as int);
        let s = walk_tail_selector(local, parents[i], declarations[i]);
    }
}

/// Borrowed fixed-struct fields retain the generic declaration/layout selection.
#[verifier::spinoff_prover]
proof fn walk_fixed_projection(bytes: Seq<u8>, layout: Layout, declaration: u16, index: int, budget: nat) -> (s: Selection)
    requires validation::schema_valid(bytes, layout, budget, false),
        match layout.head() { LayoutHead::Struct { fixed, .. } => 0 <= index < fixed.len()
            && fixed.view()[index].declaration == declaration, _ => false },
    ensures selection_exact(s, bytes, layout, Step::Field(declaration)), s.run == empty_run(), s.ordinal == index,
        validation::schema_valid(bytes.subrange(s.span.start as int, s.span.end as int), s.layout, (budget - 1) as nat, false),
{
    hide(validation::schema_valid); hide(validation::local_valid); hide(validation::child_valid);
    assert(validation::local_valid(bytes, layout, false)) by { reveal(validation::schema_valid); }
    assert(validation::children_valid(bytes, layout, 0, (budget - 1) as nat, false)) by { reveal(validation::schema_valid); }
    nested::child_bounds(bytes, layout, index, false);
    crate::preservation::child_from_children(bytes, layout, 0, index, (budget - 1) as nat, false);
    let span = validation::child_span(bytes, layout, index);
    let child = validation::child_layout(layout, index);
    assert(validation::child_valid(bytes, layout, index, (budget - 1) as nat, false));
    assert(validation::schema_valid(bytes.subrange(span.start as int, span.end as int), child, (budget - 1) as nat, false)) by {
        reveal_with_fuel(validation::child_valid, 2);
        reveal_with_fuel(validation::schema_valid, 1);
    }
    assert(crate::edit_layout::header_valid(bytes, layout)) by { reveal(validation::local_valid); }
    assert(0 <= payload_extent(bytes, layout) <= u32::MAX) by { reveal(validation::local_valid); }
    Selection { payload_len: payload_extent(bytes, layout) as u32, span, layout: child, ordinal: index as u32, run: empty_run() }
}

/// Fixed stride slots agree with generic list indices and remain disjoint.
#[verifier::spinoff_prover]
proof fn walk_stride_selection(bytes: Seq<u8>, layout: Layout, width: u32, index: int)
    requires crate::layout::descriptor_valid(layout, MAX_DEPTH as nat),
        crate::list_layout::header_shape(bytes, layout),
        match layout.head() { LayoutHead::List { element } => width_spec(element) == Some(width), _ => false },
        0 < width, 0 <= index < words::le_word(bytes, 0),
    ensures validation::child_span(bytes, layout, index) == (Span {
            start: (4 + index * width) as u32, end: (4 + (index + 1) * width) as u32 }),
        4 <= 4 + index * width < 4 + (index + 1) * width <= bytes.len() <= u32::MAX,
        forall|j: int| 0 <= j < index ==> #[trigger] validation::child_span(bytes, layout, j).end <= 4 + index * width,
{
    assert(bytes.len() == 4 + words::le_word(bytes, 0) * width);
    assert(0 <= index * width < (index + 1) * width) by(nonlinear_arith) requires index >= 0, width > 0;
    assert((index + 1) * width <= words::le_word(bytes, 0) * width) by(nonlinear_arith)
        requires index < words::le_word(bytes, 0), width > 0;
    assert forall|j: int| 0 <= j < index implies #[trigger] validation::child_span(bytes, layout, j).end <= 4 + index * width by {
        assert((j + 1) * width <= index * width) by(nonlinear_arith) requires j < index, width > 0;
    }
}

/// A streamed chunk supplies the same bytes, layout and base as list selection.
/// Any deterministic checked decoder therefore observes the same input triple;
/// no semantic validity of an unvisited leaf is assumed.
#[verifier::spinoff_prover]
proof fn walk_chunk_selection(bytes: Seq<u8>, layout: Layout, width: u32, index: int, base: u32)
    requires crate::layout::descriptor_valid(layout, MAX_DEPTH as nat),
        crate::list_layout::header_shape(bytes, layout),
        match layout.head() { LayoutHead::List { element } => width_spec(element) == Some(width), _ => false },
        0 < width, 0 <= index < words::le_word(bytes, 0),
        base + bytes.len() <= u32::MAX,
    ensures {
        let span = validation::child_span(bytes, layout, index);
        let remaining = bytes.subrange(4 + index * width, bytes.len() as int);
        &&& remaining.len() == (words::le_word(bytes, 0) - index) * width
        &&& remaining.subrange(0, width as int) == bytes.subrange(span.start as int, span.end as int)
        &&& remaining.subrange(width as int, remaining.len() as int)
            == bytes.subrange(4 + (index + 1) * width, bytes.len() as int)
        &&& base + 4 + index * width == base + span.start
        &&& base + span.end <= u32::MAX
        &&& match layout.head() { LayoutHead::List { element } =>
            validation::child_layout(layout, index) == element, _ => false }
    },
{
    walk_stride_selection(bytes, layout, width, index);
    let count = words::le_word(bytes, 0);
    assert(bytes.len() == 4 + count * width);
    assert((count - index) * width == count * width - index * width) by(nonlinear_arith);
    let start = 4 + index * width;
    let end = 4 + (index + 1) * width;
    let remaining = bytes.subrange(start, bytes.len() as int);
    assert(remaining.len() == (count - index) * width);
    assert((count - index) * width >= width) by(nonlinear_arith)
        requires count - index >= 1, width > 0;
    assert(width <= remaining.len());
    assert(end == start + width) by(nonlinear_arith)
        requires start == 4 + index * width, end == 4 + (index + 1) * width;
    assert(remaining.subrange(0, width as int) =~= bytes.subrange(start, end));
    assert(remaining.subrange(width as int, remaining.len() as int) =~= bytes.subrange(end, bytes.len() as int));
}

proof fn walk_chunk_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    reveal_with_fuel(crate::layout::descriptor_fields, 4);
    let list = Layout::List { element: &Layout::Leaf(crate::Leaf::Bool) };
    let bytes = seq![2u8, 0, 0, 0, 1, 7];
    assert(crate::layout::descriptor_valid(list, MAX_DEPTH as nat));
    assert(crate::list_layout::header_shape(bytes, list)) by {
        reveal(crate::list_layout::header_shape);
        reveal(crate::layout_head::head_spec);
        assert(width_spec(Layout::Leaf(crate::Leaf::Bool)) == Some(1u32));
        assert(words::le_word(bytes, 0) == 2);
    }
    walk_chunk_selection(bytes, list, 1, 1, 100);
    assert(bytes.subrange(5, 6) == seq![7u8]);
}

/// A local exact loan frames the complete account, including physical spare.
#[verifier::spinoff_prover]
proof fn walk_loan_frame(before: Seq<u8>, after: Seq<u8>, parent: Span, slot: Span)
    requires parent.start <= parent.end <= before.len(), slot.start <= slot.end <= parent.end - parent.start,
        overwrite_frame(before, after, parent),
        overwrite_frame(before.subrange(parent.start as int, parent.end as int),
            after.subrange(parent.start as int, parent.end as int), slot),
    ensures overwrite_frame(before, after, Span { start: (parent.start + slot.start) as u32, end: (parent.start + slot.end) as u32 }),
{
    assert forall|i: int| 0 <= i < before.len() && !(parent.start + slot.start <= i < parent.start + slot.end)
        implies #[trigger] after[i] == before[i] by {
        if parent.start <= i < parent.end {
            assert(after.subrange(parent.start as int, parent.end as int)[i - parent.start] == after[i]);
            assert(before.subrange(parent.start as int, parent.end as int)[i - parent.start] == before[i]);
        }
    }
}

/// Reacquisition transports contained offsets only when the live prefix survives.
#[verifier::spinoff_prover]
proof fn walk_relocation_transport(before: Seq<u8>, after: Seq<u8>, used: u32, span: Span)
    requires used <= before.len(), used <= after.len(), span.start <= span.end <= used,
        before.subrange(0, used as int) == after.subrange(0, used as int),
    ensures before.subrange(span.start as int, span.end as int) == after.subrange(span.start as int, span.end as int),
{
    assert forall|i: int| span.start <= i < span.end implies before[i] == after[i] by {
        assert(before.subrange(0, used as int)[i] == before[i]);
        assert(after.subrange(0, used as int)[i] == after[i]);
    }
    assert(before.subrange(span.start as int, span.end as int) =~= after.subrange(span.start as int, span.end as int));
}

/// Exact overwrites compose with the existing generic plan and root theorem.
#[verifier::spinoff_prover]
proof fn walk_overwrite_preserves(before: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), subtree_plan(plan), plan.delta == 0,
        overwrite_frame(before, after, plan.region),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        validation::schema_valid(after.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
            plan.trail.selected, (MAX_DEPTH - plan.trail.depth) as nat, true),
    ensures replacement_result(before, after, after, plan),
        validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    fixed_overwrite_refines(before, after, after, plan);
    subtree_replace_preserves(before, after, after, plan);
}

proof fn walk_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    reveal_with_fuel(crate::layout::descriptor_fields, 4);

    let list = Layout::List { element: &Layout::Leaf(crate::Leaf::Bool) };
    let root = Layout::Struct { fixed_bytes: 1,
        fixed: vstd::array::spec_array_as_slice(&[crate::FixedField { declaration: 0, offset: 0, layout: &Layout::Leaf(crate::Leaf::Unsigned(1)) }]),
        variable: vstd::array::spec_array_as_slice(&[crate::VariableField { declaration: 1, layout: &list }]) };
    let local = seq![42u8, 2, 0, 0, 0, 2, 0];
    assert(crate::layout::descriptor_valid(list, (MAX_DEPTH - 1) as nat));
    assert(crate::layout::descriptor_valid(root, MAX_DEPTH as nat)) by {
        reveal_with_fuel(crate::layout::descriptor_fields, 8);
        reveal_with_fuel(crate::layout::descriptor_valid, 8);
    }
    walk_tail_selector(local, root, 1);
    reveal(tail_route);
    assert(tail_route(seq![root, list], seq![0u32, 1], seq![1u16]));
    walk_tail_path(local, seq![root, list], seq![0u32, 1], seq![1u16]);
}

proof fn walk_stride_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    reveal_with_fuel(crate::layout::descriptor_fields, 4);

    let list = Layout::List { element: &Layout::Leaf(crate::Leaf::Bool) };
    assert(crate::list_layout::header_shape(seq![2u8, 0, 0, 0, 2, 0], list)) by {
        reveal(crate::list_layout::header_shape);
        reveal(crate::layout_head::head_spec);
        assert(width_spec(Layout::Leaf(crate::Leaf::Bool)) == Some(1u32));
        assert(words::le_word(seq![2u8, 0, 0, 0, 2, 0], 0) == 2);
    }
    walk_stride_selection(seq![2u8, 0, 0, 0, 2, 0], list, 1, 1);
}

proof fn walk_frame_witness()
{
    walk_loan_frame(seq![9u8, 10, 11, 12], seq![9u8, 20, 11, 12],
        Span { start: 1, end: 3 }, Span { start: 0, end: 1 });
}

proof fn walk_transport_witness()
{
    let before = seq![9u8, 10, 11];
    let after = before + seq![0u8];
    assert(before.subrange(0, 3) =~= after.subrange(0, 3));
    walk_relocation_transport(before, after, 3, Span { start: 1, end: 3 });
}

proof fn walk_overwrite_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    reveal_with_fuel(crate::layout::descriptor_fields, 4);

    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    let before = witnesses::account(seq![7u8]);
    let after = before.update(12, 9u8);
    let root = witnesses::BYTE;
    let plan = witnesses::replacement(before, root, 1);
    walk_overwrite_preserves(before, after, plan);
}

proof fn walk_projection_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    let root = Layout::Struct { fixed_bytes: 2, fixed: &[
        crate::FixedField { declaration: 0, offset: 0, layout: &Layout::Leaf(crate::Leaf::Unsigned(1)) },
        crate::FixedField { declaration: 1, offset: 1, layout: &Layout::Leaf(crate::Leaf::Bool) }], variable: &[] };
    walk_fixed_projection(seq![42u8, 2], root, 1, 1, MAX_DEPTH as nat);
}

/// Exact slot admission agrees with generic index selection, even on lazy leaves.
#[verifier::spinoff_prover]
proof fn walk_slot_selection(bytes: Seq<u8>, layout: Layout, width: u32, index: u32) -> (s: Selection)
    requires crate::layout::descriptor_valid(layout, MAX_DEPTH as nat),
        crate::list_layout::header_shape(bytes, layout),
        match layout.head() { LayoutHead::List { element } => width_spec(element) == Some(width), _ => false },
        0 < width, index < words::le_word(bytes, 0),
    ensures selection_exact(s, bytes, layout, Step::Index(index)),
        s.span == (Span { start: (4 + index * width) as u32, end: (4 + (index + 1) * width) as u32 }),
        s.run == empty_run(), s.ordinal == index,
        s.span.end - s.span.start == width,
{
    walk_stride_selection(bytes, layout, width, index as int);
    assert((index as int + 1) * width == index as int * width + width) by(nonlinear_arith);
    assert(crate::edit_layout::header_valid(bytes, layout));
    let span = validation::child_span(bytes, layout, index as int);
    Selection { payload_len: payload_extent(bytes, layout) as u32, span,
        layout: validation::child_layout(layout, index as int), ordinal: index, run: empty_run() }
}

/// A structurally admitted fixed region already has its certificate's width.
/// Leaf domains remain lazy, so corrupt fixed leaves can still be repaired.
#[verifier::spinoff_prover]
proof fn walk_region_width(bytes: Seq<u8>, layout: Layout, width: u32)
    requires validation::local_valid(bytes, layout, false), width_spec(layout) == Some(width),
    ensures bytes.len() == width,
{
    match layout.head() {
        LayoutHead::Struct { variable, .. } => {
            assert(variable.len() == 0);
        }
        _ => { }
    }
}

proof fn walk_region_width_witness()
{
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    walk_region_width(seq![2u8], Layout::Leaf(crate::Leaf::Bool), 1);
}

/// A field loan's local frame composes through its slot to the complete root.
#[verifier::spinoff_prover]
proof fn walk_slot_root_preserves(before: Seq<u8>, after: Seq<u8>, plan: Plan, parent: Span, slot: Span)
    requires replacement_checked(before, plan), subtree_plan(plan), plan.delta == 0,
        parent.start <= parent.end <= before.len(), slot.start <= slot.end <= parent.end - parent.start,
        plan.region == (Span { start: (parent.start + slot.start) as u32, end: (parent.start + slot.end) as u32 }),
        overwrite_frame(before, after, parent),
        overwrite_frame(before.subrange(parent.start as int, parent.end as int),
            after.subrange(parent.start as int, parent.end as int), slot),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        validation::schema_valid(after.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
            plan.trail.selected, (MAX_DEPTH - plan.trail.depth) as nat, true),
    ensures replacement_result(before, after, after, plan),
        overwrite_frame(before, after, plan.region),
        validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    walk_loan_frame(before, after, parent, slot);
    walk_overwrite_preserves(before, after, plan);
}

proof fn walk_slot_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    let root = witnesses::FIXED_LIST;
    let local = seq![2u8,0,0,0,7,8];
    let before = witnesses::account(local);
    let after = before.update(17, 9u8);
    let selection = walk_slot_selection(local, root, 1, 1);
    let path = InlinePath { steps: [Step::Index(1); MAX_DEPTH], len: 1, overflow: false };
    let trail = admission::direct_trail(before, root, path, 18, selection);
    let plan = admission::subtree_plan_for(before, path, trail, 1, before.len() as u32);
    walk_slot_root_preserves(before, after, plan, Span { start: 12, end: 18 }, Span { start: 5, end: 6 });
}

/// Splits an admitted interleaved entry, keeping key bytes shared and immutable.
///
/// # Panics
/// Panics if the supplied key and value widths exceed the remaining bytes.
#[doc(hidden)]
#[inline(always)]
pub fn split_fixed_map_entry(bytes: &mut [u8], key_width: usize, value_width: usize)
    -> (result: (&[u8], &mut [u8], &mut [u8]))
    requires key_width + value_width <= old(bytes).len(),
    ensures result.0.len() == key_width, result.1.len() == value_width,
        result.2.len() == old(bytes).len() - key_width - value_width,
        result.0@ == old(bytes)@.subrange(0, key_width as int),
        result.1@ == old(bytes)@.subrange(key_width as int, key_width + value_width),
        result.2@ == old(bytes)@.subrange(key_width + value_width, old(bytes).len() as int),
        final(bytes)@ == result.0@ + final(result.1)@ + final(result.2)@,
    no_unwind
{
    let (key, rest) = bytes.split_at_mut(key_width);
    let (value, tail) = rest.split_at_mut(value_width);
    (key, value, tail)
}

/// Fixed-map chunks select exactly the generic key/value regions and bases.
/// No key order or value-domain assumption is needed for immutable iteration.
#[verifier::spinoff_prover]
proof fn walk_map_selection(bytes: Seq<u8>, layout: Layout, width: u32, index: u32, base: u32) -> (s: Selection)
    requires crate::layout::descriptor_valid(layout, MAX_DEPTH as nat),
        crate::map_layout::map_shape(bytes, layout),
        crate::map_layout::value_width(layout) == Some(width),
        index < words::le_word(bytes, 0), base + bytes.len() <= u32::MAX,
    ensures selection_exact(s, bytes, layout, Step::Index(index)),
        s.run == empty_run(), s.ordinal == index,
        s.span == (Span { start: (4 + index * crate::map_layout::entry_stride(layout)
            + crate::map_layout::key_width(layout)) as u32,
            end: (4 + (index + 1) * crate::map_layout::entry_stride(layout)) as u32 }),
        s.span.end - s.span.start == width,
        s.span.start as int == 4 + index * crate::map_layout::entry_stride(layout) + crate::map_layout::key_width(layout),
        s.span.end as int == 4 + (index as int + 1) * crate::map_layout::entry_stride(layout),
        base + s.span.end <= u32::MAX,
        {
            let stride = crate::map_layout::entry_stride(layout);
            let key = crate::map_layout::key_width(layout);
            let remaining = bytes.subrange(4 + index * stride, bytes.len() as int);
            &&& 0 < stride <= remaining.len()
            &&& remaining.len() == (words::le_word(bytes, 0) - index) * stride
            &&& remaining.subrange(0, key as int) == crate::search::encoded_key(bytes, layout, index as int)
            &&& remaining.subrange(key as int, stride) == bytes.subrange(s.span.start as int, s.span.end as int)
            &&& base + 4 + index * stride + key == base + s.span.start
        },
{
    let count = words::le_word(bytes, 0);
    let stride = crate::map_layout::entry_stride(layout);
    let key = crate::map_layout::key_width(layout);
    assert(key > 0);
    assert(stride == key + width);
    assert(0 <= index * stride < (index + 1) * stride <= count * stride) by(nonlinear_arith)
        requires 0 <= index < count, stride > 0;
    assert((count - index) * stride == count * stride - index * stride) by(nonlinear_arith);
    let span = validation::child_span(bytes, layout, index as int);
    let start = 4 + index * stride;
    let end = 4 + (index + 1) * stride;
    assert(end == start + stride) by(nonlinear_arith) requires start == 4 + index * stride, end == 4 + (index + 1) * stride;
    assert(span.start == start + key && span.end == end);
    let remaining = bytes.subrange(start, bytes.len() as int);
    assert(remaining.subrange(0, key as int) =~= crate::search::encoded_key(bytes, layout, index as int));
    assert(remaining.subrange(key as int, stride) =~= bytes.subrange(span.start as int, span.end as int));
    assert(crate::edit_layout::header_valid(bytes, layout));
    Selection { payload_len: payload_extent(bytes, layout) as u32, span,
        layout: validation::child_layout(layout, index as int), ordinal: index, run: empty_run() }
}

/// A fixed value loan frames every key, the header and every sibling value.
#[verifier::spinoff_prover]
proof fn map_value_frame(before: Seq<u8>, after: Seq<u8>, layout: Layout, width: u32, index: u32)
    requires crate::layout::descriptor_valid(layout, MAX_DEPTH as nat),
        crate::map_layout::map_shape(before, layout),
        crate::map_layout::value_width(layout) == Some(width),
        index < words::le_word(before, 0),
        overwrite_frame(before, after, validation::child_span(before, layout, index as int)),
    ensures words::le_word(after, 0) == words::le_word(before, 0),
        crate::map_layout::map_shape(after, layout),
        forall|i: int| 0 <= i < words::le_word(before, 0) ==>
            #[trigger] crate::search::encoded_key(after, layout, i) == crate::search::encoded_key(before, layout, i),
        crate::preservation::siblings_same(before, after, layout, index as int),
{
    let selection = walk_map_selection(before, layout, width, index, 0);
    assert(crate::map_layout::key_width(layout) > 0);
    assert(crate::map_layout::entry_stride(layout) == crate::map_layout::key_width(layout) + width);
    assert(before.len() == 4 + words::le_word(before, 0) * crate::map_layout::entry_stride(layout));
    assert(selection.span.end as int == 4 + (index as int + 1) * crate::map_layout::entry_stride(layout));
    let stride = crate::map_layout::entry_stride(layout);
    let key = crate::map_layout::key_width(layout);
    assert(words::le_word(after, 0) == words::le_word(before, 0));
    assert(crate::map_layout::map_shape(after, layout));
    assert forall|i: int| 0 <= i < words::le_word(before, 0) implies
        #[trigger] crate::search::encoded_key(after, layout, i) == crate::search::encoded_key(before, layout, i) by {
        let start = 4 + i * stride;
        assert(start + key <= before.len()) by(nonlinear_arith)
            requires i >= 0, i < words::le_word(before, 0), 0 <= key <= stride, stride > 0, start == 4 + i * stride,
                before.len() == 4 + words::le_word(before, 0) * stride;
        if i <= index {
            assert(start + key <= selection.span.start) by(nonlinear_arith)
                requires i <= index, stride > 0, start == 4 + i * stride, selection.span.start == 4 + index * stride + key;
        } else {
            assert(start >= selection.span.end) by(nonlinear_arith)
                requires i > index, stride > 0, start == 4 + i * stride, selection.span.end == 4 + (index + 1) * stride;
        }
        assert(after.subrange(start, start + key) =~= before.subrange(start, start + key));
    }
    assert forall|i: int| 0 <= i < words::le_word(before, 0) && i != index implies
        #[trigger] crate::preservation::sibling_same(before, after, layout, i) by {
        let start = 4 + i * stride + key;
        let end = 4 + (i + 1) * stride;
        let old_selection = walk_map_selection(before, layout, width, i as u32, 0);
        let new_selection = walk_map_selection(after, layout, width, i as u32, 0);
        assert(old_selection.span.start == start && old_selection.span.end == end);
        assert(new_selection.span == old_selection.span);
        assert(4 <= start <= end <= before.len()) by(nonlinear_arith)
            requires i >= 0, i < words::le_word(before, 0), stride == key + width,
                start == 4 + i * stride + key, end == 4 + (i + 1) * stride,
                key > 0, width >= 0, before.len() == 4 + words::le_word(before, 0) * stride;
        if i < index {
            assert(end <= selection.span.start) by(nonlinear_arith)
                requires i < index, stride > 0, key > 0, end == 4 + (i + 1) * stride, selection.span.start == 4 + index * stride + key;
        } else {
            assert(start >= selection.span.end) by(nonlinear_arith)
                requires i > index, stride > 0, key > 0, start == 4 + i * stride + key, selection.span.end == 4 + (index + 1) * stride;
        }
        assert(after.subrange(start, end) =~= before.subrange(start, end));
    }
}

/// Unchanged encoded keys transport canonical domains and pairwise strict order.
#[verifier::spinoff_prover]
proof fn map_order_transport(before: Seq<u8>, after: Seq<u8>, layout: Layout, count: int)
    requires forall|i: int| 0 <= i < count ==> #[trigger] crate::search::encoded_key(after, layout, i)
        == crate::search::encoded_key(before, layout, i),
    ensures crate::search::keys_ordered(after, layout, count) == crate::search::keys_ordered(before, layout, count),
        crate::search::keys_canonical(after, layout, count) == crate::search::keys_canonical(before, layout, count),
{
    nested::keys_frame(before, after, layout, count);
}

/// Individual valid value overwrites preserve the complete fixed-map parent.
#[verifier::spinoff_prover]
proof fn walk_map_parent_preserves(before: Seq<u8>, after: Seq<u8>, layout: Layout, width: u32,
    index: u32, budget: nat)
    requires validation::schema_valid(before, layout, budget, true),
        crate::map_layout::value_width(layout) == Some(width),
        index < words::le_word(before, 0),
        overwrite_frame(before, after, validation::child_span(before, layout, index as int)),
        validation::schema_valid(after.subrange(validation::child_span(before, layout, index as int).start as int,
            validation::child_span(before, layout, index as int).end as int),
            validation::child_layout(layout, index as int), (budget - 1) as nat, true),
    ensures validation::schema_valid(after, layout, budget, true),
        crate::search::keys_ordered(after, layout, words::le_word(before, 0)),
        crate::search::keys_canonical(after, layout, words::le_word(before, 0)),
{
    reveal(validation::schema_valid);
    map_value_frame(before, after, layout, width, index);
    map_order_transport(before, after, layout, words::le_word(before, 0));
    assert(validation::local_valid(after, layout, true));
    assert(validation::child_valid(after, layout, index as int, (budget - 1) as nat, true)) by {
        reveal(validation::child_valid);
    }
    crate::preservation::parent_join(before, after, layout, index as int, budget, true);
}

proof fn walk_map_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    let layout = Layout::Map { key: crate::KeyLayout { width: 1, domain: crate::KeyDomain::Raw },
        value: &Layout::Leaf(crate::Leaf::Bool) };
    let before = seq![2u8,0,0,0,1,0,3,1];
    let after = before.update(7, 0u8);
    assert(crate::search::same_prefix(seq![1u8], seq![3u8], 0));
    assert(crate::search::lex_less(seq![1u8], seq![3u8]));
    assert(crate::search::encoded_key(before, layout, 0) =~= seq![1u8]);
    assert(crate::search::encoded_key(before, layout, 1) =~= seq![3u8]);
    assert(crate::search::keys_ordered(before, layout, 2));
    assert(validation::schema_valid(before, layout, MAX_DEPTH as nat, true));
    let selected = walk_map_selection(before, layout, 1, 1, 12);
    map_value_frame(before, after, layout, 1, 1);
    map_order_transport(before, after, layout, 2);
    walk_map_parent_preserves(before, after, layout, 1, 1, MAX_DEPTH as nat);
    assert(selected.span == (Span { start: 7, end: 8 }));
    let account = witnesses::account(before);
    assert(account.subrange(12,20) =~= before);
    let ingress = witnesses::replacement(account,layout,8);
    walk_map_ingress_keys(account,ingress);
    let path = InlinePath { steps: [Step::Index(1); MAX_DEPTH], len: 1, overflow: false };
    let trail = admission::direct_trail(account, layout, path, 20, selected);
    let plan = admission::subtree_plan_for(account, path, trail, 1, account.len() as u32);
    assert(account.subrange(12, 20) =~= before);
    assert(validation::schema_valid(account.subrange(12, 20), layout, MAX_DEPTH as nat, true));
    walk_slot_root_preserves(account, account.update(19, 0u8), plan,
        Span { start: 12, end: 20 }, Span { start: 7, end: 8 });
}

/// A contained checked map selection inherits key evidence from unchanged ingress.
/// This is usable only while the affine root token still owns those exact bytes.
#[verifier::spinoff_prover]
proof fn walk_map_ingress_keys(before: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.trail.selected.head(), LayoutHead::Map { .. }),
    ensures {
        let node = node_bytes(before, plan.trail.nodes[plan.trail.depth as int]);
        &&& crate::map_layout::map_shape(node, plan.trail.selected)
        &&& crate::search::keys_ordered(node, plan.trail.selected, words::le_word(node, 0))
        &&& crate::search::keys_canonical(node, plan.trail.selected, words::le_word(node, 0))
    },
{
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    reveal(validation::schema_valid);
}

}
