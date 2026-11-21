//! Indexed-map splices with stored terminal ends and immutable encoded keys.

#![cfg_attr(verus_only, allow(missing_docs))]

use super::*;
use crate::LayoutHead;
#[path = "indexed_map_pass.rs"]
pub(super) mod checked;
#[cfg(verus_only)]
use crate::{map_layout, search};

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct IndexedMap {
    pub change: MapChange,
    pub count: u32,
    pub new_count: u32,
    pub stride: u32,
    pub payload: u32,
    pub offset: u32,
    pub end_offset: u32,
    pub input_len: u32,
    pub table_delta: i64,
}

pub(super) open spec fn inserts(c: IndexedMap) -> bool { matches!(c.change.operation, MapEdit::Insert) && !c.change.found }
pub(super) open spec fn deletes(c: IndexedMap) -> bool { matches!(c.change.operation, MapEdit::Remove) && c.change.found }
pub(super) open spec fn writes(c: IndexedMap) -> bool { matches!(c.change.operation, MapEdit::Insert | MapEdit::Replace) }
pub(super) open spec fn clears(c: IndexedMap) -> bool { matches!(c.change.operation, MapEdit::Clear) }
pub(super) open spec fn source(c: IndexedMap, i: int) -> int {
    if inserts(c) && i > c.change.index { i - 1 }
    else if deletes(c) && i >= c.change.index { i + 1 } else { i }
}
pub(super) open spec fn boundary(bytes: Seq<u8>, b: int, c: IndexedMap, k: int) -> int {
    if k == 0 { 0 } else { words::le_word(bytes, b + 4 + (k - 1) * c.stride + c.change.key_width) as int }
}
pub(super) open spec fn payload_delta(c: IndexedMap) -> int { c.input_len as int - (c.end_offset - c.offset) }
pub(super) open spec fn new_end(original: Seq<u8>, plan: Plan, i: int) -> int {
    let c = plan.variable_map.unwrap();
    if writes(c) && i == c.change.index { c.offset + c.input_len }
    else { boundary(original, plan.region.start as int, c, source(c, i) + 1)
        + if i < c.change.index { 0int } else { payload_delta(c) } }
}
pub(super) open spec fn new_key(original: Seq<u8>, plan: Plan, i: int) -> Seq<u8> {
    let c = plan.variable_map.unwrap();
    if inserts(c) && i == c.change.index { plan.key@ }
    else { let at = plan.region.start + 4 + source(c, i) * c.stride;
        original.subrange(at, at + c.change.key_width) }
}

pub(super) open spec fn destination(plan: Plan) -> u32 {
    let c = plan.variable_map.unwrap();
    (c.payload + c.table_delta + c.offset) as u32
}

pub(super) open spec fn header_parameters(bytes: Seq<u8>, region: Span, layout: Layout, c: IndexedMap, key: Seq<u8>) -> bool {
    let local = bytes.subrange(region.start as int, region.end as int);
    &&& match layout.head() { LayoutHead::Map { key, value } => key.width == c.change.key_width
        && width_spec(value).is_none(), _ => false }
    &&& map_layout::header_shape(local, layout)
    &&& c.count == words::le_word(bytes, region.start as int)
    &&& c.count == words::le_word(local, 0)
    &&& (c.count > 0 || c.new_count > 0 ==> c.stride == c.change.key_width + 4)
    &&& c.payload == region.start + 4 + c.count * c.stride
    &&& region.start + 4 <= c.payload <= region.end
    &&& 0 <= c.offset <= c.end_offset <= region.end - c.payload
    &&& c.change.index <= c.count
    &&& (c.change.found ==> c.change.index < c.count)
    &&& (clears(c) ==> !c.change.found && c.change.index == 0)
    &&& c.new_count == if clears(c) { 0int }
        else { c.count + (if inserts(c) { 1int } else { 0int }) - (if deletes(c) { 1int } else { 0int }) }
    &&& c.input_len == c.change.value_width
    &&& (!writes(c) ==> c.input_len == 0)
    &&& if clears(c) { key.len() == 0 && c.offset == 0 && c.end_offset == region.end - c.payload }
        else {
            &&& key.len() == c.change.key_width
            &&& (!search::bool_keys(layout) || key[0] <= 1)
            &&& search::search_found(crate::Search { index: c.change.index, found: c.change.found }, local, layout, key)
            &&& search::search_partition(crate::Search { index: c.change.index, found: c.change.found }, local, layout, key)
            &&& (matches!(c.change.operation, MapEdit::Replace) ==> c.change.found)
            &&& c.offset == boundary(bytes, region.start as int, c, c.change.index as int)
            &&& c.end_offset == boundary(bytes, region.start as int, c,
                c.change.index + if c.change.found { 1int } else { 0int })
        }
    &&& -(u32::MAX as int) <= c.table_delta <= u32::MAX
    &&& c.table_delta == (c.new_count as int - c.count) * c.stride
    &&& (inserts(c) ==> c.table_delta == c.stride && c.end_offset == c.offset)
    &&& (deletes(c) ==> c.table_delta == -(c.stride as int) && c.input_len == 0)
    &&& (!inserts(c) && !deletes(c) && !clears(c) ==> c.table_delta == 0)
    &&& (!inserts(c) ==> c.table_delta <= 0)
    &&& boundary(bytes, region.start as int, c, c.count as int) == region.end - c.payload
}

pub(super) open spec fn parameters(bytes: Seq<u8>, region: Span, layout: Layout, c: IndexedMap, key: Seq<u8>) -> bool {
    &&& header_parameters(bytes, region, layout, c, key)
    &&& map_layout::map_shape(bytes.subrange(region.start as int, region.end as int), layout)
    &&& forall|k: int| 0 <= k <= c.count ==> 0 <= #[trigger] boundary(bytes, region.start as int, c, k) <= region.end - c.payload
    &&& boundary(bytes, region.start as int, c, c.count as int) == region.end - c.payload
    &&& forall|i: int, j: int| 0 <= i <= j <= c.count ==>
        #[trigger] boundary(bytes, region.start as int, c, i) <= #[trigger] boundary(bytes, region.start as int, c, j)
}

pub(super) open spec fn geometry(bytes: Seq<u8>, plan: Plan) -> bool {
    let c = plan.variable_map.unwrap();
    let n = fixed_map::node(plan);
    &&& plan.variable_map.is_some() && plan.map.is_none() && plan.list.is_none()
        && plan.indexed.is_none() && plan.splice.is_none()
    &&& plan.region == n.region
    &&& parameters(bytes, plan.region, n.layout, c, plan.key@)
    &&& plan.delta == c.table_delta + payload_delta(c)
    &&& plan.region.start + 4 <= c.payload + c.table_delta <= plan.region.end + plan.delta
    &&& 0 <= c.payload + c.table_delta + c.offset <= c.payload + c.table_delta + c.offset + c.input_len
        <= plan.region.end + plan.delta <= plan.new_used
}

pub(super) open spec fn header_geometry(bytes: Seq<u8>, plan: Plan) -> bool {
    let c = plan.variable_map.unwrap();
    let n = fixed_map::node(plan);
    &&& plan.variable_map.is_some() && plan.map.is_none() && plan.list.is_none()
        && plan.indexed.is_none() && plan.splice.is_none()
    &&& plan.region == n.region
    &&& header_parameters(bytes, plan.region, n.layout, c, plan.key@)
    &&& plan.delta == c.table_delta + payload_delta(c)
    &&& plan.region.start + 4 <= c.payload + c.table_delta <= plan.region.end + plan.delta
    &&& 0 <= c.payload + c.table_delta + c.offset <= c.payload + c.table_delta + c.offset + c.input_len
        <= plan.region.end + plan.delta <= plan.new_used
}

/// Numeric movement admission; carries no partition invariant.
pub(super) open spec fn bounded(bytes: Seq<u8>, plan: Plan) -> bool {
    &&& header_geometry(bytes, plan)
    &&& 12 <= plan.region.start <= plan.region.end <= plan.old_used <= bytes.len() <= u32::MAX
    &&& plan.new_used == plan.old_used + plan.delta
    &&& plan.required == if plan.new_used > plan.old_used { plan.new_used } else { plan.old_used }
    &&& plan.region.end + plan.delta <= plan.new_used <= plan.required
    &&& plan.replacement_len == plan.region.end - plan.region.start + plan.delta
    &&& replacement_checked_for_map(plan)
    &&& plan.variable_map.unwrap().payload + plan.variable_map.unwrap().table_delta
        == plan.region.start + 4 + plan.variable_map.unwrap().new_count * plan.variable_map.unwrap().stride
}

pub(super) open spec fn checked(bytes: Seq<u8>, plan: Plan) -> bool {
    plan.variable_map.is_some() ==> geometry(bytes, plan)
}

pub(super) open spec fn planned(plan: Plan, operation: MapEdit, key: Seq<u8>, input: u32) -> bool {
    plan.variable_map.is_some() && plan.variable_map.unwrap().change.operation == operation
        && plan.key@ == key && plan.variable_map.unwrap().input_len == input
}

pub(super) open spec fn removed(plan: Plan) -> Option<Span> {
    let c = plan.variable_map.unwrap();
    if c.change.found && !clears(c) { Some(Span { start: (c.payload + c.offset) as u32, end: (c.payload + c.end_offset) as u32 }) }
    else { None }
}

pub(super) open spec fn previous_exact(bytes: Seq<u8>, plan: Plan) -> bool {
    let c = plan.variable_map.unwrap();
    let n = fixed_map::node(plan);
    removed(plan) == if c.change.found && !clears(c) {
        let s = map_layout::value_span(node_bytes(bytes, n), n.layout, c.change.index);
        Some(Span { start: (n.region.start + s.start) as u32, end: (n.region.start + s.end) as u32 })
    } else { None }
}

proof fn word_in_slice(bytes: Seq<u8>, start: int, end: int, at: int)
    requires 0 <= start <= end <= bytes.len(), 0 <= at, at + 4 <= end - start,
    ensures words::le_word(bytes.subrange(start, end), at) == words::le_word(bytes, start + at),
        0 <= words::le_word(bytes, start + at) <= u32::MAX,
{ }

proof fn word_equal(before: Seq<u8>, a: int, after: Seq<u8>, b: int)
    requires 0 <= a, a + 4 <= before.len(), 0 <= b, b + 4 <= after.len(),
        after[b] == before[a], after[b+1] == before[a+1], after[b+2] == before[a+2], after[b+3] == before[a+3],
    ensures words::le_word(after, b) == words::le_word(before, a),
{ }

proof fn record_prefix(before: Seq<u8>, a: int, after: Seq<u8>, b: int, width: int, prefix: int)
    requires 0 <= a, a + width <= before.len(), 0 <= b, b + width <= after.len(),
        0 <= prefix <= width, before.subrange(a, a+width) == after.subrange(b, b+width),
    ensures before.subrange(a, a+prefix) == after.subrange(b, b+prefix),
{
    let left = before.subrange(a,a+width);
    let right = after.subrange(b,b+width);
    assert forall|k: int| 0 <= k < prefix implies #[trigger] before[a+k] == after[b+k] by {
        assert(left[k] == right[k]);
    }
    assert(before.subrange(a,a+prefix) =~= after.subrange(b,b+prefix));
}

proof fn record_word(before: Seq<u8>, a: int, after: Seq<u8>, b: int, width: int, offset: int)
    requires 0 <= a, a + width <= before.len(), 0 <= b, b + width <= after.len(),
        0 <= offset, offset + 4 <= width,
        before.subrange(a, a+width) == after.subrange(b, b+width),
    ensures words::le_word(before, a+offset) == words::le_word(after, b+offset),
{
    let left = before.subrange(a, a+width);
    let right = after.subrange(b, b+width);
    assert(left[offset] == right[offset]);
    assert(left[offset+1] == right[offset+1]);
    assert(left[offset+2] == right[offset+2]);
    assert(left[offset+3] == right[offset+3]);
    word_equal(before, a+offset, after, b+offset);
}

proof fn boundaries(bytes: Seq<u8>, b: int, e: int, layout: Layout, c: IndexedMap)
    requires 0 <= b <= e <= bytes.len(), map_layout::map_shape(bytes.subrange(b, e), layout),
        map_layout::value_width(layout).is_none(), map_layout::key_width(layout) == c.change.key_width,
        c.count == words::le_word(bytes.subrange(b, e), 0),
        c.count > 0 ==> c.stride == c.change.key_width + 4,
        c.payload == b + 4 + c.count * c.stride,
    ensures
        forall|k: int| 0 <= k <= c.count ==> 0 <= #[trigger] boundary(bytes, b, c, k) <= e - c.payload,
        boundary(bytes, b, c, c.count as int) == e - c.payload,
        forall|i: int, j: int| 0 <= i <= j <= c.count ==>
            #[trigger] boundary(bytes, b, c, i) <= #[trigger] boundary(bytes, b, c, j),
{
    hide(words::le_word);
    let local = bytes.subrange(b, e);
    assert(c.payload - b == map_layout::payload_start(local, layout)) by(nonlinear_arith)
        requires map_layout::payload_start(local, layout) == 4 + c.count * map_layout::entry_stride(layout),
            c.count == 0 || c.stride == map_layout::entry_stride(layout),
            c.payload == b + 4 + c.count * c.stride;
    assert(c.payload <= e);
    assert forall|k: int| 0 <= k <= c.count implies 0 <= #[trigger] boundary(bytes, b, c, k) <= e - c.payload by {
        if k > 0 {
            let at = 4 + (k - 1) * c.stride + c.change.key_width;
            assert(4 <= at && at + 4 <= c.payload - b) by(nonlinear_arith)
                requires at == 4 + (k - 1) * c.stride + c.change.key_width,
                    0 < k <= c.count, c.stride == c.change.key_width + 4, c.change.key_width >= 0,
                    c.payload == b + 4 + c.count * c.stride;
            word_in_slice(bytes, b, e, at);
            assert(words::le_word(local, at) <= e - c.payload);
            assert(words::le_word(local, at) == words::le_word(bytes, b + at));
        }
    }
    assert forall|k: int| 0 <= k < c.count implies
        #[trigger] boundary(bytes, b, c, k) <= #[trigger] boundary(bytes, b, c, k + 1) by {
        if k > 0 {
            let at = 4 + k * c.stride + c.change.key_width;
            table_index(4, c.stride, c.count, k);
            assert(at - c.stride == 4 + (k-1)*c.stride + c.change.key_width) by(nonlinear_arith)
                requires at == 4 + k*c.stride + c.change.key_width;
            word_in_slice(bytes, b, e, at);
            assert(words::le_word(local, at) == words::le_word(bytes, b + at));
            word_in_slice(bytes, b, e, at - c.stride);
            assert(words::le_word(local, at - c.stride) == words::le_word(bytes, b + at - c.stride));
        }
    }
    if c.count > 0 {
        let at = 4 + (c.count - 1) * c.stride + c.change.key_width;
        table_index(4, c.stride, c.count, c.count as int - 1);
        word_in_slice(bytes, b, e, at);
        assert(words::le_word(local, at) == e - c.payload);
    }
    assert forall|i: int, j: int| 0 <= i <= j <= c.count implies
        #[trigger] boundary(bytes, b, c, i) <= #[trigger] boundary(bytes, b, c, j) by {
        boundary_chain(bytes, b, c, i, j);
    }
}

proof fn boundary_chain(bytes: Seq<u8>, b: int, c: IndexedMap, i: int, j: int)
    requires 0 <= i <= j <= c.count,
        forall|k: int| 0 <= k < c.count ==>
            #[trigger] boundary(bytes, b, c, k) <= #[trigger] boundary(bytes, b, c, k + 1),
    ensures boundary(bytes, b, c, i) <= boundary(bytes, b, c, j),
    decreases j - i,
{
    hide(boundary);
    if i < j {
        boundary_chain(bytes, b, c, i, j - 1);
        assert(boundary(bytes, b, c, j - 1) <= boundary(bytes, b, c, (j - 1) + 1));
    }
}

proof fn table_index(base: int, stride: u32, count: u32, i: int)
    requires 0 <= i <= count,
    ensures base <= base + i * stride <= base + count * stride,
        i < count ==> base + (i + 1) * stride <= base + count * stride,
        base + (i + 1) * stride == base + i * stride + stride,
{ assert(base <= base + i * stride <= base + count * stride
        && (i < count ==> base + (i + 1) * stride <= base + count * stride)
        && base + (i + 1) * stride == base + i * stride + stride) by(nonlinear_arith)
    requires 0 <= i <= count, 0 <= stride; }

proof fn checked_geometry(bytes: Seq<u8>, plan: Plan)
    requires replacement_checked(bytes, plan), plan.variable_map.is_some(),
    ensures geometry(bytes, plan), bounded(bytes, plan),
        12 <= plan.region.start <= plan.region.end <= plan.old_used <= bytes.len() <= u32::MAX,
        plan.new_used == plan.old_used + plan.delta,
        plan.required == if plan.new_used > plan.old_used { plan.new_used } else { plan.old_used },
        plan.region.end + plan.delta <= plan.new_used <= plan.required,
        plan.replacement_len == plan.region.end - plan.region.start + plan.delta,
        replacement_checked_for_map(plan),
        plan.variable_map.unwrap().payload + plan.variable_map.unwrap().table_delta
            == plan.region.start + 4 + plan.variable_map.unwrap().new_count * plan.variable_map.unwrap().stride,
{ replacement_extent(bytes, plan); assert(geometry(bytes, plan));
    let c = plan.variable_map.unwrap();
    assert(c.payload + c.table_delta == plan.region.start + 4 + c.new_count * c.stride) by(nonlinear_arith)
        requires c.payload == plan.region.start + 4 + c.count * c.stride,
            c.table_delta == (c.new_count as int - c.count) * c.stride;
}

#[allow(clippy::too_many_arguments)]
#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
pub(super) fn plan_map<'key>(bytes: &[u8], used: u32, layout: &Layout, path: &InlinePath,
    operation: MapEdit, key: &'key [u8], input: u32, limit: u32) -> (result: Result<Plan<'key>>)
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used, *layout, *path, input, limit)
        && planned(result.unwrap(), operation, key@, input) && previous_exact(bytes@, result.unwrap())
        && result.unwrap().variable_map.is_some(),
{
    hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(words::le_word);
    envelope(bytes, used)?;
    if bytes.len() > limit as usize { return Err(ErrorKind::Capacity.into()); }
    let trail = trace(bytes, used, layout, path)?;
    map_in_resolved(bytes, used, layout, path, trail, operation, key, input, limit, None)
}

#[allow(clippy::too_many_arguments)]
#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[verifier::rlimit(160)]
pub(super) fn map_in_resolved<'key>(bytes: &[u8], used: u32, _layout: &Layout, path: &InlinePath,
    trail: Trail, operation: MapEdit, key: &'key [u8], input: u32, limit: u32, hint: Option<crate::Search>)
    -> (result: Result<Plan<'key>>)
    requires trail_valid(bytes@, *_layout, *path, trail, used),
        12 <= used <= bytes.len() <= limit <= u32::MAX,
        words::le_word(bytes@, 8) == used,
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used, *_layout, *path, input, limit)
        && planned(result.unwrap(), operation, key@, input) && previous_exact(bytes@, result.unwrap())
        && result.unwrap().variable_map.is_some(),
{
    hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(words::le_word);
    let n = trail.nodes[trail.depth];
    let c = prepare(bytes, n.region, &trail.selected, operation, key, input, hint, trail.local)?;
    let delta = c.table_delta + c.input_len as i64 - (c.end_offset - c.offset) as i64;
    let new_len = checked_delta(n.region.end - n.region.start, delta)?;
    let base = plan_in_trail(bytes, used, path, &trail, new_len, limit)?;
    if base.region.start != n.region.start || base.region.end != n.region.end { return Err(ErrorKind::InvalidPlan.into()); }
    let plan = Plan { key, variable_map: Some(c), ..base };
    proof {
        replacement_extent(bytes@, base); reveal(subtree_plan); reveal(planned_for); reveal(replacement_checked);
        assert(c.payload + c.table_delta == n.region.start + 4 + c.new_count * c.stride) by(nonlinear_arith)
            requires c.payload == n.region.start + 4 + c.count * c.stride,
                c.table_delta == (c.new_count as int - c.count) * c.stride;
        assert(geometry(bytes@, plan));
        assert(replacement_checked(bytes@, plan));
        assert(planned_for(plan, used, *_layout, *path, input, limit));
        if c.change.found {
            table_index(n.region.start + 4, c.stride, c.count, c.change.index as int);
            if c.change.index > 0 { table_index(n.region.start + 4, c.stride, c.count, c.change.index as int - 1); }
            word_in_slice(bytes@, n.region.start as int, n.region.end as int, 4 + c.change.index * c.stride + c.change.key_width);
            if c.change.index > 0 { word_in_slice(bytes@, n.region.start as int, n.region.end as int, 4 + (c.change.index - 1) * c.stride + c.change.key_width); }
        }
        assert(previous_exact(bytes@, plan));
    }
    Ok(plan)
}

#[allow(clippy::too_many_arguments)]
fn prepare(bytes: &[u8], region: Span, layout: &Layout, operation: MapEdit, key: &[u8], input: u32, hint: Option<crate::Search>, local_edit: bool) -> (result: Result<IndexedMap>)
    requires region.start <= region.end <= bytes.len() <= u32::MAX,
    ensures result.is_ok() ==> parameters(bytes@, region, *layout, result.unwrap(), key@)
        && result.unwrap().change.operation == operation && result.unwrap().input_len == input,
{ prepare_mode(bytes, region, layout, operation, key, input, hint, local_edit, true) }

/// Admits the header, selected search and span; untouched ends stay unchecked.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub(super) fn prepare_checked(bytes: &[u8], region: Span, layout: &Layout, operation: MapEdit, key: &[u8], input: u32, hint: Option<crate::Search>) -> (result: Result<IndexedMap>)
    requires region.start <= region.end <= bytes.len() <= u32::MAX,
    ensures result.is_ok() ==> header_parameters(bytes@, region, *layout, result.unwrap(), key@)
        && result.unwrap().change.operation == operation && result.unwrap().input_len == input,
{ prepare_mode(bytes, region, layout, operation, key, input, hint, true, false) }

#[allow(clippy::too_many_arguments)]
#[verifier::spinoff_prover]
#[verifier::rlimit(480)]
#[inline(always)]
fn prepare_mode(bytes: &[u8], region: Span, layout: &Layout, operation: MapEdit, key: &[u8], input: u32, hint: Option<crate::Search>, local_edit: bool, strict: bool) -> (result: Result<IndexedMap>)
    requires region.start <= region.end <= bytes.len() <= u32::MAX,
    ensures result.is_ok() ==> header_parameters(bytes@, region, *layout, result.unwrap(), key@)
        && (strict ==> parameters(bytes@, region, *layout, result.unwrap(), key@))
        && result.unwrap().change.operation == operation && result.unwrap().input_len == input,
{
    hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(words::le_word);
    hide(search::search_partition); hide(search::search_found); hide(parameters); hide(boundary);
    hide(crate::layout::descriptor_valid); hide(map_layout::map_shape);
    let kw = match layout.head() {
        LayoutHead::Map { key, value } => {
            if fixed_width(&value).is_some() { return Err(ErrorKind::InvalidSchema.into()); }
            key.width
        },
        _ => return Err(ErrorKind::InvalidSchema.into()),
    };
    let local = &bytes[region.start as usize..region.end as usize];
    if !local_edit { crate::validate_map_keys(local, layout).map_err(|e: Error| e.rebase(region.start))?; }
    let parts = if strict { crate::partition_map(local, layout) }
        else { crate::map_layout::partition_map_header(local, layout) }.map_err(|e: Error| e.rebase(region.start))?;
    proof {
        assert(map_layout::header_shape(local@, *layout)) by { reveal(map_layout::header_shape); reveal(map_layout::map_shape); }
        assert(map_layout::payload_start(local@, *layout) <= local.len()) by { reveal(map_layout::map_shape); reveal(map_layout::header_shape); }
    }
    let lookup = if matches!(operation, MapEdit::Clear) {
        if !key.is_empty() { return Err(ErrorKind::InvalidBoundary.into()); }
        crate::Search { index: 0, found: false }
    } else { match hint {
        Some(hint) => crate::search::search_selected(local, layout, key, hint).map_err(|e: Error| e.rebase(region.start))?,
        None => if local_edit { crate::search::search_map_local(local, layout, key).map_err(|e: Error| e.rebase(region.start))? } else { crate::search_map(local, layout, key)? },
    } };
    proof {
        assert(lookup.index <= parts.count && (lookup.found ==> lookup.index < parts.count)) by {
            reveal(search::search_found);
        }
    }
    if matches!(operation, MapEdit::Replace) && !lookup.found { return Err(ErrorKind::MissingKey.into()); }
    if !matches!(operation, MapEdit::Insert | MapEdit::Replace) && input != 0 {
        return Err(ErrorKind::InvalidBoundary.into());
    }
    let new_count = if matches!(operation, MapEdit::Clear) { 0u32 }
        else if matches!(operation, MapEdit::Insert) && !lookup.found { checked_delta(parts.count, 1)? }
        else if matches!(operation, MapEdit::Remove) && lookup.found { parts.count - 1 }
        else { parts.count };
    // Empty remove/clear need neither a representable stride nor key allocation.
    let stride = if parts.count == 0 && new_count == 0 { 0u32 }
        else { checked_u32(checked_add(kw as usize, 4)?)? };
    let payload = region.start + parts.payload.start;
    proof {
        assert(payload == region.start + 4 + parts.count * stride) by(nonlinear_arith)
            requires payload == region.start + parts.payload.start,
                parts.payload.start == 4 + parts.count * map_layout::entry_stride(*layout),
                parts.count == 0 || stride == map_layout::entry_stride(*layout);
    }
    let (offset, end_offset) = if matches!(operation, MapEdit::Clear) { (0u32, region.end - payload) }
        else {
            let offset = if lookup.index == 0 { 0u32 }
                else { read_u32(local, checked_u32(checked_add(checked_add(4, checked_mul((lookup.index - 1) as usize, stride as usize)?)?, kw as usize)?)?)? };
            let end = if lookup.found {
                read_u32(local, checked_u32(checked_add(checked_add(4, checked_mul(lookup.index as usize, stride as usize)?)?, kw as usize)?)?)?
            } else { offset };
            (offset, end)
        };
    if offset > end_offset || end_offset > region.end - payload {
        return Err(Error::new(ErrorKind::InvalidBoundary, region.start));
    }
    let _new_table = checked_u32(checked_mul(new_count as usize, stride as usize)?)?;
    let table_delta = if matches!(operation, MapEdit::Clear) { -((payload - region.start - 4) as i64) }
        else if matches!(operation, MapEdit::Insert) && !lookup.found { stride as i64 }
        else if matches!(operation, MapEdit::Remove) && lookup.found { -(stride as i64) }
        else { 0i64 };
    let c = IndexedMap { change: MapChange { operation, key_width: kw, value_width: input,
        index: lookup.index, found: lookup.found }, count: parts.count, new_count, stride, payload,
        offset, end_offset, input_len: input, table_delta };
    proof {
        if strict { boundaries(bytes@, region.start as int, region.end as int, *layout, c); }
        word_in_slice(bytes@, region.start as int, region.end as int, 0);
        if !clears(c) && c.change.index > 0 {
            word_in_slice(bytes@, region.start as int, region.end as int, 4 + (c.change.index - 1) * c.stride + kw);
        }
        if c.change.found {
            word_in_slice(bytes@, region.start as int, region.end as int, 4 + c.change.index * c.stride + kw);
        }
        assert(c.count == words::le_word(bytes@, region.start as int));
        assert(c.table_delta == (c.new_count as int - c.count) * c.stride) by(nonlinear_arith)
            requires c.payload == region.start + 4 + c.count * c.stride,
                c.new_count == if clears(c) { 0int } else { c.count + (if inserts(c) { 1int } else { 0int }) - (if deletes(c) { 1int } else { 0int }) },
                c.table_delta == if clears(c) { -(c.payload - region.start - 4) } else if inserts(c) { c.stride as int } else if deletes(c) { -(c.stride as int) } else { 0int };
        if !clears(c) {
            let start = c.change.index as int;
            let end = start + if c.change.found { 1int } else { 0int };
            assert(0 <= start <= end <= c.count);
            assert(c.offset == boundary(bytes@, region.start as int, c, start)) by { reveal(boundary); }
            assert(c.end_offset == boundary(bytes@, region.start as int, c, end)) by { reveal(boundary); }
            assert(boundary(bytes@, region.start as int, c, start) <= boundary(bytes@, region.start as int, c, end));
            assert(boundary(bytes@, region.start as int, c, end) <= region.end - c.payload);
        }
        assert(c.offset <= c.end_offset <= region.end - c.payload);
        reveal(parameters); reveal(header_parameters);
        if c.count > 0 {
            table_index(4, c.stride, c.count, c.count as int - 1);
            word_in_slice(bytes@, region.start as int, region.end as int, parts.payload.start - 4);
            assert(4 + (c.count as int - 1) * c.stride + kw == parts.payload.start - 4) by(nonlinear_arith)
                requires c.payload == region.start + 4 + c.count * c.stride,
                    c.payload == region.start + parts.payload.start, c.stride as int == kw + 4;
        }
        assert(boundary(bytes@, region.start as int, c, c.count as int) == region.end - c.payload) by {
            reveal(boundary); reveal(map_layout::header_shape);
        }
        assert(header_parameters(bytes@, region, *layout, c, key@));
        if strict { assert(parameters(bytes@, region, *layout, c, key@)); }
    }
    Ok(c)

}

pub(super) fn same_change(a: Option<IndexedMap>, b: Option<IndexedMap>) -> (result: bool)
    ensures result == (a == b),
{
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => fixed_map::same_map(Some(a.change), Some(b.change))
            && a.count == b.count && a.new_count == b.new_count && a.stride == b.stride
            && a.payload == b.payload && a.offset == b.offset && a.end_offset == b.end_offset
            && a.input_len == b.input_len && a.table_delta == b.table_delta,
        _ => false,
    }
}

pub(super) fn destination_exec(plan: &Plan) -> (result: u32)
    requires replacement_checked_for_map(*plan),
    ensures result == destination(*plan),
    no_unwind
{ let c = match plan.variable_map { Some(c) => c, None => panic!("checked edit plan") }; (c.payload as i64 + c.table_delta + c.offset as i64) as u32 }

pub(super) open spec fn replacement_checked_for_map(plan: Plan) -> bool {
    plan.variable_map.is_some() && 0 <= plan.variable_map.unwrap().payload + plan.variable_map.unwrap().table_delta
        + plan.variable_map.unwrap().offset <= u32::MAX
}

pub(super) fn removed_span(plan: &Plan) -> (result: Option<Span>)
    requires plan.variable_map.is_some(),
    ensures result == removed(*plan),
{
    let c = plan.variable_map.unwrap();
    if c.change.found && !matches!(c.change.operation, MapEdit::Clear) {
        Some(Span { start: (c.payload as u64 + c.offset as u64) as u32, end: (c.payload as u64 + c.end_offset as u64) as u32 })
    } else { None }
}

pub(super) open spec fn directory_done(original: Seq<u8>, after: Seq<u8>, plan: Plan, done: int) -> bool {
    let c = plan.variable_map.unwrap();
    forall|i: int| #![trigger after.subrange(plan.region.start + 4 + i * c.stride, plan.region.start + 4 + i * c.stride + c.change.key_width)]
        #![trigger words::le_word(after, plan.region.start + 4 + i * c.stride + c.change.key_width)]
        0 <= i < done ==> {
        let at = plan.region.start + 4 + i * c.stride;
        &&& after.subrange(at, at + c.change.key_width) == new_key(original, plan, i)
        &&& words::le_word(after, at + c.change.key_width) == new_end(original, plan, i)
    }
}

#[verifier::spinoff_prover]
proof fn directory_reframe(original: Seq<u8>, before: Seq<u8>, after: Seq<u8>, plan: Plan, done: int)
    requires bounded(original, plan), plan.variable_map.is_some(),
        before.len() == after.len(), plan.required <= before.len(), 0 <= done <= plan.variable_map.unwrap().new_count,
        directory_done(original, before, plan, done),
        forall|p: int| 0 <= p < after.len() && plan.region.start + 4 <= p < plan.region.start + 4 + done * plan.variable_map.unwrap().stride
            ==> after[p] == before[p],
    ensures directory_done(original, after, plan, done),
{
    hide(words::le_word);

    let c = plan.variable_map.unwrap();
    table_index(plan.region.start + 4,c.stride,c.new_count,done);
    assert(done as u32 as int == done);
    assert forall|i: int| #![trigger after.subrange(plan.region.start + 4 + i * c.stride, plan.region.start + 4 + i * c.stride + c.change.key_width)]
        #![trigger words::le_word(after, plan.region.start + 4 + i * c.stride + c.change.key_width)]
        0 <= i < done implies {
        let a = plan.region.start + 4 + i * c.stride;
        &&& after.subrange(a, a + c.change.key_width) == new_key(original, plan, i)
        &&& words::le_word(after, a + c.change.key_width) == new_end(original, plan, i)
    } by {
        let a = plan.region.start + 4 + i * c.stride;
        table_index(plan.region.start + 4, c.stride, done as u32, i);
        assert(before.subrange(a, a + c.change.key_width) == new_key(original, plan, i));
        assert(after.subrange(a, a + c.change.key_width) =~= before.subrange(a, a + c.change.key_width));
        word_equal(before, a + c.change.key_width, after, a + c.change.key_width);
    }
}

pub(super) open spec fn directory_ready(original: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.variable_map.unwrap();
    forall|i: int| #![trigger after.subrange(plan.region.start + 4 + i * c.stride, plan.region.start + 4 + i * c.stride + c.stride)]
        0 <= i < c.new_count && !(inserts(c) && i == c.change.index) ==> {
        let at = plan.region.start + 4 + i * c.stride;
        let old_at = plan.region.start + 4 + source(c, i) * c.stride;
        after.subrange(at, at + c.stride) == original.subrange(old_at, old_at + c.stride)
    }
}

pub(super) open spec fn directory_frame(original: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.variable_map.unwrap();
    &&& original.len() == after.len()
    &&& forall|p: int| 0 <= p < after.len() && !(plan.region.start + 4 <= p < c.payload + c.table_delta)
        ==> #[trigger] after[p] == original[p]
}

pub(super) open spec fn payload_preserved(original: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.variable_map.unwrap();
    let q = c.payload + c.table_delta;
    let at = q + c.offset;
    &&& after.len() == original.len()
    &&& forall|p: int| q <= p < at ==> #[trigger] after[p] == original[p - c.table_delta]
    &&& forall|p: int| at + c.input_len <= p < plan.new_used ==> #[trigger] after[p] == original[p - plan.delta]
}

pub(super) open spec fn movement(original: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.variable_map.unwrap();
    &&& payload_frame(original, after, plan)
    &&& plan.required <= after.len()
    &&& payload_preserved(original, after, plan)
    &&& words::le_word(after, plan.region.start as int) == c.count
    &&& directory_done(original, after, plan, c.new_count as int)
}

#[verifier::spinoff_prover]
fn shift_directory(bytes: &mut [u8], plan: &Plan)
    requires bounded(old(bytes)@, *plan), plan.variable_map.is_some(), plan.required <= old(bytes).len(),
        !inserts(plan.variable_map.unwrap()),
    ensures directory_ready(old(bytes)@, final(bytes)@, *plan), directory_frame(old(bytes)@, final(bytes)@, *plan),
    no_unwind
{


    proof {  }

    let c = match plan.variable_map { Some(c) => c, None => panic!("checked edit plan") };
    proof { table_index(plan.region.start + 4, c.stride, c.count, c.change.index as int); }
    let at = (plan.region.start as u64 + 4 + c.change.index as u64 * c.stride as u64) as u32;
    if matches!(c.change.operation, MapEdit::Remove) && c.change.found && c.change.index < c.new_count {
        match crate::move_bytes(bytes, Span { start: at + c.stride, end: c.payload }, at) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
    }
    proof {
        assert forall|i: int| #![trigger bytes@.subrange(plan.region.start + 4 + i * c.stride, plan.region.start + 4 + i * c.stride + c.stride)]
            0 <= i < c.new_count && !(inserts(c) && i == c.change.index) implies {
            let a = plan.region.start + 4 + i * c.stride;
            let s = plan.region.start + 4 + source(c, i) * c.stride;
            bytes@.subrange(a, a + c.stride) == old(bytes)@.subrange(s, s + c.stride)
        } by {
            let a = plan.region.start + 4 + i * c.stride;
            let s = plan.region.start + 4 + source(c, i) * c.stride;
            assert(a + c.stride <= c.payload + c.table_delta) by(nonlinear_arith)
                requires a == plan.region.start + 4 + i * c.stride, 0 <= i < c.new_count, 0 <= c.stride,
                    c.payload + c.table_delta == plan.region.start + 4 + c.new_count * c.stride;
            table_index(plan.region.start + 4, c.stride, c.count, source(c, i));
            assert(s == if deletes(c) && i >= c.change.index { a + c.stride }
                else if inserts(c) && i > c.change.index { a - c.stride } else { a }) by(nonlinear_arith)
                requires a == plan.region.start + 4 + i * c.stride,
                    s == plan.region.start + 4 + source(c, i) * c.stride;
            assert(if deletes(c) && i >= c.change.index { at <= a } else { a + c.stride <= at || !deletes(c) }) by(nonlinear_arith)
                requires a == plan.region.start + 4 + i * c.stride,
                    at == plan.region.start + 4 + c.change.index * c.stride,
                    c.stride > 0;
            assert(a + c.stride == plan.region.start + 4 + i * c.stride + c.stride);
            assert forall|k: int| #![trigger bytes@[a+k]] 0 <= k < c.stride implies bytes@[a + k] == old(bytes)@[s + k] by { }
            assert(bytes@.subrange(a, a + c.stride) =~= old(bytes)@.subrange(s, s + c.stride));
        }
        assert(directory_ready(old(bytes)@, bytes@, *plan));
    }
}

/// Unchanged entries initialize the complete key/end directory invariant.
#[verifier::spinoff_prover]
pub(super) proof fn header_suffix_initialized(original: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires bounded(original, plan), plan.variable_map.is_some(),
        plan.required <= after.len(), original.len() == after.len(), directory_ready(original, after, plan),
    ensures directory_done(original, after, plan, plan.variable_map.unwrap().change.index as int),
{


    let c = plan.variable_map.unwrap();
        assert(c.change.index <= c.new_count);
        table_index(plan.region.start + 4, c.stride, c.new_count, c.change.index as int);
        assert forall|j: int| #![trigger after.subrange(plan.region.start + 4 + j * c.stride, plan.region.start + 4 + j * c.stride + c.change.key_width)]
            #![trigger words::le_word(after, plan.region.start + 4 + j * c.stride + c.change.key_width)]
            0 <= j < c.change.index implies {
                let a = plan.region.start + 4 + j * c.stride;
                &&& after.subrange(a, a + c.change.key_width) == new_key(original, plan, j)
                &&& words::le_word(after, a + c.change.key_width) == new_end(original, plan, j)
            } by {
                let a = plan.region.start + 4 + j * c.stride;
                table_index(plan.region.start + 4, c.stride, c.count, j);
                table_index(plan.region.start + 4, c.stride, c.new_count, j);
                assert(after.subrange(a, a + c.stride) == original.subrange(a, a + c.stride));
                record_prefix(original, a, after, a, c.stride as int, c.change.key_width as int);
                record_word(original, a, after, a, c.stride as int, c.change.key_width as int);
            }
        assert(directory_done(original, after, plan, c.change.index as int));

}

#[verifier::spinoff_prover]
pub(super) proof fn suffix_initialized(original: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), plan.variable_map.is_some(),
        plan.required <= after.len(), original.len() == after.len(), directory_ready(original, after, plan),
    ensures directory_done(original, after, plan, plan.variable_map.unwrap().change.index as int),
{
    hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(map_layout::map_shape);
    checked_geometry(original, plan);
    let c = plan.variable_map.unwrap();
        assert(c.change.index <= c.new_count);
        table_index(plan.region.start + 4, c.stride, c.new_count, c.change.index as int);
        assert forall|j: int| #![trigger after.subrange(plan.region.start + 4 + j * c.stride, plan.region.start + 4 + j * c.stride + c.change.key_width)]
            #![trigger words::le_word(after, plan.region.start + 4 + j * c.stride + c.change.key_width)]
            0 <= j < c.change.index implies {
                let a = plan.region.start + 4 + j * c.stride;
                &&& after.subrange(a, a + c.change.key_width) == new_key(original, plan, j)
                &&& words::le_word(after, a + c.change.key_width) == new_end(original, plan, j)
            } by {
                let a = plan.region.start + 4 + j * c.stride;
                table_index(plan.region.start + 4, c.stride, c.count, j);
                table_index(plan.region.start + 4, c.stride, c.new_count, j);
                assert(after.subrange(a, a + c.stride) == original.subrange(a, a + c.stride));
                record_prefix(original, a, after, a, c.stride as int, c.change.key_width as int);
                record_word(original, a, after, a, c.stride as int, c.change.key_width as int);
            }
        assert(directory_done(original, after, plan, c.change.index as int));

}

// This helper needs the directory contract, rather than reinterpreting the
// temporarily moved bytes as a valid map with the old count.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
fn fill_directory(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>)
    requires replacement_checked(original, *plan), plan.variable_map.is_some(), plan.required <= old(bytes).len(),
        old(bytes).len() == original.len(), directory_ready(original, old(bytes)@, *plan),
    ensures directory_done(original, final(bytes)@, *plan, plan.variable_map.unwrap().new_count as int),
        directory_frame(old(bytes)@, final(bytes)@, *plan),
    no_unwind
{
    hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(map_layout::map_shape);

    proof { checked_geometry(original, *plan); }

    let c = match plan.variable_map { Some(c) => c, None => panic!("checked edit plan") };
    let ghost before = bytes@;
    let mut i = c.change.index;
    proof {
        suffix_initialized(original, bytes@, *plan);
        assert(i <= c.new_count);
        table_index(plan.region.start + 4, c.stride, c.new_count, i as int);
    }
    if i < c.new_count && matches!(c.change.operation, MapEdit::Insert | MapEdit::Replace) {
        let at = (plan.region.start as u64 + 4 + i as u64 * c.stride as u64) as u32;
        proof {
            table_index(plan.region.start + 4, c.stride, c.new_count, i as int);
            assert(at + c.stride <= c.payload + c.table_delta) by(nonlinear_arith)
                requires at == plan.region.start + 4 + i * c.stride, i < c.new_count,
                    c.payload + c.table_delta == plan.region.start + 4 + c.new_count * c.stride;
        }
        let ghost iteration = bytes@;
        if matches!(c.change.operation, MapEdit::Insert) && !c.change.found {
            write_query(bytes, at, plan.key);
        }
        let end = c.offset + c.input_len;
        proof { end_bounds(original, *plan, i as int); assert(end as int == new_end(original, *plan, i as int)); }
        crate::words::write_contained_word(bytes, (at + c.change.key_width) as usize, end);
        proof {
            directory_reframe(original, iteration, bytes@, *plan, i as int);
            assert forall|j: int| #![trigger bytes@.subrange(plan.region.start + 4 + j * c.stride, plan.region.start + 4 + j * c.stride + c.change.key_width)]
                #![trigger words::le_word(bytes@, plan.region.start + 4 + j * c.stride + c.change.key_width)]
                0 <= j < i + 1 implies {
                let a = plan.region.start + 4 + j * c.stride;
                &&& bytes@.subrange(a, a + c.change.key_width) == new_key(original, *plan, j)
                &&& words::le_word(bytes@, a + c.change.key_width) == new_end(original, *plan, j)
            } by {
                let a = plan.region.start + 4 + j * c.stride;
                if j < i {
                    assert(directory_done(original, bytes@, *plan, i as int));
                } else {
                    if !inserts(c) || j != c.change.index {
                        let s = plan.region.start + 4 + source(c, j) * c.stride;
                        assert(iteration.subrange(a, a + c.stride) == original.subrange(s, s + c.stride));
                        record_prefix(original, s, iteration, a, c.stride as int, c.change.key_width as int);
                        assert(bytes@.subrange(a, a + c.change.key_width) =~= new_key(original, *plan, j));
                    }
                }
            }
        }
        proof {
            assert forall|j: int| #![trigger bytes@.subrange(plan.region.start + 4 + j*c.stride, plan.region.start + 4 + j*c.stride + c.stride)]
                i + 1 <= j < c.new_count && !(inserts(c) && j == c.change.index) implies {
                let a = plan.region.start + 4 + j*c.stride;
                let s = plan.region.start + 4 + source(c,j)*c.stride;
                bytes@.subrange(a,a+c.stride) == original.subrange(s,s+c.stride)
            } by {
                let a = plan.region.start + 4 + j*c.stride;
                let source_at = plan.region.start + 4 + source(c,j)*c.stride;
                table_index(plan.region.start + 4,c.stride,c.new_count,j);
                assert(at+c.stride <= a) by(nonlinear_arith)
                    requires at == plan.region.start + 4 + i*c.stride, a == plan.region.start + 4 + j*c.stride,
                        i < j, c.stride > 0;
                assert(bytes@.subrange(a,a+c.stride) =~= iteration.subrange(a,a+c.stride));
                assert(iteration.subrange(a,a+c.stride) == original.subrange(source_at,source_at+c.stride));
            }
        }
        i += 1;
    }
    proof {
        assert(directory_done(original, bytes@, *plan, i as int));
        assert(directory_frame(before, bytes@, *plan));
        assert forall|j: int| #![trigger bytes@.subrange(plan.region.start + 4 + j * c.stride, plan.region.start + 4 + j * c.stride + c.stride)]
            i <= j < c.new_count ==> {
                let a = plan.region.start + 4 + j * c.stride;
                let source_at = plan.region.start + 4 + source(c, j) * c.stride;
                bytes@.subrange(a, a + c.stride) == original.subrange(source_at, source_at + c.stride)
            } by { }
    }
    fill_directory_suffix(bytes, plan, i, Ghost(original));
    proof { assert(directory_frame(before, bytes@, *plan)); }
}

/// Finishes the admitted remainder through one exact directory loan.
#[verifier::spinoff_prover]
#[verifier::rlimit(160)]
#[inline(always)]
fn fill_directory_suffix(bytes: &mut [u8], plan: &Plan, first: u32, Ghost(original): Ghost<Seq<u8>>)
    requires replacement_checked(original, *plan), plan.variable_map.is_some(),
        old(bytes).len() == original.len(), plan.required <= old(bytes).len(),
        plan.variable_map.unwrap().change.index <= first <= plan.variable_map.unwrap().new_count,
        first < plan.variable_map.unwrap().new_count ==> !(writes(plan.variable_map.unwrap()) && first == plan.variable_map.unwrap().change.index),
        directory_done(original, old(bytes)@, *plan, first as int),
        forall|j: int| #![trigger old(bytes)@.subrange(plan.region.start + 4 + j * plan.variable_map.unwrap().stride,
            plan.region.start + 4 + j * plan.variable_map.unwrap().stride + plan.variable_map.unwrap().stride)]
            first <= j < plan.variable_map.unwrap().new_count ==> {
                let c = plan.variable_map.unwrap();
                let a = plan.region.start + 4 + j * c.stride;
                let source_at = plan.region.start + 4 + source(c, j) * c.stride;
                old(bytes)@.subrange(a, a + c.stride) == original.subrange(source_at, source_at + c.stride)
            },
    ensures directory_done(original, final(bytes)@, *plan, plan.variable_map.unwrap().new_count as int),
        directory_frame(old(bytes)@, final(bytes)@, *plan),
    no_unwind
{
    hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(map_layout::map_shape);
    proof { checked_geometry(original, *plan); }
    let c = match plan.variable_map { Some(c) => c, None => panic!("checked edit plan") };
    let i = first;
    let ghost before = bytes@;
    if i < c.new_count {
        proof { table_index(plan.region.start + 4, c.stride, c.new_count, i as int); }
        let at = (plan.region.start as u64 + 4 + i as u64 * c.stride as u64) as u32;
        let table_end = (c.payload as i64 + c.table_delta) as u32;
        let length = table_end - at;
        proof {
            assert(length == (c.new_count - i) * c.stride) by(nonlinear_arith)
                requires length == table_end - at, table_end == plan.region.start + 4 + c.new_count * c.stride,
                    at == plan.region.start + 4 + i * c.stride;
        }
        let delta = c.input_len as i64 - (c.end_offset - c.offset) as i64;
        let ghost prior = bytes@;
        let ghost patched;
        {
            let suffix = borrow_destination(bytes, at, length);
            proof {
                assert forall|k: int| 0 <= k < c.new_count - i implies
                    0 <= #[trigger] words::le_word(suffix@, k * c.stride + c.change.key_width) + delta <= u32::MAX by {
                    let j = i + k;
                    let a = plan.region.start + 4 + j * c.stride;
                    let source_at = plan.region.start + 4 + source(c, j) * c.stride;
                    table_index(plan.region.start + 4, c.stride, c.new_count, j);
                    table_index(plan.region.start + 4, c.stride, c.count, source(c, j));
                    assert(prior.subrange(a, a + c.stride) == original.subrange(source_at, source_at + c.stride));
                    record_word(original, source_at, prior, a, c.stride as int, c.change.key_width as int);
                    assert(a == at + k * c.stride) by(nonlinear_arith)
                        requires a == plan.region.start + 4 + j * c.stride,
                            at == plan.region.start + 4 + i * c.stride, k == j - i;
                    word_in_slice(prior, at as int, at + length, k * c.stride + c.change.key_width);
                    end_bounds(original, *plan, j);
                    assert(new_end(original, *plan, j) == words::le_word(suffix@, k * c.stride + c.change.key_width) + delta);
                }
            }
            patch_directory_suffix(suffix, c.change.key_width as usize, c.stride as usize, delta, (c.new_count - i) as usize);
            proof { patched = suffix@; }
        }
        proof {
            directory_reframe(original, prior, bytes@, *plan, i as int);
            assert forall|j: int| #![trigger bytes@.subrange(plan.region.start + 4 + j * c.stride, plan.region.start + 4 + j * c.stride + c.change.key_width)]
                #![trigger words::le_word(bytes@, plan.region.start + 4 + j * c.stride + c.change.key_width)]
                0 <= j < c.new_count implies {
                    let a = plan.region.start + 4 + j * c.stride;
                    &&& bytes@.subrange(a, a + c.change.key_width) == new_key(original, *plan, j)
                    &&& words::le_word(bytes@, a + c.change.key_width) == new_end(original, *plan, j)
                } by {
                    if j < i {
                        assert(directory_done(original, bytes@, *plan, i as int));
                    } else {
                        let a = plan.region.start + 4 + j * c.stride;
                        let source_at = plan.region.start + 4 + source(c, j) * c.stride;
                        table_index(plan.region.start + 4, c.stride, c.new_count, j);
                        table_index(plan.region.start + 4, c.stride, c.count, source(c, j));
                        let k = j - i;
                        assert(a == at + k * c.stride) by(nonlinear_arith)
                        requires a == plan.region.start + 4 + j * c.stride,
                            at == plan.region.start + 4 + i * c.stride, k == j - i;
                        word_in_slice(prior, at as int, at + length, k * c.stride + c.change.key_width);
                        word_in_slice(bytes@, at as int, at + length, k * c.stride + c.change.key_width);
                        record_word(original, source_at, prior, a, c.stride as int, c.change.key_width as int);
                        record_prefix(original, source_at, prior, a, c.stride as int, c.change.key_width as int);
                        assert forall|p: int| 0 <= p < c.change.key_width implies #[trigger] bytes@[a + p] == prior[a + p] by {
                            assert(patched[k * c.stride + p] == prior.subrange(at as int, at + length)[k * c.stride + p]);
                        }
                        assert forall|p: int| 0 <= p < c.change.key_width implies
                            #[trigger] bytes@.subrange(a, a + c.change.key_width)[p]
                                == prior.subrange(a, a + c.change.key_width)[p] by {
                            assert(bytes@[a + p] == prior[a + p]);
                        }
                        assert(bytes@.subrange(a, a + c.change.key_width) =~= prior.subrange(a, a + c.change.key_width));
                    }
                }
            assert(directory_done(original, bytes@, *plan, c.new_count as int));
            assert forall|p: int| 0 <= p < bytes.len() && !(plan.region.start + 4 <= p < c.payload + c.table_delta)
                implies #[trigger] bytes@[p] == before[p] by {
                assert(!(at <= p < at + length));
                assert(prior[p] == before[p]);
            }
            assert(directory_frame(before, bytes@, *plan));
        }
    }
}

fn write_query(bytes: &mut [u8], at: u32, key: &[u8])
    requires at as int + key.len() <= old(bytes).len(),
    ensures final(bytes).len() == old(bytes).len(), final(bytes)@.subrange(at as int, at + key.len()) == key@,
        forall|p: int| 0 <= p < old(bytes).len() && !(at <= p < at + key.len()) ==> final(bytes)@[p] == old(bytes)@[p],
    no_unwind
{
    let mut i = 0usize;
    while i < key.len()
        invariant i <= key.len(), at as int + key.len() <= bytes.len(), bytes.len() == old(bytes).len(),
            forall|j: int| 0 <= j < i ==> bytes@[at + j] == key@[j],
            forall|p: int| 0 <= p < old(bytes).len() && !(at <= p < at + i) ==> bytes@[p] == old(bytes)@[p],
        decreases key.len() - i,
    { bytes[at as usize + i] = key[i]; i += 1; }
    assert(bytes@.subrange(at as int, at + key.len()) =~= key@);
}

proof fn end_bounds(original: Seq<u8>, plan: Plan, i: int)
    requires replacement_checked(original, plan), plan.variable_map.is_some(), 0 <= i < plan.variable_map.unwrap().new_count,
    ensures 0 <= new_end(original, plan, i) <= plan.region.end + plan.delta - (plan.variable_map.unwrap().payload + plan.variable_map.unwrap().table_delta),
{
    hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(map_layout::map_shape);

    checked_geometry(original, plan);
 }

#[verifier::spinoff_prover]
proof fn end_order(original: Seq<u8>, plan: Plan, i: int, j: int)
    requires replacement_checked(original, plan), plan.variable_map.is_some(),
        0 <= i <= j < plan.variable_map.unwrap().new_count,
    ensures new_end(original,plan,i) <= new_end(original,plan,j),
{
    hide(words::le_word); hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(map_layout::map_shape);
    checked_geometry(original,plan);
    let c = plan.variable_map.unwrap();
    let b = plan.region.start as int;
    if i < c.change.index {
        assert(boundary(original,b,c,source(c,i)+1) <= c.offset);
    }
    if j > c.change.index {
        assert(c.end_offset <= boundary(original,b,c,source(c,j)+1));
    }
    if !(writes(c) && i == c.change.index) && !(writes(c) && j == c.change.index) {
        assert(boundary(original,b,c,source(c,i)+1) <= boundary(original,b,c,source(c,j)+1));
    }
}

#[verifier::spinoff_prover]
pub(super) proof fn terminal_end(original: Seq<u8>, plan: Plan)
    requires replacement_checked(original,plan), plan.variable_map.is_some(),
    ensures if plan.variable_map.unwrap().new_count == 0 {
        plan.region.end + plan.delta == plan.variable_map.unwrap().payload + plan.variable_map.unwrap().table_delta
    } else { new_end(original,plan,plan.variable_map.unwrap().new_count as int-1)
        == plan.region.end + plan.delta - (plan.variable_map.unwrap().payload + plan.variable_map.unwrap().table_delta) },
{
    hide(words::le_word); hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(map_layout::map_shape);
    checked_geometry(original,plan);
    let c = plan.variable_map.unwrap();
    let i = c.new_count as int-1;
    if c.new_count > 0 {
        if writes(c) && i == c.change.index { assert(c.end_offset == plan.region.end-c.payload); }
        else if i < c.change.index { assert(c.end_offset == plan.region.end-c.payload); }
        else { assert(source(c,i)+1 == c.count); }
    } else if !clears(c) {
        assert(c.count == 0 || deletes(c) && c.count == 1);
        assert(c.offset == 0 && c.end_offset == plan.region.end-c.payload);
    }
}

#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
pub(super) fn move_map(bytes: &mut [u8], plan: &Plan)
    requires replacement_checked(old(bytes)@, *plan), plan.variable_map.is_some(), plan.required <= old(bytes).len(),
    ensures movement(old(bytes)@, final(bytes)@, *plan),
    no_unwind
{
    hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(map_layout::map_shape);

    proof { checked_geometry(old(bytes)@, *plan); }

    let ghost original = bytes@;
    let c = match plan.variable_map { Some(c) => c, None => panic!("checked edit plan") };
    let q = (c.payload as i64 + c.table_delta) as u32;
    let split = c.payload + c.offset;
    let end = c.payload + c.end_offset;
    if matches!(c.change.operation, MapEdit::Insert) && !c.change.found {
        // Preserve B and the outside suffix, then A, before enlarging D.
        if split < plan.old_used {
            match crate::move_bytes(bytes, Span { start: split, end: plan.old_used }, (split as i64 + plan.delta) as u32) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
        }
        if c.offset != 0 {
            match crate::move_bytes(bytes, Span { start: c.payload, end: split }, q) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
        }
        let ghost payload = bytes@;
        proof { assert(payload_preserved(original, payload, *plan)); assert(payload_frame(original, payload, *plan)); }
        // Directory shifting depends only on its old source bytes. A ghost
        // reframe supplies the unchanged metadata without validating moved data.
        shift_ready_directory(bytes, plan, Ghost(original));
        fill_directory(bytes, plan, Ghost(original));
        proof {
            assert(payload_preserved(original, bytes@, *plan));
            assert(payload_frame(original, bytes@, *plan));
        }
    } else {
        // Consume old trailing key/end records before A can overwrite them.
        shift_directory(bytes, plan);
        fill_directory(bytes, plan, Ghost(original));
        let ghost directory = bytes@;
        proof {
            assert forall|p: int| q <= p < original.len() implies #[trigger] directory[p] == original[p] by { }
        }
        if c.table_delta != 0 && c.offset != 0 {
            match crate::move_bytes(bytes, Span { start: c.payload, end: split }, q) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
        }
        let ghost prefix = bytes@;
        if plan.delta != 0 && end < plan.old_used {
            match crate::move_bytes(bytes, Span { start: end, end: plan.old_used }, q + c.offset + c.input_len) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
        }
        proof {
            directory_reframe(original, directory, bytes@, *plan, c.new_count as int);
            assert(q + c.offset + c.input_len - plan.delta == end);
            assert forall|p: int| q <= p < q + c.offset implies #[trigger] bytes@[p] == original[p - c.table_delta] by {
                assert(directory[p - c.table_delta] == original[p - c.table_delta]);
                if c.table_delta != 0 && c.offset != 0 { assert(prefix[p] == directory[p - c.table_delta]); }
                else { assert(prefix[p] == directory[p]); }
                assert(bytes@[p] == prefix[p]);
            }
            assert forall|p: int| q + c.offset + c.input_len <= p < plan.new_used implies #[trigger] bytes@[p] == original[p - plan.delta] by {
                assert(end <= p - plan.delta < plan.old_used);
                assert(prefix[p - plan.delta] == directory[p - plan.delta]);
                assert(directory[p - plan.delta] == original[p - plan.delta]);
            }
            assert(payload_preserved(original, bytes@, *plan));
        }
    }
    proof {
        assert forall|p: int| 0 <= p < original.len() && (p < plan.region.start || plan.required <= p)
            implies bytes@[p] == original[p] by { }
        assert(payload_preserved(original, bytes@, *plan));
        assert(payload_frame(original, bytes@, *plan));
        assert(words::le_word(bytes@, plan.region.start as int) == c.count);
        assert(directory_done(original, bytes@, *plan, c.new_count as int));
    }
}

#[verifier::spinoff_prover]
fn shift_ready_directory(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>)
    requires bounded(original, *plan), plan.variable_map.is_some(), plan.required <= old(bytes).len(),
        original.len() == old(bytes).len(), inserts(plan.variable_map.unwrap()),
        forall|p: int| plan.region.start <= p < plan.variable_map.unwrap().payload ==> old(bytes)@[p] == original[p],
    ensures directory_ready(original, final(bytes)@, *plan), directory_frame(old(bytes)@, final(bytes)@, *plan),
    no_unwind
{


    proof {  }

    let c = match plan.variable_map { Some(c) => c, None => panic!("checked edit plan") };
    proof { table_index(plan.region.start + 4, c.stride, c.count, c.change.index as int); }
    let at = (plan.region.start as u64 + 4 + c.change.index as u64 * c.stride as u64) as u32;
    if c.change.index < c.count {
        match crate::move_bytes(bytes, Span { start: at, end: c.payload }, at + c.stride) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
    }
    proof {
        assert forall|i: int| #![trigger bytes@.subrange(plan.region.start + 4 + i * c.stride, plan.region.start + 4 + i * c.stride + c.stride)]
            0 <= i < c.new_count && !(inserts(c) && i == c.change.index) implies {
            let a = plan.region.start + 4 + i * c.stride;
            let s = plan.region.start + 4 + source(c, i) * c.stride;
            bytes@.subrange(a, a + c.stride) == original.subrange(s, s + c.stride)
        } by {
            let a = plan.region.start + 4 + i * c.stride;
            let s = plan.region.start + 4 + source(c, i) * c.stride;
            assert(a + c.stride <= c.payload + c.table_delta && s + c.stride <= c.payload) by(nonlinear_arith)
                requires a == plan.region.start + 4 + i * c.stride, s == plan.region.start + 4 + source(c, i) * c.stride,
                    0 <= i < c.new_count, 0 <= source(c, i) < c.count,
                    c.payload == plan.region.start + 4 + c.count * c.stride,
                    c.payload + c.table_delta == plan.region.start + 4 + c.new_count * c.stride;
            assert(s == if i < c.change.index { a } else { a - c.stride }) by(nonlinear_arith)
                requires a == plan.region.start + 4 + i * c.stride,
                    s == plan.region.start + 4 + source(c, i) * c.stride,
                    source(c, i) == if i < c.change.index { i } else { i - 1 };
            assert(if i < c.change.index { a + c.stride <= at } else { at + c.stride <= a }) by(nonlinear_arith)
                requires a == plan.region.start + 4 + i * c.stride,
                    at == plan.region.start + 4 + c.change.index * c.stride,
                    i != c.change.index, c.stride > 0;
            assert(a + c.stride == plan.region.start + 4 + (i + 1) * c.stride) by(nonlinear_arith)
                requires a == plan.region.start + 4 + i * c.stride;
            assert forall|k: int| #![trigger bytes@[a+k]] 0 <= k < c.stride implies bytes@[a + k] == original[s + k] by {
                if i < c.change.index { assert(bytes@[a+k] == old(bytes)@[a+k]); }
                else { assert(bytes@[a+k] == old(bytes)@[a+k-c.stride]); }
                assert(old(bytes)@[s + k] == original[s + k]);
            }
            assert(bytes@.subrange(a, a + c.stride) =~= original.subrange(s, s + c.stride));
        }
        assert(directory_ready(original, bytes@, *plan));
    }
}

#[verifier::spinoff_prover]
pub(super) proof fn header_movement_framed(original: Seq<u8>, before: Seq<u8>, plan: Plan)
    requires bounded(original, plan), plan.variable_map.is_some(), movement(original, before, plan),
    ensures forall|after: Seq<u8>| after.len() == before.len()
        && (forall|p: int| 0 <= p < before.len() && !(destination_start(plan) <= p < destination_start(plan) + input_size(plan))
            ==> after[p] == before[p]) ==> #[trigger] movement(original, after, plan),
{




    assert forall|after: Seq<u8>| after.len() == before.len()
        && (forall|p: int| 0 <= p < before.len() && !(destination_start(plan) <= p < destination_start(plan) + input_size(plan))
            ==> after[p] == before[p]) implies #[trigger] movement(original, after, plan) by {
        directory_reframe(original, before, after, plan, plan.variable_map.unwrap().new_count as int);
        word_equal(before, plan.region.start as int, after, plan.region.start as int);
    }
}

#[verifier::spinoff_prover]
pub(super) proof fn movement_framed(original: Seq<u8>, before: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), plan.variable_map.is_some(), movement(original, before, plan),
    ensures forall|after: Seq<u8>| after.len() == before.len()
        && (forall|p: int| 0 <= p < before.len() && !(destination_start(plan) <= p < destination_start(plan) + input_size(plan))
            ==> after[p] == before[p]) ==> #[trigger] movement(original, after, plan),
{
    hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(map_layout::map_shape);

    checked_geometry(original, plan);

    assert forall|after: Seq<u8>| after.len() == before.len()
        && (forall|p: int| 0 <= p < before.len() && !(destination_start(plan) <= p < destination_start(plan) + input_size(plan))
            ==> after[p] == before[p]) implies #[trigger] movement(original, after, plan) by {
        directory_reframe(original, before, after, plan, plan.variable_map.unwrap().new_count as int);
        word_equal(before, plan.region.start as int, after, plan.region.start as int);
    }
}

pub(super) open spec fn edit_result(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.variable_map.unwrap();
    &&& original.len() == after.len() && encoded.len() == original.len()
    &&& plan.required <= original.len()
    &&& forall|p: int| 0 <= p < after.len() && (p < 8 || plan.required <= p
        || (12 <= p < plan.region.start && (plan.delta == 0 || !ancestor_changed(plan, plan.trail.depth as int, p))))
        ==> #[trigger] after[p] == original[p]
    &&& words::le_word(after, 8) == plan.new_used
    &&& ancestors_delta(original, after, plan, plan.trail.depth as int)
    &&& forall|p: int| plan.new_used <= p < plan.old_used ==> #[trigger] after[p] == 0
    &&& payload_preserved(original, after, plan)
    &&& words::le_word(after, plan.region.start as int) == c.new_count
    &&& directory_done(original, after, plan, c.new_count as int)
    &&& after.subrange(destination(plan) as int, destination(plan) + c.input_len)
        == encoded.subrange(destination(plan) as int, destination(plan) + c.input_len)
}

pub(super) open spec fn map_result(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    let n = fixed_map::node(plan);
    let c = plan.variable_map.unwrap();
    let before = node_bytes(original, n);
    let live = after.subrange(n.region.start as int, n.region.end + plan.delta);
    let value = encoded.subrange(destination(plan) as int, destination(plan) + c.input_len);
    search::rep_map(before, n.layout, search::map_model(before, n.layout)) ==> search::rep_map(live, n.layout, fixed_map::edited_map(search::map_model(before, n.layout), c.change, plan.key@, value))
}

#[verifier::spinoff_prover]
pub(super) fn finish_map(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>)
    requires replacement_checked(original, *plan), plan.variable_map.is_some(), movement(original, old(bytes)@, *plan),
    ensures edit_result(original, old(bytes)@, final(bytes)@, *plan), map_result(original, old(bytes)@, final(bytes)@, *plan),
    no_unwind
{
    hide(map_result); hide(words::le_word); hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical); hide(map_layout::map_shape);

    proof { checked_geometry(original, *plan); }

    let c = match plan.variable_map { Some(c) => c, None => panic!("checked edit plan") };
    let ghost encoded = bytes@;
    if c.new_count != c.count { crate::words::write_contained_word(bytes, (plan.region.start) as usize, c.new_count); }
    let ghost counted = bytes@;
    finish_replacement(bytes, plan, Ghost(original));
    proof {
        directory_reframe(original, encoded, bytes@, *plan, c.new_count as int);
        word_equal(counted, plan.region.start as int, bytes@, plan.region.start as int);
        assert(payload_preserved(original, bytes@, *plan));
        assert(bytes@.subrange(destination(*plan) as int, destination(*plan) + c.input_len)
            =~= encoded.subrange(destination(*plan) as int, destination(*plan) + c.input_len));
        assert(edit_result(original, encoded, bytes@, *plan));
        refinement(original, encoded, bytes@, *plan);
    }
}

proof fn result_facts(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires edit_result(original,encoded,after,plan), plan.variable_map.is_some(),
    ensures original.len() == after.len(), encoded.len() == original.len(), plan.required <= original.len(),
        directory_done(original,after,plan,plan.variable_map.unwrap().new_count as int),
        words::le_word(after,plan.region.start as int) == plan.variable_map.unwrap().new_count,
        payload_preserved(original,after,plan),
        after.subrange(destination(plan) as int,destination(plan)+plan.variable_map.unwrap().input_len)
            == encoded.subrange(destination(plan) as int,destination(plan)+plan.variable_map.unwrap().input_len),
{ }

spec fn wire_shape(after: Seq<u8>, plan: Plan) -> bool {
    let c = plan.variable_map.unwrap();
    let n = fixed_map::node(plan);
    let live = after.subrange(n.region.start as int,n.region.end+plan.delta);
    &&& map_layout::map_shape(live,n.layout)
    &&& words::le_word(live,0) == c.new_count
    &&& map_layout::payload_start(live,n.layout) == c.payload+c.table_delta-n.region.start

}

#[verifier::spinoff_prover]
proof fn resulting_shape(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original,plan), plan.variable_map.is_some(), edit_result(original,encoded,after,plan),
    ensures wire_shape(after,plan),
{
    hide(replacement_checked); hide(edit_result); hide(words::le_word); hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical);
    checked_geometry(original, plan);
    result_facts(original,encoded,after,plan);
    let c = plan.variable_map.unwrap();
    let n = fixed_map::node(plan);
    let before = node_bytes(original, n);
    let live = after.subrange(n.region.start as int, n.region.end + plan.delta);
    let value = encoded.subrange(destination(plan) as int, destination(plan) + c.input_len);
    let q = c.payload + c.table_delta;
    word_in_slice(after, n.region.start as int, n.region.end + plan.delta, 0);
    assert(words::le_word(live, 0) == c.new_count);
    assert(map_layout::payload_start(live, n.layout) == q - n.region.start) by(nonlinear_arith)
        requires map_layout::payload_start(live, n.layout) == 4 + c.new_count * map_layout::entry_stride(n.layout),
            c.new_count == 0 || c.stride == map_layout::entry_stride(n.layout),
            q == n.region.start + 4 + c.new_count * c.stride;
    assert forall|i: int| #![trigger words::le_word(live, 4 + i * c.stride + c.change.key_width)]
        0 <= i < c.new_count implies {
        let at = 4 + i * c.stride + c.change.key_width;
        &&& words::le_word(live, at) == new_end(original, plan, i)
        &&& words::le_word(live, at) <= live.len() - map_layout::payload_start(live, n.layout)
        &&& if i == 0 { 0 <= words::le_word(live, at) }
            else { words::le_word(live, at - c.stride) <= words::le_word(live, at) }
    } by {
        table_index(n.region.start + 4, c.stride, c.new_count, i);
        word_in_slice(after, n.region.start as int, n.region.end + plan.delta, 4 + i * c.stride + c.change.key_width);
        end_bounds(original, plan, i);
        assert(directory_done(original,after,plan,c.new_count as int));
        assert(words::le_word(after,plan.region.start+4+i*c.stride+c.change.key_width) == new_end(original,plan,i));
        assert(words::le_word(live,4+i*c.stride+c.change.key_width) == new_end(original,plan,i));
        if i > 0 {
            table_index(n.region.start + 4,c.stride,c.new_count,i-1);
            end_order(original,plan,i-1,i);
            assert(4+i*c.stride+c.change.key_width-c.stride == 4+(i-1)*c.stride+c.change.key_width) by(nonlinear_arith);
            word_in_slice(after, n.region.start as int, n.region.end + plan.delta, 4 + (i-1)*c.stride + c.change.key_width);
            end_bounds(original, plan, i - 1);
            assert(words::le_word(after, n.region.start + 4 + (i-1) * c.stride + c.change.key_width) == new_end(original, plan, i - 1));
            assert(words::le_word(live, 4 + i * c.stride + c.change.key_width - c.stride) == new_end(original, plan, i - 1));
        }
    }
    terminal_end(original,plan);
    if c.new_count > 0 {
        table_index(n.region.start + 4,c.stride,c.new_count,c.new_count as int-1);
        word_in_slice(after,n.region.start as int,n.region.end+plan.delta,4+(c.new_count-1)*c.stride+c.change.key_width);
    }
    assert(map_layout::map_shape(live, n.layout));

}

#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
proof fn entry_projection(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan, i: int)
    requires replacement_checked(original,plan), plan.variable_map.is_some(), edit_result(original,encoded,after,plan),
        wire_shape(after,plan), 0 <= i < plan.variable_map.unwrap().new_count,
    ensures fixed_map::entry_matches(node_bytes(original,fixed_map::node(plan)),
        after.subrange(fixed_map::node(plan).region.start as int,fixed_map::node(plan).region.end+plan.delta),
        fixed_map::node(plan).layout,plan.variable_map.unwrap().change,plan.key@,
        encoded.subrange(destination(plan) as int,destination(plan)+plan.variable_map.unwrap().input_len),i),
{
    hide(replacement_checked); hide(edit_result); hide(map_layout::map_shape); hide(words::le_word); hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical);
    checked_geometry(original, plan);
    result_facts(original,encoded,after,plan);
    let c = plan.variable_map.unwrap();
    let n = fixed_map::node(plan);
    let before = node_bytes(original, n);
    let live = after.subrange(n.region.start as int, n.region.end + plan.delta);
    let value = encoded.subrange(destination(plan) as int, destination(plan) + c.input_len);
    let q = c.payload + c.table_delta;

        table_index(n.region.start+4,c.stride,c.new_count,i);
        word_in_slice(after,n.region.start as int,n.region.end+plan.delta,4+i*c.stride+c.change.key_width);
        if i>0 {
            table_index(n.region.start+4,c.stride,c.new_count,i-1);
            word_in_slice(after,n.region.start as int,n.region.end+plan.delta,4+(i-1)*c.stride+c.change.key_width);
        }
        assert(directory_done(original,after,plan,c.new_count as int));
        assert(words::le_word(after,plan.region.start+4+i*c.stride+c.change.key_width) == new_end(original,plan,i));
        end_bounds(original,plan,i);
        if i>0 {
            assert(words::le_word(after,plan.region.start+4+(i-1)*c.stride+c.change.key_width) == new_end(original,plan,i-1));
            end_bounds(original,plan,i-1);
        }
        let s = source(c, i);
        let span = map_layout::value_span(live, n.layout, i as u32);
        let a = q + if i == 0 { 0int } else { new_end(original, plan, i - 1) };
        let z = q + new_end(original, plan, i);
        assert(span.start == a - n.region.start && span.end == z - n.region.start);
        table_index(n.region.start + 4,c.stride,c.new_count,i);
        let key_at = n.region.start + 4 + i*c.stride;
        assert(after.subrange(key_at,key_at+c.change.key_width) == new_key(original,plan,i));
        assert(search::encoded_key(live, n.layout, i) =~= after.subrange(key_at,key_at+c.change.key_width));
        assert(search::encoded_key(live, n.layout, i) == new_key(original, plan, i));
        if writes(c) && i == c.change.index {
            assert(a == destination(plan) && z == a + c.input_len);
            if c.change.found {
                let old_at = n.region.start + 4 + i*c.stride;
                table_index(n.region.start+4,c.stride,c.count,i);
                assert(search::encoded_key(before,n.layout,i) == plan.key@);
                assert(new_key(original,plan,i) =~= search::encoded_key(before,n.layout,i));
                assert(new_key(original, plan, i) == plan.key@);
            }
            assert(live.subrange(span.start as int, span.end as int) =~= value);
        } else {
            assert(0 <= s < c.count);
            table_index(n.region.start+4,c.stride,c.count,s);
            word_in_slice(original,n.region.start as int,n.region.end as int,4+s*c.stride+c.change.key_width);
            if s>0 {
                table_index(n.region.start+4,c.stride,c.count,s-1);
                word_in_slice(original,n.region.start as int,n.region.end as int,4+(s-1)*c.stride+c.change.key_width);
            }
            let old_key_at=n.region.start+4+s*c.stride;
            assert(new_key(original,plan,i) =~= search::encoded_key(before,n.layout,s));
            let old_span = map_layout::value_span(before, n.layout, s as u32);
            let start = c.payload + boundary(original, plan.region.start as int, c, s);
            let end = c.payload + boundary(original, plan.region.start as int, c, s + 1);
            let shift = if i < c.change.index { c.table_delta as int } else { plan.delta as int };
            assert(0 <= s < c.count);
            assert(old_span.start == start - n.region.start && old_span.end == end - n.region.start);
            assert(a == start + shift && z == end + shift);
            if i < c.change.index {
                assert(boundary(original,n.region.start as int,c,s+1) <= c.offset);
                assert(z <= q+c.offset);
            } else {
                assert(c.end_offset <= boundary(original,n.region.start as int,c,s));
                assert(q+c.offset+c.input_len <= a);
            }
            assert forall|k: int| #![trigger after[a+k]] 0 <= k < z-a implies after[a+k] == original[start+k] by { }
            assert(live.subrange(span.start as int, span.end as int) =~= before.subrange(old_span.start as int, old_span.end as int));
        }

}

#[verifier::spinoff_prover]
proof fn refinement(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original,plan), plan.variable_map.is_some(), edit_result(original,encoded,after,plan),
    ensures map_result(original,encoded,after,plan),
{
    hide(words::le_word); hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical);
    checked_geometry(original, plan);
    let c = plan.variable_map.unwrap();
    let n = fixed_map::node(plan);
    let before = node_bytes(original, n);
    if !search::rep_map(before, n.layout, search::map_model(before, n.layout)) { return; }
    let live = after.subrange(n.region.start as int, n.region.end + plan.delta);
    let value = encoded.subrange(destination(plan) as int, destination(plan) + c.input_len);
    let q = c.payload + c.table_delta;

    resulting_shape(original,encoded,after,plan);
    assert forall|i: int| 0 <= i < c.new_count implies
        #[trigger] fixed_map::entry_matches(before,live,n.layout,c.change,plan.key@,value,i) by {
        entry_projection(original,encoded,after,plan,i);
    }
    assert(fixed_map::entry_relation(before,live,n.layout,c.change,plan.key@,value));
    search::map_model_exact(before,n.layout);
    fixed_map::map_relation(before,live,n.layout,c.change,plan.key@,value);
}

/// The indexed result reconstructs ordered keys and recursively valid values.
#[verifier::spinoff_prover]
pub(super) proof fn selected_preserves(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, budget: nat, semantic: bool)
    requires replacement_checked(original, plan), plan.variable_map.is_some(),
        edit_result(original, encoded, after, plan),
        validation::schema_valid(node_bytes(original, fixed_map::node(plan)), fixed_map::node(plan).layout, budget, semantic),
        writes(plan.variable_map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(destination(plan) as int, destination(plan) + plan.variable_map.unwrap().input_len),
            validation::child_layout(fixed_map::node(plan).layout, 0), (budget - 1) as nat, semantic),
    ensures validation::schema_valid(after.subrange(fixed_map::node(plan).region.start as int,
        fixed_map::node(plan).region.end + plan.delta), fixed_map::node(plan).layout, budget, semantic),
{
    hide(validation::schema_valid); hide(validation::child_valid); hide(validation::local_valid);
    hide(search::lex_less); hide(search::keys_ordered); hide(search::keys_canonical);
    hide(words::le_word); hide(replacement_checked);
    checked_geometry(original, plan);
    let n = fixed_map::node(plan);
    let c = plan.variable_map.unwrap();
    let before = node_bytes(original, n);
    let live = after.subrange(n.region.start as int, n.region.end + plan.delta);
    let input = encoded.subrange(destination(plan) as int, destination(plan) + c.input_len);
    resulting_shape(original, encoded, after, plan);
    reveal_with_fuel(validation::schema_valid, 1);
    reveal(validation::local_valid);
    if semantic {
        search::map_model_exact(before, n.layout);
        refinement(original, encoded, after, plan);
    }
    reveal(validation::local_valid);
    assert(validation::local_valid(live, n.layout, semantic));
    crate::preservation::children_all(before, n.layout, 0, (budget - 1) as nat, semantic);
    assert forall|i: int| 0 <= i < validation::child_count(live, n.layout) implies
        #[trigger] validation::child_valid(live, n.layout, i, (budget - 1) as nat, semantic) by {
        entry_projection(original, encoded, after, plan, i);
        let value = fixed_map::value_at(live, n.layout, i);
        if writes(c) && i == c.change.index {
            assert(value == input);
        } else {
            let old_i = source(c, i);
            assert(old_i == fixed_map::source_index(c.change, i));
            assert(0 <= old_i < validation::child_count(before, n.layout));
            assert(validation::child_valid(before, n.layout, old_i, (budget - 1) as nat, semantic));
            assert(validation::child_layout(n.layout, old_i) == validation::child_layout(n.layout, i));
            assert(value == fixed_map::value_at(before, n.layout, old_i));
        }
        reveal(validation::child_valid);
    }
    crate::preservation::children_from_all(live, n.layout, 0, (budget - 1) as nat, semantic);
}

/// Indexed directory synthesis retains the generic ancestor and suffix frame.
#[verifier::spinoff_prover]
pub(super) proof fn replacement_frame(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), plan.variable_map.is_some(),
        edit_result(original, encoded, after, plan),
    ensures replacement_result(original, after, after, plan),
{
    checked_geometry(original, plan);
    replacement_extent(original, plan);
    reveal(target); reveal(count_published); reveal(count_changed);
}

#[cfg(verus_only)]
#[verifier::spinoff_prover]
proof fn directory_suffix_witness()
{
    use super::witnesses;
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    assert(crate::search::same_prefix(seq![1u8], seq![2u8], 0));
    assert(crate::search::lex_less(seq![1u8], seq![2u8]));
    let before = witnesses::account(seq![1u8,0,0,0,1,1,0,0,0,97]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Insert, key_width: 1, value_width: 1,
        index: 1, found: false }, count: 1, new_count: 2, stride: 5, payload: 21,
        offset: 1, end_offset: 1, input_len: 1, table_delta: 5 };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[2u8]), variable_map: Some(c), ..witnesses::replacement(before, witnesses::INDEXED_MAP, 16) };
    assert(p.key@ =~= if matches!(c.change.operation, MapEdit::Insert) { seq![2u8] } else { seq![1u8] });
    assert(crate::map_layout::map_shape(before.subrange(12,22), witnesses::INDEXED_MAP));
    assert(crate::search::encoded_key(before.subrange(12,22), witnesses::INDEXED_MAP, 0) =~= seq![1u8]);
    assert(crate::search::search_found(crate::Search { index: c.change.index, found: c.change.found }, before.subrange(12,22), witnesses::INDEXED_MAP, p.key@));
    assert(crate::search::search_partition(crate::Search { index: c.change.index, found: c.change.found }, before.subrange(12,22), witnesses::INDEXED_MAP, p.key@));
    assert(parameters(before, p.region, witnesses::INDEXED_MAP, c, p.key@));
    assert(checked(before, p));
    assert(list_checked(before, p));
    assert(replacement_checked(before, p));
    assert(directory_ready(before, before, p));
    suffix_initialized(before, before, p);
    terminal_end(before, p);
}

}
