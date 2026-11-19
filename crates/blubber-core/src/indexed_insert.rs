//! Indexed-list insertion geometry, ordered moves, and directory synthesis.

#![cfg_attr(verus_only, allow(missing_docs))]

use super::*;
#[cfg(verus_only)]
use crate::list_layout;
use crate::LayoutHead;

#[path = "indexed_insert_pass.rs"]
pub(super) mod checked;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct IndexedInsert {
    pub(super) index: u32,
    pub(super) count: u32,
    pub(super) payload: u32,
    pub(super) offset: u32,
    pub(super) table_growth: u32,
    pub(super) input_len: u32,
}

/// A cumulative boundary, including both implicit endpoints.
pub(super) open spec fn boundary(bytes: Seq<u8>, base: int, payload: int, end: int, count: int, k: int) -> int {
    if k == 0 { 0 } else if k == count { end - payload }
    else { words::le_word(bytes, base + 4 * k) as int }
}

pub(super) open spec fn boundary_step(bytes: Seq<u8>, b: int, payload: int, end: int, count: int, k: int) -> bool {
    boundary(bytes, b, payload, end, count, k) <= boundary(bytes, b, payload, end, count, k + 1)
}

pub(super) proof fn word_in_slice(bytes: Seq<u8>, start: int, end: int, at: int)
    requires 0 <= start <= end <= bytes.len(), 0 <= at, at + 4 <= end - start,
    ensures words::le_word(bytes.subrange(start, end), at) == words::le_word(bytes, start + at),
        0 <= words::le_word(bytes, start + at) <= u32::MAX,
{ }

pub(super) open spec fn parameters(bytes: Seq<u8>, region: Span, c: IndexedInsert) -> bool {
    let b = region.start as int;
    let e = region.end as int;
    &&& b + 4 <= c.payload <= e <= bytes.len() <= u32::MAX
    &&& c.count == words::le_word(bytes, b)
    &&& c.index <= c.count < u32::MAX
    &&& c.payload == b + 4 + if c.count == 0 { 0int } else { 4 * (c.count - 1) }
    &&& c.offset == boundary(bytes, b, c.payload as int, e, c.count as int, c.index as int)
    &&& 0 <= c.offset <= e - c.payload
    &&& c.table_growth == if c.count == 0 { 0u32 } else { 4u32 }
    &&& (c.count == 0 ==> e == c.payload)
    &&& forall|k: int| 0 <= k < c.count ==> #[trigger] boundary_step(bytes, b, c.payload as int, e, c.count as int, k)
    &&& forall|k: int| 0 <= k <= c.count ==> 0 <= #[trigger] boundary(bytes, b, c.payload as int, e, c.count as int, k) <= e - c.payload
}

pub(super) open spec fn insert_geometry(bytes: Seq<u8>, plan: Plan) -> bool {
    let c = plan.indexed.unwrap();
    let b = plan.region.start as int;
    let e = plan.region.end as int;
    &&& plan.indexed.is_some() && plan.list.is_none() && plan.splice.is_none()
    &&& parameters(bytes, plan.region, c)
    &&& 12 <= b <= e <= plan.old_used <= bytes.len() <= u32::MAX
    &&& plan.new_used == plan.old_used + plan.delta
    &&& plan.required == plan.new_used
    &&& plan.delta == c.table_growth + c.input_len
    &&& plan.replacement_len == e - b + c.table_growth + c.input_len
}

pub(super) open spec fn indexed_checked(bytes: Seq<u8>, plan: Plan) -> bool {
    plan.indexed.is_some() ==> insert_geometry(bytes, plan)
}

proof fn checked_geometry(bytes: Seq<u8>, plan: Plan)
    requires replacement_checked(bytes, plan), plan.indexed.is_some(),
    ensures insert_geometry(bytes, plan),
{ reveal(replacement_checked); }

pub(super) fn same_indexed(a: Option<IndexedInsert>, b: Option<IndexedInsert>) -> (result: bool)
    ensures result == (a == b),
{
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.index == b.index && a.count == b.count && a.payload == b.payload
            && a.offset == b.offset && a.table_growth == b.table_growth && a.input_len == b.input_len,
        _ => false,
    }
}

#[verifier::spinoff_prover]
fn prepare_insert(bytes: &[u8], region: Span, layout: &Layout, operation: ListEdit, input_len: u32) -> (result: Result<IndexedInsert>)
    requires region.start <= region.end <= bytes.len() <= u32::MAX,
    ensures result.is_ok() ==> parameters(bytes@, region, result.unwrap())
        && result.unwrap().input_len == input_len && operation == ListEdit::Insert(result.unwrap().index),
{
    hide(words::le_word);
    match layout.head() {
        LayoutHead::List { element } => if fixed_width(&element).is_some() { return Err(ErrorKind::InvalidSchema.into()); },
        _ => return Err(ErrorKind::InvalidSchema.into()),
    }
    let local = &bytes[region.start as usize..region.end as usize];
    let parts = crate::partition_list(local, layout).map_err(|e: Error| e.rebase(region.start))?;
    proof { word_in_slice(bytes@, region.start as int, region.end as int, 0); }
    let index = match operation { ListEdit::Insert(i) => i, _ => return Err(ErrorKind::InvalidSchema.into()) };
    if index > parts.count { return Err(ErrorKind::IndexOutOfBounds.into()); }
    let _new_count = checked_delta(parts.count, 1)?;
    let table_growth = if parts.count == 0 { 0u32 } else { 4u32 };
    let payload = region.start + parts.payload.start;
    proof { if 0 < index < parts.count { word_in_slice(bytes@, region.start as int, region.end as int, 4 * index); } }
    let offset = if index == parts.count { region.end - payload }
        else if index == 0 { 0u32 }
        else { read_u32(local, 4 * index)? };
    proof {
        assert forall|k: int| 0 <= k <= parts.count implies
            0 <= #[trigger] boundary(bytes@, region.start as int, payload as int, region.end as int, parts.count as int, k)
                <= region.end - payload by {
            if 0 < k < parts.count {
                assert(0 <= k - 1 < parts.count - 1);
                assert(words::le_word(local@, 4 + 4 * (k - 1)) <= local.len() - parts.payload.start);
                word_in_slice(bytes@, region.start as int, region.end as int, 4 * k);
                assert(words::le_word(bytes@, region.start + 4 * k) == words::le_word(local@, 4 * k));
            }
        }
    }
    proof {
        assert forall|k: int| 0 <= k < parts.count implies
            #[trigger] boundary_step(bytes@, region.start as int, payload as int, region.end as int, parts.count as int, k) by {
            if 0 < k < parts.count - 1 {
                assert(words::le_word(local@, 4 + 4 * k) >= words::le_word(local@, 4 + 4 * (k - 1)));
                word_in_slice(bytes@, region.start as int, region.end as int, 4 * k);
                assert(words::le_word(bytes@, region.start + 4 * k) == words::le_word(local@, 4 * k));
                word_in_slice(bytes@, region.start as int, region.end as int, 4 * (k + 1));
                assert(words::le_word(bytes@, region.start + 4 * (k + 1)) == words::le_word(local@, 4 * (k + 1)));
            }
        }
    }
    Ok(IndexedInsert { index, count: parts.count, payload, offset, table_growth, input_len })
}

#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
pub(super) fn plan_insert(bytes: &[u8], used: u32, path: &InlinePath,
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
    let change = prepare_insert(bytes, node.region, &trail.selected, operation, input_len)?;
    let new_len = checked_u32(checked_add(checked_add((node.region.end - node.region.start) as usize,
        change.table_growth as usize)?, input_len as usize)?)?;
    let mut result = plan_in_trail(bytes, used, path, &trail, new_len, limit);
    if let Ok(plan) = &mut result {
        // Keep the plan inside its result slot while attaching metadata.
        if plan.region.start != node.region.start || plan.region.end != node.region.end {
            return Err(ErrorKind::InvalidPlan.into());
        }
        proof { replacement_extent(bytes@, *plan); reveal(with_insert); }
        assert(indexed_checked(bytes@, with_insert(*plan, change)));
        attach_insert(plan, change, Ghost(bytes@));
    }
    result
}

pub(super) open spec fn ready_end(original: Seq<u8>, after: Seq<u8>, plan: Plan, j: int) -> bool {
    let c = plan.indexed.unwrap();
    let b = plan.region.start as int;
    if j < c.index && j < c.count - 1 {
        words::le_word(after, b + 4 + 4 * j) == words::le_word(original, b + 4 + 4 * j)
    } else if j > c.index {
        words::le_word(after, b + 4 + 4 * j) == words::le_word(original, b + 4 * j)
    } else { true }
}

pub(super) open spec fn payload_preserved(original: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.indexed.unwrap();
    let p = c.payload as int;
    let at = p + c.table_growth + c.offset;
    &&& original.len() == after.len()
    &&& forall|k: int| p + c.table_growth <= k < at ==> #[trigger] after[k] == original[k - c.table_growth]
    &&& forall|k: int| at + c.input_len <= k < plan.new_used ==> #[trigger] after[k] == original[k - plan.delta]
}

/// Before encoding, retained payload and needed directory words are protected.
pub(super) open spec fn movement(original: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.indexed.unwrap();
    &&& plan.required <= after.len()
    &&& payload_frame(original, after, plan)
    &&& payload_preserved(original, after, plan)
    &&& words::le_word(after, plan.region.start as int) == c.count
    &&& forall|j: int| 0 <= j < c.count ==> #[trigger] ready_end(original, after, plan, j)
}

#[verifier::spinoff_prover]
pub(super) fn move_insert(bytes: &mut [u8], plan: &Plan)
    requires replacement_checked(old(bytes)@, *plan), plan.indexed.is_some(), plan.required <= old(bytes).len(),
    ensures movement(old(bytes)@, final(bytes)@, *plan),
    no_unwind
{
    let ghost original = bytes@;
    proof { checked_geometry(original, *plan); reveal(target); reveal(payload_frame); }
    let c = match plan.indexed { Some(c) => c, None => panic!("checked edit plan") };
    let split = c.payload + c.offset;
    // Protect the original payload suffix and all outer suffix bytes first.
    if plan.delta != 0 && split < plan.old_used {
        match crate::move_bytes(bytes, Span { start: split, end: plan.old_used }, (split as i64 + plan.delta) as u32) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
    }
    let ghost suffix_moved = bytes@;
    // The enlarged directory may overwrite the old payload prefix only now.
    if c.table_growth != 0 && c.offset != 0 {
        match crate::move_bytes(bytes, Span { start: c.payload, end: split }, c.payload + c.table_growth) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
    }
    let ghost prefix_moved = bytes@;
    // These sources lie below the old payload and survived both payload moves.
    if c.index + 1 < c.count {
        let start = plan.region.start + 4 + 4 * c.index;
        match crate::move_bytes(bytes, Span { start, end: c.payload }, start + 4) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
    }
    proof {
        assert forall|j: int| 0 <= j < c.count implies #[trigger] ready_end(original, bytes@, *plan, j) by {
            let at = plan.region.start + 4 + 4 * j;
            if j < c.index && j < c.count - 1 {
                assert forall|k: int| at <= k < at + 4 implies bytes@[k] == original[k] by { }
            } else if j > c.index {
                assert forall|k: int| at <= k < at + 4 implies bytes@[k] == original[k - 4] by { }
            }
        }
    }
}

#[verifier::spinoff_prover]
pub(super) proof fn movement_framed(original: Seq<u8>, before: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), plan.indexed.is_some(), movement(original, before, plan),
    ensures forall|after: Seq<u8>| after.len() == before.len()
        && (forall|p: int| 0 <= p < before.len() && !(destination_start(plan) <= p < destination_start(plan) + input_size(plan))
            ==> after[p] == before[p]) ==> #[trigger] movement(original, after, plan),
{
    checked_geometry(original, plan); reveal(target); reveal(input_size); reveal(destination_start); reveal(payload_frame);
    let c = plan.indexed.unwrap();
    assert forall|after: Seq<u8>| after.len() == before.len()
        && (forall|p: int| 0 <= p < before.len() && !(destination_start(plan) <= p < destination_start(plan) + input_size(plan))
            ==> after[p] == before[p]) implies #[trigger] movement(original, after, plan) by {
        assert forall|j: int| 0 <= j < c.count implies #[trigger] ready_end(original, after, plan, j) by {
            let at = plan.region.start + 4 + 4 * j;
            assert(ready_end(original, before, plan, j));
            assert forall|k: int| at <= k < at + 4 implies after[k] == before[k] by { }
            assert(words::le_word(after, at) == words::le_word(before, at));
        }
    }
}

/// Newly stored cumulative ends, including the old implicit terminal on append.
pub(super) open spec fn new_end(original: Seq<u8>, plan: Plan, j: int) -> int {
    let c = plan.indexed.unwrap();
    let old = if j < c.index { j + 1 } else { j };
    if j == c.index { c.offset + c.input_len }
    else { boundary(original, plan.region.start as int, c.payload as int, plan.region.end as int, c.count as int, old)
        + if j > c.index { c.input_len as int } else { 0int } }
}

pub(super) open spec fn directory_done(original: Seq<u8>, after: Seq<u8>, plan: Plan, done: int) -> bool {
    forall|j: int| 0 <= j < done ==> #[trigger] words::le_word(after, plan.region.start + 4 + 4 * j) == new_end(original, plan, j)
}

pub(super) open spec fn insert_result(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.indexed.unwrap();
    let at = destination_start(plan) as int;
    &&& encoded.len() == original.len()
    &&& payload_preserved(original, after, plan)
    &&& forall|p: int| 0 <= p < after.len() && (p < 8 || plan.required <= p
            || (12 <= p < plan.region.start && (plan.delta == 0 || !ancestor_changed(plan, plan.trail.depth as int, p))))
        ==> #[trigger] after[p] == original[p]
    &&& words::le_word(after, 8) == plan.new_used
    &&& words::le_word(after, plan.region.start as int) == c.count + 1
    &&& directory_done(original, after, plan, c.count as int)
    &&& after.subrange(at, at + c.input_len) == encoded.subrange(at, at + c.input_len)
    &&& ancestors_delta(original, after, plan, plan.trail.depth as int)
}

#[verifier::spinoff_prover]
fn synthesize_directory(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>)
    requires replacement_checked(original, *plan), plan.indexed.is_some(), plan.required <= old(bytes).len(),
        movement(original, old(bytes)@, *plan),
    ensures directory_done(original, final(bytes)@, *plan, plan.indexed.unwrap().count as int),
        payload_frame(original, final(bytes)@, *plan), payload_preserved(original, final(bytes)@, *plan),
        words::le_word(final(bytes)@, plan.region.start as int) == plan.indexed.unwrap().count + 1,
        final(bytes)@.subrange(destination_start(*plan) as int, destination_start(*plan) + input_size(*plan))
            == old(bytes)@.subrange(destination_start(*plan) as int, destination_start(*plan) + input_size(*plan)),
    no_unwind
{
    proof { checked_geometry(original, *plan); reveal(target); reveal(destination_start); reveal(input_size); reveal(payload_frame); }
    let c = match plan.indexed { Some(c) => c, None => panic!("checked edit plan") };
    let ghost encoded = bytes@;
    let mut j = if c.index < c.count { c.index } else if c.count == 0 { 0u32 } else { c.count - 1 };
    proof {
        assert forall|k: int| 0 <= k < j implies
            #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * k) == new_end(original, *plan, k) by {
            assert(ready_end(original, bytes@, *plan, k));
        }
    }
    while j < c.count
        invariant j <= c.count, replacement_checked(original, *plan), insert_geometry(original, *plan), plan.indexed.is_some(),
            c == plan.indexed.unwrap(), plan.required <= bytes.len(), encoded.len() == original.len(),
            payload_preserved(original, bytes@, *plan), payload_frame(original, bytes@, *plan),
            words::le_word(bytes@, plan.region.start as int) == c.count,
            directory_done(original, bytes@, *plan, j as int),
            forall|k: int| j <= k < c.count ==> #[trigger] ready_end(original, bytes@, *plan, k),
            bytes@.subrange(destination_start(*plan) as int, destination_start(*plan) + c.input_len)
                == encoded.subrange(destination_start(*plan) as int, destination_start(*plan) + c.input_len),
        decreases c.count - j,
    {
        assert(ready_end(original, bytes@, *plan, j as int));
        let at = plan.region.start + 4 + 4 * j;
        let value = if j == c.index { { let sum = c.offset as u64 + c.input_len as u64; if sum > u32::MAX as u64 { panic!("checked insertion boundary"); } sum as u32 } }
            else if j + 1 == c.count && c.index == c.count { plan.region.end - c.payload }
            else if j > c.index {
                proof {
                    assert(ready_end(original, bytes@, *plan, j as int));
                    assert(words::le_word(bytes@, at as int) == boundary(original, plan.region.start as int,
                        c.payload as int, plan.region.end as int, c.count as int, j as int));
                    assert(0 <= boundary(original, plan.region.start as int, c.payload as int,
                        plan.region.end as int, c.count as int, j as int) <= plan.region.end - c.payload);
                }
                { let sum = crate::words::read_contained_word(bytes, (at) as usize) as u64 + c.input_len as u64; if sum > u32::MAX as u64 { panic!("checked retained boundary"); } sum as u32 }
            }
            else { crate::words::read_contained_word(bytes, (at) as usize) };
        proof {
            if j != c.index {
                let k = if j < c.index { j as int + 1 } else { j as int };
                assert(0 <= boundary(original, plan.region.start as int, c.payload as int, plan.region.end as int, c.count as int, k)
                    <= plan.region.end - c.payload);
            }
        }
        assert(value == new_end(original, *plan, j as int));
        let ghost previous = bytes@;
        assert(directory_done(original, previous, *plan, j as int));
        crate::words::write_contained_word(bytes, (at) as usize, value);
        proof {
            assert(words::le_word(bytes@, at as int) == value);
            assert forall|k: int| 0 <= k < j implies
                #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * k) == new_end(original, *plan, k) by {
                let other = plan.region.start + 4 + 4 * k;
                assert forall|p: int| other <= p < other + 4 implies bytes@[p] == previous[p] by { }
                assert(words::le_word(bytes@, other) == words::le_word(previous, other));
            }
            assert forall|k: int| j + 1 <= k < c.count implies #[trigger] ready_end(original, bytes@, *plan, k) by {
                assert(ready_end(original, previous, *plan, k));
                let other = plan.region.start + 4 + 4 * k;
                assert forall|p: int| other <= p < other + 4 implies bytes@[p] == previous[p] by { }
                assert(words::le_word(bytes@, other) == words::le_word(previous, other));
            }
        }
        j += 1;
    }
    let ghost uncounted = bytes@;
    crate::words::write_contained_word(bytes, (plan.region.start) as usize, c.count + 1);
    proof {
        assert forall|j: int| 0 <= j < c.count implies
            #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * j) == new_end(original, *plan, j) by {
            let at = plan.region.start + 4 + 4 * j;
            assert(directory_done(original, uncounted, *plan, c.count as int));
            assert forall|k: int| at <= k < at + 4 implies bytes@[k] == uncounted[k] by { }
            assert(words::le_word(bytes@, at) == words::le_word(uncounted, at));
        }
    }
}

#[verifier::spinoff_prover]
pub(super) fn finish_insert(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>)
    requires replacement_checked(original, *plan), plan.indexed.is_some(), plan.required <= old(bytes).len(),
        movement(original, old(bytes)@, *plan),
    ensures insert_result(original, old(bytes)@, final(bytes)@, *plan),
    no_unwind
{
    proof { checked_geometry(original, *plan); reveal(target); reveal(destination_start); reveal(input_size);
        reveal(payload_frame); reveal(replacement_result); reveal(count_changed); }
    let ghost c = plan.indexed.unwrap();
    let ghost encoded = bytes@;
    synthesize_directory(bytes, plan, Ghost(original));
    let ghost directory = bytes@;
    finish_replacement(bytes, plan, Ghost(original));
    proof {
        assert forall|j: int| 0 <= j < c.count implies
            #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * j) == new_end(original, *plan, j) by {
            let at = plan.region.start + 4 + 4 * j;
            assert(directory_done(original, directory, *plan, c.count as int));
            assert forall|k: int| at <= k < at + 4 implies bytes@[k] == directory[k] by { }
            assert(words::le_word(bytes@, at) == words::le_word(directory, at));
        }
    }
    proof {
        assert(payload_preserved(original, bytes@, *plan));
        assert(words::le_word(bytes@, plan.region.start as int) == c.count + 1);
        let at = destination_start(*plan) as int;
        assert(bytes@.subrange(at, at + c.input_len) =~= directory.subrange(at, at + c.input_len));
        assert(insert_result(original, encoded, bytes@, *plan));
    }
}


/// Logical elements obtained from cumulative boundaries, without a parser call.
pub(super) open spec fn items(bytes: Seq<u8>, base: int, payload: int, end: int, count: int) -> Seq<Seq<u8>> {
    Seq::new(count as nat, |i: int| bytes.subrange(
        payload + boundary(bytes, base, payload, end, count, i),
        payload + boundary(bytes, base, payload, end, count, i + 1)))
}

pub(super) open spec fn sequence_insert(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.indexed.unwrap();
    let b = plan.region.start as int;
    let e = plan.region.end as int;
    let old = items(original, b, c.payload as int, e, c.count as int);
    let input = encoded.subrange(destination_start(plan) as int, destination_start(plan) + c.input_len);
    items(after, b, c.payload + c.table_growth, e + plan.delta, c.count + 1)
        == old.subrange(0, c.index as int) + seq![input] + old.subrange(c.index as int, c.count as int)
}

pub(super) proof fn boundary_order(bytes: Seq<u8>, region: Span, c: IndexedInsert, i: int, j: int)
    requires parameters(bytes, region, c), 0 <= i <= j <= c.count,
    ensures boundary(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, i)
        <= boundary(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, j),
    decreases j - i,
{
    if i < j {
        boundary_order(bytes, region, c, i, j - 1);
        assert(boundary_step(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, j - 1));
        assert(boundary(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, j - 1)
            <= boundary(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, j));
    }
}

#[verifier::spinoff_prover]
pub(super) proof fn sequence_refinement(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), plan.indexed.is_some(),
        insert_result(original, encoded, after, plan), plan.required <= original.len(),
    ensures sequence_insert(original, encoded, after, plan),
{
    checked_geometry(original, plan); reveal(destination_start);
    let c = plan.indexed.unwrap();
    let b = plan.region.start as int;
    let e = plan.region.end as int;
    let p = c.payload as int;
    let q = p + c.table_growth;
    let at = q + c.offset;
    assert forall|k: int| 0 <= k <= c.count + 1 implies
        #[trigger] boundary(after, b, q, e + plan.delta, c.count + 1, k)
            == if k <= c.index { boundary(original, b, p, e, c.count as int, k) }
                else { boundary(original, b, p, e, c.count as int, k - 1) + c.input_len } by {
        if 0 < k < c.count + 1 {
            assert(directory_done(original, after, plan, c.count as int));
            assert(words::le_word(after, plan.region.start + 4 + 4 * (k - 1)) == new_end(original, plan, k - 1));
            assert(words::le_word(after, b + 4 * k) == new_end(original, plan, k - 1));
        }
    }
    let old = items(original, b, p, e, c.count as int);
    let input = encoded.subrange(at, at + c.input_len);
    let actual = items(after, b, q, e + plan.delta, c.count + 1);
    let expected = old.subrange(0, c.index as int) + seq![input] + old.subrange(c.index as int, c.count as int);
    assert forall|i: int| 0 <= i < c.count + 1 implies actual[i] == expected[i] by {
        let start = q + boundary(after, b, q, e + plan.delta, c.count + 1, i);
        let end = q + boundary(after, b, q, e + plan.delta, c.count + 1, i + 1);
        if i == c.index {
            assert(start == at && end == at + c.input_len);
        } else {
            let source = if i < c.index { i } else { i - 1 };
            let shift = if i < c.index { c.table_growth as int } else { plan.delta as int };
            let a = p + boundary(original, b, p, e, c.count as int, source);
            let z = p + boundary(original, b, p, e, c.count as int, source + 1);
            boundary_order(original, plan.region, c, source, source + 1);
            if i < c.index { boundary_order(original, plan.region, c, source + 1, c.index as int); }
            else { boundary_order(original, plan.region, c, c.index as int, source); }
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


/// Absolute cumulative ends project to the validator's exact relative child slice.
#[verifier::spinoff_prover]
pub(super) proof fn item_projection(bytes: Seq<u8>, b: int, e: int, p: int, count: int,
    layout: Layout, i: int)
    requires 0 <= b <= p <= e <= bytes.len() <= u32::MAX, 0 <= i < count,
        matches!(layout.head(), LayoutHead::List { .. }), list_layout::element_width(layout).is_none(),
        count == words::le_word(bytes, b), p == b + 4 + 4 * indexed_remove::words_for(count),
        list_layout::list_shape(bytes.subrange(b, e), layout),
    ensures items(bytes, b, p, e, count)[i] == bytes.subrange(b, e).subrange(
        validation::child_span(bytes.subrange(b, e), layout, i).start as int,
        validation::child_span(bytes.subrange(b, e), layout, i).end as int),
{
    hide(words::le_word);
    let local = bytes.subrange(b, e);
    word_in_slice(bytes, b, e, 0);
    if i > 0 { word_in_slice(bytes, b, e, 4 * i); }
    if i + 1 < count { word_in_slice(bytes, b, e, 4 * (i + 1)); }
    let span = validation::child_span(local, layout, i);
    assert(0 <= boundary(bytes, b, p, e, count, i) <= boundary(bytes, b, p, e, count, i + 1) <= e - p);
    assert(span.start as int == p - b + boundary(bytes, b, p, e, count, i));
    assert(span.end as int == p - b + boundary(bytes, b, p, e, count, i + 1));
    assert(items(bytes, b, p, e, count)[i] =~= local.subrange(span.start as int, span.end as int));
}

/// Schema validity supplies every original indexed element's recursive validity.
#[verifier::spinoff_prover]
pub(super) proof fn items_schema(bytes: Seq<u8>, b: int, e: int, p: int, count: int,
    layout: Layout, budget: nat, semantic: bool)
    requires 0 <= b <= p <= e <= bytes.len() <= u32::MAX,
        matches!(layout.head(), LayoutHead::List { .. }), list_layout::element_width(layout).is_none(),
        count == words::le_word(bytes, b), p == b + 4 + 4 * indexed_remove::words_for(count),
        validation::schema_valid(bytes.subrange(b, e), layout, budget, semantic),
    ensures forall|i: int| 0 <= i < count ==> #[trigger] validation::schema_valid(
        items(bytes, b, p, e, count)[i], validation::child_layout(layout, 0), (budget - 1) as nat, semantic),
{
    hide(validation::schema_valid); hide(validation::child_valid);
    let local = bytes.subrange(b, e);
    reveal_with_fuel(validation::schema_valid, 1);
    word_in_slice(bytes, b, e, 0);
    crate::preservation::children_all(local, layout, 0, (budget - 1) as nat, semantic);
    assert forall|i: int| 0 <= i < count implies #[trigger] validation::schema_valid(
        items(bytes, b, p, e, count)[i], validation::child_layout(layout, 0), (budget - 1) as nat, semantic) by {
        item_projection(bytes, b, e, p, count, layout, i);
        assert(validation::child_valid(local, layout, i, (budget - 1) as nat, semantic));
        reveal(validation::child_valid);
    }
}

/// Bounded monotone cumulative ends establish the exact indexed list shape.
#[verifier::spinoff_prover]
pub(super) proof fn shape_from_boundaries(bytes: Seq<u8>, b: int, e: int, p: int, count: int, layout: Layout)
    requires 0 <= b, b + 4 <= p <= e <= bytes.len() <= u32::MAX,
        matches!(layout.head(), LayoutHead::List { .. }), list_layout::element_width(layout).is_none(),
        count == words::le_word(bytes, b), p == b + 4 + 4 * indexed_remove::words_for(count),
        count == 0 ==> e == p,
        forall|k: int| 0 <= k <= count ==> 0 <= #[trigger] boundary(bytes, b, p, e, count, k) <= e - p,
        forall|k: int| 0 <= k < count ==> #[trigger] boundary_step(bytes, b, p, e, count, k),
    ensures list_layout::list_shape(bytes.subrange(b, e), layout),
{
    hide(words::le_word);
    let local = bytes.subrange(b, e);
    word_in_slice(bytes, b, e, 0);
    assert forall|i: int| 0 <= i < count - 1 implies {
        let end = #[trigger] words::le_word(local, 4 + 4 * i);
        &&& end <= local.len() - list_layout::payload_start(local, layout)
        &&& if i == 0 { 0 <= end } else { words::le_word(local, 4 + 4 * (i - 1)) <= end }
    } by {
        word_in_slice(bytes, b, e, 4 * (i + 1));
        assert(0 <= boundary(bytes, b, p, e, count, i + 1) <= e - p);
        if i > 0 {
            word_in_slice(bytes, b, e, 4 * i);
            assert(boundary_step(bytes, b, p, e, count, i));
        }
    }
}

/// Valid indexed elements and exact shape establish the complete recursive list.
#[verifier::spinoff_prover]
pub(super) proof fn schema_from_items(bytes: Seq<u8>, b: int, e: int, p: int, count: int,
    layout: Layout, budget: nat, semantic: bool)
    requires 0 <= b, b + 4 <= p <= e <= bytes.len() <= u32::MAX, budget > 0,
        crate::layout::descriptor_valid(layout, MAX_DEPTH as nat),
        matches!(layout.head(), LayoutHead::List { .. }), list_layout::element_width(layout).is_none(),
        count == words::le_word(bytes, b), p == b + 4 + 4 * indexed_remove::words_for(count),
        count == 0 ==> e == p,
        forall|k: int| 0 <= k <= count ==> 0 <= #[trigger] boundary(bytes, b, p, e, count, k) <= e - p,
        forall|k: int| 0 <= k < count ==> #[trigger] boundary_step(bytes, b, p, e, count, k),
        forall|i: int| 0 <= i < count ==> #[trigger] validation::schema_valid(
            items(bytes, b, p, e, count)[i], validation::child_layout(layout, 0), (budget - 1) as nat, semantic),
    ensures validation::schema_valid(bytes.subrange(b, e), layout, budget, semantic),
{
    hide(validation::schema_valid); hide(validation::child_valid);
    hide(validation::local_valid); hide(crate::layout::descriptor_valid);
    let local = bytes.subrange(b, e);
    word_in_slice(bytes, b, e, 0);
    shape_from_boundaries(bytes, b, e, p, count, layout);
    assert(words::le_word(local, 0) == count);
    assert(validation::child_count(local, layout) == count);
    assert(validation::local_valid(local, layout, semantic)) by { reveal(validation::local_valid); }
    reveal_with_fuel(validation::schema_valid, 1);
    assert forall|i: int| 0 <= i < count implies
        #[trigger] validation::child_valid(local, layout, i, (budget - 1) as nat, semantic) by {
        item_projection(bytes, b, e, p, count, layout, i);
        assert(validation::schema_valid(items(bytes, b, p, e, count)[i],
            validation::child_layout(layout, 0), (budget - 1) as nat, semantic));
        assert(validation::child_layout(layout, i) == validation::child_layout(layout, 0));
        reveal_with_fuel(validation::child_valid, 1);
        assert(validation::schema_valid(local.subrange(validation::child_span(local, layout, i).start as int,
            validation::child_span(local, layout, i).end as int), validation::child_layout(layout, i),
            (budget - 1) as nat, semantic));
    }
    crate::preservation::children_from_all(local, layout, 0, (budget - 1) as nat, semantic);
    assert(validation::children_valid(local, layout, 0, (budget - 1) as nat, semantic));
    reveal_with_fuel(validation::schema_valid, 1);
}

/// Indexed insertion establishes recursive validity of the whole selected list.
#[verifier::spinoff_prover]
pub(super) proof fn selected_preserves(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, budget: nat, semantic: bool)
    requires replacement_checked(original, plan), plan.indexed.is_some(),
        insert_result(original, encoded, after, plan), plan.required <= original.len(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        list_layout::element_width(plan.trail.selected).is_none(),
        validation::schema_valid(node_bytes(original, plan.trail.nodes[plan.trail.depth as int]),
            plan.trail.selected, budget, semantic),
        validation::schema_valid(encoded.subrange(destination_start(plan) as int,
            destination_start(plan) + plan.indexed.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (budget - 1) as nat, semantic),
    ensures validation::schema_valid(after.subrange(plan.region.start as int, plan.region.end + plan.delta),
        plan.trail.selected, budget, semantic),
{
    hide(validation::schema_valid); hide(validation::local_valid); hide(validation::child_valid);
    hide(words::le_word); hide(crate::layout::descriptor_valid);
    hide(replacement_checked);
    reveal(replacement_checked);
    checked_geometry(original, plan);
    replacement_extent(original, plan);
    let c = plan.indexed.unwrap();
    let b = plan.region.start as int;
    let e = plan.region.end as int;
    let p = c.payload as int;
    let q = p + c.table_growth;
    let layout = plan.trail.selected;
    assert(plan.region == plan.trail.nodes[plan.trail.depth as int].region);
    items_schema(original, b, e, p, c.count as int, layout, budget, semantic);
    sequence_refinement(original, encoded, after, plan);
    let old = items(original, b, p, e, c.count as int);
    let input = encoded.subrange(destination_start(plan) as int, destination_start(plan) + c.input_len);
    edited_items_valid(old, input, ListEdit::Insert(c.index), validation::child_layout(layout, 0),
        (budget - 1) as nat, semantic);
    assert forall|k: int| 0 <= k <= c.count + 1 implies
        #[trigger] boundary(after, b, q, e + plan.delta, c.count + 1, k)
            == if k <= c.index { boundary(original, b, p, e, c.count as int, k) }
                else { boundary(original, b, p, e, c.count as int, k - 1) + c.input_len } by {
        if 0 < k < c.count + 1 {
            assert(directory_done(original, after, plan, c.count as int));
            assert(words::le_word(after, plan.region.start + 4 + 4 * (k - 1)) == new_end(original, plan, k - 1));
            assert(words::le_word(after, b + 4 * k) == new_end(original, plan, k - 1));
        }
    }
    assert forall|k: int| 0 <= k <= c.count + 1 implies
        0 <= #[trigger] boundary(after, b, q, e + plan.delta, c.count + 1, k) <= e + plan.delta - q by {
        let source = if k <= c.index { k } else { k - 1 };
        assert(0 <= boundary(original, b, p, e, c.count as int, source) <= e - p);
    }
    assert forall|k: int| 0 <= k < c.count + 1 implies
        #[trigger] boundary_step(after, b, q, e + plan.delta, c.count + 1, k) by {
        if k < c.index {
            assert(boundary_step(original, b, p, e, c.count as int, k));
        } else if k > c.index {
            assert(boundary_step(original, b, p, e, c.count as int, k - 1));
        }
    }
    reveal_with_fuel(validation::schema_valid, 1);
    schema_from_items(after, b, e + plan.delta, q, c.count + 1, layout, budget, semantic);
}

/// The indexed result has the generic splice's complete ancestor and suffix frame.
#[verifier::spinoff_prover]
pub(super) proof fn replacement_frame(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), plan.indexed.is_some(),
        insert_result(original, encoded, after, plan), plan.required <= original.len(),
    ensures replacement_result(original, after, after, plan),
{
    checked_geometry(original, plan);
    reveal(target); reveal(count_published); reveal(count_changed);
}

}
