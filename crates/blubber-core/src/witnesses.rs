//! Concrete, premise-free calls keep the root theorem families non-vacuous.
//!
//! Each call must discharge the production theorem's complete precondition.
//! Inventory fingerprints pin these bodies as well as the theorem contracts.
use super::*;
use crate::{FixedField, KeyDomain, KeyLayout, Leaf, VariableField};

verus! {

pub(super) const BYTE: Layout = Layout::Leaf(Leaf::Unsigned(1));
pub(super) const FIXED_LIST: Layout = Layout::List { element: &BYTE };
pub(super) const INDEXED_LIST: Layout = Layout::List { element: &Layout::Str };
pub(super) const FIXED_MAP: Layout = Layout::Map { key: KeyLayout { width: 1, domain: KeyDomain::Raw }, value: &BYTE };
pub(super) const INDEXED_MAP: Layout = Layout::Map { key: KeyLayout { width: 1, domain: KeyDomain::Raw }, value: &Layout::Str };
pub(super) open spec fn fixed_root() -> Layout { Layout::Struct { fixed_bytes: 1,
    fixed: vstd::array::spec_array_as_slice(&[FixedField { declaration: 0, offset: 0, layout: &BYTE }]), variable: vstd::array::spec_array_as_slice(&[]) } }
pub(super) open spec fn string_root() -> Layout { Layout::Struct { fixed_bytes: 0, fixed: vstd::array::spec_array_as_slice(&[]),
    variable: vstd::array::spec_array_as_slice(&[VariableField { declaration: 0, layout: &Layout::Str }]) } }
pub(super) open spec fn list_root() -> Layout { Layout::Struct { fixed_bytes: 0, fixed: vstd::array::spec_array_as_slice(&[]),
    variable: vstd::array::spec_array_as_slice(&[VariableField { declaration: 0, layout: &FIXED_LIST }]) } }
pub(super) open spec fn map_root() -> Layout { Layout::Struct { fixed_bytes: 0, fixed: vstd::array::spec_array_as_slice(&[]),
    variable: vstd::array::spec_array_as_slice(&[VariableField { declaration: 0, layout: &FIXED_MAP }]) } }

pub(super) open spec fn path() -> InlinePath {
    InlinePath { steps: [Step::Field(0); MAX_DEPTH], len: 0, overflow: false }
}
pub(super) open spec fn field_path() -> InlinePath { InlinePath { len: 1, ..path() } }
pub(super) open spec fn account(root: Seq<u8>) -> Seq<u8> {
    seq![1u8,0,0,0,0,0,0,0, (12 + root.len()) as u8,0,0,0] + root + Seq::new((52 - root.len()) as nat, |i: int| 0u8)
}
pub(super) open spec fn root_trail(layout: Layout, used: u32) -> Trail {
    let n = Node { payload_len: 0, region: Span { start: 12, end: used }, layout, ordinal: 0, run: empty_run() };
    Trail { root: layout, selected: layout, local: false, nodes: [n; MAX_DEPTH], depth: 0 }
}
pub(super) open spec fn replacement(before: Seq<u8>, layout: Layout, input: u32) -> Plan<'static> {
    admission::subtree_plan_for(before, path(), root_trail(layout, words::le_word(before, 8) as u32), input, before.len() as u32)
}
pub(super) open spec fn entries(before: Seq<u8>, layout: Layout, op: ListEdit, input: u32, width: u32) -> Plan<'static> {
    admission::entries_plan_for(before, path(), root_trail(layout, words::le_word(before, 8) as u32), op, input, width, before.len() as u32)
}

/// One indexed entry has an exact terminal end, including an empty payload.
pub(super) proof fn single_map_shape(bytes:Seq<u8>)
    requires 9<=bytes.len()<=u32::MAX,words::le_word(bytes,0)==1,
        words::le_word(bytes,5)==bytes.len()-9,
    ensures crate::map_layout::map_shape(bytes,INDEXED_MAP),
{
    reveal(crate::layout_head::head_spec);
    assert(crate::map_layout::value_width(INDEXED_MAP).is_none());
    assert(crate::map_layout::entry_stride(INDEXED_MAP)==5);
    assert forall|i:int| 0<=i<1 implies
        0<=#[trigger] words::le_word(bytes,4+i*5+1)<=bytes.len()-9 by {assert(i==0);}
}

/// Child-bearing foundations use a real nonempty list and a present Option.
#[verifier::spinoff_prover]
proof fn foundation_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    let bytes = seq![1u8,0,0,0,7];
    let list = FIXED_LIST;
    crate::preservation::subtree_frame(bytes, bytes, Span { start: 4, end: 5 }, Span { start: 4, end: 5 }, BYTE, 2, true);
    crate::preservation::child_from_children(bytes, list, 0, 0, 2, true);
    crate::preservation::children_from_all(bytes, list, 0, 2, true);
    crate::preservation::children_all(bytes, list, 0, 2, true);
    crate::preservation::parent_join(bytes, bytes, list, 0, 3, true);
    reveal_with_fuel(crate::preservation::framed_path, 3);
    reveal_with_fuel(crate::preservation::edited_path_valid, 3);
    crate::preservation::nested_schema_join(bytes, bytes, list, seq![0int], 3, true);
    crate::preservation::option_parent(seq![1u8,7], Layout::Option { value: &BYTE }, 3, true);
    let structure = Layout::Struct { fixed_bytes: 0, fixed: vstd::array::spec_array_as_slice(&[]), variable: vstd::array::spec_array_as_slice(&[
        VariableField { declaration: 0, layout: &Layout::Str }, VariableField { declaration: 1, layout: &Layout::Str }]) };
    crate::preservation::struct_variable_join(seq![1u8,0,0,0,97,98], seq![1u8,0,0,0,97,98], structure, 0, 0, true);
    crate::preservation::directory_order(seq![1u8,0,0,0,2,0,0,0], 0, 2, 0, 1);
    crate::preservation::fixed_list_parent(bytes, bytes, list, seq![0int], None, seq![], 3, true);
}

/// Root replacement and its full ancestor frame have concrete valid inputs.
#[verifier::spinoff_prover]
proof fn subtree_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    let before = account(seq![7u8]);
    let after = before;
    let plan = replacement(before, BYTE, 1);
    root_subtree_preserves(before, after, after, plan, MAX_DEPTH as nat, true);
    fixed_write_preserves(before, after, after, plan);
    subtree_replace_preserves(before, after, after, plan);
    nested_refines(before, after, after, plan, MAX_DEPTH as nat, true);
    nested_frame(before, after, after, plan);
    let text = account(seq![97u8]);
    string_set_preserves(text, text, text, replacement(text, Layout::Str, 1));
}

/// Fixed lists witness insertion and every non-inserting operation on nonempty bytes.
#[verifier::spinoff_prover]
proof fn fixed_list_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    let before = account(seq![1u8,0,0,0,7]);
    let after = account(seq![2u8,0,0,0,7,8]);
    let after = after.take(before.len() as int);
    let plan = entries(before, FIXED_LIST, ListEdit::Insert(1), 1, 1);
    fixed_list_insert_preserves(before, after, after, plan);
    fixed_list_push_preserves(before, after, after, plan);
    fixed_list_root_preserves(before, after, after, plan, MAX_DEPTH as nat, true);
    edited_items_valid(seq![seq![7u8]], seq![8u8], ListEdit::Insert(1), BYTE, (MAX_DEPTH - 1) as nat, true);
    let empty = account(seq![0u8,0,0,0]);
    fixed_list_remove_preserves(before, empty, empty, entries(before, FIXED_LIST, ListEdit::Remove(0), 0, 1));
    fixed_list_truncate_preserves(before, empty, empty, entries(before, FIXED_LIST, ListEdit::Truncate(0), 0, 1));
    fixed_list_clear_preserves(before, empty, empty, entries(before, FIXED_LIST, ListEdit::Clear, 0, 1));
    fixed_list_replace_preserves(before, before, before, entries(before, FIXED_LIST, ListEdit::Replace(0), 1, 1));
}


/// Indexed-list insertion synthesizes a real directory; shrinking removes a child.
#[verifier::spinoff_prover]
proof fn indexed_list_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    let before = account(seq![1u8,0,0,0,97]);
    let after = account(seq![2u8,0,0,0,1,0,0,0,97,98]);
    let p = Plan { indexed: Some(IndexedInsert { index: 1, count: 1, payload: 16, offset: 1,
        table_growth: 4, input_len: 1 }), ..replacement(before, INDEXED_LIST, 10) };
    indexed_list_insert_preserves(before, after, after, p);
    indexed_list_push_preserves(before, after, after, p);
    let empty = account(seq![0u8,0,0,0]);
    let change = IndexedSplice { operation: ListEdit::Remove(0), count: 1, new_count: 0,
        first: 0, last: 1, payload: 16, offset: 0, end_offset: 1, table_shrink: 0, input_len: 0 };
    let p = Plan { splice: Some(change), ..replacement(before, INDEXED_LIST, 4) };
    indexed_list_remove_preserves(before, empty, empty, p);
    indexed_list_truncate_preserves(before, empty, empty,
        Plan { splice: Some(IndexedSplice { operation: ListEdit::Truncate(0), ..change }), ..p });
    indexed_list_clear_preserves(before, empty, empty,
        Plan { splice: Some(IndexedSplice { operation: ListEdit::Clear, ..change }), ..p });
    let p = Plan { splice: Some(IndexedSplice { operation: ListEdit::Replace(0), new_count: 1,
        input_len: 1, ..change }), ..replacement(before, INDEXED_LIST, 5) };
    indexed_list_replace_preserves(before, before, before, p);
}

/// Fixed maps witness a new key, replacement, deletion and clear on a real entry.
#[verifier::spinoff_prover]
proof fn fixed_map_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    let before = account(seq![1u8,0,0,0,1,7]);
    let after = account(seq![2u8,0,0,0,1,7,2,8]);
    assert(crate::search::same_prefix(seq![1u8], seq![2u8], 0));
    assert(crate::search::lex_less(seq![1u8], seq![2u8]));
    let plan = admission::map_insert_plan(before, path(), root_trail(FIXED_MAP, 18), 1, vstd::array::spec_array_as_slice(&[2u8]), 1, 64);
    assert(plan.region.start == 18);
    assert(plan.key@ =~= seq![2u8]);
    assert(after.subrange(18, 19) =~= plan.key@);
    let local = before.subrange(12,18);
    assert(crate::search::keys_canonical(local, FIXED_MAP, 1));
    assert(crate::search::keys_ordered(local, FIXED_MAP, 1));
    assert(crate::search::encoded_key(local, FIXED_MAP, 0) =~= seq![1u8]);
    assert(crate::search::search_found(crate::Search { index: 1, found: false }, local, FIXED_MAP, plan.key@));
    assert(crate::search::search_partition(crate::Search { index: 1, found: false }, local, FIXED_MAP, plan.key@));
    assert(fixed_map::map_checked(before, plan));
    assert(list_checked(before, plan));
    assert(trail_valid(before, FIXED_MAP, path(), plan.trail, 18));
    assert(replacement_checked(before, plan));
    assert(fixed_map::key_written(after, plan));
    fixed_map_insert_preserves(before, after, after, plan);
    let base = entries(before, FIXED_MAP, ListEdit::Replace(0), 2, 2);
    assert(replacement_checked(before, base));
    let c = MapChange { operation: MapEdit::Replace, key_width: 1, value_width: 1, index: 0, found: true };
    let plan = Plan { key: vstd::array::spec_array_as_slice(&[1u8]), map: Some(c), ..base };
    assert(plan.key@ =~= seq![1u8]);
    assert(crate::search::encoded_key(before.subrange(12,18), FIXED_MAP, 0) =~= plan.key@);
    assert(fixed_map::map_checked(before, plan));
    assert(before.subrange(plan.region.start as int, plan.region.start + 1) =~= plan.key@);
    assert(fixed_map::key_written(before, plan));
    fixed_map_replace_preserves(before, before, before, plan);
    let empty = account(seq![0u8,0,0,0]);
    let base = entries(before, FIXED_MAP, ListEdit::Remove(0), 0, 2);
    fixed_map_remove_preserves(before, empty, empty,
        Plan { key: vstd::array::spec_array_as_slice(&[1u8]), map: Some(MapChange { operation: MapEdit::Remove, ..c }), ..base });
    let base = entries(before, FIXED_MAP, ListEdit::Clear, 0, 2);
    fixed_map_clear_preserves(before, empty, empty,
        Plan { map: Some(MapChange { operation: MapEdit::Clear, index: 0, found: false, ..c }), ..base });
}

/// Indexed maps carry stored terminal ends and nonempty UTF-8 values.
#[verifier::spinoff_prover]
proof fn indexed_map_insert_preserves_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    assert(crate::search::same_prefix(seq![1u8], seq![2u8], 0));
    assert(crate::search::lex_less(seq![1u8], seq![2u8]));
    let before = account(seq![1u8,0,0,0,1,1,0,0,0,97]);
    let after = account(seq![2u8,0,0,0,1,1,0,0,0,2,2,0,0,0,97,98]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Insert, key_width: 1, value_width: 1,
        index: 1, found: false }, count: 1, new_count: 2, stride: 5, payload: 21,
        offset: 1, end_offset: 1, input_len: 1, table_delta: 5 };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[2u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 16) };
    assert(p.key@ =~= if matches!(c.change.operation, MapEdit::Insert) { seq![2u8] } else { seq![1u8] });
    single_map_shape(before.subrange(12,22));
    assert(crate::map_layout::map_shape(before.subrange(12,22), INDEXED_MAP));
    assert(crate::search::encoded_key(before.subrange(12,22), INDEXED_MAP, 0) =~= seq![1u8]);
    assert(crate::search::search_found(crate::Search { index: c.change.index, found: c.change.found }, before.subrange(12,22), INDEXED_MAP, p.key@));
    assert(crate::search::search_partition(crate::Search { index: c.change.index, found: c.change.found }, before.subrange(12,22), INDEXED_MAP, p.key@));
    assert(indexed_map::parameters(before, p.region, INDEXED_MAP, c, p.key@));
    assert(indexed_map::checked(before, p));
    assert(list_checked(before, p));
    assert(replacement_checked(before, p));
    assert(indexed_map::directory_done(before, after, p, 2)) by {
        assert(after.subrange(16,17) =~= before.subrange(16,17));
        assert(after.subrange(21,22) =~= p.key@);
    }
    assert(indexed_map::payload_preserved(before, after, p));
    indexed_map_insert_preserves(before, after, after, p);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Replace, index: 0, found: true, ..c.change },
        new_count: 1, offset: 0, end_offset: 1, table_delta: 0, ..c };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[1u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 10) };

    let empty = account(seq![0u8,0,0,0]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Remove, value_width: 0, ..c.change },
        new_count: 0, input_len: 0, table_delta: -5i64, ..c };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[1u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 4) };


}

#[verifier::spinoff_prover]
proof fn indexed_map_replace_preserves_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    assert(crate::search::same_prefix(seq![1u8], seq![2u8], 0));
    assert(crate::search::lex_less(seq![1u8], seq![2u8]));
    let before = account(seq![1u8,0,0,0,1,1,0,0,0,97]);
    let after = account(seq![2u8,0,0,0,1,1,0,0,0,2,2,0,0,0,97,98]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Insert, key_width: 1, value_width: 1,
        index: 1, found: false }, count: 1, new_count: 2, stride: 5, payload: 21,
        offset: 1, end_offset: 1, input_len: 1, table_delta: 5 };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[2u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 16) };

    let c = IndexedMap { change: MapChange { operation: MapEdit::Replace, index: 0, found: true, ..c.change },
        new_count: 1, offset: 0, end_offset: 1, table_delta: 0, ..c };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[1u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 10) };
    assert(p.key@ =~= if matches!(c.change.operation, MapEdit::Insert) { seq![2u8] } else { seq![1u8] });
    single_map_shape(before.subrange(12,22));
    assert(crate::map_layout::map_shape(before.subrange(12,22), INDEXED_MAP));
    assert(crate::search::encoded_key(before.subrange(12,22), INDEXED_MAP, 0) =~= seq![1u8]);
    assert(crate::search::search_found(crate::Search { index: c.change.index, found: c.change.found }, before.subrange(12,22), INDEXED_MAP, p.key@));
    assert(crate::search::search_partition(crate::Search { index: c.change.index, found: c.change.found }, before.subrange(12,22), INDEXED_MAP, p.key@));
    assert(indexed_map::parameters(before, p.region, INDEXED_MAP, c, p.key@));
    assert(indexed_map::checked(before, p));
    assert(list_checked(before, p));
    assert(replacement_checked(before, p));
    indexed_map_replace_preserves(before, before, before, p);
    let empty = account(seq![0u8,0,0,0]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Remove, value_width: 0, ..c.change },
        new_count: 0, input_len: 0, table_delta: -5i64, ..c };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[1u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 4) };


}

#[verifier::spinoff_prover]
proof fn indexed_map_remove_preserves_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    assert(crate::search::same_prefix(seq![1u8], seq![2u8], 0));
    assert(crate::search::lex_less(seq![1u8], seq![2u8]));
    let before = account(seq![1u8,0,0,0,1,1,0,0,0,97]);
    let after = account(seq![2u8,0,0,0,1,1,0,0,0,2,2,0,0,0,97,98]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Insert, key_width: 1, value_width: 1,
        index: 1, found: false }, count: 1, new_count: 2, stride: 5, payload: 21,
        offset: 1, end_offset: 1, input_len: 1, table_delta: 5 };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[2u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 16) };

    let c = IndexedMap { change: MapChange { operation: MapEdit::Replace, index: 0, found: true, ..c.change },
        new_count: 1, offset: 0, end_offset: 1, table_delta: 0, ..c };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[1u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 10) };

    let empty = account(seq![0u8,0,0,0]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Remove, value_width: 0, ..c.change },
        new_count: 0, input_len: 0, table_delta: -5i64, ..c };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[1u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 4) };
    assert(p.key@ =~= if matches!(c.change.operation, MapEdit::Insert) { seq![2u8] } else { seq![1u8] });
    single_map_shape(before.subrange(12,22));
    assert(crate::map_layout::map_shape(before.subrange(12,22), INDEXED_MAP));
    assert(crate::search::encoded_key(before.subrange(12,22), INDEXED_MAP, 0) =~= seq![1u8]);
    assert(crate::search::search_found(crate::Search { index: c.change.index, found: c.change.found }, before.subrange(12,22), INDEXED_MAP, p.key@));
    assert(crate::search::search_partition(crate::Search { index: c.change.index, found: c.change.found }, before.subrange(12,22), INDEXED_MAP, p.key@));
    assert(indexed_map::parameters(before, p.region, INDEXED_MAP, c, p.key@));
    assert(indexed_map::checked(before, p));
    assert(list_checked(before, p));
    assert(replacement_checked(before, p));
    indexed_map_remove_preserves(before, empty, empty, p);

}

#[verifier::spinoff_prover]
proof fn indexed_map_clear_preserves_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    assert(crate::search::same_prefix(seq![1u8], seq![2u8], 0));
    assert(crate::search::lex_less(seq![1u8], seq![2u8]));
    let before = account(seq![1u8,0,0,0,1,1,0,0,0,97]);
    let after = account(seq![2u8,0,0,0,1,1,0,0,0,2,2,0,0,0,97,98]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Insert, key_width: 1, value_width: 1,
        index: 1, found: false }, count: 1, new_count: 2, stride: 5, payload: 21,
        offset: 1, end_offset: 1, input_len: 1, table_delta: 5 };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[2u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 16) };

    let c = IndexedMap { change: MapChange { operation: MapEdit::Replace, index: 0, found: true, ..c.change },
        new_count: 1, offset: 0, end_offset: 1, table_delta: 0, ..c };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[1u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 10) };

    let empty = account(seq![0u8,0,0,0]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Remove, value_width: 0, ..c.change },
        new_count: 0, input_len: 0, table_delta: -5i64, ..c };
    let p = Plan { key: vstd::array::spec_array_as_slice(&[1u8]), variable_map: Some(c), ..replacement(before, INDEXED_MAP, 4) };

    indexed_map_clear_preserves(before, empty, empty, Plan { key: vstd::array::spec_array_as_slice(&[]), variable_map: Some(IndexedMap {
        change: MapChange { operation: MapEdit::Clear, found: false, ..c.change }, ..c }), ..p });
}



/// Generated offsets and absent-key removal have admissible concrete roots.
#[verifier::spinoff_prover]
proof fn simple_admission_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    reveal_with_fuel(admission::prefix_layout, 3);
    reveal_with_fuel(admission::prefix_offset, 3);
    let before = account(seq![7u8]);
    fixed_admission_matches_generic(before, before, before, fixed_root(), field_path(), 13, 0, 1);
    let before = account(seq![1u8,0,0,0,1,7]);
    assert(crate::search::same_prefix(seq![1u8], seq![2u8], 0));
    assert(crate::search::lex_less(seq![1u8], seq![2u8]));
    let plan = Plan { key: vstd::array::spec_array_as_slice(&[2u8]), map: Some(MapChange { operation: MapEdit::Remove,
        key_width: 1, value_width: 1, index: 1, found: false }),
        ..entries(before, FIXED_MAP, ListEdit::Truncate(1), 0, 2) };
    assert(plan.key@ =~= seq![2u8]);
    assert(crate::search::encoded_key(before.subrange(12,18), FIXED_MAP, 0) =~= seq![1u8]);
    missing_fixed_remove_identity(before, plan);
    missing_fixed_remove_preserves(before, before, before, plan, MAX_DEPTH as nat, true);
}









/// A real one-edge string projection inhabits the nested ancestor premises.
#[verifier::spinoff_prover]
proof fn nested_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    let before = account(seq![97u8]);
    let root = string_root();
    let selection = Selection { payload_len: 1, span: Span { start: 0, end: 1 },
        layout: Layout::Str, ordinal: 0, run: empty_run() };
    assert(selection_exact(selection, before.subrange(12,13), root, Step::Field(0)));
    admission::direct_checked(before, root, field_path(), 13, selection);
    let trail = admission::direct_trail(before, root, field_path(), 13, selection);
    admission::subtree_checked(before, field_path(), trail, 1, 64);
    let plan = admission::subtree_plan_for(before, field_path(), trail, 1, 64);
    subtree_replace_preserves(before, before, before, plan);
    string_set_preserves(before, before, before, plan);
    nested_refines(before, before, before, plan, MAX_DEPTH as nat, true);
    nested_frame(before, before, before, plan);
}

/// Fixed header geometry shared by the concrete insertion witnesses.
pub(super) broadcast proof fn fixed_witness_header(bytes: Seq<u8>)
    requires bytes.len() == 8, words::le_word(bytes, 0) == 2,
    ensures #[trigger] crate::map_layout::header_shape(bytes, FIXED_MAP),
{
    reveal(crate::layout_head::head_spec);
    reveal(crate::map_layout::header_shape);
    assert(crate::map_layout::value_width(FIXED_MAP) == Some(1u32));
    assert(crate::map_layout::entry_stride(FIXED_MAP) == 2);
}


}
