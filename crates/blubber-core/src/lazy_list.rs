//! Empty-list subtree refinement without source-directory premises.
use super::*;
use vstd::prelude::*;

verus! {

/// The generic subtree publication implements both clear and truncate to zero.
#[verifier::spinoff_prover]
proof fn lazy_clear_refines(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), subtree_plan(plan),
        plan.replacement_len == 4, plan.required <= before.len(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        crate::layout::descriptor_valid(plan.trail.selected, MAX_DEPTH as nat),
        encoded.subrange(plan.region.start as int, plan.region.start + 4) == seq![0u8,0,0,0],
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        after.subrange(plan.region.start as int, plan.region.start + 4) == seq![0u8,0,0,0],
        words::le_word(after, 8) == plan.new_used,
        forall|p: int| 0 <= p < after.len() && (p < 8 || plan.required <= p) ==> after[p] == before[p],
{
    reveal(subtree_plan);
    crate::patch_pass::cleared_list_valid(plan.trail.selected);
    assert(after.subrange(plan.region.start as int, plan.region.start + 4) =~= seq![0u8,0,0,0]);
    // An empty list has no active children at any positive inherited budget.
    reveal_with_fuel(validation::schema_valid, 1);
    reveal_with_fuel(validation::children_valid, 1);
    subtree_replace_preserves(before, encoded, after, plan);
}

/// The witness discards a nonempty list and shrinks its used extent.
proof fn lazy_clear_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    let layout = witnesses::INDEXED_LIST;
    let before = witnesses::account(seq![1u8,0,0,0,97]);
    let after = witnesses::account(seq![0u8,0,0,0]);
    let plan = witnesses::replacement(before, layout, 4);
    assert(plan.region.start == 12);
    assert(after.subrange(12,16) =~= seq![0u8,0,0,0]);
    lazy_clear_refines(before, after, after, plan);
    assert(plan.new_used < plan.old_used);
}

}
