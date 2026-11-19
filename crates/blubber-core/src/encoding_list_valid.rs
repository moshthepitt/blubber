//! Independent validity and measurement observations for generic list writers.

use crate::encoding_list::{self as list, Elements};
use crate::encoding_list_fixed_inputs::FixedInputs;
use crate::encoding_list_text_inputs::TextInputs;
use crate::encoding_product::Source;
use crate::encoding_product::{self as product, Item};
use crate::{Layout, MAX_DEPTH};
use vstd::prelude::*;

verus! {

/// Every child uses the selected descriptor and satisfies its own library theorem.
pub open spec fn child_contract(items: Seq<Item>, child: Layout, budget: nat) -> bool {
    crate::layout::descriptor_valid(child, budget)
        && forall|i: int| 0 <= i < items.len() ==> items[i].layout == child
            && crate::validation::schema_valid(items[i].bytes, child, budget, true)
}

/// Count, exact child intervals and every nonterminal cumulative end are written.
pub proof fn packed(items: Seq<Item>, width: Option<u32>, before: Seq<u8>)
    requires list::uniform(items, width), items.len() <= u32::MAX,
        before.len() == list::size(items, width.is_none()) <= u32::MAX,
    ensures list::output(before, items, width).len() == before.len(),
        crate::words::le_word(list::output(before, items, width), 0) == items.len(),
        forall|i: int| 0 <= i < items.len() ==>
            crate::encoding_struct::child_bytes(list::output(before, items, width),
                list::initial(items, width), items, i) == items[i].bytes,
        width.is_none() ==> forall|i: int| 0 <= i < items.len() - 1 ==>
            #[trigger] crate::words::le_word(list::output(before, items, width), 4 + 4*i)
                == product::extent(items.take(i + 1)),
{
    let counted = product::splice(before, 0, product::word(items.len() as int));
    let cursor = list::initial(items, width);
    list::initial_fits(items, width);
    crate::encoding_struct::packed_children(counted, cursor, items);
    crate::encoding_struct::packed_directory(counted, cursor, items);
    crate::encoding_struct::consumed_frame(counted, cursor, items, 0, 4);
    crate::encoding_struct::encoded_word(items.len() as u32);
    assert(counted.subrange(0, 4) =~= product::word(items.len() as int));
    crate::words::word_subrange(counted, 0, 4, 0);
    crate::words::word_subrange(list::output(before, items, width), 0, 4, 0);
    if width.is_none() {
        assert forall|i: int| 0 <= i < items.len() - 1 implies
            #[trigger] crate::words::le_word(list::output(before, items, width), 4 + 4*i)
                == product::extent(items.take(i + 1)) by {
            assert(list::uniform(items.take(i), width));
            list::uniform_sums(items.take(i), width);
            list::extent_prefix(items, i);
            assert(crate::words::le_word(list::output(before, items, width), 4 + 4*i)
                == product::extent(items.take(i)) + items[i].bytes.len());
        }
    }
}

/// Independent list partitions select the exact intervals written by the driver.
pub proof fn regions(items: Seq<Item>, child: Layout, width: Option<u32>, after: Seq<u8>)
    requires list::uniform(items, width), items.len() <= u32::MAX,
        crate::layout::width_spec(child) == width,
        after.len() == list::size(items, width.is_none()) <= u32::MAX,
        crate::words::le_word(after, 0) == items.len(),
        width.is_none() ==> forall|i: int| 0 <= i < items.len() - 1 ==>
            #[trigger] crate::words::le_word(after, 4 + 4*i) == product::extent(items.take(i + 1)),
    ensures crate::list_layout::list_shape(after, Layout::List { element: &child }),
        forall|i: int| 0 <= i < items.len() ==>
            after.subrange(crate::list_layout::element_span(after, Layout::List { element: &child }, i as u32).start as int,
                crate::list_layout::element_span(after, Layout::List { element: &child }, i as u32).end as int)
                == crate::encoding_struct::child_bytes(after, list::initial(items, width), items, i),
{
    reveal(crate::layout_head::head_spec);
    let layout = Layout::List { element: &child };
    if let Some(stride) = width {
        list::uniform_extent(items, stride);
        assert(after.len() == 4 + items.len() * stride);
    } else {
        assert(crate::list_layout::payload_start(after, layout)
            == 4 + list::directory(items.len() as int, true));
        assert forall|i: int| 0 <= i < items.len() - 1 implies {
            let end = #[trigger] crate::words::le_word(after, 4 + 4*i);
            end <= after.len() - crate::list_layout::payload_start(after, layout)
                && (i == 0 || crate::words::le_word(after, 4 + 4*(i - 1)) <= end)
        } by {
            list::extent_prefix(items, i + 1);
            if i > 0 { list::extent_prefix(items, i); }
        }
        if items.len() == 0 {
            reveal_with_fuel(product::extent, 1);
            assert(after.len() == 4);
        }
    }
    assert forall|i: int| 0 <= i < items.len() implies
        after.subrange(crate::list_layout::element_span(after, layout, i as u32).start as int,
            crate::list_layout::element_span(after, layout, i as u32).end as int)
            == crate::encoding_struct::child_bytes(after, list::initial(items, width), items, i) by {
        assert(list::uniform(items.take(i), width));
        list::uniform_sums(items.take(i), width);
        list::extent_prefix(items, i);
        list::extent_prefix(items, i + 1);
        if let Some(stride) = width {
            list::uniform_extent(items.take(i), stride);
            assert(items.take(i).len() == i);
            assert((i + 1)*stride == i*stride + stride) by(nonlinear_arith);
        } else {
            if i == 0 { assert(items.take(0) =~= Seq::<Item>::empty()); }
            else { assert(crate::words::le_word(after, 4 + 4*(i - 1)) == product::extent(items.take(i))); }
            if i + 1 == items.len() {
                assert(items.take(i + 1) =~= items);
            } else {
                assert(crate::words::le_word(after, 4 + 4*i) == product::extent(items.take(i + 1)));
            }
        }
    }
}

/// Exact list output implies complete validity at the configured child depth.
pub proof fn complete(items: Seq<Item>, child: Layout, width: Option<u32>,
    before: Seq<u8>, after: Seq<u8>, budget: nat)
    requires list::uniform(items, width), items.len() <= u32::MAX,
        crate::layout::width_spec(child) == width,
        before.len() == list::size(items, width.is_none()) <= u32::MAX,
        after == list::output(before, items, width),
        1 < budget <= MAX_DEPTH, child_contract(items, child, (budget - 1) as nat),
    ensures crate::validation::schema_valid(after, Layout::List { element: &child }, budget, true),
{
    hide(crate::validation::schema_valid); hide(crate::validation::child_valid);
    hide(crate::validation::children_valid); hide(crate::validation::local_valid);
    hide(crate::layout::descriptor_valid);
    packed(items, width, before);
    regions(items, child, width, after);
    reveal(crate::layout_head::head_spec);
    let layout = Layout::List { element: &child };
    reveal(child_contract);
    reveal(crate::layout::descriptor_valid);
    assert(crate::layout::descriptor_valid(layout, budget));
    crate::encoding_metadata::descriptor_budget(child, (budget - 1) as nat, (MAX_DEPTH - 1) as nat);
    assert(crate::layout::descriptor_valid(layout, MAX_DEPTH as nat));
    assert forall|i: int| 0 <= i < crate::validation::child_count(after, layout) implies
        #[trigger] crate::validation::child_valid(after, layout, i, (budget - 1) as nat, true) by {
        assert(0 <= i < items.len());
        assert(crate::encoding_struct::child_bytes(after, list::initial(items, width), items, i) == items[i].bytes);
        assert(after.subrange(crate::list_layout::element_span(after, layout, i as u32).start as int,
            crate::list_layout::element_span(after, layout, i as u32).end as int) == items[i].bytes);
        crate::validation::child_definition(after, layout, i, (budget - 1) as nat, true);
        assert(crate::validation::child_layout(layout, i) == child);
        assert(crate::validation::child_span(after, layout, i) == crate::list_layout::element_span(after, layout, i as u32));
        assert(crate::validation::schema_valid(items[i].bytes, child, (budget - 1) as nat, true));
    }
    crate::preservation::children_from_all(after, layout, 0, (budget - 1) as nat, true);
    reveal(crate::validation::local_valid);
    assert(crate::validation::local_valid(after, layout, true));
    assert(crate::validation::children_valid(after, layout, 0, (budget - 1) as nat, true));
    reveal_with_fuel(crate::validation::schema_valid, 2);
    reveal_with_fuel(crate::validation::children_valid, 1);
    assert(crate::validation::schema_valid(after, layout, budget, true));
    assert(layout == (Layout::List { element: &child }));
}

/// Equal decoded words have equal canonical bytes at their exact intervals.
pub proof fn word_bytes(bytes: Seq<u8>, at: int, value: u32)
    requires 0 <= at, at + 4 <= bytes.len(), crate::words::le_word(bytes, at) == value,
    ensures bytes.subrange(at, at + 4) == product::word(value as int),
{
    crate::words::word_roundtrip(bytes, at);
    let a = bytes[at]; let b = bytes[at + 1]; let c = bytes[at + 2]; let d = bytes[at + 3];
    assert(a == value as u8 && b == (value / 256) as u8
        && c == (value / 65536) as u8 && d == (value / 16777216) as u8) by(bit_vector)
        requires value as int == a as int + 256*b as int + 65536*c as int + 16777216*d as int;
    assert(bytes.subrange(at, at + 4) =~= product::word(value as int));
}

/// Every payload byte belongs to one element, including around empty children.
pub proof fn payload_index(items: Seq<Item>, offset: int) -> (index: int)
    requires 0 <= offset < product::extent(items),
    ensures 0 <= index < items.len(),
        product::extent(items.take(index)) <= offset
            < product::extent(items.take(index)) + items[index].bytes.len(),
    decreases items.len(),
{
    reveal_with_fuel(product::extent, 2);
    assert(items.len() > 0);
    if offset < items[0].bytes.len() {
        assert(items.take(0) =~= Seq::<Item>::empty());
        0
    } else {
        let index = payload_index(items.drop_first(), offset - items[0].bytes.len());
        assert(items.take(index + 1) =~= seq![items[0]] + items.drop_first().take(index));
        list::extent_concat(seq![items[0]], items.drop_first().take(index));
        index + 1
    }
}

/// All measured bytes are written, independently of destination history.
pub proof fn history(items: Seq<Item>, width: Option<u32>, left: Seq<u8>, right: Seq<u8>)
    requires list::uniform(items, width), items.len() <= u32::MAX,
        left.len() == list::size(items, width.is_none()) <= u32::MAX,
        right.len() == left.len(),
    ensures list::output(left, items, width) == list::output(right, items, width),
{
    hide(product::paint_all); hide(list::output); hide(product::extent);
    packed(items, width, left); packed(items, width, right);
    let a = list::output(left, items, width);
    let b = list::output(right, items, width);
    word_bytes(a, 0, items.len() as u32); word_bytes(b, 0, items.len() as u32);
    let payload = 4 + list::directory(items.len() as int, width.is_none());
    assert forall|i: int| 0 <= i < a.len() implies a[i] == b[i] by {
        if i < 4 {
            assert(a.subrange(0, 4)[i] == b.subrange(0, 4)[i]);
        } else if i < payload {
            let index = (i - 4) / 4;
            assert(0 <= index < items.len() - 1);
            list::extent_prefix(items, index + 1);
            let end = product::extent(items.take(index + 1)) as u32;
            word_bytes(a, 4 + 4*index, end); word_bytes(b, 4 + 4*index, end);
            assert(4 + 4*index <= i < 8 + 4*index);
            assert(a.subrange(4 + 4*index, 8 + 4*index)[i - 4 - 4*index]
                == b.subrange(4 + 4*index, 8 + 4*index)[i - 4 - 4*index]);
        } else {
            let index = payload_index(items, i - payload);
            assert(list::uniform(items.take(index), width));
            list::uniform_sums(items.take(index), width);
            list::extent_prefix(items, index);
            list::extent_prefix(items, index + 1);
            let start = payload + product::extent(items.take(index));
            assert(crate::encoding_struct::child_bytes(a, list::initial(items, width), items, index) == items[index].bytes);
            assert(crate::encoding_struct::child_bytes(b, list::initial(items, width), items, index) == items[index].bytes);
            assert(a.subrange(start, start + items[index].bytes.len()) == items[index].bytes);
            assert(b.subrange(start, start + items[index].bytes.len()) == items[index].bytes);
            assert(a[i] == items[index].bytes[i - start] && b[i] == items[index].bytes[i - start]);
        }
    }
    assert(a =~= b);
}

/// Arbitrary fixed-list counts and implementing children establish full validity.
pub proof fn encoder_fixed_list_valid<E: Elements>(source: &E, child: Layout,
    width: u32, before: Seq<u8>, after: Seq<u8>, budget: nat)
    requires source.trusted_input(), list::uniform(source.items(), Some(width)),
        source.items().len() <= u32::MAX, crate::layout::width_spec(child) == Some(width),
        before.len() == list::size(source.items(), false) <= u32::MAX,
        after == list::output(before, source.items(), Some(width)),
        1 < budget <= MAX_DEPTH, child_contract(source.items(), child, (budget - 1) as nat),
    ensures crate::validation::schema_valid(after, Layout::List { element: &child }, budget, true),
        forall|other: Seq<u8>| other.len() == before.len() ==>
            list::output(other, source.items(), Some(width)) == after,
{
    hide(crate::validation::schema_valid); hide(product::paint_all); hide(list::output);
    assert(after == list::output(before, source.items(), Some(width)));
    assert forall|other: Seq<u8>| other.len() == before.len() implies
        list::output(other, source.items(), Some(width)) == after by { history(source.items(), Some(width), before, other); }
    complete(source.items(), child, Some(width), before, after, budget);
}

/// Arbitrary indexed-list counts preserve empty values and implicit terminal ends.
pub proof fn encoder_indexed_list_valid<E: Elements>(source: &E, child: Layout,
    before: Seq<u8>, after: Seq<u8>, budget: nat)
    requires source.trusted_input(), list::uniform(source.items(), None),
        source.items().len() <= u32::MAX, crate::layout::width_spec(child).is_none(),
        before.len() == list::size(source.items(), true) <= u32::MAX,
        after == list::output(before, source.items(), None),
        1 < budget <= MAX_DEPTH, child_contract(source.items(), child, (budget - 1) as nat),
    ensures crate::validation::schema_valid(after, Layout::List { element: &child }, budget, true),
        forall|other: Seq<u8>| other.len() == before.len() ==>
            list::output(other, source.items(), None) == after,
{
    hide(crate::validation::schema_valid); hide(product::paint_all); hide(list::output);
    assert(after == list::output(before, source.items(), None));
    assert forall|other: Seq<u8>| other.len() == before.len() implies
        list::output(other, source.items(), None) == after by { history(source.items(), None, before, other); }
    complete(source.items(), child, None, before, after, budget);
}

/// Repeating the actual driver's deterministic observation preserves every error.
///
/// The driver proves equality with `measured`, including count/directory checks,
/// child error order and cumulative overflow. The immutable projection contract
/// fixes child observations; this theorem needs no allocation or saved lengths.
pub proof fn encoder_list_measure_pure<E: Elements>(source: &E, indexed: bool,
    stride: Option<u32>, first: crate::Result<u32>, second: crate::Result<u32>)
    requires source.trusted_input(),
        stride.is_some() ==> !indexed && list::uniform(source.items(), stride),
        first == list::measured(source, indexed, stride),
        second == list::measured(source, indexed, stride),
    ensures first == second,
        first.is_ok() ==> first.unwrap() == second.unwrap(),
        first.is_err() ==> first.unwrap_err() == second.unwrap_err(),
{
}


/// Fixed and empty encodings use the actual generic writer and primitive children.
pub(crate) fn fixed_list_witness()
    ensures true,
{
    let source = FixedInputs { values: &[[0], [255]] };
    proof {
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(product::extent, 3);
    }
    let mut bytes = [19u8; 8];
    let ghost before = bytes@.subrange(1, 7);
    let first = source.at(0); let second = source.at(1);
    proof {
        crate::encoding_leaf::encoder_leaf_valid(&first, first.bytes(), 1);
        crate::encoding_leaf::encoder_leaf_valid(&second, second.bytes(), 1);
    }
    list::encode(&source, &mut bytes[1..7], Some(1), true);
    proof { encoder_fixed_list_valid(&source, first.layout(), 1, before, bytes@.subrange(1, 7), 2); }
    assert(bytes[0] == 19 && bytes[7] == 19);
    let empty = FixedInputs { values: &[] };
    proof { reveal_with_fuel(product::extent, 1); }
    let mut zero = [19u8; 4]; let ghost previous = zero@;
    list::encode(&empty, &mut zero, Some(1), true);
    proof { encoder_fixed_list_valid(&empty, first.layout(), 1, previous, zero@, 2); }
}

/// A nonempty singleton supplies a compact, no-premise non-vacuity witness.
pub(crate) fn indexed_list_witness()
    ensures true,
{
    proof {
        reveal(crate::layout_head::head_spec); reveal_strlit("a");
        reveal_with_fuel(vstd::utf8::encode_utf8, 2);
        reveal_with_fuel(product::extent, 2);
    }
    let source = TextInputs { values: &["a"] };
    let child = source.at(0);
    proof { crate::encoding_text::encoder_string_valid(&child, child.bytes(), 1); }
    let mut bytes = [19u8; 5]; let ghost before = bytes@;
    list::encode(&source, &mut bytes, None, true);
    proof { encoder_indexed_list_valid(&source, Layout::Str, before, bytes@, 2); }
}

/// Indexed output has an empty first/last child and an implicit terminal end.
pub(crate) fn indexed_list_partition_witness()
    ensures true,
{
    hide(crate::validation::schema_valid); hide(crate::validation::children_valid);
    hide(crate::validation::child_valid); hide(crate::validation::local_valid);
    hide(product::paint_all); hide(list::output);
    proof {
        reveal(crate::layout_head::head_spec);
        reveal_strlit(""); reveal_strlit("a");
        reveal_with_fuel(vstd::utf8::encode_utf8, 3);
    }
    let source = TextInputs { values: &["", "a", ""] };
    let mut bytes = [19u8; 15]; let ghost before = bytes@.subrange(1, 14);
    let first = source.at(0); let second = source.at(1); let third = source.at(2);
    proof {
        crate::encoding_text::encoder_string_valid(&first, first.bytes(), 1);
        crate::encoding_text::encoder_string_valid(&second, second.bytes(), 1);
        crate::encoding_text::encoder_string_valid(&third, third.bytes(), 1);
        reveal_with_fuel(product::extent, 4);
    }
    list::encode(&source, &mut bytes[1..14], None, true);
    proof { encoder_indexed_list_valid(&source, Layout::Str, before, bytes@.subrange(1, 14), 2); }
    assert(bytes[0] == 19 && bytes[14] == 19);
    let empty = TextInputs { values: &[] };
    let mut zero = [19u8; 4]; let ghost previous = zero@;
    list::encode(&empty, &mut zero, None, true);
    proof { encoder_indexed_list_valid(&empty, Layout::Str, previous, zero@, 2); }
}

/// Repeated real measurements exercise both storage classes and an empty list.
pub(crate) fn list_measure_witness()
    ensures true,
{
    let fixed = FixedInputs { values: &[[0], [255]] };
    let a = list::measure(&fixed, false, Some(1));
    let b = list::measure(&fixed, false, Some(1));
    proof { encoder_list_measure_pure(&fixed, false, Some(1), a, b); }
    proof {
        reveal(crate::layout_head::head_spec);
        reveal_strlit(""); reveal_strlit("a");
        reveal_with_fuel(vstd::utf8::encode_utf8, 3);
    }
    let indexed = TextInputs { values: &["", "a", ""] };
    let a = list::measure(&indexed, true, None);
    let b = list::measure(&indexed, true, None);
    proof { encoder_list_measure_pure(&indexed, true, None, a, b); }
    let repeated = crate::encoding_list_repeated_inputs::RepeatedInputs { count: u32::MAX as usize, value: "" };
    let a = list::measure(&repeated, true, None);
    let b = list::measure(&repeated, true, None);
    proof { encoder_list_measure_pure(&repeated, true, None, a, b); }
    assert(a.is_err());
    let empty = TextInputs { values: &[] };
    let a = list::measure(&empty, true, None);
    let b = list::measure(&empty, true, None);
    proof { encoder_list_measure_pure(&empty, true, None, a, b); }
}

}
