//! Constructive correspondence between compact admissions and generic planning.
//!
//! The generic planners prove success and exact plan identity from arithmetic
//! and run checks. Admissions derive those checks before invoking equivalence.

use super::*;

verus! {

/// The generic replacement planner's complete successful-input condition.
pub(super) open spec fn subtree_ready(bytes: Seq<u8>, trail: Trail, input: u32, limit: u32) -> bool {
    let used = words::le_word(bytes, 8);
    let region = trail.nodes[trail.depth as int].region;
    let delta = input as int - (region.end - region.start);
    &&& width_spec(trail.selected).is_some() ==> input == width_spec(trail.selected).unwrap()
    &&& 12 <= used + delta <= limit <= u32::MAX
    &&& forall|i: int| 0 <= i < trail.depth ==> run_safe(bytes, trail.nodes[i].run, delta as i64)
        && (delta != 0 ==> run_payload(bytes, trail.nodes[i].run, trail.nodes[i].payload_len))
}

/// Exact successful value returned by the production generic subtree planner.
pub(super) open spec fn subtree_plan_for(bytes: Seq<u8>, path: InlinePath, trail: Trail,
    input: u32, limit: u32) -> Plan<'static> {
    let region = trail.nodes[trail.depth as int].region;
    let used = words::le_word(bytes, 8) as u32;
    let delta = (input as int - (region.end - region.start)) as i64;
    let new_used = (used + delta) as u32;
    Plan { key: &[], map: None, variable_map: None, region, indexed: None, splice: None,
        list: None, trail, path,
        discriminator: [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]],
        old_used: used, replacement_len: input, delta, new_used,
        required: if new_used > used { new_used } else { used }, storage_limit: limit }
}

/// Exact fixed-entry geometry, shared with the executable generic planner.
pub(super) open spec fn entries_plan_for(bytes: Seq<u8>, path: InlinePath, trail: Trail,
    operation: ListEdit, input: u32, width: u32, limit: u32) -> Plan<'static> {
    let n = trail.nodes[trail.depth as int];
    let count = words::le_word(bytes, n.region.start as int) as u32;
    let indices = list_indices(operation, count);
    let start = (n.region.start + 4 + indices.0 * width) as u32;
    let end = (n.region.start + 4 + indices.1 * width) as u32;
    let base = subtree_plan_for(bytes, path, trail, input, limit);
    let delta = (input as int - (end - start)) as i64;
    let new_used = (base.old_used + delta) as u32;
    Plan { region: Span { start, end }, delta, new_used,
        required: if new_used > base.old_used { new_used } else { base.old_used },
        list: Some(ListChange { operation, width, old_count: count, new_count: indices.2 as u32,
            first: indices.0 as u32, last: indices.1 as u32 }), ..base }
}

/// Representable operation, matching input width, capacity and ancestor checks.
pub(super) open spec fn entries_ready(bytes: Seq<u8>, trail: Trail, operation: ListEdit,
    input: u32, width: u32, limit: u32) -> bool {
    let n = trail.nodes[trail.depth as int];
    let count = words::le_word(bytes, n.region.start as int) as u32;
    let indices = list_indices(operation, count);
    let delta = input as int - (indices.1 - indices.0) * width;
    &&& match operation {
        ListEdit::Insert(i) => i <= count && count < u32::MAX,
        ListEdit::Remove(i) | ListEdit::Replace(i) => i < count,
        _ => true,
    }
    &&& input == list_input(operation, width)
    &&& 12 <= words::le_word(bytes, 8) + delta <= limit <= u32::MAX
    &&& forall|i: int| 0 <= i < trail.depth ==> run_safe(bytes, trail.nodes[i].run, delta as i64)
        && (delta != 0 ==> run_payload(bytes, trail.nodes[i].run, trail.nodes[i].payload_len))
}


/// A compact direct-field admission constructs the generic resolver's trail.
pub(super) open spec fn direct_trail(before: Seq<u8>, layout: Layout, path: InlinePath,
    used: u32, s: Selection) -> Trail {
    let empty = Node { payload_len: 0, region: Span { start: 0, end: 0 }, layout: Layout::Str,
        ordinal: 0, run: empty_run() };
    let root = Node { payload_len: s.payload_len, region: Span { start: 12, end: used },
        layout, ordinal: s.ordinal, run: shifted(s.run, 12) };
    let selected = Node { payload_len: 0,
        region: Span { start: (12 + s.span.start) as u32, end: (12 + s.span.end) as u32 },
        layout: s.layout, ordinal: 0, run: empty_run() };
    let nodes = vstd::array::spec_array_update([empty; MAX_DEPTH], 0, root);
    let nodes = vstd::array::spec_array_update(nodes, 1, selected);
    Trail { root: layout, selected: s.layout, local: true, nodes, depth: 1 }
}

/// Root validity supplies the child-header check omitted by compact admission.
#[verifier::spinoff_prover]
pub(super) proof fn direct_checked(before: Seq<u8>, layout: Layout, path: InlinePath,
    used: u32, s: Selection)
    requires !path.overflow, path.len == 1,
        12 <= used <= before.len() <= u32::MAX, words::le_word(before, 8) == used,
        selection_exact(s, before.subrange(12, used as int), layout, path.steps[0]),
        validation::schema_valid(before.subrange(12, used as int), layout, MAX_DEPTH as nat, true),
    ensures trail_valid(before, layout, path, direct_trail(before, layout, path, used, s), used),
        validation::local_valid(node_bytes(before, direct_trail(before, layout, path, used, s).nodes[1]), s.layout, true),
        validation::schema_valid(node_bytes(before, direct_trail(before, layout, path, used, s).nodes[1]),
            s.layout, (MAX_DEPTH - 1) as nat, true),
{
    hide(validation::local_valid);
    let live = before.subrange(12, used as int);
    crate::preservation::child_from_children(live, layout, 0, s.ordinal as int, (MAX_DEPTH - 1) as nat, true);
    let trail = direct_trail(before, layout, path, used, s);
    assert(live.subrange(s.span.start as int, s.span.end as int) =~= node_bytes(before, trail.nodes[1]));
    reveal(validation::local_valid);
    assert(crate::edit_layout::header_valid(node_bytes(before, trail.nodes[1]), s.layout));
    assert(edge_valid(before, trail.nodes[0], path.steps[0], trail.nodes[1]));
    assert forall|i: int| #![trigger trail.nodes[i]] 0 <= i <= trail.depth implies {
        let n = trail.nodes[i];
        &&& 12 <= n.region.start <= n.region.end <= used <= before.len()
        &&& n.region.start <= trail.nodes[trail.depth as int].region.start
        &&& trail.nodes[trail.depth as int].region.end <= n.region.end
    } by { assert(i == 0 || i == 1); }
}

/// The generic subtree planner returns this checked plan on every ready input.
#[verifier::spinoff_prover]
pub(super) proof fn subtree_checked(before: Seq<u8>, path: InlinePath, trail: Trail, input: u32, limit: u32)
    requires trail_valid(before, trail.root, path, trail, words::le_word(before, 8) as u32),
        12 <= words::le_word(before, 8) <= before.len() <= limit <= u32::MAX,
        subtree_ready(before, trail, input, limit),
    ensures replacement_checked(before, subtree_plan_for(before, path, trail, input, limit)),
        subtree_plan(subtree_plan_for(before, path, trail, input, limit)),
        planned_for(subtree_plan_for(before, path, trail, input, limit), words::le_word(before, 8) as u32,
            trail.root, path, input, limit),
{
    let p = subtree_plan_for(before, path, trail, input, limit);
    assert(trail.nodes[trail.depth as int].region.end <= words::le_word(before, 8));
}

/// A fixed struct projection uses exactly the certified packed-field offset.
pub(super) open spec fn fixed_index(layout: Layout, step: Step) -> int {
    match (layout.head(), step) {
        (LayoutHead::Struct { fixed, .. }, Step::Field(id)) =>
            if exists|i: int| 0 <= i < fixed.len() && fixed.view()[i].declaration == id {
                choose|i: int| 0 <= i < fixed.len() && fixed.view()[i].declaration == id
            } else { -1 },
        _ => -1,
    }
}

pub(super) open spec fn prefix_layout(root: Layout, path: InlinePath, depth: nat) -> Layout
    decreases depth,
{
    if depth == 0 { root } else {
        let parent = prefix_layout(root, path, (depth - 1) as nat);
        validation::child_layout(parent, fixed_index(parent, path.steps[depth - 1]))
    }
}

pub(super) open spec fn prefix_offset(root: Layout, path: InlinePath, depth: nat) -> int
    decreases depth,
{
    if depth == 0 { 0 } else {
        let parent = prefix_layout(root, path, (depth - 1) as nat);
        prefix_offset(root, path, (depth - 1) as nat) + match parent.head() {
            LayoutHead::Struct { fixed, .. } => fixed.view()[fixed_index(parent, path.steps[depth - 1])].offset as int,
            _ => 0,
        }
    }
}

/// This metadata relation is established by generated offset assertions and
/// preserved by the editor's fixed-field projection chain.
pub(super) open spec fn fixed_prefix(root: Layout, path: InlinePath, offset: u32, width: u32) -> bool {
    &&& !path.overflow && path.len < MAX_DEPTH
    &&& offset == prefix_offset(root, path, path.len as nat)
    &&& width_spec(prefix_layout(root, path, path.len as nat)) == Some(width)
    &&& forall|i: usize| #![trigger prefix_layout(root, path, i as nat)] i < path.len ==> {
        let parent = prefix_layout(root, path, i as nat);
        match parent.head() {
            LayoutHead::Struct { fixed, .. } => 0 <= fixed_index(parent, path.steps[i as int]) < fixed.len(),
            _ => false,
        }
    }
}

/// Reconstruct every fixed-prefix ancestor from metadata and recursive validity.
#[verifier::spinoff_prover]
pub(super) proof fn fixed_trail(before: Seq<u8>, root: Layout, path: InlinePath,
    used: u32, offset: u32, width: u32) -> (trail: Trail)
    requires 12 <= used <= before.len() <= u32::MAX, words::le_word(before, 8) == used,
        fixed_prefix(root, path, offset, width),
        validation::schema_valid(before.subrange(12, used as int), root, MAX_DEPTH as nat, true),
    ensures 12 + offset + width <= used, trail_valid(before, root, path, trail, used),
        trail.selected == prefix_layout(root, path, path.len as nat),
        trail.nodes[trail.depth as int].region == (Span { start: (12 + offset) as u32, end: (12 + offset + width) as u32 }),
        forall|i: int| 0 <= i < trail.depth ==> #[trigger] trail.nodes[i].run == empty_run(),
{
    hide(validation::local_valid);
    let empty = Node { payload_len: 0, region: Span { start: 0, end: 0 }, layout: Layout::Str,
        ordinal: 0, run: empty_run() };
    let mut nodes = [empty; MAX_DEPTH];
    nodes = vstd::array::spec_array_update(nodes, 0, Node { payload_len: 0,
        region: Span { start: 12, end: used }, layout: root, ordinal: 0, run: empty_run() });
    fixed_trail_from(before, root, path, used, offset, width, nodes, 0)
}

/// Exact partial resolver state after a fixed-field projection prefix.
spec fn prefix_state(before: Seq<u8>, root: Layout, path: InlinePath, used: u32,
    nodes: [Node; MAX_DEPTH], depth: usize) -> bool {
    &&& depth <= path.len < MAX_DEPTH && !path.overflow
    &&& 12 <= used <= before.len() <= u32::MAX
    &&& nodes[0].region == (Span { start: 12, end: used }) && nodes[0].layout == root
    &&& nodes[depth as int].layout == prefix_layout(root, path, depth as nat)
    &&& nodes[depth as int].region.start == 12 + prefix_offset(root, path, depth as nat)
    &&& validation::schema_valid(node_bytes(before, nodes[depth as int]), nodes[depth as int].layout,
        (MAX_DEPTH - depth) as nat, true)
    &&& forall|i: int| #![trigger nodes[i]] 0 <= i <= depth ==> {
        let n = nodes[i];
        &&& 12 <= n.region.start <= n.region.end <= used
        &&& n.region.start <= nodes[depth as int].region.start
        &&& nodes[depth as int].region.end <= n.region.end
    }
    &&& forall|i: int| 0 <= i < depth ==> edge_valid(before, nodes[i], path.steps[i], nodes[i + 1])
        && nodes[i].run == empty_run()
}

/// Induction composes independently verified metadata steps.
#[verifier::spinoff_prover]
proof fn fixed_trail_from(before: Seq<u8>, root: Layout, path: InlinePath,
    used: u32, offset: u32, width: u32, nodes: [Node; MAX_DEPTH], depth: usize) -> (trail: Trail)
    requires prefix_state(before, root, path, used, nodes, depth), fixed_prefix(root, path, offset, width),
    ensures 12 + offset + width <= used, trail_valid(before, root, path, trail, used),
        trail.selected == prefix_layout(root, path, path.len as nat),
        trail.nodes[trail.depth as int].region == (Span { start: (12 + offset) as u32, end: (12 + offset + width) as u32 }),
        forall|i: int| 0 <= i < trail.depth ==> #[trigger] trail.nodes[i].run == empty_run(),
    decreases path.len - depth,
{
    hide(prefix_state);
    assert(depth <= path.len < MAX_DEPTH) by { reveal(prefix_state); }
    if depth == path.len {
        fixed_terminal(before, root, path, used, offset, width, nodes, depth)
    } else {
        let nodes = fixed_step(before, root, path, used, offset, width, nodes, depth);
        fixed_trail_from(before, root, path, used, offset, width, nodes, (depth + 1) as usize)
    }
}

/// One terminal fixed selection has exactly its declared width.
#[verifier::spinoff_prover]
proof fn fixed_terminal(before: Seq<u8>, root: Layout, path: InlinePath,
    used: u32, offset: u32, width: u32, nodes: [Node; MAX_DEPTH], depth: usize) -> (trail: Trail)
    requires prefix_state(before, root, path, used, nodes, depth), fixed_prefix(root, path, offset, width),
        depth == path.len,
    ensures 12 + offset + width <= used, trail_valid(before, root, path, trail, used),
        trail.selected == prefix_layout(root, path, path.len as nat),
        trail.nodes[trail.depth as int].region == (Span { start: (12 + offset) as u32, end: (12 + offset + width) as u32 }),
        forall|i: int| 0 <= i < trail.depth ==> #[trigger] trail.nodes[i].run == empty_run(),
{
    hide(validation::schema_valid); hide(validation::local_valid); hide(validation::child_valid);

    let selected = nodes[depth as int];
    assert(validation::local_valid(node_bytes(before, selected), selected.layout, true)) by {
        reveal(validation::schema_valid);
    }
    assert(crate::edit_layout::header_valid(node_bytes(before, selected), selected.layout)) by {
        reveal(validation::local_valid);
    }
    assert(selected.region.end - selected.region.start == width) by {
        reveal(validation::local_valid);
    }
    Trail { root, selected: selected.layout, local: true, nodes, depth }
}

/// A single fixed field gets its exact selection from schema metadata.
#[verifier::spinoff_prover]
proof fn fixed_selection(local: Seq<u8>, layout: Layout, step: Step, budget: nat) -> (s: Selection)
    requires validation::schema_valid(local, layout, budget, true),
        match layout.head() { LayoutHead::Struct { fixed, .. } => 0 <= fixed_index(layout, step) < fixed.len(), _ => false },
    ensures selection_exact(s, local, layout, step), s.run == empty_run(), s.ordinal == fixed_index(layout, step),
        validation::schema_valid(local.subrange(s.span.start as int, s.span.end as int),
            s.layout, (budget - 1) as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    hide(crate::layout::descriptor_valid); hide(crate::search::lex_less);
    let index = fixed_index(layout, step);
    assert(validation::local_valid(local, layout, true)) by { reveal(validation::schema_valid); }
    assert(validation::children_valid(local, layout, 0, (budget - 1) as nat, true)) by { reveal(validation::schema_valid); }
    nested::child_bounds(local, layout, index, true);
    crate::preservation::child_from_children(local, layout, 0, index, (budget - 1) as nat, true);
    let span = validation::child_span(local, layout, index);
    let child = validation::child_layout(layout, index);
    assert(validation::child_valid(local, layout, index, (budget - 1) as nat, true));
    assert(validation::schema_valid(local.subrange(span.start as int, span.end as int), child,
        (budget - 1) as nat, true)) by {
        reveal_with_fuel(validation::child_valid, 2);
        reveal_with_fuel(validation::schema_valid, 1);
    }
    assert(0 <= payload_extent(local, layout) <= u32::MAX) by { reveal(validation::local_valid); }
    assert(crate::edit_layout::header_valid(local, layout)) by { reveal(validation::local_valid); }
    Selection { payload_len: payload_extent(local, layout) as u32, span, layout: child,
        ordinal: index as u32, run: empty_run() }
}

/// Unfold exactly one metadata step without expanding the earlier path.
#[verifier::spinoff_prover]
proof fn prefix_next(root: Layout, path: InlinePath, offset: u32, width: u32, depth: usize)
    requires fixed_prefix(root, path, offset, width), depth < path.len,
    ensures ({ let parent = prefix_layout(root, path, depth as nat);
        let index = fixed_index(parent, path.steps[depth as int]);
        &&& match parent.head() { LayoutHead::Struct { fixed, .. } => 0 <= index < fixed.len(), _ => false }
        &&& prefix_layout(root, path, (depth + 1) as nat) == validation::child_layout(parent, index)
        &&& prefix_offset(root, path, (depth + 1) as nat) == prefix_offset(root, path, depth as nat)
            + validation::child_span(Seq::empty(), parent, index).start
    }),
{
    reveal_with_fuel(prefix_layout, 1); reveal_with_fuel(prefix_offset, 1);
}

/// One fixed parent produces the next exact resolver state.
#[verifier::spinoff_prover]
proof fn fixed_step(before: Seq<u8>, root: Layout, path: InlinePath,
    used: u32, offset: u32, width: u32, nodes: [Node; MAX_DEPTH], depth: usize) -> (updated: [Node; MAX_DEPTH])
    requires prefix_state(before, root, path, used, nodes, depth), fixed_prefix(root, path, offset, width), depth < path.len,
    ensures prefix_state(before, root, path, used, updated, (depth + 1) as usize),
{
    hide(validation::schema_valid); hide(validation::local_valid); hide(validation::child_valid);
    hide(crate::layout::descriptor_valid); hide(prefix_layout); hide(prefix_offset);
    hide(crate::search::lex_less); hide(crate::search::keys_ordered); hide(crate::search::keys_canonical);
    prefix_next(root, path, offset, width, depth);
    let mut nodes = nodes;
    let parent = nodes[depth as int];
    let local = node_bytes(before, parent);
    let index = fixed_index(parent.layout, path.steps[depth as int]);
    assert(prefix_layout(root, path, depth as nat) == parent.layout);
    assert(match parent.layout.head() {
        LayoutHead::Struct { fixed, .. } => 0 <= index < fixed.len(), _ => false,
    });
    let s = fixed_selection(local, parent.layout, path.steps[depth as int], (MAX_DEPTH - depth) as nat);
    let span = s.span;
    let child = s.layout;
    let next = Node { payload_len: 0, region: Span {
            start: (parent.region.start + span.start) as u32,
            end: (parent.region.start + span.end) as u32 },
        layout: child, ordinal: 0, run: empty_run() };
    assert(local.subrange(span.start as int, span.end as int) =~= node_bytes(before, next));
    let previous = nodes;
    nodes = vstd::array::spec_array_update(nodes, depth as int, Node {
        payload_len: s.payload_len, ordinal: s.ordinal, run: s.run, ..parent });
    nodes = vstd::array::spec_array_update(nodes, depth as int + 1, next);
    assert(selects(parent.layout, path.steps[depth as int], index));
    assert(expected_run(local, parent.layout, index) == empty_run());
    assert(edge_valid(before, nodes[depth as int], path.steps[depth as int], next));
    assert(prefix_layout(root, path, (depth + 1) as nat) == child);
    assert(prefix_offset(root, path, (depth + 1) as nat) == prefix_offset(root, path, depth as nat) + span.start);
    assert forall|i: int| 0 <= i < depth implies
        edge_valid(before, nodes[i], path.steps[i], nodes[i + 1]) && nodes[i].run == empty_run() by {
        assert(edge_valid(before, previous[i], path.steps[i], previous[i + 1]));
    }
    assert forall|i: int| #![trigger nodes[i]] 0 <= i <= depth + 1 implies {
        let n = nodes[i];
        &&& 12 <= n.region.start <= n.region.end <= used
        &&& n.region.start <= next.region.start && next.region.end <= n.region.end
    } by { if i <= depth { assert(previous[i].region == nodes[i].region); } }

    nodes
}

/// Fixed-entry readiness constructs the same plan the shared generic loop returns.
#[verifier::spinoff_prover]
pub(super) proof fn entries_checked(before: Seq<u8>, path: InlinePath, trail: Trail,
    operation: ListEdit, input: u32, width: u32, limit: u32)
    requires trail_valid(before, trail.root, path, trail, words::le_word(before, 8) as u32),
        12 <= words::le_word(before, 8) <= before.len() <= limit <= u32::MAX,
        entries_ready(before, trail, operation, input, width, limit),
        match trail.selected.head() {
            LayoutHead::List { element } => width_spec(element) == Some(width),
            LayoutHead::Map { key, value } => width_spec(value).is_some()
                && width == key.width + width_spec(value).unwrap(),
            _ => false,
        },
    ensures replacement_checked(before, entries_plan_for(before, path, trail, operation, input, width, limit)),
        planned_list_for(entries_plan_for(before, path, trail, operation, input, width, limit), operation),
{
    let n = trail.nodes[trail.depth as int];
    let local = node_bytes(before, n);
    indexed_insert::word_in_slice(before, n.region.start as int, n.region.end as int, 0);
    let count = words::le_word(before, n.region.start as int) as u32;
    assert(local.len() == 4 + count as int * width);
    let indices = list_indices(operation, count);
    assert(0 <= indices.0 <= indices.1 <= count && 0 <= indices.2 <= u32::MAX);
    assert(0 <= indices.0 * width <= indices.1 * width <= count as int * width) by(nonlinear_arith)
        requires 0 <= indices.0 <= indices.1 <= count, width >= 0;
    let plan = entries_plan_for(before, path, trail, operation, input, width, limit);
    assert(plan.region.start == n.region.start + 4 + indices.0 * width);
    assert(plan.region.end == n.region.start + 4 + indices.1 * width);
    assert(plan.delta == input as int - (indices.1 - indices.0) * width) by(nonlinear_arith)
        requires plan.region.start == n.region.start + 4 + indices.0 * width,
            plan.region.end == n.region.start + 4 + indices.1 * width,
            plan.delta == input as int - (plan.region.end - plan.region.start);
    assert(n.region.end + plan.delta == n.region.start + 4 + indices.2 * width) by(nonlinear_arith)
        requires n.region.end == n.region.start + 4 + count as int * width,
            plan.delta == input as int - (indices.1 - indices.0) * width,
            indices.2 == count - (indices.1 - indices.0)
                + if matches!(operation, ListEdit::Insert(_) | ListEdit::Replace(_)) { 1int } else { 0int },
            input == list_input(operation, width);
    assert(list_checked(before, plan));
}


/// Exact attachment performed by the fixed-map generic planner after search.
pub(super) open spec fn map_tail_plan<'key>(before: Seq<u8>, path: InlinePath, trail: Trail,
    width: u32, query: &'key [u8], limit: u32) -> Plan<'key> {
    let count = words::le_word(before, trail.nodes[trail.depth as int].region.start as int) as u32;
    let stride = (width + query.len()) as u32;
    let base = entries_plan_for(before, path, trail, ListEdit::Insert(count), stride, stride, limit);
    Plan { key: query, map: Some(MapChange { operation: MapEdit::Insert,
        key_width: query.len() as u32, value_width: width, index: count, found: false }), ..base }
}


/// Equal-size writes need no end checks; empty runs admit every safe byte delta.
#[verifier::spinoff_prover]
pub(super) proof fn idle_runs(before: Seq<u8>, trail: Trail, path: InlinePath, used: u32, delta: i64)
    requires trail_valid(before, trail.root, path, trail, used), before.len() <= u32::MAX,
        delta == 0 || (forall|i: int| 0 <= i < trail.depth ==> #[trigger] trail.nodes[i].run.count == 0),
    ensures forall|i: int| 0 <= i < trail.depth ==> run_safe(before, trail.nodes[i].run, delta)
        && (delta != 0 ==> run_payload(before, trail.nodes[i].run, trail.nodes[i].payload_len)),
{
    assert forall|i: int| 0 <= i < trail.depth implies run_safe(before, trail.nodes[i].run, delta)
        && (delta != 0 ==> run_payload(before, trail.nodes[i].run, trail.nodes[i].payload_len)) by {
        let run = trail.nodes[i].run;
        if delta == 0 {
            assert(edge_valid(before, trail.nodes[i], path.steps[i], trail.nodes[i + 1]));
            assert forall|j: int| 0 <= j < run.count implies #[trigger] patch_safe_at(before, run, delta, j) by {
                run_index_bounds(run, before.len() as int, j);
            }
        }
    }
}


/// The strict generic map planner's searched tail case, before any plan exists.
pub(super) open spec fn map_tail_ready(before: Seq<u8>, trail: Trail, width: u32, query: &[u8], limit: u32) -> bool {
    let n = trail.nodes[trail.depth as int];
    let local = node_bytes(before, n);
    let count = words::le_word(local, 0) as u32;
    &&& !trail.local
    &&& validation::local_valid(local, n.layout, true)
    &&& match n.layout.head() { LayoutHead::Map { key, value } => width_spec(value) == Some(width)
            && key.width == query.len(), _ => false }
    &&& !crate::search::bool_keys(n.layout) || query@[0] <= 1
    &&& query.len() + width <= u32::MAX
    &&& crate::search::search_found(crate::Search { index: count, found: false }, local, n.layout, query@)
    &&& crate::search::search_partition(crate::Search { index: count, found: false }, local, n.layout, query@)
    &&& entries_ready(before, trail, ListEdit::Insert(count), (query.len() + width) as u32,
        (query.len() + width) as u32, limit)
}

/// Strictly ordered keys make the tail split the unique generic lookup result.
#[verifier::spinoff_prover]
pub(super) proof fn tail_lookup(before: Seq<u8>, layout: Layout, key: Seq<u8>, result: crate::Search)
    requires crate::search::keys_ordered(before, layout, words::le_word(before, 0)),
        crate::search::search_found(crate::Search { index: words::le_word(before, 0) as u32, found: false }, before, layout, key),
        crate::search::search_partition(crate::Search { index: words::le_word(before, 0) as u32, found: false }, before, layout, key),
        crate::search::search_found(result, before, layout, key), crate::search::search_partition(result, before, layout, key),
    ensures result == (crate::Search { index: words::le_word(before, 0) as u32, found: false }),
{
    hide(crate::search::lex_less);
    if result.index < words::le_word(before, 0) {
        crate::search::lex_antisymmetric(key, crate::search::encoded_key(before, layout, result.index as int));
    }
}


/// The generic fixed-map attachment preserves all checked entry geometry.
#[verifier::spinoff_prover]
pub(super) proof fn map_tail_attachment(before: Seq<u8>, path: InlinePath, trail: Trail,
    width: u32, query: &[u8], limit: u32)
    requires trail_valid(before, trail.root, path, trail, words::le_word(before, 8) as u32),
        12 <= words::le_word(before, 8) <= before.len() <= limit <= u32::MAX,
        map_tail_ready(before, trail, width, query, limit),
    ensures replacement_checked(before, map_tail_plan(before, path, trail, width, query, limit)),
        planned_map(map_tail_plan(before, path, trail, width, query, limit), MapEdit::Insert, query@, width),
{
    hide(crate::search::lex_less); hide(crate::search::keys_ordered); hide(crate::search::keys_canonical);
    hide(crate::search::search_found); hide(crate::search::search_partition);
    let n = trail.nodes[trail.depth as int];
    indexed_insert::word_in_slice(before, n.region.start as int, n.region.end as int, 0);
    let count = words::le_word(before, n.region.start as int) as u32;
    let stride = (width + query.len()) as u32;
    entries_checked(before, path, trail, ListEdit::Insert(count), stride, stride, limit);
    fixed_map::attached_request(map_tail_plan(before, path, trail, width, query, limit));
}


/// The generic fixed-map planner's exact vacant insertion at any ordinal.
pub(super) open spec fn map_insert_plan<'key>(before: Seq<u8>, path: InlinePath, trail: Trail,
    width: u32, query: &'key [u8], index: u32, limit: u32) -> Plan<'key> {
    let stride = (width + query.len()) as u32;
    let base = entries_plan_for(before, path, trail, ListEdit::Insert(index), stride, stride, limit);
    Plan { key: query, map: Some(MapChange { operation: MapEdit::Insert,
        key_width: query.len() as u32, value_width: width, index, found: false }), ..base }
}

/// Strict generic search, entry arithmetic and capacity admit this vacant slot.
pub(super) open spec fn map_insert_ready(before: Seq<u8>, trail: Trail, width: u32,
    query: &[u8], index: u32, limit: u32) -> bool {
    let n = trail.nodes[trail.depth as int];
    let local = node_bytes(before, n);
    &&& !trail.local
    &&& validation::local_valid(local, n.layout, true)
    &&& match n.layout.head() { LayoutHead::Map { key, value } => width_spec(value) == Some(width)
            && key.width == query.len(), _ => false }
    &&& !crate::search::bool_keys(n.layout) || query@[0] <= 1
    &&& query.len() + width <= u32::MAX
    &&& crate::search::search_found(crate::Search { index, found: false }, local, n.layout, query@)
    &&& crate::search::search_partition(crate::Search { index, found: false }, local, n.layout, query@)
    &&& entries_ready(before, trail, ListEdit::Insert(index), (query.len() + width) as u32,
        (query.len() + width) as u32, limit)
}

/// Strict order makes every vacant search partition unique, including the middle.
#[verifier::spinoff_prover]
pub(super) proof fn insert_lookup(before: Seq<u8>, layout: Layout, key: Seq<u8>, index: u32, result: crate::Search)
    requires crate::search::keys_ordered(before, layout, words::le_word(before, 0)),
        crate::search::search_found(crate::Search { index, found: false }, before, layout, key),
        crate::search::search_partition(crate::Search { index, found: false }, before, layout, key),
        crate::search::search_found(result, before, layout, key),
        crate::search::search_partition(result, before, layout, key),
    ensures result == (crate::Search { index, found: false }),
{
    hide(crate::search::lex_less);
    if result.index < index {
        crate::search::lex_antisymmetric(key, crate::search::encoded_key(before, layout, result.index as int));
    } else if index < result.index {
        crate::search::lex_antisymmetric(key, crate::search::encoded_key(before, layout, index as int));
    }
}

/// Attaching the searched map request preserves the generic checked entry plan.
#[verifier::spinoff_prover]
pub(super) proof fn map_insert_attachment(before: Seq<u8>, path: InlinePath, trail: Trail,
    width: u32, query: &[u8], index: u32, limit: u32)
    requires trail_valid(before, trail.root, path, trail, words::le_word(before, 8) as u32),
        12 <= words::le_word(before, 8) <= before.len() <= limit <= u32::MAX,
        map_insert_ready(before, trail, width, query, index, limit),
    ensures replacement_checked(before, map_insert_plan(before, path, trail, width, query, index, limit)),
        planned_map(map_insert_plan(before, path, trail, width, query, index, limit), MapEdit::Insert, query@, width),
{
    hide(crate::search::lex_less); hide(crate::search::keys_ordered); hide(crate::search::keys_canonical);
    hide(crate::search::search_found); hide(crate::search::search_partition);
    let stride = (width + query.len()) as u32;
    entries_checked(before, path, trail, ListEdit::Insert(index), stride, stride, limit);
    fixed_map::attached_request(map_insert_plan(before, path, trail, width, query, index, limit));
}

}
