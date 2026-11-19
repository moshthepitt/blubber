//! Erased composition induction for the complete library encoder algebra.
//!
//! A trace records the byte relations established by the library kernels.
//! Primitive domains come from the leaf/string proofs; child traces come from
//! the same induction. No trace contains a child `schema_valid` premise.
//! Typed projection and metadata glue transport these relations to application
//! types. Field selection and true handwritten marker contracts stay trusted.

use crate::encoding_list as list;
use crate::encoding_map as map;
use crate::encoding_product::{self as product, Item};
use crate::encoding_struct as structure;
use crate::{Layout, LayoutHead, MAX_DEPTH};
use vstd::prelude::*;

verus! {

/// Erased constructor trace; neither runtime allocation nor an app proof input.
pub ghost struct Graph {
    /// Descriptor selected by the projected type.
    pub layout: Layout,
    /// Exact bytes established by that constructor's writer contract.
    pub bytes: Seq<u8>,
    /// Destination history used only by product/list writer relations.
    pub before: Seq<u8>,
    /// Projected child constructor traces in declaration order.
    pub children: Seq<Graph>,
    /// Canonical keys for map constructors; unused by other constructors.
    pub keys: Seq<Seq<u8>>,
}

/// Field/list views of the same projected child types and bytes.
pub open spec fn items(graph: Graph) -> Seq<Item> {
    Seq::new(graph.children.len(), |i: int| Item { bytes: graph.children[i].bytes,
        layout: graph.children[i].layout,
        fixed: crate::layout::width_spec(graph.children[i].layout).is_some() })
}

/// Map views of the same projected child types and canonical key bytes.
pub open spec fn entries(graph: Graph) -> Seq<map::Entry> {
    Seq::new(graph.children.len(), |i: int| map::Entry { key: graph.keys[i], value: graph.children[i].bytes })
}

/// Constructor equations supplied by successful measured library encoding.
///
/// Products include native fixed packing and balanced internal grouping. Empty
/// collections and absent options still require their complete descriptor to
/// fit the configured budget. A trace derives validity, never assumes it.
pub open spec fn trace(graph: Graph, budget: nat) -> bool
    decreases budget,
{
    &&& 0 < budget <= MAX_DEPTH
    &&& graph.bytes.len() <= u32::MAX
    &&& crate::layout::descriptor_valid(graph.layout, budget)
    &&& (forall|i: int| 0 <= i < graph.children.len() ==>
        trace(#[trigger] graph.children[i], (budget - 1) as nat))
    &&& match graph.layout.head() {
        LayoutHead::Leaf(_) => graph.children.len() == 0
            && crate::leaves::fixed_value_valid(graph.bytes, graph.layout, budget),
        LayoutHead::Str => graph.children.len() == 0 && crate::utf8::utf8_valid(graph.bytes, 0),
        LayoutHead::Option { value } => budget > 1 && (if graph.children.len() == 0 {
            graph.bytes == seq![0u8]
        } else { graph.children.len() == 1 && graph.children[0].layout == value
            && graph.bytes == seq![1u8] + graph.children[0].bytes }),
        LayoutHead::Struct { .. } => budget > 1 && structure::agrees(items(graph), graph.layout)
            && graph.before.len() == structure::size(items(graph)) <= u32::MAX
            && graph.bytes == product::paint_all(graph.before, structure::initial(items(graph)), items(graph)),
        LayoutHead::List { element } => budget > 1 && graph.children.len() <= u32::MAX
            && (forall|i: int| 0 <= i < graph.children.len() ==> (#[trigger] graph.children[i]).layout == element)
            && list::uniform(items(graph), crate::layout::width_spec(element))
            && graph.before.len() == list::size(items(graph), crate::layout::width_spec(element).is_none()) <= u32::MAX
            && graph.bytes == list::output(graph.before, items(graph), crate::layout::width_spec(element)),
        LayoutHead::Map { key, value } => budget > 1 && graph.children.len() <= u32::MAX
            && graph.keys.len() == graph.children.len()
            && (forall|i: int| 0 <= i < graph.children.len() ==> (#[trigger] graph.children[i]).layout == value)
            && map::uniform(entries(graph), key.width, crate::layout::width_spec(value))
            && crate::encoding_map_valid::keys(entries(graph), key)
            && graph.bytes.len() == map::size(entries(graph), key.width, crate::layout::width_spec(value).is_none())
            && crate::words::le_word(graph.bytes, 0) == graph.children.len()
            && map::written(graph.bytes, entries(graph), key.width, crate::layout::width_spec(value), graph.children.len() as int),
    }
}

/// Extracts one recursive constructor premise without opening child bodies.
pub proof fn trace_child(graph: Graph, budget: nat, index: int)
    requires trace(graph, budget), 0 <= index < graph.children.len(),
    ensures trace(graph.children[index], (budget - 1) as nat),
{
    reveal_with_fuel(trace, 1);
}

/// Product equations reconstruct complete struct validity without a source tree.
pub proof fn struct_valid(items: Seq<Item>, layout: Layout, before: Seq<u8>, after: Seq<u8>, budget: nat)
    requires structure::agrees(items, layout), 1 < budget <= MAX_DEPTH,
        crate::layout::descriptor_valid(layout, budget),
        before.len() == structure::size(items) <= u32::MAX, after.len() <= u32::MAX,
        after == product::paint_all(before, structure::initial(items), items),
        forall|i: int| 0 <= i < items.len() ==>
            crate::validation::schema_valid(items[i].bytes, items[i].layout, (budget - 1) as nat, true),
    ensures crate::validation::schema_valid(after, layout, budget, true),
{
    hide(crate::validation::schema_valid); hide(crate::validation::child_valid);
    hide(crate::validation::children_valid); hide(crate::validation::local_valid);
    hide(crate::layout::descriptor_valid); hide(crate::layout::width_spec);
    hide(structure::agrees); hide(product::paint_all);
    // Representable output and exact writes also bound its original loan.
    structure::initial_fits(items);
    structure::packed_children(before, structure::initial(items), items);
    assert(before.len() == after.len());
    structure::initial_fits(items);
    structure::packed_directory(before, structure::initial(items), items);
    crate::encoding_metadata::descriptor_budget(layout, budget, MAX_DEPTH as nat);
    structure::local_struct(items, layout, after);
    structure::struct_children(items, layout, after, (budget - 1) as nat);
    reveal_with_fuel(crate::validation::schema_valid, 1);
    reveal_with_fuel(crate::validation::children_valid, 1);
}

/// Child traversal depends only on the independent descriptor head.
pub proof fn children_head(bytes: Seq<u8>, left: Layout, right: Layout, next: int, budget: nat)
    requires left.head() == right.head(),
        crate::validation::children_valid(bytes, left, next, budget, true),
    ensures crate::validation::children_valid(bytes, right, next, budget, true),
    decreases crate::validation::child_count(bytes, left) - next,
{
    hide(crate::validation::schema_valid);
    reveal_with_fuel(crate::validation::children_valid, 2);
    if next < crate::validation::child_count(bytes, left) {
        assert(crate::validation::child_span(bytes, left, next) == crate::validation::child_span(bytes, right, next));
        assert(crate::validation::child_layout(left, next) == crate::validation::child_layout(right, next));
        crate::validation::child_definition(bytes, left, next, budget, true);
        crate::validation::child_definition(bytes, right, next, budget, true);
        children_head(bytes, left, right, next + 1, budget);
    }
}

/// Canonical and strict key relations depend only on the map head.
pub proof fn key_relations_head(bytes: Seq<u8>, left: Layout, right: Layout, count: int)
    requires left.head() == right.head(),
    ensures crate::search::keys_canonical(bytes, left, count) == crate::search::keys_canonical(bytes, right, count),
        crate::search::keys_ordered(bytes, left, count) == crate::search::keys_ordered(bytes, right, count),
{
    hide(crate::words::le_word); hide(crate::search::lex_less);
    assert(crate::search::bool_keys(left) == crate::search::bool_keys(right));
    assert forall|i: int| #[trigger] crate::search::encoded_key(bytes, left, i) == crate::search::encoded_key(bytes, right, i) by { }
    if crate::search::keys_canonical(bytes, left, count) {
        assert forall|i: int| 0 <= i < count implies !crate::search::bool_keys(right)
            || (#[trigger] crate::search::encoded_key(bytes, right, i))[0] <= 1 by {
            assert(crate::search::encoded_key(bytes, left, i) == crate::search::encoded_key(bytes, right, i));
        }
    }
    if crate::search::keys_canonical(bytes, right, count) {
        assert forall|i: int| 0 <= i < count implies !crate::search::bool_keys(left)
            || (#[trigger] crate::search::encoded_key(bytes, left, i))[0] <= 1 by {
            assert(crate::search::encoded_key(bytes, left, i) == crate::search::encoded_key(bytes, right, i));
        }
    }
    if crate::search::keys_ordered(bytes, left, count) {
        assert forall|i: int, j: int| 0 <= i < j < count implies crate::search::lex_less(
            #[trigger] crate::search::encoded_key(bytes, right, i), #[trigger] crate::search::encoded_key(bytes, right, j)) by {
            assert(crate::search::encoded_key(bytes, left, i) == crate::search::encoded_key(bytes, right, i));
            assert(crate::search::encoded_key(bytes, left, j) == crate::search::encoded_key(bytes, right, j));
        }
    }
    if crate::search::keys_ordered(bytes, right, count) {
        assert forall|i: int, j: int| 0 <= i < j < count implies crate::search::lex_less(
            #[trigger] crate::search::encoded_key(bytes, left, i), #[trigger] crate::search::encoded_key(bytes, left, j)) by {
            assert(crate::search::encoded_key(bytes, left, i) == crate::search::encoded_key(bytes, right, i));
            assert(crate::search::encoded_key(bytes, left, j) == crate::search::encoded_key(bytes, right, j));
        }
    }
}

/// Local independent validity is unchanged by an equal descriptor head.
pub proof fn local_head(bytes: Seq<u8>, left: Layout, right: Layout)
    requires left.head() == right.head(),
    ensures crate::validation::local_valid(bytes, left, true) == crate::validation::local_valid(bytes, right, true),
{
    hide(crate::words::le_word); hide(crate::search::lex_less);
    hide(crate::validation::schema_valid); hide(crate::validation::children_valid);
    hide(crate::layout::descriptor_valid); hide(crate::leaves::fixed_value_valid);
    assert(crate::layout::width_spec(left) == crate::layout::width_spec(right));
    assert(crate::layout::descriptor_valid(left, MAX_DEPTH as nat) == crate::layout::descriptor_valid(right, MAX_DEPTH as nat)) by {
        reveal_with_fuel(crate::layout::descriptor_valid, 1);
    }
    assert(crate::leaves::fixed_value_valid(bytes, left, MAX_DEPTH as nat) == crate::leaves::fixed_value_valid(bytes, right, MAX_DEPTH as nat)) by {
        reveal_with_fuel(crate::leaves::fixed_value_valid, 1);
    }
    assert forall|i: int| #[trigger] crate::search::encoded_key(bytes, left, i) == crate::search::encoded_key(bytes, right, i) by { }
    assert(crate::struct_layout::struct_shape(bytes, left) == crate::struct_layout::struct_shape(bytes, right));
    assert(crate::list_layout::list_shape(bytes, left) == crate::list_layout::list_shape(bytes, right));
    assert(crate::map_layout::map_shape(bytes, left) == crate::map_layout::map_shape(bytes, right));
    assert(crate::leaves::option_shape(bytes, left) == crate::leaves::option_shape(bytes, right));
    if matches!(left.head(), LayoutHead::Map { .. }) { key_relations_head(bytes, left, right, crate::words::le_word(bytes, 0)); }
}

/// Flat and recursive metadata with the same head have the same validity.
pub proof fn validity_head(bytes: Seq<u8>, left: Layout, right: Layout, budget: nat)
    requires left.head() == right.head(), crate::validation::schema_valid(bytes, left, budget, true),
    ensures crate::validation::schema_valid(bytes, right, budget, true),
{
    hide(crate::validation::children_valid); hide(crate::validation::child_valid);
    hide(crate::validation::local_valid);
    reveal_with_fuel(crate::validation::schema_valid, 4);
    reveal_with_fuel(crate::validation::children_valid, 1);
    children_head(bytes, left, right, 0, (budget - 1) as nat);
    local_head(bytes, left, right);
    reveal_with_fuel(crate::validation::children_valid, 1);
}

/// All primitive/option/product/list/map constructor graphs are schema-valid.
///
/// Induction ranges over arbitrary graph shapes, implementing child types and
/// representable counts at either configured depth. It starts from the byte
/// equations proved by the actual library writers, without a field-mapping
/// claim or per-application verification obligation.
pub proof fn encoder_graph_valid(graph: Graph, budget: nat)
    requires trace(graph, budget),
    ensures crate::validation::schema_valid(graph.bytes, graph.layout, budget, true),
    decreases budget,
{
    hide(crate::validation::schema_valid); hide(crate::validation::child_valid);
    hide(crate::validation::children_valid); hide(crate::validation::local_valid);
    hide(crate::layout::descriptor_valid); hide(crate::layout::width_spec);
    hide(crate::utf8::utf8_valid); hide(crate::leaves::fixed_value_valid);
    hide(product::paint_all); hide(list::output); hide(map::written);
    reveal_with_fuel(trace, 2);
    assert forall|i: int| 0 <= i < graph.children.len() implies
        #[trigger] crate::validation::schema_valid(graph.children[i].bytes, graph.children[i].layout, (budget - 1) as nat, true) by {
        trace_child(graph, budget, i);
        encoder_graph_valid(graph.children[i], (budget - 1) as nat);
    }
    crate::encoding_metadata::descriptor_budget(graph.layout, budget, MAX_DEPTH as nat);
    match graph.layout.head() {
        LayoutHead::Leaf(_) | LayoutHead::Str => {
            reveal(crate::validation::local_valid);
            reveal(crate::leaves::fixed_value_valid); reveal(crate::layout::width_spec);
            reveal_with_fuel(crate::validation::children_valid, 1);
            reveal_with_fuel(crate::validation::schema_valid, 1);
            assert(crate::validation::schema_valid(graph.bytes, graph.layout, budget, true));
        },
        LayoutHead::Option { value } => {
            crate::encoding_metadata::descriptor_budget(graph.layout, budget, MAX_DEPTH as nat);
            if graph.children.len() == 0 {
                assert(crate::validation::child_count(graph.bytes, graph.layout) == 0);
            } else {
                assert(graph.bytes.subrange(1, graph.bytes.len() as int) =~= graph.children[0].bytes);
                crate::validation::child_definition(graph.bytes, graph.layout, 0, (budget - 1) as nat, true);
            }
            reveal(crate::validation::local_valid);
            reveal(crate::leaves::fixed_value_valid);
            reveal(crate::layout::width_spec);
            reveal_with_fuel(crate::validation::children_valid, 2);
            reveal_with_fuel(crate::validation::schema_valid, 1);
            assert(crate::validation::schema_valid(graph.bytes, graph.layout, budget, true));
        },
        LayoutHead::Struct { .. } => {
            struct_valid(items(graph), graph.layout, graph.before, graph.bytes, budget);
            assert(crate::validation::schema_valid(graph.bytes, graph.layout, budget, true));
        },
        LayoutHead::List { element } => {
            reveal(crate::layout::descriptor_valid);
            assert(crate::encoding_list_valid::child_contract(items(graph), element, (budget - 1) as nat));
            crate::encoding_list_valid::complete(items(graph), element, crate::layout::width_spec(element), graph.before, graph.bytes, budget);
            reveal(crate::layout_head::head_spec);
            validity_head(graph.bytes, Layout::List { element: &element }, graph.layout, budget);
            assert(crate::validation::schema_valid(graph.bytes, graph.layout, budget, true));
        },
        LayoutHead::Map { key, value } => {
            reveal(crate::layout_head::head_spec);
            reveal_with_fuel(crate::layout::descriptor_valid, 1);
            crate::encoding_map_valid::complete(entries(graph), key, value, graph.bytes, crate::layout::width_spec(value), budget);
            validity_head(graph.bytes, Layout::Map { key, value: &value }, graph.layout, budget);
            assert(crate::validation::schema_valid(graph.bytes, graph.layout, budget, true));
        },
    }
}

/// Complete graph writes preserve every byte outside the exact source loan.
pub proof fn encoder_graph_frame(graph: Graph, budget: nat, before: Seq<u8>, after: Seq<u8>, start: int)
    requires trace(graph, budget), 0 <= start <= before.len(),
        graph.bytes.len() <= before.len() - start,
        after == product::splice(before, start, graph.bytes),
    ensures after.len() == before.len(),
        after.subrange(start, start + graph.bytes.len()) == graph.bytes,
        crate::validation::schema_valid(after.subrange(start, start + graph.bytes.len()), graph.layout, budget, true),
        forall|i: int| 0 <= i < before.len() && !(start <= i < start + graph.bytes.len())
            ==> #[trigger] after[i] == before[i],
{
    encoder_graph_valid(graph, budget);
    assert(after.subrange(start, start + graph.bytes.len()) =~= graph.bytes);
    assert forall|i: int| 0 <= i < before.len() && !(start <= i < start + graph.bytes.len())
        implies #[trigger] after[i] == before[i] by { }
}
/// A real map encode supplies all equations of the complete graph induction.
pub(crate) fn graph_witness()
    ensures true,
{
    use crate::encoding_map::Entries;
    use crate::encoding_product::Source;
    proof {
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid, 3);
        reveal_with_fuel(map::extent, 3);
    }
    let source = crate::encoding_map_fixed_inputs::FixedInputs { values: &[([0], [1]), ([255], [3])] };
    let first = source.value(0); let second = source.value(1);
    proof { reveal_with_fuel(map::fold, 4); }
    let measured = map::measure(&source, 1, false, Some(1));
    assert(measured == Ok(8));
    let mut bytes = [19u8; 8];
    map::encode(&source, &mut bytes, 1, Some(1), false);
    proof {
        crate::encoding_leaf::encoder_leaf_valid(&first, first.bytes(), 1);
        crate::encoding_leaf::encoder_leaf_valid(&second, second.bytes(), 1);
        let a = Graph { layout: Layout::Leaf(crate::Leaf::Raw(1)), bytes: first.bytes(), before: Seq::empty(), children: Seq::empty(), keys: Seq::empty() };
        let b = Graph { layout: Layout::Leaf(crate::Leaf::Raw(1)), bytes: second.bytes(), before: Seq::empty(), children: Seq::empty(), keys: Seq::empty() };
        reveal(crate::validation::schema_valid); reveal(crate::validation::local_valid);
        assert(trace(a, 1)); assert(trace(b, 1));
        let graph = Graph { layout: Layout::Map { key: crate::KeyLayout { width: 1, domain: crate::KeyDomain::Raw },
            value: &Layout::Leaf(crate::Leaf::Raw(1)) }, bytes: bytes@, before: Seq::empty(),
            children: seq![a, b], keys: Seq::new(2, |i:int| source.entries()[i].key) };
        assert forall|i: int| 0 <= i < 2 implies entries(graph)[i] == source.entries()[i] by { if i == 0 { } else { assert(i == 1); } }
        assert(entries(graph) =~= source.entries());
        assert(crate::search::lex_witness(graph.keys[0], graph.keys[1], 0));
        assert(crate::search::lex_less(graph.keys[0], graph.keys[1]));
        assert(trace(graph, 2));
        encoder_graph_valid(graph, 2);
    }
}

/// A real primitive loan exercises independent validity and both byte frames.
pub(crate) fn graph_frame_witness()
    ensures true,
{
    use crate::encoding::Encoder;
    let source = crate::encoding_leaf::boolean(&[1]);
    let mut bytes = [19u8; 3];
    let ghost before = bytes@;
    crate::encoding::encode_frame(&source, &mut bytes, 1);
    proof {
        crate::encoding_leaf::encoder_leaf_valid(&source, source.bytes(), 1);
        let graph = Graph { layout: Layout::Leaf(crate::Leaf::Bool), bytes: source.bytes(), before: Seq::empty(),
            children: Seq::empty(), keys: Seq::empty() };
        reveal(crate::layout_head::head_spec);
        reveal(crate::validation::schema_valid); reveal(crate::validation::local_valid);
        assert(trace(graph, 1));
        assert(bytes@ =~= product::splice(before, 1, source.bytes()));
        encoder_graph_frame(graph, 1, before, bytes@, 1);
    }
    assert(bytes@ == seq![19u8, 1u8, 19u8]);
}

}
