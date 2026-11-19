//! Directory-first indexed-list splices and sequence refinement.

#![cfg_attr(verus_only, allow(missing_docs))]

use super::*;
#[cfg(verus_only)]
use crate::list_layout;
use crate::LayoutHead;
#[cfg(verus_only)]
use indexed_insert::{boundary, boundary_step, items};

#[path = "indexed_splice_pass.rs"]
pub(super) mod checked;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct IndexedSplice {
    pub(super) operation: ListEdit,
    pub(super) count: u32,
    pub(super) new_count: u32,
    pub(super) first: u32,
    pub(super) last: u32,
    pub(super) payload: u32,
    pub(super) offset: u32,
    pub(super) end_offset: u32,
    pub(super) table_shrink: u32,
    pub(super) input_len: u32,
}

pub(super) open spec fn words_for(count: int) -> int { if count == 0 { 0int } else { count - 1 } }
pub(super) open spec fn inserted(c: IndexedSplice) -> int { if matches!(c.operation, ListEdit::Replace(_)) { 1int } else { 0int } }
pub(super) open spec fn payload_delta(c: IndexedSplice) -> int { c.input_len as int - (c.end_offset - c.offset) }

pub(super) open spec fn parameters(bytes: Seq<u8>, region: Span, c: IndexedSplice) -> bool {
    let b = region.start as int;
    let e = region.end as int;
    let indices = list_indices(c.operation, c.count);
    &&& !matches!(c.operation, ListEdit::Insert(_))
    &&& b + 4 <= c.payload <= e <= bytes.len() <= u32::MAX
    &&& c.count == words::le_word(bytes, b)
    &&& c.payload == b + 4 + 4 * words_for(c.count as int)
    &&& 0 <= indices.0 <= indices.1 <= c.count
    &&& c.first == indices.0 && c.last == indices.1 && c.new_count == indices.2
    &&& c.new_count == c.count - (c.last - c.first) + inserted(c)
    &&& c.new_count <= c.count
    &&& (inserted(c) == 0 ==> c.input_len == 0)
    &&& c.offset == boundary(bytes, b, c.payload as int, e, c.count as int, c.first as int)
    &&& c.end_offset == boundary(bytes, b, c.payload as int, e, c.count as int, c.last as int)
    &&& 0 <= c.offset <= c.end_offset <= e - c.payload
    &&& c.table_shrink == 4 * (words_for(c.count as int) - words_for(c.new_count as int))
    &&& (c.count == 0 ==> e == c.payload)
    &&& forall|k: int| 0 <= k < c.count ==> #[trigger] boundary_step(bytes, b, c.payload as int, e, c.count as int, k)
    &&& forall|k: int| 0 <= k <= c.count ==> 0 <= #[trigger] boundary(bytes, b, c.payload as int, e, c.count as int, k) <= e - c.payload
}

pub(super) open spec fn geometry(bytes: Seq<u8>, plan: Plan) -> bool {
    let c = plan.splice.unwrap();
    &&& plan.splice.is_some() && plan.indexed.is_none() && plan.list.is_none()
    &&& parameters(bytes, plan.region, c)
    &&& 12 <= plan.region.start <= plan.region.end <= plan.old_used <= bytes.len() <= u32::MAX
    &&& plan.new_used == plan.old_used + plan.delta
    &&& plan.required == if plan.new_used > plan.old_used { plan.new_used } else { plan.old_used }
    &&& plan.delta == payload_delta(c) - c.table_shrink
    &&& plan.replacement_len == plan.region.end - plan.region.start + plan.delta
    &&& plan.region.start + 4 <= plan.region.end + plan.delta <= plan.new_used
}

pub(super) open spec fn splice_checked(bytes: Seq<u8>, plan: Plan) -> bool {
    plan.splice.is_some() ==> geometry(bytes, plan)
}

proof fn checked_geometry(bytes: Seq<u8>, plan: Plan)
    requires replacement_checked(bytes, plan), plan.splice.is_some(),
    ensures geometry(bytes, plan),
{ reveal(replacement_checked); }

pub(super) fn same_splice(a: Option<IndexedSplice>, b: Option<IndexedSplice>) -> (result: bool)
    ensures result == (a == b),
{
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let operation = match (a.operation, b.operation) {
                (ListEdit::Remove(i), ListEdit::Remove(j)) | (ListEdit::Replace(i), ListEdit::Replace(j))
                | (ListEdit::Truncate(i), ListEdit::Truncate(j)) | (ListEdit::Insert(i), ListEdit::Insert(j)) => i == j,
                (ListEdit::Pop, ListEdit::Pop) | (ListEdit::Clear, ListEdit::Clear) => true,
                _ => false,
            };
            operation && a.count == b.count && a.new_count == b.new_count && a.first == b.first && a.last == b.last
                && a.payload == b.payload && a.offset == b.offset && a.end_offset == b.end_offset
                && a.table_shrink == b.table_shrink && a.input_len == b.input_len
        },
        _ => false,
    }
}

proof fn word_in_slice(bytes: Seq<u8>, start: int, end: int, at: int)
    requires 0 <= start <= end <= bytes.len(), 0 <= at, at + 4 <= end - start,
    ensures words::le_word(bytes.subrange(start, end), at) == words::le_word(bytes, start + at),
        0 <= words::le_word(bytes, start + at) <= u32::MAX,
{ }

pub(super) proof fn boundary_order(bytes: Seq<u8>, region: Span, c: IndexedSplice, i: int, j: int)
    requires parameters(bytes, region, c), 0 <= i <= j <= c.count,
    ensures boundary(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, i)
        <= boundary(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, j),
    decreases j - i,
{
    if i < j {
        boundary_order(bytes, region, c, i, j - 1);
        assert(boundary_step(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, j - 1));
    }
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
fn prepare_splice(bytes: &[u8], region: Span, layout: &Layout, operation: ListEdit, input_len: u32) -> (result: Result<IndexedSplice>)
    requires region.start <= region.end <= bytes.len() <= u32::MAX,
    ensures result.is_ok() ==> parameters(bytes@, region, result.unwrap())
        && result.unwrap().input_len == input_len && result.unwrap().operation == operation,
{
    hide(words::le_word);
    match layout.head() {
        LayoutHead::List { element } => if fixed_width(&element).is_some() { return Err(ErrorKind::InvalidSchema.into()); },
        _ => return Err(ErrorKind::InvalidSchema.into()),
    }
    let local = &bytes[region.start as usize..region.end as usize];
    let parts = crate::partition_list(local, layout).map_err(|e: Error| e.rebase(region.start))?;
    proof { word_in_slice(bytes@, region.start as int, region.end as int, 0); }
    let count = parts.count;
    let (first, last, new_count) = match operation {
        ListEdit::Insert(_) => return Err(ErrorKind::InvalidSchema.into()),
        ListEdit::Remove(i) | ListEdit::Replace(i) => {
            if i >= count { return Err(ErrorKind::IndexOutOfBounds.into()); }
            (i, i + 1, if matches!(operation, ListEdit::Remove(_)) { count - 1 } else { count })
        },
        ListEdit::Pop => (if count == 0 { 0u32 } else { count - 1 }, count, if count == 0 { 0u32 } else { count - 1 }),
        ListEdit::Truncate(n) => { let kept = if n < count { n } else { count }; (kept, count, kept) },
        ListEdit::Clear => (0u32, count, 0u32),
    };
    if !matches!(operation, ListEdit::Replace(_)) && input_len != 0 { return Err(ErrorKind::InvalidBoundary.into()); }
    let payload = region.start + parts.payload.start;
    proof {
        assert forall|k: int| 0 <= k <= count implies
            0 <= #[trigger] boundary(bytes@, region.start as int, payload as int, region.end as int, count as int, k)
                <= region.end - payload by {
            if 0 < k < count {
                assert(0 <= k - 1 < count - 1);
                assert(words::le_word(local@, 4 + 4 * (k - 1)) <= local.len() - parts.payload.start);
                word_in_slice(bytes@, region.start as int, region.end as int, 4 * k);
            }
        }
        assert forall|k: int| 0 <= k < count implies
            #[trigger] boundary_step(bytes@, region.start as int, payload as int, region.end as int, count as int, k) by {
            if 0 < k < count - 1 {
                assert(words::le_word(local@, 4 + 4 * k) >= words::le_word(local@, 4 + 4 * (k - 1)));
                word_in_slice(bytes@, region.start as int, region.end as int, 4 * k);
                word_in_slice(bytes@, region.start as int, region.end as int, 4 * (k + 1));
            }
        }
        if 0 < first < count { word_in_slice(bytes@, region.start as int, region.end as int, 4 * first); }
        if 0 < last < count { word_in_slice(bytes@, region.start as int, region.end as int, 4 * last); }
    }
    let offset = if first == 0 { 0u32 } else if first == count { region.end - payload } else { read_u32(local, 4 * first)? };
    let end_offset = if last == 0 { 0u32 } else if last == count { region.end - payload } else { read_u32(local, 4 * last)? };
    let old_words = if count == 0 { 0u32 } else { count - 1 };
    let new_words = if new_count == 0 { 0u32 } else { new_count - 1 };
    let c = IndexedSplice { operation, count, new_count, first, last, payload, offset, end_offset,
        table_shrink: 4 * (old_words - new_words), input_len };
    proof {
        // The partition establishes adjacent order; this loop-free selection
        // crosses at most one element unless it removes a terminal suffix.
        if first + 1 == last && last < count {
            assert(boundary_step(bytes@, region.start as int, payload as int, region.end as int, count as int, first as int));
        }
    }
    Ok(c)
}

#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
pub(super) fn plan_splice(bytes: &[u8], used: u32, path: &InlinePath,
    trail: Trail, operation: ListEdit, input_len: u32, limit: u32) -> (result: Result<Plan<'static>>)
    requires trail_valid(bytes@, trail.root, *path, trail, used), bytes.len() <= u32::MAX,
        12 <= used <= bytes.len() <= limit, words::le_word(bytes@, 8) == used,
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used, trail.root, *path, input_len, limit)
        && planned_list_for(result.unwrap(), operation),
{
    proof { reveal(subtree_plan); reveal(planned_for); reveal(planned_list_for); reveal(input_size); }
    let _layout = trail.root;
    let node = trail.nodes[trail.depth];
    let c = prepare_splice(bytes, node.region, &trail.selected, operation, input_len)?;
    let delta = input_len as i64 - (c.end_offset - c.offset) as i64 - c.table_shrink as i64;
    let new_len = checked_delta(node.region.end - node.region.start, delta)?;
    let mut result = plan_in_trail(bytes, used, path, &trail, new_len, limit);
    if let Ok(plan) = &mut result {
        if plan.region.start != node.region.start || plan.region.end != node.region.end {
            return Err(ErrorKind::InvalidPlan.into());
        }
        proof { replacement_extent(bytes@, *plan); reveal(with_splice); }
        assert(splice_checked(bytes@, with_splice(*plan, c)));
        attach_splice(plan, c, Ghost(bytes@));
    }
    result
}

pub(super) open spec fn source_boundary(c: IndexedSplice, j: int) -> int {
    if j < c.first { j + 1 } else { j + 1 + c.last - c.first - inserted(c) }
}

pub(super) open spec fn new_end(original: Seq<u8>, plan: Plan, j: int) -> int {
    let c = plan.splice.unwrap();
    boundary(original, plan.region.start as int, c.payload as int, plan.region.end as int,
        c.count as int, source_boundary(c, j)) + if j < c.first { 0int } else { payload_delta(c) }
}

pub(super) open spec fn directory_done(original: Seq<u8>, after: Seq<u8>, plan: Plan, done: int) -> bool {
    forall|j: int| 0 <= j < done ==> #[trigger] words::le_word(after, plan.region.start + 4 + 4 * j) == new_end(original, plan, j)
}

spec fn directory_frame(before: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.splice.unwrap();
    before.len() == after.len() && forall|p: int| 0 <= p < before.len()
        && !(plan.region.start + 4 <= p < c.payload - c.table_shrink) ==> after[p] == before[p]
}

#[verifier::spinoff_prover]
fn compact_directory(bytes: &mut [u8], plan: &Plan)
    requires replacement_checked(old(bytes)@, *plan), plan.splice.is_some(), plan.required <= old(bytes).len(),
    ensures directory_done(old(bytes)@, final(bytes)@, *plan, words_for(plan.splice.unwrap().new_count as int)),
        directory_frame(old(bytes)@, final(bytes)@, *plan),
    no_unwind
{
    let ghost original = bytes@;
    proof { checked_geometry(original, *plan); }
    let c = match plan.splice { Some(c) => c, None => panic!("checked edit plan") };
    let stored = if c.new_count == 0 { 0u32 } else { c.new_count - 1 };
    let mut j = c.first;
    // Earlier ends already have their final values. Later sources stay ahead
    // of each destination, so the ascending stream also compacts the table.
    proof {
        assert forall|k: int| 0 <= k < c.first && k < stored implies
            #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * k) == new_end(original, *plan, k) by { }
    }
    while j < stored
        invariant c.first <= j <= c.count, stored == words_for(c.new_count as int),
            geometry(original, *plan), c == plan.splice.unwrap(),
            plan.required <= bytes.len(), directory_frame(original, bytes@, *plan),
            directory_done(original, bytes@, *plan, if j < stored { j as int } else { stored as int }),
            forall|p: int| plan.region.start + 4 + 4 * j <= p < original.len() ==> bytes@[p] == original[p],
        decreases if j < stored { stored - j } else { 0int },
    {
        let source = j + (c.last - c.first - if matches!(c.operation, ListEdit::Replace(_)) { 1u32 } else { 0u32 });
        let src = plan.region.start + 4 + 4 * source;
        let at = plan.region.start + 4 + 4 * j;
        let old_end = crate::words::read_contained_word(bytes, (src) as usize);
        proof {
            assert(words::le_word(bytes@, src as int) == words::le_word(original, src as int));
            boundary_order(original, plan.region, c, c.last as int, source as int + 1);
            assert(c.end_offset <= old_end <= plan.region.end - c.payload);
        }
        let value = (old_end as i64 + c.input_len as i64 - (c.end_offset - c.offset) as i64) as u32;
        assert(value == new_end(original, *plan, j as int));
        let ghost previous = bytes@;
        if value != crate::words::read_contained_word(bytes, (at) as usize) { crate::words::write_contained_word(bytes, (at) as usize, value); }
        proof {
            assert(words::le_word(bytes@, at as int) == value);
            assert forall|k: int| 0 <= k < j implies
                #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * k) == new_end(original, *plan, k) by {
                let other = plan.region.start + 4 + 4 * k;
                assert forall|p: int| other <= p < other + 4 implies bytes@[p] == previous[p] by { }
                assert(words::le_word(bytes@, other) == words::le_word(previous, other));
            }
        }
        j += 1;
    }
}

pub(super) open spec fn payload_preserved(original: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.splice.unwrap();
    let q = c.payload - c.table_shrink;
    let at = q + c.offset;
    &&& original.len() == after.len()
    &&& forall|p: int| q <= p < at ==> #[trigger] after[p] == original[p + c.table_shrink]
    &&& forall|p: int| at + c.input_len <= p < plan.new_used ==> #[trigger] after[p] == original[p - plan.delta]
}

pub(super) open spec fn movement(original: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.splice.unwrap();
    &&& plan.required <= after.len()
    &&& payload_frame(original, after, plan)
    &&& payload_preserved(original, after, plan)
    &&& words::le_word(after, plan.region.start as int) == c.count
    &&& directory_done(original, after, plan, words_for(c.new_count as int))
}

#[verifier::spinoff_prover]
pub(super) fn move_splice(bytes: &mut [u8], plan: &Plan)
    requires replacement_checked(old(bytes)@, *plan), plan.splice.is_some(), plan.required <= old(bytes).len(),
    ensures movement(old(bytes)@, final(bytes)@, *plan),
    no_unwind
{
    let ghost original = bytes@;
    proof { checked_geometry(original, *plan); reveal(target); reveal(payload_frame); }
    let c = match plan.splice { Some(c) => c, None => panic!("checked edit plan") };
    compact_directory(bytes, plan);
    let ghost directory = bytes@;
    // The old trailing table words have been consumed before the prefix can
    // overwrite them. Its destination cannot reach the later suffix source.
    if c.table_shrink != 0 && c.offset != 0 {
        match crate::move_bytes(bytes, Span { start: c.payload, end: c.payload + c.offset }, c.payload - c.table_shrink) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
    }
    let ghost prefix = bytes@;
    let split = c.payload + c.end_offset;
    if plan.delta != 0 && split < plan.old_used {
        match crate::move_bytes(bytes, Span { start: split, end: plan.old_used },
            c.payload - c.table_shrink + c.offset + c.input_len) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
    }
    proof {
        assert forall|j: int| 0 <= j < words_for(c.new_count as int) implies
            #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * j) == new_end(original, *plan, j) by {
            let at = plan.region.start + 4 + 4 * j;
            assert(directory_done(original, directory, *plan, words_for(c.new_count as int)));
            assert forall|k: int| at <= k < at + 4 implies bytes@[k] == directory[k] by { }
            assert(words::le_word(bytes@, at) == words::le_word(directory, at));
        }
    }
}

#[verifier::spinoff_prover]
pub(super) proof fn movement_framed(original: Seq<u8>, before: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), plan.splice.is_some(), movement(original, before, plan),
    ensures forall|after: Seq<u8>| after.len() == before.len()
        && (forall|p: int| 0 <= p < before.len() && !(destination_start(plan) <= p < destination_start(plan) + input_size(plan))
            ==> after[p] == before[p]) ==> #[trigger] movement(original, after, plan),
{
    checked_geometry(original, plan); reveal(target); reveal(input_size); reveal(destination_start); reveal(payload_frame);
    let c = plan.splice.unwrap();
    assert forall|after: Seq<u8>| after.len() == before.len()
        && (forall|p: int| 0 <= p < before.len() && !(destination_start(plan) <= p < destination_start(plan) + input_size(plan))
            ==> after[p] == before[p]) implies #[trigger] movement(original, after, plan) by {
        assert forall|j: int| 0 <= j < words_for(c.new_count as int) implies
            #[trigger] words::le_word(after, plan.region.start + 4 + 4 * j) == new_end(original, plan, j) by {
            let at = plan.region.start + 4 + 4 * j;
            assert(directory_done(original, before, plan, words_for(c.new_count as int)));
            assert forall|k: int| at <= k < at + 4 implies after[k] == before[k] by { }
            assert(words::le_word(after, at) == words::le_word(before, at));
        }
    }
}

pub(super) open spec fn splice_result(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.splice.unwrap();
    let at = destination_start(plan) as int;
    &&& original.len() == encoded.len()
    &&& payload_preserved(original, after, plan)
    &&& forall|p: int| 0 <= p < after.len() && (p < 8 || plan.required <= p
            || (12 <= p < plan.region.start && (plan.delta == 0 || !ancestor_changed(plan, plan.trail.depth as int, p))))
        ==> #[trigger] after[p] == original[p]
    &&& words::le_word(after, 8) == plan.new_used
    &&& words::le_word(after, plan.region.start as int) == c.new_count
    &&& directory_done(original, after, plan, words_for(c.new_count as int))
    &&& after.subrange(at, at + c.input_len) == encoded.subrange(at, at + c.input_len)
    &&& forall|p: int| plan.new_used <= p < plan.old_used ==> #[trigger] after[p] == 0
    &&& ancestors_delta(original, after, plan, plan.trail.depth as int)
}

#[verifier::spinoff_prover]
pub(super) fn finish_splice(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>)
    requires replacement_checked(original, *plan), plan.splice.is_some(), plan.required <= old(bytes).len(),
        movement(original, old(bytes)@, *plan),
    ensures splice_result(original, old(bytes)@, final(bytes)@, *plan),
    no_unwind
{
    proof { checked_geometry(original, *plan); reveal(target); reveal(destination_start);
        reveal(payload_frame); reveal(replacement_result); reveal(count_changed); }
    let c = match plan.splice { Some(c) => c, None => panic!("checked edit plan") };
    let ghost encoded = bytes@;
    if c.new_count != c.count { crate::words::write_contained_word(bytes, (plan.region.start) as usize, c.new_count); }
    let ghost counted = bytes@;
    finish_replacement(bytes, plan, Ghost(original));
    proof {
        assert forall|j: int| 0 <= j < words_for(c.new_count as int) implies
            #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * j) == new_end(original, *plan, j) by {
            let at = plan.region.start + 4 + 4 * j;
            assert(directory_done(original, encoded, *plan, words_for(c.new_count as int)));
            assert forall|k: int| at <= k < at + 4 implies bytes@[k] == encoded[k] by { }
            assert(words::le_word(bytes@, at) == words::le_word(encoded, at));
        }
        assert(payload_preserved(original, bytes@, *plan));
        assert(words::le_word(bytes@, plan.region.start as int) == c.new_count);
        let at = destination_start(*plan) as int;
        assert(bytes@.subrange(at, at + c.input_len) =~= encoded.subrange(at, at + c.input_len));
    }
}

pub(super) open spec fn sequence_splice(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.splice.unwrap();
    let b = plan.region.start as int;
    let old = items(original, b, c.payload as int, plan.region.end as int, c.count as int);
    let input = encoded.subrange(destination_start(plan) as int, destination_start(plan) + c.input_len);
    items(after, b, c.payload - c.table_shrink, plan.region.end + plan.delta, c.new_count as int)
        == edited_items(old, input, c.operation)
}

#[verifier::spinoff_prover]
proof fn resulting_boundaries(original: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires geometry(original, plan), original.len() == after.len(), plan.required <= original.len(),
        directory_done(original, after, plan, words_for(plan.splice.unwrap().new_count as int)),
    ensures forall|k: int| 0 <= k <= plan.splice.unwrap().new_count ==> #[trigger] boundary(after,
        plan.region.start as int, plan.splice.unwrap().payload - plan.splice.unwrap().table_shrink,
        plan.region.end + plan.delta, plan.splice.unwrap().new_count as int, k)
        == if k <= plan.splice.unwrap().first {
            boundary(original, plan.region.start as int, plan.splice.unwrap().payload as int, plan.region.end as int,
                plan.splice.unwrap().count as int, k)
        } else {
            boundary(original, plan.region.start as int, plan.splice.unwrap().payload as int, plan.region.end as int,
                plan.splice.unwrap().count as int, k + plan.splice.unwrap().last - plan.splice.unwrap().first - inserted(plan.splice.unwrap()))
                + payload_delta(plan.splice.unwrap())
        },
{
    let c = plan.splice.unwrap();
    let b = plan.region.start as int;
    let e = plan.region.end as int;
    let p = c.payload as int;
    let q = p - c.table_shrink;
    assert forall|k: int| 0 <= k <= c.new_count implies
        #[trigger] boundary(after, b, q, e + plan.delta, c.new_count as int, k)
            == if k <= c.first { boundary(original, b, p, e, c.count as int, k) }
                else { boundary(original, b, p, e, c.count as int, k + c.last - c.first - inserted(c)) + payload_delta(c) } by {
        if 0 < k < c.new_count {
            assert(directory_done(original, after, plan, words_for(c.new_count as int)));
            assert(words::le_word(after, plan.region.start + 4 + 4 * (k - 1)) == new_end(original, plan, k - 1));
            assert(words::le_word(after, b + 4 * k) == new_end(original, plan, k - 1));
        }
    }
}

#[verifier::spinoff_prover]
pub(super) proof fn sequence_refinement(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), plan.splice.is_some(),
        splice_result(original, encoded, after, plan), plan.required <= original.len(),
    ensures sequence_splice(original, encoded, after, plan),
{
    checked_geometry(original, plan); reveal(destination_start);
    let c = plan.splice.unwrap();
    let b = plan.region.start as int;
    let e = plan.region.end as int;
    let p = c.payload as int;
    let q = p - c.table_shrink;
    let at = q + c.offset;
    resulting_boundaries(original, after, plan);
    let old = items(original, b, p, e, c.count as int);
    let input = encoded.subrange(at, at + c.input_len);
    let actual = items(after, b, q, e + plan.delta, c.new_count as int);
    let expected = edited_items(old, input, c.operation);
    assert(expected.len() == c.new_count);
    assert forall|i: int| 0 <= i < c.new_count implies actual[i] == expected[i] by {
        let source = if i < c.first { i } else { i + c.last - c.first - inserted(c) };
        let start = q + boundary(after, b, q, e + plan.delta, c.new_count as int, i);
        let end = q + boundary(after, b, q, e + plan.delta, c.new_count as int, i + 1);
        if inserted(c) == 1 && i == c.first {
            assert(start == at && end == at + c.input_len);
        } else {
            assert(expected[i] == old[source]);
            let shift = if i < c.first { -(c.table_shrink as int) } else { plan.delta as int };
            let a = p + boundary(original, b, p, e, c.count as int, source);
            let z = p + boundary(original, b, p, e, c.count as int, source + 1);
            boundary_order(original, plan.region, c, source, source + 1);
            if i < c.first { boundary_order(original, plan.region, c, source + 1, c.first as int); }
            else { boundary_order(original, plan.region, c, c.last as int, source); }
            assert(start == a + shift && end == z + shift);
            assert(actual[i].len() == old[source].len());
            assert forall|k: int| 0 <= k < end - start implies actual[i][k] == old[source][k] by {
                assert(after[start + k] == original[start + k - shift]);
            }
            assert(actual[i] =~= old[source]);
        }
    }
    assert(actual =~= expected);
}

/// Indexed removal/replacement retains valid children and exact directory shape.
#[verifier::spinoff_prover]
pub(super) proof fn selected_preserves(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, budget: nat, semantic: bool)
    requires replacement_checked(original, plan), plan.splice.is_some(),
        splice_result(original, encoded, after, plan), plan.required <= original.len(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        list_layout::element_width(plan.trail.selected).is_none(),
        validation::schema_valid(node_bytes(original, plan.trail.nodes[plan.trail.depth as int]),
            plan.trail.selected, budget, semantic),
        inserted(plan.splice.unwrap()) == 1 ==> validation::schema_valid(encoded.subrange(destination_start(plan) as int,
            destination_start(plan) + plan.splice.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (budget - 1) as nat, semantic),
    ensures validation::schema_valid(after.subrange(plan.region.start as int, plan.region.end + plan.delta),
        plan.trail.selected, budget, semantic),
{
    hide(validation::schema_valid); hide(validation::local_valid); hide(validation::child_valid);
    hide(words::le_word); hide(crate::layout::descriptor_valid);
    checked_geometry(original, plan);
    replacement_extent(original, plan);
    let c = plan.splice.unwrap();
    let b = plan.region.start as int;
    let e = plan.region.end as int;
    let p = c.payload as int;
    let q = p - c.table_shrink;
    let layout = plan.trail.selected;
    assert(plan.region == plan.trail.nodes[plan.trail.depth as int].region);
    indexed_insert::items_schema(original, b, e, p, c.count as int, layout, budget, semantic);
    sequence_refinement(original, encoded, after, plan);
    let old = items(original, b, p, e, c.count as int);
    let input = encoded.subrange(destination_start(plan) as int, destination_start(plan) + c.input_len);
    edited_items_valid(old, input, c.operation, validation::child_layout(layout, 0), (budget - 1) as nat, semantic);
    resulting_boundaries(original, after, plan);
    assert forall|k: int| 0 <= k <= c.new_count implies
        0 <= #[trigger] boundary(after, b, q, e + plan.delta, c.new_count as int, k) <= e + plan.delta - q by {
        let source = if k <= c.first { k } else { k + c.last - c.first - inserted(c) };
        if k <= c.first {
            boundary_order(original, plan.region, c, source, c.first as int);
        } else {
            boundary_order(original, plan.region, c, c.last as int, source);
        }
    }
    assert forall|k: int| 0 <= k < c.new_count implies
        #[trigger] boundary_step(after, b, q, e + plan.delta, c.new_count as int, k) by {
        if k < c.first {
            assert(boundary_step(original, b, p, e, c.count as int, k));
        } else if k > c.first {
            let source = k + c.last - c.first - inserted(c);
            assert(boundary_step(original, b, p, e, c.count as int, source));
        } else if inserted(c) == 0 {
            boundary_order(original, plan.region, c, c.last as int, c.last as int + 1);
            assert(boundary(original, b, p, e, c.count as int, c.last as int) == c.end_offset);
        } else {
            assert(boundary(original, b, p, e, c.count as int, c.last as int) == c.end_offset);
            assert(boundary(original, b, p, e, c.count as int, c.first as int) == c.offset);
        }
    }
    reveal_with_fuel(validation::schema_valid, 1);
    indexed_insert::schema_from_items(after, b, e + plan.delta, q, c.new_count as int, layout, budget, semantic);
}

/// The indexed splice retains the generic plan's ancestor and physical frame.
#[verifier::spinoff_prover]
pub(super) proof fn replacement_frame(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), plan.splice.is_some(),
        splice_result(original, encoded, after, plan), plan.required <= original.len(),
    ensures replacement_result(original, after, after, plan),
{
    checked_geometry(original, plan);
    reveal(target); reveal(count_published); reveal(count_changed);
}

}
