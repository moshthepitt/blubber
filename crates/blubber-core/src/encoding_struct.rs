//! Generic product packing and independent struct validity.

use crate::encoding_product::{self as product, Cursor, Item};
use vstd::prelude::*;

verus! {

/// Packed width of fixed declarations in a field sequence.
pub open spec fn fixed(items: Seq<Item>) -> nat
    decreases items.len(),
{
    if items.len() == 0 { 0 } else {
        (if items[0].fixed { items[0].bytes.len() } else { 0nat }) + fixed(items.drop_first())
    }
}

/// Concatenated width of variable declarations in a field sequence.
pub open spec fn variable(items: Seq<Item>) -> nat
    decreases items.len(),
{
    if items.len() == 0 { 0 } else {
        (if items[0].fixed { 0nat } else { items[0].bytes.len() }) + variable(items.drop_first())
    }
}

/// Number of variable declarations, including empty children.
pub open spec fn variables(items: Seq<Item>) -> nat
    decreases items.len(),
{
    if items.len() == 0 { 0 } else {
        (if items[0].fixed { 0nat } else { 1nat }) + variables(items.drop_first())
    }
}

/// Additive field summaries do not depend on internal product grouping.
pub proof fn sums(left: Seq<Item>, right: Seq<Item>)
    ensures fixed(left + right) == fixed(left) + fixed(right),
        variable(left + right) == variable(left) + variable(right),
        variables(left + right) == variables(left) + variables(right),
    decreases left.len(),
{
    if left.len() > 0 {
        assert((left + right).drop_first() =~= left.drop_first() + right);
        sums(left.drop_first(), right);
    }
}

/// Exact next cursor and byte frames after one projected field write.
pub proof fn paint_once(before: Seq<u8>, cursor: Cursor, item: Item)
    requires product::fits(cursor, before.len() as int, item),
    ensures product::paint(before, cursor, item).len() == before.len(),
        product::geometry(product::next(cursor, item), before.len() as int),
        product::paint(before, cursor, item).subrange(
            if item.fixed { cursor.fixed as int } else { cursor.payload as int },
            (if item.fixed { cursor.fixed as int } else { cursor.payload as int }) + item.bytes.len()) == item.bytes,
        forall|i: int| 0 <= i < before.len()
            && !(if item.fixed { cursor.fixed <= i < cursor.fixed + item.bytes.len() }
                 else { cursor.payload <= i < cursor.payload + item.bytes.len()
                    || (cursor.directory < cursor.directory_limit && cursor.directory <= i < cursor.directory + 4) })
            ==> #[trigger] product::paint(before, cursor, item)[i] == before[i],
{
    let after = product::paint(before, cursor, item);
    if item.fixed {
        assert(after.subrange(cursor.fixed as int, cursor.fixed + item.bytes.len()) =~= item.bytes);
    } else {
        assert(after.subrange(cursor.payload as int, cursor.payload + item.bytes.len()) =~= item.bytes);
    }
    assert forall|i: int| 0 <= i < before.len()
        && !(if item.fixed { cursor.fixed <= i < cursor.fixed + item.bytes.len() }
             else { cursor.payload <= i < cursor.payload + item.bytes.len()
                || (cursor.directory < cursor.directory_limit && cursor.directory <= i < cursor.directory + 4) })
        implies #[trigger] product::paint(before, cursor, item)[i] == before[i] by { }
}

/// Later child writes preserve every interval already consumed by the cursor.
pub proof fn consumed_frame(before: Seq<u8>, cursor: Cursor, items: Seq<Item>, start: int, end: int)
    requires product::geometry(cursor, before.len() as int),
        product::fits_all(cursor, before.len() as int, items),
        0 <= start <= end <= before.len(),
        end <= cursor.fixed || (cursor.fixed_limit <= start && end <= cursor.directory)
            || (cursor.directory_limit <= start && end <= cursor.payload),
    ensures product::paint_all(before, cursor, items).subrange(start, end) == before.subrange(start, end),
        product::paint_all(before, cursor, items).len() == before.len(),
    decreases items.len(),
{
    if items.len() > 0 {
        reveal_with_fuel(product::fits_all, 2);
        paint_once(before, cursor, items[0]);
        let next = product::next(cursor, items[0]);
        let after = product::paint(before, cursor, items[0]);
        assert(after.subrange(start, end) =~= before.subrange(start, end));
        consumed_frame(after, next, items.drop_first(), start, end);
    }
}

/// Complete field byte loans in the declaration-independent packed regions.
pub open spec fn child_bytes(after: Seq<u8>, cursor: Cursor, items: Seq<Item>, i: int) -> Seq<u8> {
    let start = if items[i].fixed { cursor.fixed + fixed(items.take(i)) }
        else { cursor.payload + variable(items.take(i)) };
    after.subrange(start, start + items[i].bytes.len())
}

/// Every exact child output survives all later product writes.
pub proof fn packed_children(before: Seq<u8>, cursor: Cursor, items: Seq<Item>)
    requires product::geometry(cursor, before.len() as int),
        product::fits_all(cursor, before.len() as int, items),
    ensures product::paint_all(before, cursor, items).len() == before.len(),
        forall|i: int| 0 <= i < items.len() ==>
            child_bytes(product::paint_all(before, cursor, items), cursor, items, i) == items[i].bytes,
    decreases items.len(),
{
    if items.len() > 0 {
        reveal_with_fuel(product::fits_all, 2);
        paint_once(before, cursor, items[0]);
        let after = product::paint(before, cursor, items[0]);
        let next = product::next(cursor, items[0]);
        packed_children(after, next, items.drop_first());
        let start = if items[0].fixed { cursor.fixed as int } else { cursor.payload as int };
        consumed_frame(after, next, items.drop_first(), start, start + items[0].bytes.len());
        assert forall|i: int| 0 <= i < items.len() implies
            child_bytes(product::paint_all(before, cursor, items), cursor, items, i) == items[i].bytes by {
            reveal_with_fuel(product::paint_all, 2);
            if i == 0 {
                assert(items.take(0) =~= Seq::<Item>::empty());
                assert(child_bytes(product::paint_all(before, cursor, items), cursor, items, i)
                    == product::paint_all(after, next, items.drop_first()).subrange(start, start + items[0].bytes.len()));
            } else {
                assert(child_bytes(product::paint_all(after, next, items.drop_first()), next, items.drop_first(), i - 1)
                    == items.drop_first()[i - 1].bytes);
                assert(items.take(i) =~= seq![items[0]] + items.drop_first().take(i - 1));
                sums(seq![items[0]], items.drop_first().take(i - 1));
                reveal_with_fuel(fixed, 2); reveal_with_fuel(variable, 2);
                assert(child_bytes(product::paint_all(before, cursor, items), cursor, items, i)
                    == child_bytes(product::paint_all(after, next, items.drop_first()), next, items.drop_first(), i - 1));
            }
        }
    }
}
/// Canonical directory byte construction decodes to its exact u32 value.
pub proof fn encoded_word(value: u32)
    ensures crate::words::le_word(product::word(value as int), 0) == value,
{
    let a = value as u8;
    let b = (value / 256) as u8;
    let c = (value / 65536) as u8;
    let d = (value / 16777216) as u8;
    assert(a as int + 256*b as int + 65536*c as int + 16777216*d as int == value) by(bit_vector)
        requires a == value as u8, b == (value / 256) as u8,
            c == (value / 65536) as u8, d == (value / 16777216) as u8;
}

/// All stored cumulative ends are determined by the same projected children.
pub proof fn packed_directory(before: Seq<u8>, cursor: Cursor, items: Seq<Item>)
    requires product::geometry(cursor, before.len() as int),
        product::fits_all(cursor, before.len() as int, items),
    ensures forall|i: int| 0 <= i < items.len() && !items[i].fixed
        && cursor.directory + 4*variables(items.take(i)) < cursor.directory_limit ==>
        crate::words::le_word(product::paint_all(before, cursor, items),
            cursor.directory + 4*variables(items.take(i)))
                == cursor.end + variable(items.take(i)) + items[i].bytes.len(),
    decreases items.len(),
{
    if items.len() > 0 {
        reveal_with_fuel(product::fits_all, 2);
        paint_once(before, cursor, items[0]);
        let after = product::paint(before, cursor, items[0]);
        let next = product::next(cursor, items[0]);
        packed_directory(after, next, items.drop_first());
        if !items[0].fixed && cursor.directory < cursor.directory_limit {
            encoded_word((cursor.end + items[0].bytes.len()) as u32);
            assert(after.subrange(cursor.directory as int, cursor.directory + 4)
                =~= product::word(cursor.end + items[0].bytes.len()));
            crate::words::word_subrange(after, cursor.directory as int, cursor.directory + 4, 0);
            consumed_frame(after, next, items.drop_first(), cursor.directory as int, cursor.directory + 4);
            crate::words::word_subrange(product::paint_all(after, next, items.drop_first()),
                cursor.directory as int, cursor.directory + 4, 0);
        }
        assert forall|i: int| 0 <= i < items.len() && !items[i].fixed
            && cursor.directory + 4*variables(items.take(i)) < cursor.directory_limit implies
            crate::words::le_word(product::paint_all(before, cursor, items),
                cursor.directory + 4*variables(items.take(i)))
                    == cursor.end + variable(items.take(i)) + items[i].bytes.len() by {
            if i > 0 {
                assert(items.take(i) =~= seq![items[0]] + items.drop_first().take(i - 1));
                sums(seq![items[0]], items.drop_first().take(i - 1));
                reveal_with_fuel(variables, 2); reveal_with_fuel(variable, 2);
                assert(items.drop_first()[i - 1] == items[i]);
                assert(cursor.directory < cursor.directory_limit);
                assert(next.directory == cursor.directory + if items[0].fixed { 0int } else { 4int });
                assert(next.end == cursor.end + if items[0].fixed { 0int } else { items[0].bytes.len() as int });
                assert(next.directory + 4*variables(items.drop_first().take(i - 1)) < cursor.directory_limit);
                assert(crate::words::le_word(product::paint_all(after, next, items.drop_first()),
                    next.directory + 4*variables(items.drop_first().take(i - 1)))
                    == next.end + variable(items.drop_first().take(i - 1)) + items[i].bytes.len());
            }
        }
    }
}
/// Prefix extents and counts are bounded by the complete projected sequence.
pub proof fn prefix(items: Seq<Item>, index: int)
    requires 0 <= index < items.len(),
    ensures fixed(items.take(index)) <= fixed(items),
        fixed(items.take(index + 1)) <= fixed(items),
        variable(items.take(index)) <= variable(items),
        variable(items.take(index + 1)) <= variable(items),
        variables(items.take(index)) <= variables(items),
        variables(items.take(index + 1)) <= variables(items),
        fixed(items.take(index + 1)) == fixed(items.take(index))
            + if items[index].fixed { items[index].bytes.len() } else { 0nat },
        variable(items.take(index + 1)) == variable(items.take(index))
            + if items[index].fixed { 0nat } else { items[index].bytes.len() },
        variables(items.take(index + 1)) == variables(items.take(index))
            + if items[index].fixed { 0nat } else { 1nat },
{
    assert(items =~= items.take(index) + items.skip(index));
    assert(items =~= items.take(index + 1) + items.skip(index + 1));
    assert(items.take(index + 1) =~= items.take(index) + seq![items[index]]);
    sums(items.take(index), items.skip(index));
    sums(items.take(index + 1), items.skip(index + 1));
    sums(items.take(index), seq![items[index]]);
    reveal_with_fuel(fixed, 2); reveal_with_fuel(variable, 2); reveal_with_fuel(variables, 2);
}

/// Fixed-only gaps contain no variable payload bytes.
pub proof fn zero_variables(items: Seq<Item>)
    requires variables(items) == 0,
    ensures variable(items) == 0,
    decreases items.len(),
{
    if items.len() > 0 { zero_variables(items.drop_first()); }
}

/// Variable ordinals impose declaration order without counting fixed fields.
pub proof fn variable_order(items: Seq<Item>, left: int, right: int)
    requires 0 <= left < items.len(), 0 <= right < items.len(),
        !items[left].fixed, !items[right].fixed,
        variables(items.take(left)) < variables(items.take(right)),
    ensures left < right,
{
    if right <= left {
        prefix(items.take(left + 1), right);
        assert(items.take(left + 1).take(right) =~= items.take(right));
        prefix(items, left);
    }
}

/// Adjacent variable declarations have no payload gap despite interleaved fixed fields.
pub proof fn adjacent_variables(items: Seq<Item>, left: int, right: int)
    requires 0 <= left < right < items.len(), !items[left].fixed, !items[right].fixed,
        variables(items.take(right)) == variables(items.take(left)) + 1,
    ensures variable(items.take(right)) == variable(items.take(left)) + items[left].bytes.len(),
{
    prefix(items, left);
    assert(items.take(right) =~= items.take(left + 1) + items.subrange(left + 1, right));
    sums(items.take(left + 1), items.subrange(left + 1, right));
    zero_variables(items.subrange(left + 1, right));
}

/// The final variable child reaches the implicit terminal end.
pub proof fn final_variable(items: Seq<Item>, index: int)
    requires 0 <= index < items.len(), !items[index].fixed,
        variables(items.take(index)) + 1 == variables(items),
    ensures variable(items.take(index)) + items[index].bytes.len() == variable(items),
{
    prefix(items, index);
    assert(items =~= items.take(index + 1) + items.skip(index + 1));
    sums(items.take(index + 1), items.skip(index + 1));
    zero_variables(items.skip(index + 1));
}

/// Static descriptors select the same field types, ordinals and packed widths.
///
/// Trusted projections may exchange values of the same field type. This
/// relation records types and storage classes, without claiming value mapping.
pub open spec fn agrees(items: Seq<Item>, layout: crate::Layout) -> bool {
    match layout.head() {
        crate::LayoutHead::Struct { fixed_bytes, fixed: f, variable: v } =>
            items.len() <= u16::MAX && fixed_bytes == fixed(items) && v.len() == variables(items)
                && f.len() + v.len() == items.len()
                && (forall|i: int| 0 <= i < f.len() ==> {
                    let field = #[trigger] f.view()[i];
                    let d = field.declaration as int;
                    &&& 0 <= d < items.len()
                    &&& items[d].fixed
                    &&& i == d - variables(items.take(d))
                    &&& field.offset == fixed(items.take(d))
                    &&& field.layout == items[d].layout
                    &&& crate::layout::width_spec(field.layout) == Some(items[d].bytes.len() as u32)
                    &&& items[d].bytes.len() <= u32::MAX
                })
                && (forall|i: int| 0 <= i < v.len() ==> {
                    let field = #[trigger] v.view()[i];
                    let d = field.declaration as int;
                    &&& 0 <= d < items.len()
                    &&& !items[d].fixed
                    &&& i == variables(items.take(d))
                    &&& field.layout == items[d].layout
                }),
        _ => false,
    }
}

/// Initial cursor for the exact three-region struct destination.
pub open spec fn initial(items: Seq<Item>) -> Cursor {
    let directory = if variables(items) > 0 { 4 * (variables(items) - 1) } else { 0int };
    Cursor { fixed: 0, fixed_limit: fixed(items) as usize,
        directory: fixed(items) as usize, directory_limit: (fixed(items) + directory) as usize,
        payload: (fixed(items) + directory) as usize, end: 0 }
}

/// Exact measured struct extent, including the elided terminal end.
pub open spec fn size(items: Seq<Item>) -> nat {
    fixed(items) + variable(items) + if variables(items) > 0 { 4 * (variables(items) - 1) as nat } else { 0nat }
}
/// Measurement's byte sum equals the packed fixed and variable extents.
pub proof fn measured_extent(items: Seq<Item>)
    ensures product::extent(items) == fixed(items) + variable(items),
    decreases items.len(),
{
    if items.len() > 0 { measured_extent(items.drop_first()); }
}

/// Remaining measured extents establish every loan and the final exact cursor.
pub proof fn packing_fits(items: Seq<Item>, cursor: Cursor, length: int)
    requires product::geometry(cursor, length),
        cursor.fixed + fixed(items) == cursor.fixed_limit,
        cursor.payload + variable(items) == length,
        cursor.end + variable(items) <= u32::MAX,
        cursor.directory_limit - cursor.directory <= 4*variables(items),
    ensures product::fits_all(cursor, length, items),
        product::advance(cursor, items).fixed == cursor.fixed_limit,
        product::advance(cursor, items).directory == cursor.directory_limit,
        product::advance(cursor, items).payload == length,
        product::advance(cursor, items).end == cursor.end + variable(items),
    decreases items.len(),
{
    if items.len() > 0 {
        assert(product::fits(cursor, length, items[0]));
        paint_once(Seq::new(length as nat, |i: int| 0u8), cursor, items[0]);
        packing_fits(items.drop_first(), product::next(cursor, items[0]), length);
    }
}

/// Successful whole-product measurement gives exact loans without extra input premises.
pub proof fn initial_fits(items: Seq<Item>)
    requires size(items) <= u32::MAX,
    ensures product::geometry(initial(items), size(items) as int),
        product::fits_all(initial(items), size(items) as int, items),
        product::advance(initial(items), items).fixed == fixed(items),
        product::advance(initial(items), items).directory == initial(items).directory_limit,
        product::advance(initial(items), items).payload == size(items),
{
    packing_fits(items, initial(items), size(items) as int);
}

/// Arbitrary implementing children retain their validity in every exact loan.
///
/// Child validity is supplied by the child library proofs or explicit trusted
/// true-marker contracts. The result is independent of grouping, destination
/// history and any native Rust field layout.
pub proof fn encoder_product_valid<F: product::Fields>(source: &F, before: Seq<u8>,
    after: Seq<u8>, cursor: Cursor, budget: nat)
    requires source.trusted_input(), product::geometry(cursor, before.len() as int),
        product::fits_all(cursor, before.len() as int, source.items()),
        after == product::paint_all(before, cursor, source.items()),
        forall|i: int| 0 <= i < source.items().len() ==>
            crate::validation::schema_valid(source.items()[i].bytes, source.items()[i].layout, budget, true),
    ensures after.len() == before.len(),
        forall|i: int| 0 <= i < source.items().len() ==>
            child_bytes(after, cursor, source.items(), i) == source.items()[i].bytes
                && crate::validation::schema_valid(child_bytes(after, cursor, source.items(), i),
                    source.items()[i].layout, budget, true),
        forall|i: int| 0 <= i < source.items().len() && !source.items()[i].fixed
            && cursor.directory + 4*variables(source.items().take(i)) < cursor.directory_limit ==>
            crate::words::le_word(after, cursor.directory + 4*variables(source.items().take(i)))
                == cursor.end + variable(source.items().take(i)) + source.items()[i].bytes.len(),
{
    hide(crate::validation::schema_valid);
    hide(product::paint_all);
    packed_children(before, cursor, source.items());
    packed_directory(before, cursor, source.items());
}

/// A struct variable ordinal resolves to its measured projected child interval.
pub proof fn variable_region(items: Seq<Item>, layout: crate::Layout, after: Seq<u8>, index: int)
    requires agrees(items, layout), after.len() == size(items) <= u32::MAX,
        match layout.head() { crate::LayoutHead::Struct { variable, .. } => 0 <= index < variable.len(), _ => false },
        forall|i: int| 0 <= i < items.len() && !items[i].fixed
            && initial(items).directory + 4*variables(items.take(i)) < initial(items).directory_limit ==>
            crate::words::le_word(after, initial(items).directory + 4*variables(items.take(i)))
                == variable(items.take(i)) + items[i].bytes.len(),
    ensures match layout.head() {
        crate::LayoutHead::Struct { variable: fields, .. } => {
            let d = fields.view()[index].declaration as int;
            let span = crate::struct_layout::variable_span(after, layout, index as u16);
            &&& span.start == initial(items).payload + variable(items.take(d))
            &&& span.end == initial(items).payload + variable(items.take(d)) + items[d].bytes.len()
            &&& span.start <= span.end <= after.len()
        }, _ => false,
    },
{
    let (bytes, fields) = match layout.head() {
        crate::LayoutHead::Struct { fixed_bytes, variable, .. } => (fixed_bytes, variable),
        _ => { assert(false); return; },
    };
    let d = fields.view()[index].declaration as int;
    prefix(items, d);
    if index == 0 {
        zero_variables(items.take(d));
    } else {
        let p = fields.view()[index - 1].declaration as int;
        variable_order(items, p, d);
        adjacent_variables(items, p, d);
        assert(crate::words::le_word(after, bytes + 4*(index - 1)) == variable(items.take(d)));
    }
    assert(crate::struct_layout::payload_start(layout) == initial(items).payload);
    assert(initial(items).payload + variable(items.take(d)) + items[d].bytes.len() <= after.len());
    assert(index <= u16::MAX);
    if index + 1 == fields.len() {
        final_variable(items, d);
    } else {
        assert(crate::words::le_word(after, bytes + 4*index) == variable(items.take(d)) + items[d].bytes.len());
    }
    let span = crate::struct_layout::variable_span(after, layout, index as u16);
    assert(span.start == initial(items).payload + variable(items.take(d)));
    assert(span.end == initial(items).payload + variable(items.take(d)) + items[d].bytes.len());
}

/// Exact stored cumulative ends for the projected variable fields.
pub open spec fn directory_values(items: Seq<Item>, after: Seq<u8>) -> bool {
    forall|i: int| 0 <= i < items.len() && !items[i].fixed
        && initial(items).directory + 4*variables(items.take(i)) < initial(items).directory_limit ==>
        crate::words::le_word(after, initial(items).directory + 4*variables(items.take(i)))
            == variable(items.take(i)) + items[i].bytes.len()
}

/// Exact cumulative ends establish the independent local struct partition.
pub proof fn local_struct(items: Seq<Item>, layout: crate::Layout, after: Seq<u8>)
    requires agrees(items, layout), after.len() == size(items) <= u32::MAX,
        directory_values(items, after),
        crate::layout::descriptor_valid(layout, crate::MAX_DEPTH as nat),
    ensures crate::validation::local_valid(after, layout, true),
{
    hide(fixed); hide(variable); hide(variables); hide(crate::layout::width_spec);
    hide(crate::validation::schema_valid); hide(crate::validation::child_valid);
    hide(crate::validation::children_valid); hide(crate::validation::local_valid);
    hide(crate::layout::descriptor_valid); hide(crate::utf8::utf8_valid);
    hide(crate::leaves::fixed_value_valid);
    let (bytes, f, v) = match layout.head() {
        crate::LayoutHead::Struct { fixed_bytes, fixed, variable } => (fixed_bytes, fixed, variable),
        _ => { assert(false); return; },
    };
    assert(crate::struct_layout::payload_start(layout) == initial(items).payload);
    assert forall|j: int| 0 <= j < v.len() - 1 implies {
        let end = #[trigger] crate::words::le_word(after, bytes + 4*j);
        &&& end <= after.len() - crate::struct_layout::payload_start(layout)
        &&& if j == 0 { 0 <= end }
            else { crate::words::le_word(after, bytes + 4*(j - 1)) <= end }
    } by {
        let d = v.view()[j].declaration as int;
        prefix(items, d);
        assert(crate::words::le_word(after, bytes + 4*j) == variable(items.take(d)) + items[d].bytes.len());
        if j > 0 {
            let p = v.view()[j-1].declaration as int;
            variable_order(items, p, d);
            adjacent_variables(items, p, d);
            assert(crate::words::le_word(after, bytes + 4*(j-1)) == variable(items.take(d)));
        }
    }
    assert(crate::struct_layout::payload_start(layout) <= after.len());
    if v.len() == 0 {
        zero_variables(items);
        assert(after.len() == bytes);
    }
    assert(crate::struct_layout::struct_shape(after, layout));
    assert(crate::validation::local_valid(after, layout, true)) by { reveal(crate::validation::local_valid); }
}

/// Every statically selected child occupies its exact validator interval.
pub proof fn struct_children(items: Seq<Item>, layout: crate::Layout, after: Seq<u8>, budget: nat)
    requires agrees(items, layout), after.len() == size(items) <= u32::MAX,
        directory_values(items, after),
        forall|i: int| 0 <= i < items.len() ==>
            child_bytes(after, initial(items), items, i) == items[i].bytes
                && crate::validation::schema_valid(items[i].bytes, items[i].layout, budget, true),
    ensures crate::validation::children_valid(after, layout, 0, budget, true),
{
    hide(fixed); hide(variable); hide(variables); hide(crate::layout::width_spec);
    hide(crate::validation::schema_valid); hide(crate::validation::child_valid);
    hide(crate::validation::children_valid); hide(crate::validation::local_valid);
    hide(crate::layout::descriptor_valid); hide(crate::utf8::utf8_valid);
    hide(crate::leaves::fixed_value_valid);
    let (bytes, f, v) = match layout.head() {
        crate::LayoutHead::Struct { fixed_bytes, fixed, variable } => (fixed_bytes, fixed, variable),
        _ => { assert(false); return; },
    };
    assert(crate::struct_layout::payload_start(layout) == initial(items).payload);
    assert forall|i: int| 0 <= i < crate::validation::child_count(after, layout) implies
        #[trigger] crate::validation::child_valid(after, layout, i, budget, true) by {
        crate::validation::child_definition(after, layout, i, budget, true);
        if i < f.len() {
            let d = f.view()[i].declaration as int;
            prefix(items, d);
            assert(child_bytes(after, initial(items), items, d) == items[d].bytes);
            assert(crate::validation::schema_valid(items[d].bytes, items[d].layout, budget, true));
            assert(crate::validation::child_layout(layout, i) == items[d].layout);
            assert(after.subrange(crate::validation::child_span(after, layout, i).start as int,
                crate::validation::child_span(after, layout, i).end as int)
                    == child_bytes(after, initial(items), items, d));
        } else {
            let j = i - f.len();
            let d = v.view()[j].declaration as int;
            variable_region(items, layout, after, j);
            assert(child_bytes(after, initial(items), items, d) == items[d].bytes);
            assert(crate::validation::schema_valid(items[d].bytes, items[d].layout, budget, true));
            assert(crate::validation::child_layout(layout, i) == items[d].layout);
            assert(after.subrange(crate::validation::child_span(after, layout, i).start as int,
                crate::validation::child_span(after, layout, i).end as int)
                    == child_bytes(after, initial(items), items, d));
        }
    }
    crate::preservation::children_from_all(after, layout, 0, budget, true);
}

/// Successful measured generic encoding establishes complete independent validity.
///
/// The descriptor and child-type agreement come from the shared field metadata;
/// child validity comes from library proofs or named true-marker contracts.
/// The projected values themselves remain the trusted derive boundary.
pub proof fn encoder_struct_valid<F: product::Fields>(source: &F, layout: crate::Layout,
    before: Seq<u8>, after: Seq<u8>, measured: u32, budget: nat)
    requires source.trusted_input(), agrees(source.items(), layout),
        1 < budget <= crate::MAX_DEPTH,
        crate::layout::descriptor_valid(layout, crate::MAX_DEPTH as nat),
        measured == product::extent(source.items())
            + if variables(source.items()) > 0 { 4*(variables(source.items()) - 1) } else { 0int },
        before.len() == measured,
        after == product::paint_all(before, initial(source.items()), source.items()),
        forall|i: int| 0 <= i < source.items().len() ==>
            crate::validation::schema_valid(source.items()[i].bytes, source.items()[i].layout, (budget - 1) as nat, true),
    ensures crate::validation::schema_valid(after, layout, budget, true),
{
    hide(product::paint_all); hide(product::fits_all); hide(crate::validation::schema_valid);
    hide(agrees); hide(fixed); hide(variable); hide(variables);
    let items = source.items();
    assert(after == product::paint_all(before, initial(items), items));
    measured_extent(items);
    assert(before.len() == size(items));
    initial_fits(items);
    encoder_product_valid(source, before, after, initial(items), (budget - 1) as nat);
    local_struct(items, layout, after);
    struct_children(items, layout, after, (budget - 1) as nat);
    reveal(crate::validation::schema_valid);
}
/// Actual balanced encoder with a bool and two empty variable children.
#[cfg(verus_only)]
pub(crate) fn product_witness()
    ensures true,
{
    use crate::encoding_product::{Field, Fields, Pair, Source};
    proof { reveal_strlit(""); reveal(crate::layout_head::head_spec); }
    let source = Pair { left: Field { source: crate::encoding_leaf::boolean(&[1]) },
        right: Pair { left: Field { source: crate::encoding_text::text("") },
            right: Field { source: crate::encoding_text::text("") } } };
    let mut output = [19u8; 5];
    let cursor = product::cursor(5, 1, 4);
    proof {
        reveal_with_fuel(product::fits_all, 4);
        crate::encoding_leaf::encoder_leaf_valid(&source.left.source, source.left.source.bytes(), 1);
        crate::encoding_text::encoder_string_valid(&source.right.left.source, source.right.left.source.bytes(), 1);
        crate::encoding_text::encoder_string_valid(&source.right.right.source, source.right.right.source.bytes(), 1);
        assert forall|i: int| 0 <= i < source.items().len() implies
            crate::validation::schema_valid(source.items()[i].bytes, source.items()[i].layout, 1, true) by { }
    }
    let ghost before = output@;
    let end = product::write_fields(&source, &mut output, cursor);
    proof { encoder_product_valid(&source, before, output@, cursor, 1); }
    proof { reveal_with_fuel(product::advance, 4); }
    proof { reveal_with_fuel(product::advance, 4); }
    product::finish(end, 5);
    proof { reveal_with_fuel(product::paint_all, 4); }
    assert(output@ =~= seq![1u8, 0, 0, 0, 0]);
}
/// Concrete full struct proof through measured production writers and metadata.
#[cfg(verus_only)]
pub(crate) fn struct_witness()
    ensures true,
{
    use crate::encoding_product::{Field, Fields, Pair, Source};
    proof { reveal_strlit(""); reveal(crate::layout_head::head_spec); }
    let source = Pair { left: Field { source: crate::encoding_leaf::boolean(&[1]) },
        right: Pair { left: Field { source: crate::encoding_text::text("") },
            right: Field { source: crate::encoding_text::text("") } } };
    let ghost fixed_fields = [crate::FixedField { declaration: 0, offset: 0, layout: &crate::Layout::Leaf(crate::Leaf::Bool) }];
    let ghost variable_fields = [crate::VariableField { declaration: 1, layout: &crate::Layout::Str },
        crate::VariableField { declaration: 2, layout: &crate::Layout::Str }];
    let ghost layout = crate::Layout::Struct { fixed_bytes: 1, fixed: &fixed_fields, variable: &variable_fields };
    proof {
        reveal_with_fuel(product::extent, 4);
        reveal_with_fuel(fixed, 4); reveal_with_fuel(variable, 4); reveal_with_fuel(variables, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
        crate::encoding_leaf::encoder_leaf_valid(&source.left.source, source.left.source.bytes(), 1);
        crate::encoding_text::encoder_string_valid(&source.right.left.source, source.right.left.source.bytes(), 1);
        crate::encoding_text::encoder_string_valid(&source.right.right.source, source.right.right.source.bytes(), 1);
        assert(agrees(source.items(), layout));
        let f = crate::layout_head::Fields::Fixed(&fixed_fields).view();
        let v = crate::layout_head::Fields::Variable(&variable_fields).view();
        crate::layout::descriptor_fields_step(f, v, 1, 1, 2, 1, crate::MAX_DEPTH as nat);
        assert(crate::layout::descriptor_fields(f, v, 1, 1, 2, 1, crate::MAX_DEPTH as nat));
        crate::layout::descriptor_fields_step(f, v, 1, 1, 1, 1, crate::MAX_DEPTH as nat);
        assert(crate::layout::descriptor_fields(f, v, 1, 1, 1, 1, crate::MAX_DEPTH as nat));
        crate::layout::descriptor_fields_step(f, v, 1, 1, 0, 1, crate::MAX_DEPTH as nat);
        assert(crate::layout::descriptor_fields(f, v, 1, 1, 0, 1, crate::MAX_DEPTH as nat));
        crate::layout::descriptor_fields_step(f, v, 1, 0, 0, 0, crate::MAX_DEPTH as nat);
        assert(crate::layout::descriptor_fields(f, v, 1, 0, 0, 0, crate::MAX_DEPTH as nat));
        assert(crate::layout::descriptor_valid(layout, crate::MAX_DEPTH as nat));
        assert forall|i: int| 0 <= i < source.items().len() implies
            crate::validation::schema_valid(source.items()[i].bytes, source.items()[i].layout, 1, true) by { }
        initial_fits(source.items());
    }
    let measured = match product::measure_fields(&source, 4) { Ok(size) => size, Err(_) => panic!("measured struct") };
    assert(measured == 5);
    let mut output = [19u8; 5];
    let ghost before = output@;
    let cursor = product::cursor(5, 1, 4);
    proof { assert(cursor == initial(source.items())); }
    let end = product::write_fields(&source, &mut output, cursor);
    proof { reveal_with_fuel(product::advance, 4); }
    product::finish(end, 5);
    proof { encoder_struct_valid(&source, layout, before, output@, measured, 2); }
}

}
