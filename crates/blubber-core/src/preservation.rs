//! Recursive schema framing and parent joins over exact encoded child bytes.
//!
//! These ghost lemmas use the validator's independent encoding relation. They
//! do not infer global validity from a local edit admission.

use crate::layout::{descriptor_valid, width_spec};
use crate::validation::{
    child_count, child_layout, child_span, child_valid, children_valid, local_valid, schema_valid,
};
use crate::words::le_word;
use crate::{Layout, LayoutHead, Span, MAX_DEPTH};
use vstd::prelude::*;

verus! {

/// Relocation preserves validity when the exact subtree bytes and layout agree.
pub(crate) proof fn subtree_frame(before: Seq<u8>, after: Seq<u8>, old: Span, new: Span,
    layout: Layout, budget: nat, semantic: bool)
    requires old.start <= old.end <= before.len(), new.start <= new.end <= after.len(),
        before.subrange(old.start as int, old.end as int)
            == after.subrange(new.start as int, new.end as int),
    ensures schema_valid(before.subrange(old.start as int, old.end as int), layout, budget, semantic)
        == schema_valid(after.subrange(new.start as int, new.end as int), layout, budget, semantic),
{ }

/// Extracts one child from the recursive suffix relation.
pub(crate) proof fn child_from_children(bytes: Seq<u8>, layout: Layout, next: int,
    index: int, budget: nat, semantic: bool)
    requires children_valid(bytes, layout, next, budget, semantic),
        0 <= next <= index < child_count(bytes, layout),
    ensures child_valid(bytes, layout, index, budget, semantic),
    decreases index - next,
{
    reveal_with_fuel(children_valid, 2);
    if next < index {
        child_from_children(bytes, layout, next + 1, index, budget, semantic);
    }
}

/// Pointwise child validity rebuilds the finite recursive suffix relation.
pub(crate) proof fn children_from_all(bytes: Seq<u8>, layout: Layout, next: int,
    budget: nat, semantic: bool)
    requires 0 <= next <= child_count(bytes, layout),
        forall|i: int| next <= i < child_count(bytes, layout)
            ==> #[trigger] child_valid(bytes, layout, i, budget, semantic),
    ensures children_valid(bytes, layout, next, budget, semantic),
    decreases child_count(bytes, layout) - next,
{
    hide(child_valid);
    reveal_with_fuel(children_valid, 2);
    if next < child_count(bytes, layout) {
        children_from_all(bytes, layout, next + 1, budget, semantic);
        assert(child_valid(bytes, layout, next, budget, semantic));
        assert(children_valid(bytes, layout, next + 1, budget, semantic));
        crate::validation::children_split(bytes, layout, next, budget, semantic);
    }
}

/// The recursive suffix relation is equivalent to validity at every ordinal.
pub(crate) proof fn children_all(bytes: Seq<u8>, layout: Layout, next: int,
    budget: nat, semantic: bool)
    requires 0 <= next <= child_count(bytes, layout),
    ensures children_valid(bytes, layout, next, budget, semantic)
        == (forall|i: int| next <= i < child_count(bytes, layout)
            ==> #[trigger] child_valid(bytes, layout, i, budget, semantic)),
{
    if children_valid(bytes, layout, next, budget, semantic) {
        assert forall|i: int| next <= i < child_count(bytes, layout)
            implies #[trigger] child_valid(bytes, layout, i, budget, semantic) by {
            child_from_children(bytes, layout, next, i, budget, semantic);
        }
    } else if forall|i: int| next <= i < child_count(bytes, layout)
        ==> #[trigger] child_valid(bytes, layout, i, budget, semantic) {
        children_from_all(bytes, layout, next, budget, semantic);
    }
}

/// Exact containment and byte identity for one relocated sibling interval.
pub(crate) open spec fn sibling_same(before: Seq<u8>, after: Seq<u8>, layout: Layout, i: int) -> bool {
    let old = child_span(before, layout, i);
    let new = child_span(after, layout, i);
    &&& old.start <= old.end <= before.len()
    &&& new.start <= new.end <= after.len()
    &&& before.subrange(old.start as int, old.end as int)
        == after.subrange(new.start as int, new.end as int)
}

/// Every unselected child retains its exact bytes in a contained new interval.
pub(crate) open spec fn siblings_same(before: Seq<u8>, after: Seq<u8>, layout: Layout, selected: int) -> bool {
    forall|i: int| 0 <= i < child_count(before, layout) && (i < selected || selected < i)
        ==> #[trigger] sibling_same(before, after, layout, i)
}

/// Rebuilds a parent from a valid edited child and byte-identical siblings.
///
/// The caller must establish the new parent's local shape, including map keys.
/// That obligation is distinct from recursive child preservation.
pub(crate) proof fn parent_join(before: Seq<u8>, after: Seq<u8>, layout: Layout,
    selected: int, budget: nat, semantic: bool)
    requires schema_valid(before, layout, budget, semantic),
        local_valid(after, layout, semantic),
        child_count(after, layout) == child_count(before, layout),
        0 <= selected < child_count(before, layout),
        child_valid(after, layout, selected, (budget - 1) as nat, semantic),
        siblings_same(before, after, layout, selected),
    ensures schema_valid(after, layout, budget, semantic),
{
    children_all(before, layout, 0, (budget - 1) as nat, semantic);
    assert forall|i: int| 0 <= i < child_count(after, layout)
        implies #[trigger] child_valid(after, layout, i, (budget - 1) as nat, semantic) by {
        if i != selected {
            assert(sibling_same(before, after, layout, i));
            let old = child_span(before, layout, i);
            let new = child_span(after, layout, i);
            assert(child_valid(before, layout, i, (budget - 1) as nat, semantic));
            subtree_frame(before, after, old, new, child_layout(layout, i),
                (budget - 1) as nat, semantic);
            assert(schema_valid(after.subrange(new.start as int, new.end as int),
                child_layout(layout, i), (budget - 1) as nat, semantic));
        }
    }
    children_all(after, layout, 0, (budget - 1) as nat, semantic);
}

/// Exact sibling frames and new local partitions along an ordinal path.
///
/// This is a composition premise, not a property inferred from local admission.
pub(crate) open spec fn framed_path(before: Seq<u8>, after: Seq<u8>, layout: Layout,
    path: Seq<int>, semantic: bool) -> bool
    decreases path.len(),
{
    if path.len() == 0 { true } else {
        let selected = path[0];
        let old = child_span(before, layout, selected);
        let new = child_span(after, layout, selected);
        &&& local_valid(after, layout, semantic)
        &&& child_count(before, layout) == child_count(after, layout)
        &&& 0 <= selected < child_count(before, layout)
        &&& old.start <= old.end <= before.len()
        &&& new.start <= new.end <= after.len()
        &&& siblings_same(before, after, layout, selected)
        &&& framed_path(before.subrange(old.start as int, old.end as int),
            after.subrange(new.start as int, new.end as int), child_layout(layout, selected),
            path.subrange(1, path.len() as int), semantic)
    }
}

/// Validity of just the final edited subtree, at its remaining depth budget.
pub(crate) open spec fn edited_path_valid(after: Seq<u8>, layout: Layout, path: Seq<int>,
    budget: nat, semantic: bool) -> bool
    decreases path.len(),
{
    if path.len() == 0 { schema_valid(after, layout, budget, semantic) } else {
        let span = child_span(after, layout, path[0]);
        budget > 0 && edited_path_valid(after.subrange(span.start as int, span.end as int),
            child_layout(layout, path[0]), path.subrange(1, path.len() as int),
            (budget - 1) as nat, semantic)
    }
}

/// Induction lifts one valid edited subtree through framed parent partitions.
///
/// A production splice theorem must separately establish `framed_path` from
/// its checked plan and exact byte-result contract.
pub(crate) proof fn nested_schema_join(before: Seq<u8>, after: Seq<u8>, layout: Layout,
    path: Seq<int>, budget: nat, semantic: bool)
    requires schema_valid(before, layout, budget, semantic),
        framed_path(before, after, layout, path, semantic),
        edited_path_valid(after, layout, path, budget, semantic),
    ensures schema_valid(after, layout, budget, semantic),
    decreases path.len(),
{
    reveal_with_fuel(framed_path, 2);
    reveal_with_fuel(edited_path_valid, 2);
    if path.len() > 0 {
        let selected = path[0];
        let old = child_span(before, layout, selected);
        let new = child_span(after, layout, selected);
        child_from_children(before, layout, 0, selected, (budget - 1) as nat, semantic);
        nested_schema_join(before.subrange(old.start as int, old.end as int),
            after.subrange(new.start as int, new.end as int), child_layout(layout, selected),
            path.subrange(1, path.len() as int), (budget - 1) as nat, semantic);
        parent_join(before, after, layout, selected, budget, semantic);
    }
}

/// A present Option keeps its tag and validates the replacement's exact span.
pub(crate) proof fn option_parent(after: Seq<u8>, layout: Layout, budget: nat, semantic: bool)
    requires budget > 1, descriptor_valid(layout, MAX_DEPTH as nat),
        matches!(layout.head(), LayoutHead::Option { .. }),
        1 <= after.len() <= u32::MAX, after[0] == 1,
        width_spec(child_layout(layout, 0)).is_some()
            ==> after.len() == 1 + width_spec(child_layout(layout, 0)).unwrap(),
        schema_valid(after.subrange(1, after.len() as int), child_layout(layout, 0),
            (budget - 1) as nat, semantic),
    ensures schema_valid(after, layout, budget, semantic),
{
    assert(local_valid(after, layout, semantic));
    assert(child_count(after, layout) == 1);
    assert(child_valid(after, layout, 0, (budget - 1) as nat, semantic));
    reveal_with_fuel(children_valid, 3);
}

/// A uniform shift of a struct's variable ends preserves its local partition.
///
/// The edited interval must retain a nonnegative length. Ends before that
/// interval stay fixed; all later ends move by exactly the payload delta.
pub(crate) proof fn struct_variable_join(before: Seq<u8>, after: Seq<u8>, layout: Layout,
    index: int, delta: int, semantic: bool)
    requires local_valid(before, layout, semantic),
        matches!(layout.head(), LayoutHead::Struct { .. }),
        0 <= after.len() <= u32::MAX, after.len() == before.len() + delta,
        (match layout.head() { LayoutHead::Struct { fixed_bytes, variable, .. } => {
            &&& 0 <= index < variable.len() <= 65535
            &&& forall|i: int| 0 <= i < variable.len() - 1 ==> #[trigger]
                le_word(after, fixed_bytes as int + 4 * i)
                    == le_word(before, fixed_bytes as int + 4 * i) + if i < index { 0int } else { delta }
            &&& crate::struct_layout::payload_start(layout)
                <= crate::struct_layout::variable_span(before, layout, index as u16).start
                <= crate::struct_layout::variable_span(before, layout, index as u16).end as int + delta
        }, _ => false }),
    ensures local_valid(after, layout, semantic),
        child_count(after, layout) == child_count(before, layout),
{
    let (fixed_bytes, fixed, variable) = match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } => (fixed_bytes, fixed, variable),
        _ => { assert(false); return; },
    };
    let p = crate::struct_layout::payload_start(layout);
    let selected = crate::struct_layout::variable_span(before, layout, index as u16);
    assert(p <= after.len()) by {
        if index + 1 == variable.len() {
            assert(selected.end == before.len());
        } else {
            assert(le_word(before, fixed_bytes as int + 4 * index) <= before.len() - p);
            assert(selected.end == p + le_word(before, fixed_bytes as int + 4 * index));
        }
    }
    assert forall|i: int| 0 <= i < variable.len() - 1 implies {
        let end = #[trigger] le_word(after, fixed_bytes as int + 4 * i);
        &&& end <= after.len() - p
        &&& if i == 0 { 0 <= end }
            else { le_word(after, fixed_bytes as int + 4 * (i - 1)) <= end }
    } by {
        let old_end = le_word(before, fixed_bytes as int + 4 * i);
        assert(old_end <= before.len() - p);
        if i < index {
            assert(old_end <= selected.start - p) by {
                // Monotonicity across an arbitrary directory interval is
                // established by the separately decreasing chain lemma.
                directory_order(before, fixed_bytes as int, variable.len() - 1,
                    i, index - 1);
            }
        } else if i == index && i > 0 {
            assert(selected.start == p + le_word(before, fixed_bytes as int + 4 * (i - 1)));
            assert(selected.end == p + old_end);
        }
    }
    assert(crate::struct_layout::struct_shape(after, layout));
}

/// Adjacent monotone ends imply order between any two directory entries.
pub(crate) proof fn directory_order(bytes: Seq<u8>, first: int, count: int, i: int, j: int)
    requires 0 <= i <= j < count,
        forall|k: int| 0 < k < count ==> #[trigger] le_word(bytes, first + 4 * (k - 1))
            <= le_word(bytes, first + 4 * k),
    ensures le_word(bytes, first + 4 * i) <= le_word(bytes, first + 4 * j),
    decreases j - i,
{
    if i < j {
        directory_order(bytes, first, count, i, j - 1);
    }
}

/// Every retained child of a fixed-stride list is checked at its new ordinal.
///
/// `source` maps retained children to their old ordinals. The optional inserted
/// ordinal covers insert/replace/push; None covers remove/pop/truncate/clear.
pub(crate) proof fn fixed_list_parent(before: Seq<u8>, after: Seq<u8>, layout: Layout,
    source: Seq<int>, inserted: Option<int>, input: Seq<u8>, budget: nat, semantic: bool)
    requires schema_valid(before, layout, budget, semantic),
        matches!(layout.head(), LayoutHead::List { .. }),
        width_spec(child_layout(layout, 0)).is_some(),
        4 <= after.len() <= u32::MAX,
        after.len() == 4 + crate::words::le_word(after, 0)
            * width_spec(child_layout(layout, 0)).unwrap(),
        source.len() == crate::words::le_word(after, 0),
        inserted.is_some() ==> 0 <= inserted.unwrap() < source.len()
            && schema_valid(input, child_layout(layout, 0), (budget - 1) as nat, semantic),
        forall|i: int| #![trigger child_span(after, layout, i)] 0 <= i < source.len() ==> {
            let new = child_span(after, layout, i);
            &&& new.start <= new.end <= after.len()
            &&& if inserted == Some(i) {
                after.subrange(new.start as int, new.end as int) == input
            } else {
                let old = child_span(before, layout, source[i]);
                &&& 0 <= source[i] < child_count(before, layout)
                &&& old.start <= old.end <= before.len()
                &&& after.subrange(new.start as int, new.end as int)
                    == before.subrange(old.start as int, old.end as int)
            }
        },
    ensures schema_valid(after, layout, budget, semantic),
{
    assert(local_valid(after, layout, semantic));
    children_all(before, layout, 0, (budget - 1) as nat, semantic);
    assert forall|i: int| 0 <= i < child_count(after, layout)
        implies #[trigger] child_valid(after, layout, i, (budget - 1) as nat, semantic) by {
        if inserted != Some(i) {
            let old = child_span(before, layout, source[i]);
            let new = child_span(after, layout, i);
            assert(child_layout(layout, source[i]) == child_layout(layout, i));
            assert(child_valid(before, layout, source[i], (budget - 1) as nat, semantic));
            subtree_frame(before, after, old, new, child_layout(layout, i),
                (budget - 1) as nat, semantic);
        }
    }
    children_all(after, layout, 0, (budget - 1) as nat, semantic);
}

}
