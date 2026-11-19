//! Sorted fixed-value maps composed with the checked fixed-entry splice.

#![cfg_attr(verus_only, allow(missing_docs))]

use super::*;
use crate::LayoutHead;
#[cfg(verus_only)]
use crate::{map_layout, search};

verus! {

/// One sorted-map operation, selected by an encoded key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapEdit {
    /// Inserts an absent key or replaces its existing value, returning old bytes.
    Insert,
    /// Removes a matching entry; absence succeeds without change.
    Remove,
    /// Replaces a matching value; absence returns `MissingKey`.
    Replace,
    /// Removes every entry; requires an empty query key.
    Clear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MapChange {
    pub operation: MapEdit,
    pub key_width: u32,
    pub value_width: u32,
    pub index: u32,
    pub found: bool,
}

pub(super) open spec fn node(plan: Plan) -> Node { plan.trail.nodes[plan.trail.depth as int] }

pub(super) open spec fn entry_operation(change: MapChange, count: u32) -> ListEdit {
    match change.operation {
        MapEdit::Insert => if change.found { ListEdit::Replace(change.index) } else { ListEdit::Insert(change.index) },
        MapEdit::Replace => ListEdit::Replace(change.index),
        MapEdit::Remove => if change.found { ListEdit::Remove(change.index) } else { ListEdit::Truncate(count) },
        MapEdit::Clear => ListEdit::Clear,
    }
}

pub(super) open spec fn input_len(plan: Plan) -> u32 {
    let change = plan.map.unwrap();
    if matches!(change.operation, MapEdit::Insert | MapEdit::Replace) { change.value_width } else { 0u32 }
}

pub(super) open spec fn value_start(plan: Plan) -> u32 {
    if matches!(plan.map.unwrap().operation, MapEdit::Insert | MapEdit::Replace) {
        (plan.region.start + plan.map.unwrap().key_width) as u32
    } else { plan.region.start }
}

pub(super) open spec fn map_checked(bytes: Seq<u8>, plan: Plan) -> bool {
    plan.map.is_some() ==> {
        let change = plan.map.unwrap();
        let n = node(plan);
        let local = node_bytes(bytes, n);
        &&& plan.list.is_some() && plan.indexed.is_none() && plan.splice.is_none() && plan.variable_map.is_none()
        &&& match n.layout.head() { LayoutHead::Map { key, value } => key.width == change.key_width
            && width_spec(value) == Some(change.value_width), _ => false }
        &&& plan.list.unwrap().operation == entry_operation(change, plan.list.unwrap().old_count)
        &&& (!plan.trail.local ==> search::keys_canonical(local, n.layout, plan.list.unwrap().old_count as int))
        &&& (!plan.trail.local ==> search::keys_ordered(local, n.layout, plan.list.unwrap().old_count as int))
        &&& if matches!(change.operation, MapEdit::Clear) { plan.key.len() == 0 }
            else {
                &&& plan.key.len() == change.key_width
                &&& (!search::bool_keys(n.layout) || plan.key@[0] <= 1)
                &&& search::search_found(crate::Search { index: change.index, found: change.found }, local, n.layout, plan.key@)
                &&& search::search_partition(crate::Search { index: change.index, found: change.found }, local, n.layout, plan.key@)
                &&& (matches!(change.operation, MapEdit::Replace) ==> change.found)
            }
    }
}

/// Captured map operation, encoded query, and value-only input length.
pub closed spec fn planned_map_for(plan: Plan, operation: MapEdit, key: Seq<u8>, input: u32) -> bool {
    plan.map.is_some() && plan.variable_map.is_none() && plan.map.unwrap().operation == operation && plan.key@ == key && input_len(plan) == input
}

/// Exposes the exact request identity of an attached fixed-map plan.
pub(super) proof fn attached_request(plan: Plan)
    requires plan.map.is_some(), plan.variable_map.is_none(),
    ensures planned_map_for(plan, plan.map.unwrap().operation, plan.key@, input_len(plan)),
{ }

/// Plans a sorted map edit with fixed-width values.
///
/// `Insert` is an upsert. `Replace` requires presence; absent `Remove` is a no-op.
/// Insert/replace take exactly the value width, all other operations take zero.
/// Clear requires an empty query. Stored keys are validated before searching.
/// The plan borrows only the external query, allowing readiness to repeat the
/// search after storage relocation. Apply copies new keys and exposes only value
/// bytes. Copy/decode `removed_span()` before apply for remove or replacement.
#[verifier::spinoff_prover]
#[allow(clippy::too_many_arguments)] // The core gateway keeps the shared planning parameters explicit.
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
pub fn plan_map_edit<'key>(bytes: &[u8], used_len: u32, layout: &Layout, path: &InlinePath,
    operation: MapEdit, key: &'key [u8], input: u32, storage_limit: u32)
    -> (result: Result<Plan<'key>>)
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used_len, *layout, *path, input, storage_limit)
        && planned_map_for(result.unwrap(), operation, key@, input)
        && map_previous_exact(bytes@, result.unwrap()) && fixed_map_plan(result.unwrap()),
{
    envelope(bytes, used_len)?;
    if bytes.len() > storage_limit as usize { return Err(ErrorKind::Capacity.into()); }
    let trail = trace(bytes, used_len, layout, path)?;
    map_in_resolved(bytes, used_len, layout, path, trail, operation, key, input, storage_limit, None)
}

#[allow(clippy::too_many_arguments)]
#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
pub(super) fn map_in_resolved<'key>(bytes: &[u8], used_len: u32, layout: &Layout, path: &InlinePath,
    trail: Trail, operation: MapEdit, key: &'key [u8], input: u32, storage_limit: u32, hint: Option<crate::Search>)
    -> (result: Result<Plan<'key>>)
    requires trail_valid(bytes@, *layout, *path, trail, used_len),
        12 <= used_len <= bytes.len() <= storage_limit <= u32::MAX,
        words::le_word(bytes@, 8) == used_len,
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used_len, *layout, *path, input, storage_limit)
        && planned_map_for(result.unwrap(), operation, key@, input)
        && map_previous_exact(bytes@, result.unwrap()) && fixed_map_plan(result.unwrap()),
        matches!(operation, MapEdit::Insert) && hint.is_none()
            && admission::map_tail_ready(bytes@, trail, input, key, storage_limit)
            ==> result == Ok(admission::map_tail_plan(bytes@, *path, trail, input, key, storage_limit)),
        forall|index: u32| matches!(operation, MapEdit::Insert) && hint.is_none()
            && admission::map_insert_ready(bytes@, trail, input, key, index, storage_limit)
            ==> result == Ok(admission::map_insert_plan(bytes@, *path, trail, input, key, index, storage_limit)),
{
    hide(search::lex_less);
    hide(search::keys_ordered);
    hide(search::keys_canonical);
    let n = trail.nodes[trail.depth];
    let (key_width, value_width) = if let Layout::Flat {checked} = trail.selected {
        proof { reveal(crate::layout_head::head_spec); }
        if checked.header(0) != 8 {return Err(ErrorKind::InvalidSchema.into());}
        let value = Layout::Flat {checked:checked.only_child()};
        let width = match fixed_width(&value) {
            Some(width) => width, None => return Err(ErrorKind::InvalidSchema.into()),
        };
        (checked.header(6), width)
    } else {match trail.selected.head() {
        LayoutHead::Map { key, value } => (key.width, match fixed_width(&value) {
            Some(width) => width, None => return Err(ErrorKind::InvalidSchema.into()),
        }),
        _ => return Err(ErrorKind::InvalidSchema.into()),
    }};
    let local = &bytes[n.region.start as usize..n.region.end as usize];
    if !trail.local { crate::validate_map_keys(local, &trail.selected).map_err(|e: Error| e.rebase(n.region.start))?; }
    let count = read_u32(local, 0)?;
    let lookup = if matches!(operation, MapEdit::Clear) {
        if !key.is_empty() { return Err(ErrorKind::InvalidBoundary.into()); }
        crate::Search { index: 0, found: false }
    } else { match hint {
        Some(hint) => crate::search::search_selected(local, &trail.selected, key, hint).map_err(|e: Error| e.rebase(n.region.start))?,
        None => if trail.local { crate::search::search_map_local(local, &trail.selected, key).map_err(|e: Error| e.rebase(n.region.start))? } else { crate::search_map(local, &trail.selected, key)? },
    } };
    proof {
        if matches!(operation, MapEdit::Insert) && hint.is_none()
            && admission::map_tail_ready(bytes@, trail, input, key, storage_limit) {
            indexed_insert::word_in_slice(bytes@, n.region.start as int, n.region.end as int, 0);
            admission::tail_lookup(local@, trail.selected, key@, lookup);
        }
    }
    proof {
        assert forall|index: u32| matches!(operation, MapEdit::Insert) && hint.is_none()
            && admission::map_insert_ready(bytes@, trail, input, key, index, storage_limit)
            implies lookup == (crate::Search { index, found: false }) by {
            admission::insert_lookup(local@, trail.selected, key@, index, lookup);
        }
    }
    if matches!(operation, MapEdit::Replace) && !lookup.found { return Err(ErrorKind::MissingKey.into()); }
    let expected = if matches!(operation, MapEdit::Insert | MapEdit::Replace) { value_width } else { 0u32 };
    if input != expected { return Err(ErrorKind::InvalidBoundary.into()); }
    let change = MapChange { operation, key_width, value_width, index: lookup.index, found: lookup.found };
    let entry_op = match operation {
        MapEdit::Insert => if lookup.found { ListEdit::Replace(lookup.index) } else { ListEdit::Insert(lookup.index) },
        MapEdit::Replace => ListEdit::Replace(lookup.index),
        MapEdit::Remove => if lookup.found { ListEdit::Remove(lookup.index) } else { ListEdit::Truncate(count) },
        MapEdit::Clear => ListEdit::Clear,
    };
    // Empty no-ops need no representable stride. A later insertion checks it.
    let width = if count == 0 && matches!(operation, MapEdit::Clear | MapEdit::Remove) { 0u32 }
        else { checked_u32(checked_add(key_width as usize, value_width as usize)?)? };
    let entry_input = if matches!(operation, MapEdit::Insert | MapEdit::Replace) { width } else { 0u32 };
    let base = plan_entries(bytes, used_len, layout, path, trail, entry_op, entry_input, width, storage_limit)?;
    let plan = Plan { key, map: Some(change), ..base };
    proof { previous_refinement(bytes@, plan); }
    Ok(plan)
}

pub(super) fn same_map(a: Option<MapChange>, b: Option<MapChange>) -> (result: bool)
    ensures result == (a == b),
{
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let operation = matches!((a.operation, b.operation),
                (MapEdit::Insert, MapEdit::Insert) | (MapEdit::Remove, MapEdit::Remove)
                | (MapEdit::Replace, MapEdit::Replace) | (MapEdit::Clear, MapEdit::Clear));
            operation && a.key_width == b.key_width && a.value_width == b.value_width
                && a.index == b.index && a.found == b.found
        },
        _ => false,
    }
}

pub(super) fn input_size_exec(plan: &Plan) -> (result: u32)
    requires plan.map.is_some(),
    ensures result == input_len(*plan),
    no_unwind
{
    let change = match plan.map { Some(c) => c, None => panic!("checked edit plan") };
    if matches!(change.operation, MapEdit::Insert | MapEdit::Replace) { change.value_width } else { 0 }
}

pub(super) fn value_start_exec(plan: &Plan) -> (result: u32)
    requires plan.map.is_some(), replacement_checked_for_extent(*plan),
    ensures result == value_start(*plan),
    no_unwind
{
    let change = match plan.map { Some(c) => c, None => panic!("checked map plan") };
    if matches!(change.operation, MapEdit::Insert | MapEdit::Replace) {
        plan.region.start + change.key_width
    } else { plan.region.start }
}

pub(super) open spec fn replacement_checked_for_extent(plan: Plan) -> bool {
    plan.map.is_some() ==> plan.region.start + plan.map.unwrap().key_width <= u32::MAX
        || !matches!(plan.map.unwrap().operation, MapEdit::Insert | MapEdit::Replace)
}

pub(super) open spec fn removed(plan: Plan) -> Option<Span> {
    let change = plan.map.unwrap();
    if change.found && matches!(change.operation, MapEdit::Insert | MapEdit::Replace | MapEdit::Remove) {
        Some(Span { start: (plan.region.start + change.key_width) as u32, end: plan.region.end })
    } else { None }
}

/// Returned coordinates select the queried old value, excluding its key.
pub closed spec fn map_previous_exact(bytes: Seq<u8>, plan: Plan) -> bool {
    let change = plan.map.unwrap();
    let n = node(plan);
    removed(plan) == if change.found && !matches!(change.operation, MapEdit::Clear) {
        let span = map_layout::value_span(node_bytes(bytes, n), n.layout, change.index);
        Some(Span { start: (n.region.start + span.start) as u32, end: (n.region.start + span.end) as u32 })
    } else { None }
}

#[verifier::spinoff_prover]
pub(super) proof fn previous_refinement(bytes: Seq<u8>, plan: Plan)
    requires replacement_admitted(bytes,plan), plan.map.is_some(),
    ensures map_previous_exact(bytes, plan),
{
    let change = plan.map.unwrap();
    if change.found && !matches!(change.operation, MapEdit::Clear) {
        let n = node(plan);
        let width = plan.list.unwrap().width as int;
        let i = change.index as int;
        assert((i + 1) * width == i * width + width) by(nonlinear_arith);
        assert(4 + (i + 1) * width <= n.region.end - n.region.start) by(nonlinear_arith)
            requires 0 <= i < plan.list.unwrap().old_count,
                n.region.end == n.region.start + 4 + plan.list.unwrap().old_count * width,
                0 <= width;
    }
}

pub(super) fn removed_span(plan: &Plan) -> (result: Option<Span>)
    requires plan.map.is_some(),
    ensures result == removed(*plan),
{
    let change = plan.map.unwrap();
    if change.found && matches!(change.operation, MapEdit::Insert | MapEdit::Replace | MapEdit::Remove) {
        Some(Span { start: (plan.region.start as u64 + change.key_width as u64) as u32, end: plan.region.end })
    } else { None }
}

pub(super) open spec fn key_byte(plan: Plan, p: int) -> bool {
    plan.map.is_some() && matches!(plan.map.unwrap().operation, MapEdit::Insert)
        && !plan.map.unwrap().found && plan.region.start <= p < plan.region.start + plan.map.unwrap().key_width
}

pub(super) open spec fn key_written(bytes: Seq<u8>, plan: Plan) -> bool {
    plan.map.is_some() && matches!(plan.map.unwrap().operation, MapEdit::Insert | MapEdit::Replace) ==>
        bytes.subrange(plan.region.start as int, plan.region.start + plan.map.unwrap().key_width) == plan.key@
}

#[verifier::spinoff_prover]
pub(super) fn write_key(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>)
    requires replacement_admitted(original,*plan), payload_frame(original, old(bytes)@, *plan),
        plan.required <= old(bytes).len(),
        (plan.map.is_some() && plan.map.unwrap().found ==> key_written(old(bytes)@, *plan)),
    ensures final(bytes).len() == old(bytes).len(), key_written(final(bytes)@, *plan),
        payload_frame(original, final(bytes)@, *plan),
        forall|p: int| #![trigger final(bytes)@[p]] 0 <= p < old(bytes).len() && !key_byte(*plan, p) ==> final(bytes)@[p] == old(bytes)@[p],
    no_unwind
{
    if let Some(change) = plan.map {
        if matches!(change.operation, MapEdit::Insert) && !change.found {
            let mut i = 0usize;
            while i < plan.key.len()
                invariant i <= plan.key.len(), bytes.len() == old(bytes).len(),
                    replacement_checked_for_extent(*plan),
                    plan.region.start + plan.key.len() <= bytes.len(),
                    plan.key.len() == change.key_width,
                    forall|p: int| #![trigger bytes@[p]] 0 <= p < old(bytes).len() && !(plan.region.start <= p < plan.region.start + i)
                        ==> bytes@[p] == old(bytes)@[p],
                    forall|j: int| 0 <= j < i ==> bytes@[plan.region.start + j] == plan.key@[j],
                decreases plan.key.len() - i,
            {
                bytes[plan.region.start as usize + i] = plan.key[i];
                i += 1;
            }
            assert(bytes@.subrange(plan.region.start as int, plan.region.start + change.key_width) =~= plan.key@);
        }
    }
}

pub(super) proof fn occupied_key(original: Seq<u8>, plan: Plan)
    requires replacement_admitted(original,plan),
    ensures plan.map.is_some() && plan.map.unwrap().found
        && matches!(plan.map.unwrap().operation, MapEdit::Insert | MapEdit::Replace)
        ==> key_written(original, plan) && plan.delta == 0,
{
    if let Some(change) = plan.map {
        if change.found && matches!(change.operation, MapEdit::Insert | MapEdit::Replace) {
            let local = node_bytes(original, node(plan));
            assert(plan.delta == 0) by(nonlinear_arith)
                requires plan.replacement_len == plan.list.unwrap().width,
                    plan.region.end == node(plan).region.start + 4 + plan.list.unwrap().last * plan.list.unwrap().width,
                    plan.region.start == node(plan).region.start + 4 + plan.list.unwrap().first * plan.list.unwrap().width,
                    plan.list.unwrap().last == plan.list.unwrap().first + 1,
                    plan.delta == plan.replacement_len as int - (plan.region.end - plan.region.start);
            assert(plan.region.start == node(plan).region.start + 4 + change.index * plan.list.unwrap().width);
            assert(local.subrange((plan.region.start - node(plan).region.start) as int,
                plan.region.start - node(plan).region.start + change.key_width) == plan.key@);
            assert(original.subrange(plan.region.start as int, plan.region.start + change.key_width) =~=
                local.subrange((plan.region.start - node(plan).region.start) as int,
                    plan.region.start - node(plan).region.start + change.key_width));
        }
    }
}

pub(super) open spec fn map_result(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let n = node(plan);
    let change = plan.map.unwrap();
    let before = node_bytes(original, n);
    let live = after.subrange(n.region.start as int, n.region.end + plan.delta);
    let value = encoded.subrange(value_start(plan) as int, value_start(plan) + input_len(plan));
    let model = search::map_model(before, n.layout);
    let expected = match change.operation {
        MapEdit::Insert | MapEdit::Replace => model.insert(plan.key@, value),
        MapEdit::Remove => model.remove(plan.key@),
        MapEdit::Clear => Map::empty(),
    };
    search::rep_map(before, n.layout, search::map_model(before, n.layout)) ==> search::rep_map(live, n.layout, expected)
}

pub(super) open spec fn writes(change: MapChange) -> bool { matches!(change.operation, MapEdit::Insert | MapEdit::Replace) }
pub(super) open spec fn inserts(change: MapChange) -> bool { matches!(change.operation, MapEdit::Insert) && !change.found }
pub(super) open spec fn deletes(change: MapChange) -> bool { matches!(change.operation, MapEdit::Remove) && change.found }
pub(super) open spec fn source_index(change: MapChange, i: int) -> int {
    if inserts(change) && i > change.index { i - 1 }
    else if deletes(change) && i >= change.index { i + 1 } else { i }
}
pub(super) open spec fn value_at(bytes: Seq<u8>, layout: Layout, i: int) -> Seq<u8> {
    let span = map_layout::value_span(bytes, layout, i as u32);
    bytes.subrange(span.start as int, span.end as int)
}
pub(super) open spec fn edited_map(model: Map<Seq<u8>, Seq<u8>>, change: MapChange, key: Seq<u8>, value: Seq<u8>) -> Map<Seq<u8>, Seq<u8>> {
    match change.operation {
        MapEdit::Insert | MapEdit::Replace => model.insert(key, value),
        MapEdit::Remove => model.remove(key),
        MapEdit::Clear => Map::empty(),
    }
}
pub(super) open spec fn entry_matches(before: Seq<u8>, live: Seq<u8>, layout: Layout, change: MapChange, key: Seq<u8>, value: Seq<u8>, i: int) -> bool {
    let k = search::encoded_key(live, layout, i);
    let v = value_at(live, layout, i);
    &&& (writes(change) && i == change.index ==> k == key && v == value)
    &&& (!(writes(change) && i == change.index) ==> 0 <= source_index(change, i) < words::le_word(before, 0)
        && k == search::encoded_key(before, layout, source_index(change, i))
        && v == value_at(before, layout, source_index(change, i)))
}
pub(super) open spec fn entry_relation(before: Seq<u8>, live: Seq<u8>, layout: Layout, change: MapChange, key: Seq<u8>, value: Seq<u8>) -> bool {
    &&& map_layout::map_shape(live, layout)
    &&& words::le_word(live, 0) == if matches!(change.operation, MapEdit::Clear) { 0int }
        else { words::le_word(before, 0) + (if inserts(change) { 1int } else { 0int }) - (if deletes(change) { 1int } else { 0int }) }
    &&& forall|i: int| #![trigger search::encoded_key(live, layout, i)]
        #![trigger entry_matches(before, live, layout, change, key, value, i)]
        0 <= i < words::le_word(live, 0) ==> entry_matches(before, live, layout, change, key, value, i)
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn entry_projection(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), replacement_result(original, encoded, after, plan),
        key_written(encoded, plan), list_model_refines(original, encoded, after, plan),
    ensures plan.map.is_some() ==> entry_relation(node_bytes(original, node(plan)),
        after.subrange(node(plan).region.start as int, node(plan).region.end + plan.delta), node(plan).layout,
        plan.map.unwrap(), plan.key@, encoded.subrange(value_start(plan) as int, value_start(plan) + input_len(plan))),
{
    hide(map_checked);
    hide(search::lex_less);
    hide(search::keys_ordered);
    hide(search::keys_canonical);
    if plan.map.is_none() { return; }
    let n = node(plan);
    let change = plan.map.unwrap();
    assert(plan.list.is_some() && plan.indexed.is_none() && plan.splice.is_none() && plan.variable_map.is_none()
        && match n.layout.head() { LayoutHead::Map { key, value } => key.width == change.key_width
            && width_spec(value) == Some(change.value_width), _ => false }) by { reveal(map_checked); }
    let list = plan.list.unwrap();
    reveal(replacement_checked);
    let before = node_bytes(original, n);
    let live = after.subrange(n.region.start as int, n.region.end + plan.delta);
    let count = list.old_count as int;
    let new_count = list.new_count as int;
    let index = change.index as int;
    let kw = change.key_width as int;
    let width = list.width as int;
    let base = n.region.start as int + 4;
    let old_items = fixed_items(original, base, count, width);
    let items = fixed_items(after, base, new_count, width);
    let input = encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len);
    let value = encoded.subrange(value_start(plan) as int, value_start(plan) + input_len(plan));
    let writing = matches!(change.operation, MapEdit::Insert | MapEdit::Replace);
    let inserts = matches!(change.operation, MapEdit::Insert) && !change.found;
    let deletes = matches!(change.operation, MapEdit::Remove) && change.found;
    let source = |i: int| if inserts && i > index { i - 1 }
        else if deletes && i >= index { i + 1 } else { i };
    assert(list_checked(original, plan));
    assert(list.operation == entry_operation(change, list.old_count)) by { reveal(map_checked); }
    assert(map_layout::key_width(n.layout) == change.key_width) by { reveal(map_checked); }
    assert(map_layout::value_width(n.layout) == Some(change.value_width)) by { reveal(map_checked); }
    assert(words::le_word(live, 0) == new_count);
    assert(words::le_word(before, 0) == count);
    assert(new_count == if matches!(change.operation, MapEdit::Clear) { 0int }
        else { count + (if inserts { 1int } else { 0int }) - (if deletes { 1int } else { 0int }) });
    assert(map_layout::map_shape(live, n.layout)) by(nonlinear_arith)
        requires live.len() == 4 + new_count * width,
            new_count == 0 || width == map_layout::entry_stride(n.layout),
            words::le_word(live, 0) == new_count,
            map_layout::value_width(n.layout).is_some(),
            4 <= live.len() <= u32::MAX;
    assert forall|i: int| #![trigger old_items[i]] 0 <= i < count implies {
        &&& old_items[i].subrange(0, kw) == search::encoded_key(before, n.layout, i)
        &&& old_items[i].subrange(kw, width) == {
            let span = map_layout::value_span(before, n.layout, i as u32);
            before.subrange(span.start as int, span.end as int)
        }
    } by {
        assert(width == map_layout::entry_stride(n.layout));
        assert(4 + i * width + kw <= 4 + (i + 1) * width <= before.len()) by(nonlinear_arith)
            requires 0 <= i < count, 0 <= width, kw <= width, before.len() == 4 + count * width;
        assert(i * width + width == (i + 1) * width) by(nonlinear_arith);
        let span = map_layout::value_span(before, n.layout, i as u32);
        assert(span.start == 4 + i * width + kw && span.end == 4 + (i + 1) * width);
        assert(old_items[i].subrange(0, kw) =~= search::encoded_key(before, n.layout, i));
        assert(old_items[i].subrange(kw, width) =~= before.subrange(span.start as int, span.end as int));
    }
    assert forall|i: int| 0 <= i < new_count implies
        #[trigger] entry_matches(before, live, n.layout, change, plan.key@, value, i) by {
        let item = items[i];
        assert(items == edited_items(old_items, input, list.operation));
        assert(item == if writing && i == index { input } else { old_items[source(i)] });
        assert(4 + i * width + kw <= 4 + (i + 1) * width <= live.len()) by(nonlinear_arith)
            requires 0 <= i < new_count, 0 <= width, kw <= width, live.len() == 4 + new_count * width;
        assert(item.subrange(0, kw) == search::encoded_key(live, n.layout, i));
        let span = map_layout::value_span(live, n.layout, i as u32);
        assert(i * width + width == (i + 1) * width) by(nonlinear_arith);
        assert(span.start == 4 + i * width + kw && span.end == 4 + (i + 1) * width);
        assert(item.subrange(kw, width) == live.subrange(span.start as int, span.end as int));
        if !(writing && i == index) {
            assert(0 <= source(i) < count);
            assert(old_items[source(i)].subrange(kw, width) == value_at(before, n.layout, source(i)));
        }
        if writing && i == index {
            assert(input.subrange(0, kw) == plan.key@);
            assert(input.subrange(kw, width) == value);
        }
    }

}

#[verifier::spinoff_prover]
pub(super) proof fn map_relation(before: Seq<u8>, live: Seq<u8>, layout: Layout, change: MapChange, key: Seq<u8>, value: Seq<u8>)
    requires search::rep_map(before, layout, search::map_model(before, layout)),
        entry_relation(before, live, layout, change, key, value),
        !matches!(change.operation, MapEdit::Clear) ==> {
            &&& key.len() == map_layout::key_width(layout)
            &&& (!search::bool_keys(layout) || key[0] <= 1)
            &&& search::search_found(crate::Search { index: change.index, found: change.found }, before, layout, key)
            &&& search::search_partition(crate::Search { index: change.index, found: change.found }, before, layout, key)
            &&& (matches!(change.operation, MapEdit::Replace) ==> change.found)
        },
    ensures search::rep_map(live, layout, edited_map(search::map_model(before, layout), change, key, value)),
{
    let count = words::le_word(before, 0);
    let new_count = words::le_word(live, 0);
    let index = change.index as int;
    let writes = writes(change);
    let inserts = inserts(change);
    let deletes = deletes(change);
    let source = |i: int| source_index(change, i);
    let model = search::map_model(before, layout);
    let expected = edited_map(model, change, key, value);
    assert(search::keys_canonical(live, layout, new_count)) by {
        assert forall|i: int| 0 <= i < new_count implies !search::bool_keys(layout)
            || (#[trigger] search::encoded_key(live, layout, i))[0] <= 1 by { }
    }
    assert(search::keys_ordered(live, layout, new_count)) by {
        assert forall|i: int, j: int| 0 <= i < j < new_count implies
            search::lex_less(#[trigger] search::encoded_key(live, layout, i),
                #[trigger] search::encoded_key(live, layout, j)) by {
            if writes && i == index { assert(source(j) > index || inserts && source(j) >= index); }
            else if writes && j == index { assert(source(i) < index); }
            else { assert(source(i) < source(j)); }
        }
    }
    let keys = Seq::new(new_count as nat, |i: int| search::encoded_key(live, layout, i));
    let old_keys = Seq::new(count as nat, |i: int| search::encoded_key(before, layout, i));
    assert(expected.dom() =~= keys.to_set()) by {
        assert forall|k: Seq<u8>| #[trigger] expected.dom().contains(k) <==> keys.to_set().contains(k) by {
            if keys.to_set().contains(k) {
                let i = choose|i: int| 0 <= i < new_count && keys[i] == k;
                if !(writes && i == index) {
                    assert(old_keys[source(i)] == k);
                    assert(model.dom().contains(k));
                    if matches!(change.operation, MapEdit::Remove) {
                        assert(source(i) != index || !change.found);
                        if k == key {
                            if source(i) < index { search::lex_irreflexive(k, key); }
                            else { search::lex_irreflexive(key, k); }
                        }
                    }
                }
            }
            if expected.dom().contains(k) {
                if writes && k == key { assert(keys[index] == k); }
                else {
                    assert(old_keys.to_set().contains(k));
                    let j = choose|j: int| 0 <= j < count && old_keys[j] == k;
                    let i = if inserts && j >= index { j + 1 }
                        else if deletes && j > index { j - 1 } else { j };
                    if writes && j == index && change.found { assert(k == key); }
                    if deletes && j == index { assert(k == key); }
                    assert(0 <= i < new_count && source(i) == j);
                    assert(keys[i] == k);
                }
                assert(keys.to_set().contains(k));
            }
        }
    }
    assert forall|i: int| 0 <= i < new_count implies {
        let span = map_layout::value_span(live, layout, i as u32);
        #[trigger] expected[search::encoded_key(live, layout, i)]
            == live.subrange(span.start as int, span.end as int)
    } by {
        let k = search::encoded_key(live, layout, i);
        if !(writes && i == index) {
            if writes && k == key {
                if source(i) < index { search::lex_irreflexive(k, key); }
                else { search::lex_irreflexive(key, k); }
            }
            let old_span = map_layout::value_span(before, layout, source(i) as u32);
            assert(model[k] == before.subrange(old_span.start as int, old_span.end as int));
        }
    }
}

#[verifier::spinoff_prover]
pub(super) proof fn refinement(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), replacement_result(original, encoded, after, plan),
        key_written(encoded, plan), list_model_refines(original, encoded, after, plan),
    ensures plan.map.is_some() ==> map_result(original, encoded, after, plan),
{
    if plan.map.is_none() { return; }
    let n = node(plan);
    let before = node_bytes(original, n);
    if !search::rep_map(before, n.layout, search::map_model(before, n.layout)) { return; }
    let live = after.subrange(n.region.start as int, n.region.end + plan.delta);
    entry_projection(original, encoded, after, plan);
    search::map_model_exact(before, n.layout);
    map_relation(before, live, n.layout, plan.map.unwrap(), plan.key@,
        encoded.subrange(value_start(plan) as int, value_start(plan) + input_len(plan)));
}

/// The edited fixed map retains valid keys and every recursive value schema.
#[verifier::spinoff_prover]
pub(super) proof fn selected_preserves(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, budget: nat, semantic: bool)
    requires replacement_checked(original, plan), plan.map.is_some(),
        replacement_result(original, encoded, after, plan), key_written(encoded, plan),
        validation::schema_valid(node_bytes(original, node(plan)), node(plan).layout, budget, semantic),
        writes(plan.map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(value_start(plan) as int, value_start(plan) + input_len(plan)),
            validation::child_layout(node(plan).layout, 0), (budget - 1) as nat, semantic),
    ensures validation::schema_valid(after.subrange(node(plan).region.start as int,
        node(plan).region.end + plan.delta), node(plan).layout, budget, semantic),
{
    hide(validation::schema_valid);
    hide(validation::child_valid);
    hide(search::lex_less);
    let n = node(plan);
    let before = node_bytes(original, n);
    let live = after.subrange(n.region.start as int, n.region.end + plan.delta);
    let change = plan.map.unwrap();
    let input = encoded.subrange(value_start(plan) as int, value_start(plan) + input_len(plan));
    super::list_refinement(original, encoded, after, plan);
    entry_projection(original, encoded, after, plan);
    reveal_with_fuel(validation::schema_valid, 1);
    if semantic {
        search::map_model_exact(before, n.layout);
        refinement(original, encoded, after, plan);
        assert(validation::local_valid(live, n.layout, semantic));
    } else {
        assert(validation::local_valid(live, n.layout, semantic));
    }
    crate::preservation::children_all(before, n.layout, 0, (budget - 1) as nat, semantic);
    assert forall|i: int| 0 <= i < validation::child_count(live, n.layout) implies
        #[trigger] validation::child_valid(live, n.layout, i, (budget - 1) as nat, semantic) by {
        assert(entry_matches(before, live, n.layout, change, plan.key@, input, i));
        let value = value_at(live, n.layout, i);
        if writes(change) && i == change.index {
            assert(value == input);
        } else {
            let old_i = source_index(change, i);
            assert(0 <= old_i < validation::child_count(before, n.layout));
            assert(validation::child_valid(before, n.layout, old_i, (budget - 1) as nat, semantic));
            assert(validation::child_layout(n.layout, old_i) == validation::child_layout(n.layout, i));
            assert(value == value_at(before, n.layout, old_i));
        }
        reveal(validation::child_valid);
    }
    crate::preservation::children_all(live, n.layout, 0, (budget - 1) as nat, semantic);
}

}
