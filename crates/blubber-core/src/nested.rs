//! Recursive reconstruction of an edited child and its ancestor headers.

use super::*;
#[cfg(verus_only)]
use crate::preservation;
use vstd::prelude::*;

verus! {

/// One byte of a mathematical little-endian word.
spec fn word_byte(value: u32, index: int) -> u8 {
    (if index == 0 { value as int % 256 }
    else if index == 1 { value as int / 256 % 256 }
    else if index == 2 { value as int / 65536 % 256 }
    else { value as int / 16777216 }) as u8
}

/// A parent's prefix with precisely the selected child's stored ends shifted.
spec fn patched_prefix(bytes: Seq<u8>, run: PatchRun, end: int, delta: int) -> Seq<u8> {
    Seq::new(end as nat, |p: int| if run_changed(run, run.count as int, p) {
        let k = (p - run.offset) / (run.stride as int);
        let at = run_at(run, k);
        word_byte((words::le_word(bytes, at) + delta) as u32, p - at)
    } else { bytes[p] })
}

/// Rebuilds a parent from its patched prefix, edited child and original suffix.
spec fn rebuild(bytes: Seq<u8>, layout: Layout, selected: int, child: Seq<u8>) -> Seq<u8> {
    let span = validation::child_span(bytes, layout, selected);
    let delta = child.len() as int - (span.end - span.start);
    patched_prefix(bytes, expected_run(bytes, layout, selected), span.start as int, delta)
        + child + bytes.subrange(span.end as int, bytes.len() as int)
}

/// Independent recursive encoding of one replacement along wire ordinals.
pub(super) closed spec fn nested_apply(bytes: Seq<u8>, layout: Layout, path: Seq<int>,
    replacement: Seq<u8>) -> Seq<u8>
    decreases path.len(),
{
    if path.len() == 0 { replacement } else {
        let selected = path[0];
        let span = validation::child_span(bytes, layout, selected);
        rebuild(bytes, layout, selected,
            nested_apply(bytes.subrange(span.start as int, span.end as int),
                validation::child_layout(layout, selected), path.subrange(1, path.len() as int),
                replacement))
    }
}

/// Fixed schema encodings have their declared width in both validation modes.
#[verifier::spinoff_prover]
proof fn fixed_extent(bytes: Seq<u8>, layout: Layout, budget: nat, semantic: bool)
    requires validation::schema_valid(bytes, layout, budget, semantic),
        width_spec(layout).is_some(),
    ensures bytes.len() == width_spec(layout).unwrap(),
{ }

/// Untruncated mathematical coordinates for fixed-stride collection children.
pub(super) open spec fn fixed_coordinates(bytes: Seq<u8>, layout: Layout, index: int) -> (int, int) {
    match layout.head() {
        LayoutHead::List { element } => (4 + index * width_spec(element).unwrap(),
            4 + (index + 1) * width_spec(element).unwrap()),
        LayoutHead::Map { .. } => (4 + index * crate::map_layout::entry_stride(layout) + crate::map_layout::key_width(layout),
            4 + (index + 1) * crate::map_layout::entry_stride(layout)),
        _ => (0int, 0int),
    }
}

/// Metadata and a valid parent put every child inside its exact byte slice.
#[verifier::spinoff_prover]
pub(super) proof fn child_bounds(bytes: Seq<u8>, layout: Layout, index: int, semantic: bool)
    requires validation::local_valid(bytes, layout, semantic),
        0 <= index < validation::child_count(bytes, layout),
    ensures 0 <= index <= u32::MAX,
        ({ let span = validation::child_span(bytes, layout, index);
        &&& span.start <= span.end <= bytes.len()
        &&& matches!(layout.head(), LayoutHead::List { .. } | LayoutHead::Map { .. })
            && width_spec(validation::child_layout(layout, index)).is_some() ==> {
            let raw = fixed_coordinates(bytes, layout, index);
            span.start == raw.0 && span.end == raw.1
        }
    }),
{
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } => {
            crate::edit_layout::fixed_bounds(fixed.view(), variable.view(), fixed_bytes,
                0, 0, 0, MAX_DEPTH as nat);
            if index >= fixed.len() {
                let i = index - fixed.len();
                let p = crate::struct_layout::payload_start(layout);
                if i > 0 {
                    assert(words::le_word(bytes, fixed_bytes as int + 4 * (i - 1)) <= bytes.len() - p);
                }
                if i + 1 < variable.len() {
                    assert(words::le_word(bytes, fixed_bytes as int + 4 * i) <= bytes.len() - p);
                    if i > 0 {
                        assert(words::le_word(bytes, fixed_bytes as int + 4 * (i - 1))
                            <= words::le_word(bytes, fixed_bytes as int + 4 * i));
                    }
                }
            }
        },
        LayoutHead::List { .. } | LayoutHead::Map { .. } => {
            validation::leaf_span_bounds(bytes, layout, index, semantic);
            if width_spec(validation::child_layout(layout, index)).is_some() {
                let raw = fixed_coordinates(bytes, layout, index);
                match layout.head() {
                    LayoutHead::List { element } => {
                        let w = width_spec(element).unwrap();
                        assert(0 <= raw.0 <= raw.1 <= bytes.len()) by(nonlinear_arith)
                            requires raw.0 == 4 + index * w, raw.1 == 4 + (index + 1) * w,
                                bytes.len() == 4 + words::le_word(bytes, 0) * w,
                                0 <= index < words::le_word(bytes, 0), w >= 0;
                    },
                    LayoutHead::Map { .. } => {
                        let stride = crate::map_layout::entry_stride(layout);
                        let key = crate::map_layout::key_width(layout);
                        assert(0 <= raw.0 <= raw.1 <= bytes.len()) by(nonlinear_arith)
                            requires raw.0 == 4 + index * stride + key, raw.1 == 4 + (index + 1) * stride,
                                bytes.len() == 4 + words::le_word(bytes, 0) * stride,
                                0 <= index < words::le_word(bytes, 0), stride >= key >= 0;
                        assert(raw.1 == 4 + index * stride + stride) by(nonlinear_arith)
                            requires raw.1 == 4 + (index + 1) * stride;
                    },
                    _ => {},
                }
                assert(0 <= index <= u32::MAX);
                assert(validation::child_span(bytes, layout, index).start == raw.0);
                assert(validation::child_span(bytes, layout, index).end == raw.1);
            }
        },
        _ => {},
    }
}

/// Division identifies the unique containing word in a checked strided run.
proof fn containing_word(run: PatchRun, p: int)
    requires run.stride >= 4, run_changed(run, run.count as int, p),
    ensures 0 <= (p - run.offset) / (run.stride as int) < run.count,
        run_at(run, (p - run.offset) / (run.stride as int)) <= p
            < run_at(run, (p - run.offset) / (run.stride as int)) + 4,
{
    let k = choose|k: int| 0 <= k < run.count
        && #[trigger] run_at(run, k) <= p < run_at(run, k) + 4;
    assert((p - run.offset) / (run.stride as int) == k) by(nonlinear_arith)
        requires run.stride >= 4, run.offset + k * run.stride <= p
            < run.offset + k * run.stride + 4;
}

/// The recursive prefix is exactly the production run's word delta and frame.
#[verifier::spinoff_prover]
proof fn prefix_exact(bytes: Seq<u8>, run: PatchRun, end: int, delta: i64)
    requires 0 <= end <= bytes.len(), run_within(run, 0, end),
        run_safe(bytes, run, delta),
    ensures patched_prefix(bytes, run, end, delta as int).len() == end,
        patch_delta(bytes, patched_prefix(bytes, run, end, delta as int), run, delta, run.count as int),
        forall|p: int| 0 <= p < end && !run_changed(run, run.count as int, p)
            ==> #[trigger] patched_prefix(bytes, run, end, delta as int)[p] == bytes[p],
{
    let prefix = patched_prefix(bytes, run, end, delta as int);
    assert forall|k: int| 0 <= k < run.count implies #[trigger]
        words::encode_exact(prefix, run_at(run, k),
            (words::le_word(bytes, run_at(run, k)) + delta) as u32) by {
        run_index_bounds(run, end, k);
        assert(patch_safe_at(bytes, run, delta, k));
        let at = run_at(run, k);
        let value = (words::le_word(bytes, at) + delta) as u32;
        assert forall|j: int| 0 <= j < 4 implies prefix[at + j]
            == #[trigger] word_byte((words::le_word(bytes, at) + delta) as u32, j) by {
            assert(run_changed(run, run.count as int, at + j));
            assert((at + j - run.offset) / (run.stride as int) == k) by(nonlinear_arith)
                requires at == run.offset + k * run.stride, run.stride >= 4, 0 <= j < 4;
        }
        assert(prefix[at] == word_byte(value, 0));
        assert(prefix[at + 1] == word_byte(value, 1));
        assert(prefix[at + 2] == word_byte(value, 2));
        assert(prefix[at + 3] == word_byte(value, 3));
        assert(0 <= value as int / 16777216 <= 255);
    }
}

/// Wire ordinals of a suffix of the checked ancestor trail.
pub(super) open spec fn ordinals(plan: Plan, first: int) -> Seq<int> {
    Seq::new((plan.trail.depth - first) as nat, |i: int| plan.trail.nodes[first + i].ordinal as int)
}

/// Exact live bytes of one ancestor after the common signed splice.
pub(super) open spec fn node_after(after: Seq<u8>, plan: Plan, i: int) -> Seq<u8> {
    after.subrange(plan.trail.nodes[i].region.start as int,
        plan.trail.nodes[i].region.end + plan.delta)
}

/// Recursive child extraction follows the checked trail's absolute regions.
#[verifier::spinoff_prover]
pub(super) proof fn trail_schema(before: Seq<u8>, plan: Plan, budget: nat, semantic: bool, i: int)
    requires replacement_checked(before, plan), 0 <= i <= plan.trail.depth,
        validation::schema_valid(before.subrange(12, plan.old_used as int),
            plan.trail.root, budget, semantic),
    ensures budget > i,
        validation::schema_valid(node_bytes(before, plan.trail.nodes[i]),
            plan.trail.nodes[i].layout, (budget - i) as nat, semantic),
    decreases i,
{
    hide(validation::local_valid);
    hide(validation::child_valid);
    if i > 0 {
        trail_schema(before, plan, budget, semantic, i - 1);
        let n = plan.trail.nodes[i - 1];
        let next = plan.trail.nodes[i];
        assert(edge_valid(before, n, plan.path.steps[i - 1], next));
        let bytes = node_bytes(before, n);
        preservation::child_from_children(bytes, n.layout, 0, n.ordinal as int,
            (budget - i) as nat, semantic);
        reveal(validation::child_valid);
        let span = validation::child_span(bytes, n.layout, n.ordinal as int);
        assert(validation::schema_valid(bytes.subrange(span.start as int, span.end as int),
            validation::child_layout(n.layout, n.ordinal as int), (budget - i) as nat, semantic));
        assert(bytes.subrange(span.start as int, span.end as int) =~= node_bytes(before, next));
    }
}

/// The expected run is entirely in the parent's prefix, before its child.
#[verifier::spinoff_prover]
pub(super) proof fn run_is_header(bytes: Seq<u8>, layout: Layout, selected: int, semantic: bool)
    requires validation::local_valid(bytes, layout, semantic),
        0 <= selected < validation::child_count(bytes, layout),
    ensures run_within(expected_run(bytes, layout, selected), 0,
        validation::child_span(bytes, layout, selected).start as int),
{
    child_bounds(bytes, layout, selected, semantic);
    assert(0 <= selected <= u32::MAX);
    let run = expected_run(bytes, layout, selected);
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } => {
            let p = crate::struct_layout::payload_start(layout);
            if fixed.len() <= selected < fixed.len() + variable.len() - 1 {
                let j = selected - fixed.len();
                assert(run.offset == fixed_bytes + 4 * j);
                assert(run.count == variable.len() - 1 - j);
                assert(run.offset + (run.count - 1) * run.stride + 4 == p);
                assert(p <= validation::child_span(bytes, layout, selected).start);
            }
        },
        LayoutHead::List { element } => if width_spec(element).is_none()
            && selected < words::le_word(bytes, 0) - 1 {
            let count = words::le_word(bytes, 0);
            let p = crate::list_layout::payload_start(bytes, layout);
            assert(run.offset == 4 + 4 * selected);
            assert(run.count == count - 1 - selected);
            assert(run.offset + (run.count - 1) * run.stride + 4 == p);
            assert(p <= validation::child_span(bytes, layout, selected).start);
        },
        LayoutHead::Map { .. } => if crate::map_layout::value_width(layout).is_none() {
            let stride = crate::map_layout::entry_stride(layout);
            let key = crate::map_layout::key_width(layout);
            let count = words::le_word(bytes, 0);
            assert(4 + selected * stride + key + (count - selected - 1) * stride + 4
                == 4 + count * stride) by(nonlinear_arith)
                requires stride == key + 4;
            assert(4 + selected * stride + key <= 4 + count * stride) by(nonlinear_arith)
                requires 0 <= selected < count, stride == key + 4, key >= 0;
            assert(run.offset == 4 + selected * stride + key);
            assert(run.count == count - selected);
            assert(run.offset + (run.count - 1) * run.stride + 4 == 4 + count * stride);
            assert(4 + count * stride <= validation::child_span(bytes, layout, selected).start);
        },
        _ => {},
    }
}

/// Translating a contained run changes only its absolute base address.
#[verifier::spinoff_prover]
proof fn translated_run(run: PatchRun, base: u32, limit: int, p: int)
    requires 0 <= limit, base + limit <= u32::MAX, run_within(run, 0, limit),
    ensures run_changed(shifted(run, base), run.count as int, base + p)
        == run_changed(run, run.count as int, p),
        forall|k: int| 0 <= k < run.count ==> #[trigger] run_at(shifted(run, base), k)
            == base + run_at(run, k),
{
    if run.count > 0 {
        run_index_bounds(run, limit, 0);
        assert forall|k: int| 0 <= k < run.count implies
            #[trigger] run_at(shifted(run, base), k) == base + run_at(run, k) by { }
        if run_changed(run, run.count as int, p) {
            let k = choose|k: int| 0 <= k < run.count && #[trigger] run_at(run, k) <= p < run_at(run, k) + 4;
            assert(run_at(shifted(run, base), k) <= base + p < run_at(shifted(run, base), k) + 4);
        }
        if run_changed(shifted(run, base), run.count as int, base + p) {
            let k = choose|k: int| 0 <= k < run.count && #[trigger] run_at(shifted(run, base), k)
                <= base + p < run_at(shifted(run, base), k) + 4;
            assert(run_at(run, k) <= p < run_at(run, k) + 4);
        }
    }
}

/// A byte in a parent's prefix belongs to no other ancestor's patch run.
#[verifier::spinoff_prover]
proof fn only_parent_run(before: Seq<u8>, plan: Plan, i: int, p: int)
    requires replacement_checked(before, plan), 0 <= i < plan.trail.depth,
        plan.trail.nodes[i].region.start <= p < plan.trail.nodes[i + 1].region.start,
    ensures ancestor_changed(plan, plan.trail.depth as int, p)
        == run_changed(plan.trail.nodes[i].run, plan.trail.nodes[i].run.count as int, p),
{
    replacement_geometry(before, plan);
    if ancestor_changed(plan, plan.trail.depth as int, p) {
        let j = choose|j: int| 0 <= j < plan.trail.depth &&
            run_changed(plan.trail.nodes[j].run, plan.trail.nodes[j].run.count as int, p);
        let run = plan.trail.nodes[j].run;
        let k = choose|k: int| 0 <= k < run.count && #[trigger]
            run_at(run, k) <= p < run_at(run, k) + 4;
        run_index_bounds(run, plan.trail.nodes[j + 1].region.start as int, k);
        if j < i { trail_order(before, plan, j + 1, i); }
        if i < j { trail_order(before, plan, i + 1, j); }
        assert(j == i);
    }
}

/// Exact word encodings and the unpatched byte frame determine the prefix.
#[verifier::spinoff_prover]
proof fn prefix_from_words(before: Seq<u8>, after: Seq<u8>, base: u32, end: int,
    run: PatchRun, delta: i64)
    requires before.len() == after.len() <= u32::MAX,
        0 <= end, base + end <= before.len(), run_within(run, 0, end),
        run_safe(before, shifted(run, base), delta),
        patch_delta(before, after, shifted(run, base), delta, run.count as int),
        forall|p: int| 0 <= p < end && !run_changed(run, run.count as int, p)
            ==> #[trigger] after[base + p] == before[base + p],
    ensures after.subrange(base as int, base + end) == patched_prefix(
        before.subrange(base as int, before.len() as int), run, end, delta as int),
{
    let bytes = before.subrange(base as int, before.len() as int);
    let prefix = patched_prefix(bytes, run, end, delta as int);
    assert forall|p: int| 0 <= p < end implies after[base + p] == #[trigger] prefix[p] by {
        if run_changed(run, run.count as int, p) {
            containing_word(run, p);
            let k = (p - run.offset) / (run.stride as int);
            let at = run_at(run, k);
            run_index_bounds(run, end, k);
            translated_run(run, base, end, p);
            assert(run_at(shifted(run, base), k) == base + at);
            assert(patch_safe_at(before, shifted(run, base), delta, k));
            assert(words::encode_exact(after, run_at(shifted(run, base), k),
                (words::le_word(before, run_at(shifted(run, base), k)) + delta) as u32));
            words::word_subrange(before, base as int, before.len() as int, at);
            assert(p - at == 0 || p - at == 1 || p - at == 2 || p - at == 3);
        }
    }
    assert(after.subrange(base as int, base + end) =~= prefix);
}

/// A valid recursive encoding has a valid local header.
proof fn valid_local(bytes: Seq<u8>, layout: Layout, budget: nat, semantic: bool)
    requires validation::schema_valid(bytes, layout, budget, semantic),
    ensures validation::local_valid(bytes, layout, semantic),
{ }

/// Each production ancestor is the recursive rebuild of its actual child.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn production_parent(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan,
    budget: nat, semantic: bool, i: int)
    requires replacement_checked(before, plan), replacement_result(before, encoded, after, plan),
        0 <= i < plan.trail.depth,
        validation::schema_valid(before.subrange(12, plan.old_used as int),
            plan.trail.root, budget, semantic),
    ensures node_after(after, plan, i) == rebuild(node_bytes(before, plan.trail.nodes[i]),
        plan.trail.nodes[i].layout, plan.trail.nodes[i].ordinal as int, node_after(after, plan, i + 1)),
{
    hide(validation::local_valid);
    hide(validation::schema_valid);
    hide(fixed_map::map_checked);
    hide(indexed_map::checked);
    hide(indexed_insert::indexed_checked);
    hide(indexed_remove::splice_checked);
    trail_schema(before, plan, budget, semantic, i);
    let n = plan.trail.nodes[i];
    let child = plan.trail.nodes[i + 1];
    let old = node_bytes(before, n);
    let new = node_after(after, plan, i);
    let edited = node_after(after, plan, i + 1);
    let span = validation::child_span(old, n.layout, n.ordinal as int);
    assert(edge_valid(before, n, plan.path.steps[i], child));
    valid_local(old, n.layout, (budget - i) as nat, semantic);
    run_is_header(old, n.layout, n.ordinal as int, semantic);
    let run = expected_run(old, n.layout, n.ordinal as int);
    assert(child.region.start <= child.region.end + plan.delta <= n.region.end + plan.delta);
    assert(n.region.start <= n.region.end + plan.delta <= plan.new_used);
    let prefix = patched_prefix(old, run, span.start as int, plan.delta as int);
    let rebuilt = rebuild(old, n.layout, n.ordinal as int, edited);
    assert(edited.len() as int - (span.end - span.start) == plan.delta);
    assert(rebuilt.len() == new.len());
    assert forall|p: int| 0 <= p < span.start && !run_changed(run, run.count as int, p)
        implies #[trigger] after[n.region.start + p] == before[n.region.start + p] by {
        translated_run(run, n.region.start, span.start as int, p);
        only_parent_run(before, plan, i, n.region.start + p);
        assert(!count_changed(plan, n.region.start + p));
    }
    assert(run_safe(before, shifted(run, n.region.start), plan.delta));
    assert(patch_delta(before, after, shifted(run, n.region.start), plan.delta, run.count as int));
    prefix_from_words(before, after, n.region.start, span.start as int, run, plan.delta);
    assert(before.subrange(n.region.start as int, before.len() as int).subrange(0, span.start as int)
        =~= old.subrange(0, span.start as int));
    assert(patched_prefix(before.subrange(n.region.start as int, before.len() as int), run,
        span.start as int, plan.delta as int) =~= prefix) by {
        assert forall|p: int| 0 <= p < span.start implies
            patched_prefix(before.subrange(n.region.start as int, before.len() as int), run,
                span.start as int, plan.delta as int)[p] == #[trigger] prefix[p] by {
            if run_changed(run, run.count as int, p) {
                containing_word(run, p);
                let k = (p - run.offset) / (run.stride as int);
                run_index_bounds(run, span.start as int, k);
                words::word_subrange(before, n.region.start as int, n.region.end as int, run_at(run, k));
                words::word_subrange(before, n.region.start as int, before.len() as int, run_at(run, k));
            }
        }
    }
    assert forall|p: int| 0 <= p < new.len() implies #[trigger] new[p] == rebuilt[p] by {
        if p < span.start {
            assert(new[p] == prefix[p]);
        } else if p < span.end + plan.delta {
            assert(new[p] == edited[p - span.start]);
        } else {
            assert(n.region.start + p >= target(plan).start + plan.replacement_len);
            assert(new[p] == old[p - plan.delta]);
        }
    }
    assert(new =~= rebuilt);
}

/// Induction identifies all production ancestors with the recursive encoding.
#[verifier::spinoff_prover]
pub(super) proof fn production_nested(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan,
    budget: nat, semantic: bool, i: int)
    requires replacement_checked(before, plan), replacement_result(before, encoded, after, plan),
        0 <= i <= plan.trail.depth,
        validation::schema_valid(before.subrange(12, plan.old_used as int),
            plan.trail.root, budget, semantic),
    ensures node_after(after, plan, i) == nested_apply(node_bytes(before, plan.trail.nodes[i]),
        plan.trail.nodes[i].layout, ordinals(plan, i), node_after(after, plan, plan.trail.depth as int)),
    decreases plan.trail.depth - i,
{
    reveal(nested_apply);
    if i < plan.trail.depth {
        production_parent(before, encoded, after, plan, budget, semantic, i);
        production_nested(before, encoded, after, plan, budget, semantic, i + 1);
        assert(edge_valid(before, plan.trail.nodes[i], plan.path.steps[i], plan.trail.nodes[i + 1]));
        let old = node_bytes(before, plan.trail.nodes[i]);
        let span = validation::child_span(old, plan.trail.nodes[i].layout,
            plan.trail.nodes[i].ordinal as int);
        assert(old.subrange(span.start as int, span.end as int) =~= node_bytes(before, plan.trail.nodes[i + 1]));
        assert(ordinals(plan, i).subrange(1, ordinals(plan, i).len() as int) =~= ordinals(plan, i + 1));
    }
}

/// Exact parent header deltas, independent of the rebuilding implementation.
spec fn parent_words(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int, delta: int) -> bool {
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } => forall|i: int| 0 <= i < variable.len() - 1
            ==> #[trigger] words::le_word(after, fixed_bytes as int + 4 * i)
                == words::le_word(before, fixed_bytes as int + 4 * i)
                    + if selected < fixed.len() || i < selected - fixed.len() { 0int } else { delta },
        LayoutHead::List { element } => words::le_word(after, 0) == words::le_word(before, 0)
            && (width_spec(element).is_none() ==> forall|i: int| 0 <= i < words::le_word(before, 0) - 1
                ==> #[trigger] words::le_word(after, 4 + 4 * i)
                    == words::le_word(before, 4 + 4 * i) + if i < selected { 0int } else { delta }),
        LayoutHead::Map { .. } => words::le_word(after, 0) == words::le_word(before, 0)
            && (crate::map_layout::value_width(layout).is_none() ==> forall|i: int|
                0 <= i < words::le_word(before, 0) ==> #[trigger] words::le_word(after,
                    4 + i * crate::map_layout::entry_stride(layout) + crate::map_layout::key_width(layout))
                    == words::le_word(before, 4 + i * crate::map_layout::entry_stride(layout)
                        + crate::map_layout::key_width(layout)) + if i < selected { 0int } else { delta })
            && forall|i: int| 0 <= i < words::le_word(before, 0)
                ==> #[trigger] crate::search::encoded_key(after, layout, i)
                    == crate::search::encoded_key(before, layout, i),
        LayoutHead::Option { .. } => after[0] == before[0],
        _ => false,
    }
}

/// A suffix run changes each end at or after its first ordinal exactly once.
#[verifier::spinoff_prover]
proof fn directory_projection(before: Seq<u8>, after: Seq<u8>, run: PatchRun, prefix: int,
    base: int, stride: int, total: int, first: int, delta: i64, i: int)
    requires 0 <= base, stride >= 4, 0 <= first, 0 <= i < total,
        base + (total - 1) * stride + 4 <= prefix <= before.len(), prefix <= after.len(),
        run.count == if first < total { total - first } else { 0int },
        run.count > 0 ==> run.offset == base + first * stride && run.stride == stride,
        run_safe(before, run, delta), patch_delta(before, after, run, delta, run.count as int),
        forall|p: int| 0 <= p < prefix && !run_changed(run, run.count as int, p)
            ==> #[trigger] after[p] == before[p],
    ensures words::le_word(after, base + i * stride)
        == words::le_word(before, base + i * stride) + if i < first { 0int } else { delta as int },
{
    let at = base + i * stride;
    assert(at + 4 <= prefix) by(nonlinear_arith)
        requires 0 <= i < total, stride >= 4, base + (total - 1) * stride + 4 <= prefix, at == base + i * stride;
    if first <= i {
        let k = i - first;
        assert(run.offset + k * run.stride == at) by(nonlinear_arith)
            requires run.offset == base + first * stride, run.stride == stride,
                k == i - first, at == base + i * stride;
        assert(run_at(run, k) == at);
        assert(patch_safe_at(before, run, delta, k));
        assert(words::encode_exact(after, run_at(run, k),
            (words::le_word(before, run_at(run, k)) + delta) as u32));
    } else {
        assert forall|p: int| at <= p < at + 4 implies !run_changed(run, run.count as int, p) by {
            if run_changed(run, run.count as int, p) {
                let k = choose|k: int| 0 <= k < run.count && #[trigger]
                    run_at(run, k) <= p < run_at(run, k) + 4;
                assert(at + 4 <= run_at(run, k)) by(nonlinear_arith)
                    requires i < first, k >= 0, stride >= 4,
                        run.offset == base + first * stride, run.stride == stride, at == base + i * stride;
            }
        }
        assert(after[at] == before[at]);
        assert(after[at + 1] == before[at + 1]);
        assert(after[at + 2] == before[at + 2]);
        assert(after[at + 3] == before[at + 3]);
    }
}

/// Struct metadata classifies every variable field and orders fixed intervals.
#[verifier::spinoff_prover]
proof fn fields_metadata(fixed: Seq<crate::Field>, variable: Seq<crate::Field>, size: u32,
    i: int, j: int, offset: int, budget: nat)
    requires crate::layout::descriptor_fields(fixed, variable, size, i, j, offset, budget),
    ensures forall|k: int| j <= k < variable.len() ==> #[trigger] width_spec(variable[k].layout).is_none(),
        forall|k: int| i <= k < fixed.len() ==> #[trigger] fixed[k].offset >= offset,
        forall|k: int, l: int| i <= k < l < fixed.len() ==> #[trigger] fixed[k].offset
            + width_spec(fixed[k].layout).unwrap() <= #[trigger] fixed[l].offset,
    decreases fixed.len() + variable.len() - i - j,
{
    reveal_with_fuel(crate::layout::descriptor_fields, 2);
    if i < fixed.len() && (j == variable.len() || fixed[i].declaration <= variable[j].declaration) {
        fields_metadata(fixed, variable, size, i + 1, j,
            offset + width_spec(fixed[i].layout).unwrap(), budget);
    } else if j < variable.len() {
        fields_metadata(fixed, variable, size, i, j + 1, offset, budget);
    }
}

/// One variable-field lookup retains its metadata width classification.
proof fn variable_domain(fixed: Seq<crate::Field>, variable: Seq<crate::Field>, size: u32,
    i: int, j: int, offset: int, budget: nat, selected: int)
    requires crate::layout::descriptor_fields(fixed, variable, size, i, j, offset, budget),
        j <= selected < variable.len(),
    ensures width_spec(variable[selected].layout).is_none(),
    decreases fixed.len() + variable.len() - i - j,
{
    reveal_with_fuel(crate::layout::descriptor_fields, 2);
    if i < fixed.len() && (j == variable.len() || fixed[i].declaration <= variable[j].declaration) {
        variable_domain(fixed, variable, size, i + 1, j,
            offset + width_spec(fixed[i].layout).unwrap(), budget, selected);
    } else if j < selected {
        variable_domain(fixed, variable, size, i, j + 1, offset, budget, selected);
    }
}

/// Equal-width children keep their exact parent extent.
#[verifier::spinoff_prover]
proof fn fixed_child_delta(before: Seq<u8>, layout: Layout, selected: int, child: Seq<u8>,
    budget: nat, semantic: bool)
    requires validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        width_spec(validation::child_layout(layout, selected)).is_some(),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
    ensures child.len() == validation::child_span(before, layout, selected).end
        - validation::child_span(before, layout, selected).start,
{
    hide(validation::schema_valid);
    fixed_extent(child, validation::child_layout(layout, selected), budget, semantic);
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } => {
            crate::edit_layout::fixed_bounds(fixed.view(), variable.view(), fixed_bytes,
                0, 0, 0, MAX_DEPTH as nat);
            fields_metadata(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0, MAX_DEPTH as nat);
            if selected >= fixed.len() {
                variable_domain(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0,
                    MAX_DEPTH as nat, selected - fixed.len());
                assert(width_spec(variable.view()[selected - fixed.len()].layout).is_none());
                assert(false);
            }
            assert(selected < fixed.len());
            assert(fixed.view()[selected].offset + width_spec(fixed.view()[selected].layout).unwrap() <= fixed_bytes);
            assert(validation::child_span(before, layout, selected).end
                == fixed.view()[selected].offset + width_spec(fixed.view()[selected].layout).unwrap());
        },
        LayoutHead::List { .. } | LayoutHead::Map { .. } => {
            validation::leaf_span_bounds(before, layout, selected, semantic);
        },
        LayoutHead::Option { value } => {
            assert(before[0] == 1);
            assert(before.len() == 1 + width_spec(value).unwrap());
            assert(validation::child_span(before, layout, selected).start == 1);
            assert(validation::child_span(before, layout, selected).end == before.len());
        },
        _ => { assert(false); },
    }
}

/// Byte-identical keys retain their domain and strict order.
#[verifier::spinoff_prover]
pub(super) proof fn keys_frame(before: Seq<u8>, after: Seq<u8>, layout: Layout, count: int)
    requires forall|i: int| 0 <= i < count ==> #[trigger] crate::search::encoded_key(after, layout, i)
        == crate::search::encoded_key(before, layout, i),
    ensures crate::search::keys_ordered(after, layout, count) == crate::search::keys_ordered(before, layout, count),
        crate::search::keys_canonical(after, layout, count) == crate::search::keys_canonical(before, layout, count),
{
    hide(crate::search::lex_less);
    assert forall|i: int, j: int| 0 <= i < j < count implies
        crate::search::lex_less(#[trigger] crate::search::encoded_key(after, layout, i),
            #[trigger] crate::search::encoded_key(after, layout, j))
        == crate::search::lex_less(crate::search::encoded_key(before, layout, i),
            crate::search::encoded_key(before, layout, j)) by {
        assert(crate::search::encoded_key(after, layout, i) == crate::search::encoded_key(before, layout, i));
        assert(crate::search::encoded_key(after, layout, j) == crate::search::encoded_key(before, layout, j));
    }
    if crate::search::keys_ordered(before, layout, count) {
        assert forall|i: int, j: int| 0 <= i < j < count implies
            crate::search::lex_less(#[trigger] crate::search::encoded_key(after, layout, i),
                #[trigger] crate::search::encoded_key(after, layout, j)) by {
            assert(crate::search::encoded_key(after, layout, i) == crate::search::encoded_key(before, layout, i));
            assert(crate::search::encoded_key(after, layout, j) == crate::search::encoded_key(before, layout, j));
        }
    } else if crate::search::keys_ordered(after, layout, count) {
        assert forall|i: int, j: int| 0 <= i < j < count implies
            crate::search::lex_less(#[trigger] crate::search::encoded_key(before, layout, i),
                #[trigger] crate::search::encoded_key(before, layout, j)) by {
            assert(crate::search::encoded_key(after, layout, i) == crate::search::encoded_key(before, layout, i));
            assert(crate::search::encoded_key(after, layout, j) == crate::search::encoded_key(before, layout, j));
        }
    }
    if crate::search::keys_canonical(before, layout, count) {
        assert forall|i: int| 0 <= i < count implies !crate::search::bool_keys(layout)
            || (#[trigger] crate::search::encoded_key(after, layout, i))[0] <= 1 by {
            assert(crate::search::encoded_key(after, layout, i) == crate::search::encoded_key(before, layout, i));
        }
    } else if crate::search::keys_canonical(after, layout, count) {
        assert forall|i: int| 0 <= i < count implies !crate::search::bool_keys(layout)
            || (#[trigger] crate::search::encoded_key(before, layout, i))[0] <= 1 by {
            assert(crate::search::encoded_key(after, layout, i) == crate::search::encoded_key(before, layout, i));
        }
    }
}

/// Local struct validity exposes the declaration-ordered metadata relation.
proof fn struct_metadata(bytes: Seq<u8>, layout: Layout, semantic: bool)
    requires validation::local_valid(bytes, layout, semantic),
        matches!(layout.head(), LayoutHead::Struct { .. }),
    ensures match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } =>
            crate::layout::descriptor_fields(fixed.view(), variable.view(), fixed_bytes,
                0, 0, 0, MAX_DEPTH as nat),
        _ => false,
    },
{ }

/// Local parent shape follows from the exact end deltas and a valid child.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn parent_partition_struct(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires matches!(layout.head(), LayoutHead::Struct { .. }),
        validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after.len() <= u32::MAX,
        after.len() == before.len() + child.len()
            - (validation::child_span(before, layout, selected).end - validation::child_span(before, layout, selected).start),
        parent_words(before, after, layout, selected, after.len() as int - before.len()),
    ensures validation::local_valid(after, layout, semantic),
        validation::child_count(after, layout) == validation::child_count(before, layout),
{
    hide(validation::schema_valid);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    hide(crate::search::encoded_key);
    hide(crate::search::lex_less);
    hide(crate::layout::descriptor_valid);
    hide(crate::layout::descriptor_fields);
    reveal_with_fuel(crate::layout::descriptor_valid, 1);
    child_bounds(before, layout, selected, semantic);
    let span = validation::child_span(before, layout, selected);
    let delta = after.len() as int - before.len();
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, budget, semantic);
        assert(delta == 0);
    }
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } => {
            if selected < fixed.len() {
                struct_metadata(before, layout, semantic);
                assert(crate::layout::descriptor_fields(fixed.view(), variable.view(), fixed_bytes,
                    0, 0, 0, MAX_DEPTH as nat));
                crate::edit_layout::fixed_bounds(fixed.view(), variable.view(), fixed_bytes,
                    0, 0, 0, MAX_DEPTH as nat);
                fixed_child_delta(before, layout, selected, child, budget, semantic);
                assert(delta == 0);
                assert(crate::struct_layout::struct_shape(after, layout));
            }
            if selected >= fixed.len() {
                assert(0 <= selected - fixed.len() < variable.len() <= 65535);
                assert(crate::struct_layout::payload_start(layout) <= span.start);
                assert(span.start <= span.end as int + delta);
                assert(span == crate::struct_layout::variable_span(before, layout, (selected - fixed.len()) as u16));
                preservation::struct_variable_join(before, after, layout,
                    selected - fixed.len(), delta, semantic);
            }
        },
        _ => { assert(false); },
    }
    assert(validation::local_valid(after, layout, semantic));
}

/// Local parent shape follows from the exact end deltas and a valid child.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn parent_partition_list(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires matches!(layout.head(), LayoutHead::List { .. }),
        validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after.len() <= u32::MAX,
        after.len() == before.len() + child.len()
            - (validation::child_span(before, layout, selected).end - validation::child_span(before, layout, selected).start),
        parent_words(before, after, layout, selected, after.len() as int - before.len()),
    ensures validation::local_valid(after, layout, semantic),
        validation::child_count(after, layout) == validation::child_count(before, layout),
{
    hide(validation::schema_valid);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    hide(crate::search::encoded_key);
    hide(crate::search::lex_less);
    hide(crate::layout::descriptor_valid);
    hide(crate::layout::descriptor_fields);
    reveal_with_fuel(crate::layout::descriptor_valid, 1);
    child_bounds(before, layout, selected, semantic);
    let span = validation::child_span(before, layout, selected);
    let delta = after.len() as int - before.len();
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, budget, semantic);
        assert(delta == 0);
    }
    match layout.head() {
        LayoutHead::List { element } => if width_spec(element).is_none() {
            let count = words::le_word(before, 0);
            let p = crate::list_layout::payload_start(before, layout);
            assert(p <= span.start);
            assert(p <= after.len());
            assert forall|i: int| 0 <= i < count - 1 implies {
                let end = #[trigger] words::le_word(after, 4 + 4 * i);
                &&& end <= after.len() - p
                &&& if i == 0 { 0 <= end }
                    else { words::le_word(after, 4 + 4 * (i - 1)) <= end }
            } by {
                if i < selected {
                    if selected > 0 {
                        preservation::directory_order(before, 4, count - 1, i, selected - 1);
                    }
                    assert(words::le_word(before, 4 + 4 * i) <= span.start - p);
                } else if i == selected && i > 0 {
                    assert(span.start == p + words::le_word(before, 4 + 4 * (i - 1)));
                    assert(span.end == p + words::le_word(before, 4 + 4 * i));
                }
            }
        },
        _ => { assert(false); },
    }
    assert(validation::local_valid(after, layout, semantic));
}

/// Local parent shape follows from the exact end deltas and a valid child.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn parent_partition_map(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires matches!(layout.head(), LayoutHead::Map { .. }),
        validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after.len() <= u32::MAX,
        after.len() == before.len() + child.len()
            - (validation::child_span(before, layout, selected).end - validation::child_span(before, layout, selected).start),
        parent_words(before, after, layout, selected, after.len() as int - before.len()),
    ensures validation::local_valid(after, layout, semantic),
        validation::child_count(after, layout) == validation::child_count(before, layout),
{
    hide(validation::schema_valid);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    hide(crate::search::encoded_key);
    hide(crate::search::lex_less);
    hide(crate::layout::descriptor_valid);
    hide(crate::layout::descriptor_fields);
    reveal_with_fuel(crate::layout::descriptor_valid, 1);
    child_bounds(before, layout, selected, semantic);
    let span = validation::child_span(before, layout, selected);
    let delta = after.len() as int - before.len();
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, budget, semantic);
        assert(delta == 0);
    }
    match layout.head() {
        LayoutHead::Map { .. } => if crate::map_layout::value_width(layout).is_none() {
            let count = words::le_word(before, 0);
            let stride = crate::map_layout::entry_stride(layout);
            let key = crate::map_layout::key_width(layout);
            let p = crate::map_layout::payload_start(before, layout);
            assert(p <= span.start);
            assert(p <= after.len());
            assert forall|i: int| 0 <= i < count implies {
                let end = #[trigger] words::le_word(after, 4 + i * stride + key);
                &&& end <= after.len() - p
                &&& if i == 0 { 0 <= end }
                    else { words::le_word(after, 4 + (i - 1) * stride + key) <= end }
            } by {
                if i < selected {
                    // Adjacent map records use a strided version of the same chain.
                    map_end_order(before, layout, i, selected - 1);
                    assert(words::le_word(before, 4 + i * stride + key) <= span.start - p);
                } else if i == selected && i > 0 {
                    assert(span.start == p + words::le_word(before, 4 + (i - 1) * stride + key));
                    assert(span.end == p + words::le_word(before, 4 + i * stride + key));
                }
            }
        },
        _ => { assert(false); },
    }
    keys_frame(before, after, layout, words::le_word(before, 0));
    assert(validation::local_valid(after, layout, semantic));
}

/// Local parent shape follows from the exact end deltas and a valid child.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn parent_partition_option(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires matches!(layout.head(), LayoutHead::Option { .. }),
        validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after.len() <= u32::MAX,
        after.len() == before.len() + child.len()
            - (validation::child_span(before, layout, selected).end - validation::child_span(before, layout, selected).start),
        parent_words(before, after, layout, selected, after.len() as int - before.len()),
    ensures validation::local_valid(after, layout, semantic),
        validation::child_count(after, layout) == validation::child_count(before, layout),
{
    hide(validation::schema_valid);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    hide(crate::search::encoded_key);
    hide(crate::search::lex_less);
    hide(crate::layout::descriptor_valid);
    hide(crate::layout::descriptor_fields);
    reveal_with_fuel(crate::layout::descriptor_valid, 1);
    child_bounds(before, layout, selected, semantic);
    let span = validation::child_span(before, layout, selected);
    let delta = after.len() as int - before.len();
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, budget, semantic);
        assert(delta == 0);
    }
}

/// Local parent shape follows from the exact end deltas and a valid child.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn parent_partition(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after.len() <= u32::MAX,
        after.len() == before.len() + child.len()
            - (validation::child_span(before, layout, selected).end - validation::child_span(before, layout, selected).start),
        parent_words(before, after, layout, selected, after.len() as int - before.len()),
    ensures validation::local_valid(after, layout, semantic),
        validation::child_count(after, layout) == validation::child_count(before, layout),
{
    match layout.head() {
        LayoutHead::Struct { .. } => parent_partition_struct(before, after, layout, selected, child, budget, semantic),
        LayoutHead::List { .. } => parent_partition_list(before, after, layout, selected, child, budget, semantic),
        LayoutHead::Map { .. } => parent_partition_map(before, after, layout, selected, child, budget, semantic),
        LayoutHead::Option { .. } => parent_partition_option(before, after, layout, selected, child, budget, semantic),
        _ => { assert(false); },
    }
}

/// Adjacent map ends imply order across any finite strided interval.
proof fn map_end_order(bytes: Seq<u8>, layout: Layout, i: int, j: int)
    requires validation::local_valid(bytes, layout, false),
        matches!(layout.head(), LayoutHead::Map { .. }),
        crate::map_layout::value_width(layout).is_none(),
        0 <= i <= j < words::le_word(bytes, 0),
    ensures words::le_word(bytes, 4 + i * crate::map_layout::entry_stride(layout) + crate::map_layout::key_width(layout))
        <= words::le_word(bytes, 4 + j * crate::map_layout::entry_stride(layout) + crate::map_layout::key_width(layout)),
    decreases j - i,
{
    if i < j { map_end_order(bytes, layout, i, j - 1); }
}

/// Sequence concatenation gives the rebuilt parent's exact mathematical size.
proof fn rebuild_geometry(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int, child: Seq<u8>)
    requires after == rebuild(before, layout, selected, child),
        validation::child_span(before, layout, selected).start
            <= validation::child_span(before, layout, selected).end <= before.len(),
    ensures after.len() == before.len() + child.len()
        - (validation::child_span(before, layout, selected).end - validation::child_span(before, layout, selected).start),
{ }

/// Reconstruction publishes the prefix words and frames both sibling regions.
#[verifier::spinoff_prover]
proof fn rebuild_frame(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, delta: i64)
    requires after == rebuild(before, layout, selected, child),
        validation::child_span(before, layout, selected).start
            <= validation::child_span(before, layout, selected).end <= before.len(),
        delta == child.len() as int - (validation::child_span(before, layout, selected).end
            - validation::child_span(before, layout, selected).start),
        run_within(expected_run(before, layout, selected), 0,
            validation::child_span(before, layout, selected).start as int),
        run_safe(before, expected_run(before, layout, selected), delta),
    ensures after.len() == before.len() + delta,
        patch_delta(before, after, expected_run(before, layout, selected), delta,
            expected_run(before, layout, selected).count as int),
        forall|p: int| 0 <= p < validation::child_span(before, layout, selected).start
            && !run_changed(expected_run(before, layout, selected), expected_run(before, layout, selected).count as int, p)
            ==> #[trigger] after[p] == before[p],
        forall|p: int| validation::child_span(before, layout, selected).end + delta <= p < after.len()
            ==> #[trigger] after[p] == before[p - delta],
        after.subrange(validation::child_span(before, layout, selected).start as int,
            validation::child_span(before, layout, selected).end + delta) == child,
{
    let span = validation::child_span(before, layout, selected);
    let run = expected_run(before, layout, selected);
    let prefix = patched_prefix(before, run, span.start as int, delta as int);
    prefix_exact(before, run, span.start as int, delta);
    assert forall|k: int| 0 <= k < run.count implies #[trigger]
        words::encode_exact(after, run_at(run, k),
            (words::le_word(before, run_at(run, k)) + delta) as u32) by {
        run_index_bounds(run, span.start as int, k);
        assert(words::encode_exact(prefix, run_at(run, k),
            (words::le_word(before, run_at(run, k)) + delta) as u32));
    }
    assert(after.subrange(span.start as int, span.end + delta) =~= child);
}

/// A header word outside the child and the patched run is byte-identical.
proof fn unchanged_word(before: Seq<u8>, after: Seq<u8>, at: int, start: int, end: int,
    delta: int, run: PatchRun)
    requires 0 <= at, at + 4 <= before.len(), at + 4 <= after.len(),
        (at + 4 <= start && (forall|p: int| at <= p < at + 4 ==> !run_changed(run, run.count as int, p)))
            || (delta == 0 && end <= at),
        forall|p: int| 0 <= p < start && !run_changed(run, run.count as int, p)
            ==> #[trigger] after[p] == before[p],
        forall|p: int| end + delta <= p < after.len() ==> #[trigger] after[p] == before[p - delta],
    ensures words::le_word(after, at) == words::le_word(before, at),
{
    assert(after[at] == before[at]);
    assert(after[at + 1] == before[at + 1]);
    assert(after[at + 2] == before[at + 2]);
    assert(after[at + 3] == before[at + 3]);
}

/// The reconstructed header has precisely the parent-level word relation.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rebuild_words_struct(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires matches!(layout.head(), LayoutHead::Struct { .. }),
        validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after == rebuild(before, layout, selected, child), after.len() <= u32::MAX,
        run_safe(before, expected_run(before, layout, selected), (after.len() as int - before.len()) as i64),
    ensures parent_words(before, after, layout, selected, after.len() as int - before.len()),
{
    hide(rebuild);
    hide(patched_prefix);
    hide(validation::schema_valid);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    child_bounds(before, layout, selected, semantic);
    run_is_header(before, layout, selected, semantic);
    let span = validation::child_span(before, layout, selected);
    rebuild_geometry(before, after, layout, selected, child);
    let delta = after.len() as int - before.len();
    let run = expected_run(before, layout, selected);
    assert(after.len() == before.len() + child.len() - (span.end - span.start));
    rebuild_frame(before, after, layout, selected, child, delta as i64);
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, budget, semantic);
        assert(delta == 0);
        if let LayoutHead::Struct { fixed_bytes, fixed, variable } = layout.head() {
            if selected >= fixed.len() {
                variable_domain(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0,
                    MAX_DEPTH as nat, selected - fixed.len());
                assert(false);
            }
        }
        assert(run.count == 0);
    }
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } => {
            crate::edit_layout::fixed_bounds(fixed.view(), variable.view(), fixed_bytes,
                0, 0, 0, MAX_DEPTH as nat);
            fields_metadata(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0, MAX_DEPTH as nat);
            assert forall|i: int| 0 <= i < variable.len() - 1 implies #[trigger]
                words::le_word(after, fixed_bytes as int + 4 * i)
                    == words::le_word(before, fixed_bytes as int + 4 * i)
                        + if selected < fixed.len() || i < selected - fixed.len() { 0int } else { delta } by {
                if selected < fixed.len() {
                    assert(span.end <= fixed_bytes);
                    unchanged_word(before, after, fixed_bytes as int + 4 * i,
                        span.start as int, span.end as int, delta, run);
                } else {
                    directory_projection(before, after, run, span.start as int, fixed_bytes as int,
                        4, variable.len() as int - 1, selected - fixed.len(), delta as i64, i);
                }
            }
        },
        _ => { assert(false); },
    }
}

/// The reconstructed header has precisely the parent-level word relation.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rebuild_words_list(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires matches!(layout.head(), LayoutHead::List { .. }),
        validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after == rebuild(before, layout, selected, child), after.len() <= u32::MAX,
        run_safe(before, expected_run(before, layout, selected), (after.len() as int - before.len()) as i64),
    ensures parent_words(before, after, layout, selected, after.len() as int - before.len()),
{
    hide(rebuild);
    hide(patched_prefix);
    hide(validation::schema_valid);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    child_bounds(before, layout, selected, semantic);
    run_is_header(before, layout, selected, semantic);
    let span = validation::child_span(before, layout, selected);
    rebuild_geometry(before, after, layout, selected, child);
    let delta = after.len() as int - before.len();
    let run = expected_run(before, layout, selected);
    assert(after.len() == before.len() + child.len() - (span.end - span.start));
    rebuild_frame(before, after, layout, selected, child, delta as i64);
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, budget, semantic);
        assert(delta == 0);
        if let LayoutHead::Struct { fixed_bytes, fixed, variable } = layout.head() {
            if selected >= fixed.len() {
                variable_domain(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0,
                    MAX_DEPTH as nat, selected - fixed.len());
                assert(false);
            }
        }
        assert(run.count == 0);
    }
    match layout.head() {
        LayoutHead::List { element } => {
            assert(4 <= span.start);
            assert forall|p: int| 0 <= p < 4 implies !run_changed(run, run.count as int, p) by {
                if run_changed(run, run.count as int, p) {
                    let k = choose|k: int| 0 <= k < run.count && #[trigger]
                        run_at(run, k) <= p < run_at(run, k) + 4;
                    run_index_bounds(run, span.start as int, k);
                }
            }
            unchanged_word(before, after, 0, span.start as int, span.end as int, delta, run);
            if width_spec(element).is_none() {
                assert forall|i: int| 0 <= i < words::le_word(before, 0) - 1 implies #[trigger]
                    words::le_word(after, 4 + 4 * i) == words::le_word(before, 4 + 4 * i)
                        + if i < selected { 0int } else { delta } by {
                    directory_projection(before, after, run, span.start as int, 4, 4,
                        words::le_word(before, 0) - 1, selected, delta as i64, i);
                }
            }
        },
        _ => { assert(false); },
    }
}

/// Rebuilding one map value leaves every key's exact bytes unchanged.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rebuild_map_keys(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires matches!(layout.head(), LayoutHead::Map { .. }),
        validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after == rebuild(before, layout, selected, child), after.len() <= u32::MAX,
        run_safe(before, expected_run(before, layout, selected), (after.len() as int - before.len()) as i64),
    ensures forall|i: int| 0 <= i < words::le_word(before, 0)
        ==> #[trigger] crate::search::encoded_key(after, layout, i) == crate::search::encoded_key(before, layout, i),
{
    hide(validation::local_valid);
    hide(rebuild);
    hide(patched_prefix);
    hide(validation::schema_valid);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    child_bounds(before, layout, selected, semantic);
    run_is_header(before, layout, selected, semantic);
    let span = validation::child_span(before, layout, selected);
    rebuild_geometry(before, after, layout, selected, child);
    let delta = after.len() as int - before.len();
    let run = expected_run(before, layout, selected);
    assert(after.len() == before.len() + child.len() - (span.end - span.start));
    rebuild_frame(before, after, layout, selected, child, delta as i64);
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, budget, semantic);
        assert(delta == 0);
        if let LayoutHead::Struct { fixed_bytes, fixed, variable } = layout.head() {
            if selected >= fixed.len() {
                variable_domain(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0,
                    MAX_DEPTH as nat, selected - fixed.len());
                assert(false);
            }
        }
        assert(run.count == 0);
    }

    map_run_geometry(before, layout, selected, semantic);
    let stride = crate::map_layout::entry_stride(layout);
    let key = crate::map_layout::key_width(layout);
    let count = words::le_word(before, 0);
    assert(4 + count * stride <= after.len());
            assert forall|i: int| 0 <= i < count implies #[trigger] crate::search::encoded_key(after, layout, i)
                == crate::search::encoded_key(before, layout, i) by {
                let at = 4 + i * stride;
                assert(at + key <= before.len()) by(nonlinear_arith)
                    requires 0 <= i < count, stride >= key > 0,
                        4 + count * stride <= before.len(), at == 4 + i * stride;
                assert(at + key <= after.len()) by(nonlinear_arith)
                    requires 0 <= i < count, stride >= key > 0,
                        4 + count * stride <= after.len(), at == 4 + i * stride;
                assert forall|p: int| at <= p < at + key implies #[trigger] after[p] == before[p] by {
                    if crate::map_layout::value_width(layout).is_some() {
                        if i <= selected {
                            assert(p < span.start) by(nonlinear_arith)
                                requires at == 4 + i * stride, span.start == 4 + selected * stride + key,
                                    stride >= key, i <= selected, p < at + key;
                        } else {
                            assert(span.end <= p) by(nonlinear_arith)
                                requires span.end == 4 + (selected + 1) * stride, at == 4 + i * stride,
                                    stride >= 0, selected < i, at <= p;
                        }
                    } else {
                        assert(at + key <= crate::map_layout::payload_start(before, layout)) by(nonlinear_arith)
                            requires at == 4 + i * stride, 0 <= i < count, stride == key + 4,
                                crate::map_layout::payload_start(before, layout) == 4 + count * stride;
                        assert(p < span.start);
                        if run_changed(run, run.count as int, p) {
                        let k = choose|k: int| 0 <= k < run.count && #[trigger]
                            run_at(run, k) <= p < run_at(run, k) + 4;
                        let j = selected + k;
                        assert(run.offset + k * run.stride == 4 + j * stride + key) by(nonlinear_arith)
                            requires run.offset == 4 + selected * stride + key,
                                run.stride == stride, j == selected + k;
                        assert(run_at(run, k) == 4 + j * stride + key);
                        if j < i {
                            assert(run_at(run, k) + 4 <= at) by(nonlinear_arith)
                                requires stride == key + 4, j < i,
                                    run_at(run, k) == 4 + j * stride + key, at == 4 + i * stride;
                        } else {
                            assert(at + key <= run_at(run, k)) by(nonlinear_arith)
                                requires i <= j, stride >= 0,
                                    run_at(run, k) == 4 + j * stride + key, at == 4 + i * stride;
                        }
                        }
                    }
                }
                assert(crate::search::encoded_key(after, layout, i) =~= crate::search::encoded_key(before, layout, i));
            }
}

/// The reconstructed header has precisely the parent-level word relation.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rebuild_words_map(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires matches!(layout.head(), LayoutHead::Map { .. }),
        validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after == rebuild(before, layout, selected, child), after.len() <= u32::MAX,
        run_safe(before, expected_run(before, layout, selected), (after.len() as int - before.len()) as i64),
    ensures parent_words(before, after, layout, selected, after.len() as int - before.len()),
{
    hide(words::le_word);
    hide(crate::search::encoded_key);
    hide(rebuild);
    hide(patched_prefix);
    hide(validation::schema_valid);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    child_bounds(before, layout, selected, semantic);
    run_is_header(before, layout, selected, semantic);
    let span = validation::child_span(before, layout, selected);
    rebuild_geometry(before, after, layout, selected, child);
    let delta = after.len() as int - before.len();
    let run = expected_run(before, layout, selected);
    assert(after.len() == before.len() + child.len() - (span.end - span.start));
    rebuild_frame(before, after, layout, selected, child, delta as i64);
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, budget, semantic);
        assert(delta == 0);
        if let LayoutHead::Struct { fixed_bytes, fixed, variable } = layout.head() {
            if selected >= fixed.len() {
                variable_domain(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0,
                    MAX_DEPTH as nat, selected - fixed.len());
                assert(false);
            }
        }
        assert(run.count == 0);
    }
    match layout.head() {
        LayoutHead::Map { .. } => {
            let stride = crate::map_layout::entry_stride(layout);
            let key = crate::map_layout::key_width(layout);
            let count = words::le_word(before, 0);
            map_run_geometry(before, layout, selected, semantic);
            assert(4 <= span.start);
            assert(run.count == 0 || run.offset >= 4 && run.stride >= 4);
            assert forall|p: int| 0 <= p < 4 implies !run_changed(run, run.count as int, p) by {
                if run_changed(run, run.count as int, p) {
                    let k = choose|k: int| 0 <= k < run.count && #[trigger]
                        run_at(run, k) <= p < run_at(run, k) + 4;
                    run_index_bounds(run, span.start as int, k);
                    assert(run_at(run, k) >= 4) by(nonlinear_arith)
                        requires run.offset >= 4, run.stride >= 4, k >= 0,
                            run_at(run, k) == run.offset + k * run.stride;
                }
            }
            unchanged_word(before, after, 0, span.start as int, span.end as int, delta, run);
            if crate::map_layout::value_width(layout).is_none() {
                assert(4 + key + (count - 1) * stride + 4 == crate::map_layout::payload_start(before, layout))
                    by(nonlinear_arith)
                    requires stride == key + 4,
                        crate::map_layout::payload_start(before, layout) == 4 + count * stride;
                assert(crate::map_layout::payload_start(before, layout) <= span.start);
                assert(span.start <= after.len());
                assert forall|i: int| 0 <= i < count implies #[trigger]
                    words::le_word(after, 4 + i * stride + key)
                        == words::le_word(before, 4 + i * stride + key) + if i < selected { 0int } else { delta } by {
                    directory_projection(before, after, run, span.start as int, 4 + key, stride,
                        count, selected, delta as i64, i);
                }
            }
            rebuild_map_keys(before, after, layout, selected, child, budget, semantic);
        },
        _ => { assert(false); },
    }
}

/// The reconstructed header has precisely the parent-level word relation.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rebuild_words_option(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires matches!(layout.head(), LayoutHead::Option { .. }),
        validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after == rebuild(before, layout, selected, child), after.len() <= u32::MAX,
        run_safe(before, expected_run(before, layout, selected), (after.len() as int - before.len()) as i64),
    ensures parent_words(before, after, layout, selected, after.len() as int - before.len()),
{
    hide(rebuild);
    hide(patched_prefix);
    hide(validation::schema_valid);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    child_bounds(before, layout, selected, semantic);
    run_is_header(before, layout, selected, semantic);
    let span = validation::child_span(before, layout, selected);
    rebuild_geometry(before, after, layout, selected, child);
    let delta = after.len() as int - before.len();
    let run = expected_run(before, layout, selected);
    assert(after.len() == before.len() + child.len() - (span.end - span.start));
    rebuild_frame(before, after, layout, selected, child, delta as i64);
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, budget, semantic);
        assert(delta == 0);
        if let LayoutHead::Struct { fixed_bytes, fixed, variable } = layout.head() {
            if selected >= fixed.len() {
                variable_domain(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0,
                    MAX_DEPTH as nat, selected - fixed.len());
                assert(false);
            }
        }
        assert(run.count == 0);
    }
    match layout.head() {
        LayoutHead::Option { .. } => {},
        _ => { assert(false); },
    }
}

/// The reconstructed header has precisely the parent-level word relation.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rebuild_words(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires validation::local_valid(before, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after == rebuild(before, layout, selected, child), after.len() <= u32::MAX,
        run_safe(before, expected_run(before, layout, selected), (after.len() as int - before.len()) as i64),
    ensures parent_words(before, after, layout, selected, after.len() as int - before.len()),
{
    match layout.head() {
        LayoutHead::Struct { .. } => rebuild_words_struct(before, after, layout, selected, child, budget, semantic),
        LayoutHead::List { .. } => rebuild_words_list(before, after, layout, selected, child, budget, semantic),
        LayoutHead::Map { .. } => rebuild_words_map(before, after, layout, selected, child, budget, semantic),
        LayoutHead::Option { .. } => rebuild_words_option(before, after, layout, selected, child, budget, semantic),
        _ => { assert(false); },
    }
}

/// Parent end-word deltas translate each child interval at the edited join.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn span_transport(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int, i: int, semantic: bool)
    requires validation::local_valid(before, layout, semantic), validation::local_valid(after, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        0 <= i < validation::child_count(before, layout),
        parent_words(before, after, layout, selected, after.len() as int - before.len()),
        width_spec(validation::child_layout(layout, selected)).is_some() ==> after.len() == before.len(),
    ensures ({ let old = validation::child_span(before, layout, i);
        let new = validation::child_span(after, layout, i);
        let delta = after.len() as int - before.len();
        &&& new.start == old.start + if i > selected { delta } else { 0int }
        &&& new.end == old.end + if i >= selected { delta } else { 0int }
    }),
{
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    child_bounds(before, layout, i, semantic);
    child_bounds(after, layout, i, semantic);
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } => {
            if selected < fixed.len() {
                assert(after.len() == before.len()) by {
                    crate::edit_layout::fixed_bounds(fixed.view(), variable.view(), fixed_bytes,
                        0, 0, 0, MAX_DEPTH as nat);
                }
            }
        },
        _ => {},
    }
}

/// Distinct child intervals are disjoint in their physical wire order.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn child_separation(bytes: Seq<u8>, layout: Layout, i: int, j: int, semantic: bool)
    requires validation::local_valid(bytes, layout, semantic),
        0 <= i < j < validation::child_count(bytes, layout),
    ensures validation::child_span(bytes, layout, i).end <= validation::child_span(bytes, layout, j).start,
{
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    child_bounds(bytes, layout, i, semantic);
    child_bounds(bytes, layout, j, semantic);
    let a = validation::child_span(bytes, layout, i);
    let b = validation::child_span(bytes, layout, j);
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } => {
            crate::edit_layout::fixed_bounds(fixed.view(), variable.view(), fixed_bytes,
                0, 0, 0, MAX_DEPTH as nat);
            fields_metadata(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0, MAX_DEPTH as nat);
            if i < fixed.len() {
                if j < fixed.len() {
                    assert(a.end == fixed.view()[i].offset + width_spec(fixed.view()[i].layout).unwrap());
                } else {
                    assert(a.end <= fixed_bytes <= crate::struct_layout::payload_start(layout) <= b.start);
                }
            } else {
                preservation::directory_order(bytes, fixed_bytes as int, variable.len() as int - 1,
                    i - fixed.len(), j - fixed.len() - 1);
            }
        },
        LayoutHead::List { element } => if let Some(width) = width_spec(element) {
            assert(a.end <= b.start) by(nonlinear_arith)
                requires a.end == 4 + (i + 1) * width, b.start == 4 + j * width,
                    width >= 0, i < j;
        } else {
            preservation::directory_order(bytes, 4, words::le_word(bytes, 0) - 1, i, j - 1);
        },
        LayoutHead::Map { .. } => if crate::map_layout::value_width(layout).is_some() {
            let stride = crate::map_layout::entry_stride(layout);
            let key = crate::map_layout::key_width(layout);
            assert(a.end <= b.start) by(nonlinear_arith)
                requires a.end == 4 + (i + 1) * stride, b.start == 4 + j * stride + key,
                    stride >= key >= 0, i < j;
        } else {
            map_end_order(bytes, layout, i, j - 1);
        },
        _ => { assert(false); },
    }
}

/// A selected map entry makes its complete header arithmetic fit the wire.
#[verifier::spinoff_prover]
proof fn map_run_geometry(bytes: Seq<u8>, layout: Layout, selected: int, semantic: bool)
    requires validation::local_valid(bytes, layout, semantic),
        matches!(layout.head(), LayoutHead::Map { .. }),
        0 <= selected < validation::child_count(bytes, layout),
    ensures 0 < crate::map_layout::key_width(layout) <= crate::map_layout::entry_stride(layout),
        4 + words::le_word(bytes, 0) * crate::map_layout::entry_stride(layout) <= bytes.len(),
        crate::map_layout::value_width(layout).is_none() ==> {
            let run = expected_run(bytes, layout, selected);
            let stride = crate::map_layout::entry_stride(layout);
            let key = crate::map_layout::key_width(layout);
            &&& run.stride == stride
            &&& run.count == words::le_word(bytes, 0) - selected
            &&& run.offset == 4 + selected * stride + key
            &&& run.offset >= 4
            &&& run.offset + (run.count - 1) * run.stride + 4 == crate::map_layout::payload_start(bytes, layout)
        },
{
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    let count = words::le_word(bytes, 0);
    let stride = crate::map_layout::entry_stride(layout);
    let key = crate::map_layout::key_width(layout);
    assert(4 + stride <= bytes.len()) by(nonlinear_arith)
        requires count >= 1, stride >= 0, 4 + count * stride <= bytes.len();
    assert(0 <= 4 + selected * stride + key <= u32::MAX) by(nonlinear_arith)
        requires 0 <= selected < count, stride >= key >= 0,
            4 + count * stride <= bytes.len() <= u32::MAX;
    if crate::map_layout::value_width(layout).is_none() {
        assert(4 + selected * stride + key + (count - selected - 1) * stride + 4
            == 4 + count * stride) by(nonlinear_arith)
            requires stride == key + 4;
    }
}

/// Stored ancestor end words never overlap any child's encoded bytes.
#[verifier::spinoff_prover]
proof fn run_disjoint_child_struct(bytes: Seq<u8>, layout: Layout, selected: int, i: int, p: int, semantic: bool)
    requires matches!(layout.head(), LayoutHead::Struct { .. }),
        validation::local_valid(bytes, layout, semantic),
        0 <= selected < validation::child_count(bytes, layout),
        0 <= i < validation::child_count(bytes, layout),
        validation::child_span(bytes, layout, i).start <= p < validation::child_span(bytes, layout, i).end,
    ensures !run_changed(expected_run(bytes, layout, selected), expected_run(bytes, layout, selected).count as int, p),
{
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    child_bounds(bytes, layout, i, semantic);
    run_is_header(bytes, layout, selected, semantic);
    let run = expected_run(bytes, layout, selected);
    if run_changed(run, run.count as int, p) {
        let k = choose|k: int| 0 <= k < run.count && #[trigger]
            run_at(run, k) <= p < run_at(run, k) + 4;
        match layout.head() {
            LayoutHead::Struct { fixed_bytes, fixed, variable } => {
                crate::edit_layout::fixed_bounds(fixed.view(), variable.view(), fixed_bytes,
                    0, 0, 0, MAX_DEPTH as nat);
                if i < fixed.len() {
                    assert(p < fixed_bytes <= run.offset);
                } else {
                    let payload = crate::struct_layout::payload_start(layout);
                    run_index_bounds(run, payload, k);
                    assert(payload <= validation::child_span(bytes, layout, i).start);
                }
            },
            _ => { assert(false); },
        }
    }
}

/// Stored ancestor end words never overlap any child's encoded bytes.
#[verifier::spinoff_prover]
proof fn run_disjoint_child_list(bytes: Seq<u8>, layout: Layout, selected: int, i: int, p: int, semantic: bool)
    requires matches!(layout.head(), LayoutHead::List { .. }),
        validation::local_valid(bytes, layout, semantic),
        0 <= selected < validation::child_count(bytes, layout),
        0 <= i < validation::child_count(bytes, layout),
        validation::child_span(bytes, layout, i).start <= p < validation::child_span(bytes, layout, i).end,
    ensures !run_changed(expected_run(bytes, layout, selected), expected_run(bytes, layout, selected).count as int, p),
{
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    child_bounds(bytes, layout, i, semantic);
    run_is_header(bytes, layout, selected, semantic);
    let run = expected_run(bytes, layout, selected);
    if run_changed(run, run.count as int, p) {
        let k = choose|k: int| 0 <= k < run.count && #[trigger]
            run_at(run, k) <= p < run_at(run, k) + 4;
        match layout.head() {
            LayoutHead::List { .. } => {
                let payload = crate::list_layout::payload_start(bytes, layout);
                run_index_bounds(run, payload, k);
                assert(payload <= validation::child_span(bytes, layout, i).start);
            },
            _ => { assert(false); },
        }
    }
}

/// Stored ancestor end words never overlap any child's encoded bytes.
#[verifier::spinoff_prover]
proof fn run_disjoint_child_map(bytes: Seq<u8>, layout: Layout, selected: int, i: int, p: int, semantic: bool)
    requires matches!(layout.head(), LayoutHead::Map { .. }),
        validation::local_valid(bytes, layout, semantic),
        0 <= selected < validation::child_count(bytes, layout),
        0 <= i < validation::child_count(bytes, layout),
        validation::child_span(bytes, layout, i).start <= p < validation::child_span(bytes, layout, i).end,
    ensures !run_changed(expected_run(bytes, layout, selected), expected_run(bytes, layout, selected).count as int, p),
{
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    child_bounds(bytes, layout, i, semantic);
    run_is_header(bytes, layout, selected, semantic);
    let run = expected_run(bytes, layout, selected);
    if run_changed(run, run.count as int, p) {
        let k = choose|k: int| 0 <= k < run.count && #[trigger]
            run_at(run, k) <= p < run_at(run, k) + 4;
        match layout.head() {
            LayoutHead::Map { .. } => {
                map_run_geometry(bytes, layout, selected, semantic);
                let payload = crate::map_layout::payload_start(bytes, layout);
                run_index_bounds(run, payload, k);
                assert(payload <= validation::child_span(bytes, layout, i).start);
            },
            _ => { assert(false); },
        }
    }
}

/// Stored ancestor end words never overlap any child's encoded bytes.
#[verifier::spinoff_prover]
proof fn run_disjoint_child(bytes: Seq<u8>, layout: Layout, selected: int, i: int, p: int, semantic: bool)
    requires validation::local_valid(bytes, layout, semantic),
        0 <= selected < validation::child_count(bytes, layout),
        0 <= i < validation::child_count(bytes, layout),
        validation::child_span(bytes, layout, i).start <= p < validation::child_span(bytes, layout, i).end,
    ensures !run_changed(expected_run(bytes, layout, selected), expected_run(bytes, layout, selected).count as int, p),
{
    match layout.head() {
        LayoutHead::Struct { .. } => run_disjoint_child_struct(bytes, layout, selected, i, p, semantic),
        LayoutHead::List { .. } => run_disjoint_child_list(bytes, layout, selected, i, p, semantic),
        LayoutHead::Map { .. } => run_disjoint_child_map(bytes, layout, selected, i, p, semantic),
        _ => {},
    }
}

/// Parent word correspondence retains the number of immediate children.
proof fn parent_counts(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int, semantic: bool)
    requires validation::local_valid(before, layout, semantic), validation::local_valid(after, layout, semantic),
        parent_words(before, after, layout, selected, after.len() as int - before.len()),
    ensures validation::child_count(before, layout) == validation::child_count(after, layout),
{ }

/// Every locally valid encoding fits the wire address space.
proof fn local_extent(bytes: Seq<u8>, layout: Layout, semantic: bool)
    requires validation::local_valid(bytes, layout, semantic),
    ensures bytes.len() <= u32::MAX,
{ }

/// Every sibling retains its exact encoding across recursive reconstruction.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rebuild_sibling(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool, i: int)
    requires validation::local_valid(before, layout, semantic), validation::local_valid(after, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        0 <= i < validation::child_count(before, layout), i != selected,
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after == rebuild(before, layout, selected, child),
        parent_words(before, after, layout, selected, after.len() as int - before.len()),
        run_safe(before, expected_run(before, layout, selected), (after.len() as int - before.len()) as i64),
    ensures preservation::sibling_same(before, after, layout, i),
{
    hide(validation::child_span);
    hide(validation::child_layout);
    hide(validation::local_valid);
    hide(parent_words);
    hide(rebuild);
    hide(patched_prefix);
    hide(validation::schema_valid);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    parent_counts(before, after, layout, selected, semantic);
    local_extent(before, layout, semantic);
    local_extent(after, layout, semantic);
    child_bounds(before, layout, i, semantic);
    child_bounds(after, layout, i, semantic);
    run_is_header(before, layout, selected, semantic);
    child_bounds(before, layout, selected, semantic);
    rebuild_geometry(before, after, layout, selected, child);
    let delta = after.len() as int - before.len();
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, budget, semantic);
    }
    span_transport(before, after, layout, selected, i, semantic);
    rebuild_frame(before, after, layout, selected, child, delta as i64);
    let a = validation::child_span(before, layout, i);
    let b = validation::child_span(after, layout, i);
    let span = validation::child_span(before, layout, selected);
    if i < selected { child_separation(before, layout, i, selected, semantic); }
    else { child_separation(before, layout, selected, i, semantic); }
    assert forall|q: int| 0 <= q < a.end - a.start implies
        before.subrange(a.start as int, a.end as int)[q]
            == #[trigger] after.subrange(b.start as int, b.end as int)[q] by {
        if i < selected {
            run_disjoint_child(before, layout, selected, i, a.start + q, semantic);
        }
    }
    assert(before.subrange(a.start as int, a.end as int) =~= after.subrange(b.start as int, b.end as int));
}

/// Pointwise sibling reconstruction supplies the complete parent frame.
#[verifier::spinoff_prover]
proof fn rebuild_all_siblings(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires validation::local_valid(before, layout, semantic), validation::local_valid(after, layout, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
        after == rebuild(before, layout, selected, child),
        parent_words(before, after, layout, selected, after.len() as int - before.len()),
        run_safe(before, expected_run(before, layout, selected), (after.len() as int - before.len()) as i64),
    ensures preservation::siblings_same(before, after, layout, selected),
{
    hide(rebuild);
    hide(validation::local_valid);
    hide(validation::schema_valid);
    hide(validation::child_span);
    hide(validation::child_layout);
    hide(preservation::sibling_same);
    assert forall|i: int| 0 <= i < validation::child_count(before, layout) && (i < selected || selected < i)
        implies #[trigger] preservation::sibling_same(before, after, layout, i) by {
        rebuild_sibling(before, after, layout, selected, child, budget, semantic, i);
    }

}

/// Exact edited-child bytes transfer the input's recursive validity.
proof fn edited_child_valid(after: Seq<u8>, layout: Layout, selected: int, child: Seq<u8>, budget: nat, semantic: bool)
    requires after.subrange(validation::child_span(after, layout, selected).start as int,
            validation::child_span(after, layout, selected).end as int) == child,
        validation::schema_valid(child, validation::child_layout(layout, selected), budget, semantic),
    ensures validation::child_valid(after, layout, selected, budget, semantic),
{ }

/// One independently rebuilt parent preserves its complete recursive schema.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn rebuild_preserves(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int,
    child: Seq<u8>, budget: nat, semantic: bool)
    requires validation::schema_valid(before, layout, budget, semantic),
        0 <= selected < validation::child_count(before, layout),
        validation::schema_valid(child, validation::child_layout(layout, selected), (budget - 1) as nat, semantic),
        after == rebuild(before, layout, selected, child), after.len() <= u32::MAX,
        run_safe(before, expected_run(before, layout, selected), (after.len() as int - before.len()) as i64),
    ensures validation::schema_valid(after, layout, budget, semantic),
        preservation::framed_path(before, after, layout, seq![selected], semantic),
        after.subrange(validation::child_span(after, layout, selected).start as int,
            validation::child_span(after, layout, selected).end as int) == child,
{
    hide(validation::child_span);
    hide(validation::child_layout);
    hide(validation::local_valid);
    hide(rebuild);
    hide(patched_prefix);
    hide(validation::schema_valid);
    hide(validation::local_valid);
    valid_local(before, layout, budget, semantic);
    rebuild_words(before, after, layout, selected, child, (budget - 1) as nat, semantic);
    child_bounds(before, layout, selected, semantic);
    run_is_header(before, layout, selected, semantic);
    let span = validation::child_span(before, layout, selected);
    rebuild_geometry(before, after, layout, selected, child);
    let delta = after.len() as int - before.len();
    rebuild_frame(before, after, layout, selected, child, delta as i64);
    parent_partition(before, after, layout, selected, child, (budget - 1) as nat, semantic);
    child_bounds(after, layout, selected, semantic);
    reveal_with_fuel(preservation::framed_path, 2);
    if width_spec(validation::child_layout(layout, selected)).is_some() {
        fixed_child_delta(before, layout, selected, child, (budget - 1) as nat, semantic);
    }
    span_transport(before, after, layout, selected, selected, semantic);
    let new = validation::child_span(after, layout, selected);
    assert(after.subrange(new.start as int, new.end as int) == child);
    edited_child_valid(after, layout, selected, child, (budget - 1) as nat, semantic);
    rebuild_all_siblings(before, after, layout, selected, child, (budget - 1) as nat, semantic);
    preservation::parent_join(before, after, layout, selected, budget, semantic);
}

/// Readiness's absolute end-word checks also hold in each exact parent slice.
#[verifier::spinoff_prover]
proof fn local_run_safe(before: Seq<u8>, plan: Plan, budget: nat, semantic: bool, i: int)
    requires replacement_checked(before, plan), 0 <= i < plan.trail.depth,
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, budget, semantic),
    ensures run_safe(node_bytes(before, plan.trail.nodes[i]),
        expected_run(node_bytes(before, plan.trail.nodes[i]), plan.trail.nodes[i].layout,
            plan.trail.nodes[i].ordinal as int), plan.delta),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    trail_schema(before, plan, budget, semantic, i);
    let n = plan.trail.nodes[i];
    let old = node_bytes(before, n);
    valid_local(old, n.layout, (budget - i) as nat, semantic);
    assert(edge_valid(before, n, plan.path.steps[i], plan.trail.nodes[i + 1]));
    run_is_header(old, n.layout, n.ordinal as int, semantic);
    let span = validation::child_span(old, n.layout, n.ordinal as int);
    let run = expected_run(old, n.layout, n.ordinal as int);
    assert(edge_valid(before, n, plan.path.steps[i], plan.trail.nodes[i + 1]));
    assert forall|k: int| 0 <= k < run.count implies #[trigger]
        patch_safe_at(old, run, plan.delta, k) by {
        translated_run(run, n.region.start, span.start as int, run_at(run, k));
        run_index_bounds(run, span.start as int, k);
        assert(run_at(n.run, k) == n.region.start + run_at(run, k));
        assert(patch_safe_at(before, n.run, plan.delta, k));
        words::word_subrange(before, n.region.start as int, n.region.end as int, run_at(run, k));
    }
}

/// One checked trail suffix has the expected first ordinal and shorter tail.
proof fn ordinal_suffix(plan: Plan, i: int)
    requires 0 <= i <= plan.trail.depth,
    ensures ordinals(plan, i).len() == plan.trail.depth - i,
        i < plan.trail.depth ==> ordinals(plan, i)[0] == plan.trail.nodes[i].ordinal as int,
        i < plan.trail.depth ==> ordinals(plan, i).subrange(1, ordinals(plan, i).len() as int) == ordinals(plan, i + 1),
{
    if i < plan.trail.depth {
        assert(ordinals(plan, i).subrange(1, ordinals(plan, i).len() as int) =~= ordinals(plan, i + 1));
    }
}

/// A terminal path has no enclosing sibling-frame obligations.
proof fn empty_frame(before: Seq<u8>, after: Seq<u8>, layout: Layout, path: Seq<int>, semantic: bool)
    requires path.len() == 0,
    ensures preservation::framed_path(before, after, layout, path, semantic),
{ }

/// A framed immediate parent and framed child compose one recursive path step.
proof fn prepend_frame(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int, path: Seq<int>, semantic: bool)
    requires path.len() > 0, path[0] == selected,
        preservation::framed_path(before, after, layout, seq![selected], semantic),
        preservation::framed_path(
            before.subrange(validation::child_span(before, layout, selected).start as int,
                validation::child_span(before, layout, selected).end as int),
            after.subrange(validation::child_span(after, layout, selected).start as int,
                validation::child_span(after, layout, selected).end as int),
            validation::child_layout(layout, selected), path.subrange(1, path.len() as int), semantic),
    ensures preservation::framed_path(before, after, layout, path, semantic),
{
    hide(validation::child_span);
    hide(validation::child_layout);
    hide(validation::local_valid);
    reveal_with_fuel(preservation::framed_path, 2);
}

/// A valid edited terminal encoding lifts through every production ancestor.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
pub(super) proof fn lift_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan,
    budget: nat, semantic: bool, i: int)
    requires replacement_checked(before, plan), replacement_result(before, encoded, after, plan),
        0 <= i <= plan.trail.depth,
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, budget, semantic),
        validation::schema_valid(node_after(after, plan, plan.trail.depth as int), plan.trail.selected,
            (budget - plan.trail.depth) as nat, semantic),
    ensures validation::schema_valid(node_after(after, plan, i), plan.trail.nodes[i].layout,
            (budget - i) as nat, semantic),
        preservation::framed_path(node_bytes(before, plan.trail.nodes[i]), node_after(after, plan, i),
            plan.trail.nodes[i].layout, ordinals(plan, i), semantic),
    decreases plan.trail.depth - i,
{
    hide(preservation::framed_path);
    hide(ordinals);
    hide(validation::child_span);
    hide(validation::child_layout);
    hide(validation::local_valid);
    hide(rebuild);
    hide(patched_prefix);
    hide(validation::schema_valid);
    hide(validation::local_valid);
    trail_schema(before, plan, budget, semantic, i);
    ordinal_suffix(plan, i);
    if i < plan.trail.depth {
        lift_preserves(before, encoded, after, plan, budget, semantic, i + 1);
        production_parent(before, encoded, after, plan, budget, semantic, i);
        local_run_safe(before, plan, budget, semantic, i);
        let n = plan.trail.nodes[i];
        let child = plan.trail.nodes[i + 1];
        let old = node_bytes(before, n);
        let new = node_after(after, plan, i);
        let edited = node_after(after, plan, i + 1);
        assert(edge_valid(before, n, plan.path.steps[i], child));
        assert(child.region.start <= child.region.end + plan.delta <= n.region.end + plan.delta);
        assert(new.len() == old.len() + plan.delta);
        assert(edited.len() == child.region.end - child.region.start + plan.delta);
        assert((budget - i - 1) as nat == (budget - (i + 1)) as nat);
        rebuild_preserves(old, new, n.layout, n.ordinal as int, edited, (budget - i) as nat, semantic);
        let span = validation::child_span(old, n.layout, n.ordinal as int);
        assert(old.subrange(span.start as int, span.end as int) =~= node_bytes(before, child));
        assert(ordinals(plan, i).subrange(1, ordinals(plan, i).len() as int) == ordinals(plan, i + 1));
        prepend_frame(old, new, n.layout, n.ordinal as int, ordinals(plan, i), semantic);
    } else {
        empty_frame(node_bytes(before, plan.trail.nodes[i]), node_after(after, plan, i),
            plan.trail.nodes[i].layout, ordinals(plan, i), semantic);
    }
}

}
