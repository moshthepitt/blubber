//! Independent map validity from the actual generic writer's exact regions.

use crate::encoding_map::{self as map, Entries, Entry};
use crate::{KeyDomain, KeyLayout, Layout, MAX_DEPTH};
use vstd::prelude::*;

verus! {

/// Canonical key domains and strict adjacent order supplied by key preflight.
pub open spec fn keys(entries: Seq<Entry>, key: KeyLayout) -> bool {
    (forall|i: int| 0 <= i < entries.len() ==>
        !matches!(key.domain, KeyDomain::Bool) || (#[trigger] entries[i]).key[0] <= 1)
    && (forall|i: int| 0 < i < entries.len() ==>
        crate::search::lex_less((#[trigger] entries[i - 1]).key, (#[trigger] entries[i]).key))
}

/// Adjacent strict order implies all-pairs uniqueness for arbitrary counts.
pub proof fn ordered(entries: Seq<Entry>, key: KeyLayout, left: int, right: int)
    requires keys(entries, key), 0 <= left < right < entries.len(),
    ensures crate::search::lex_less(entries[left].key, entries[right].key),
    decreases right - left,
{
    assert(crate::search::lex_less(entries[right - 1].key, entries[right].key));
    if left + 1 < right {
        ordered(entries, key, left, right - 1);
        crate::search::lex_transitive(entries[left].key, entries[right - 1].key, entries[right].key);
    }
}

/// Written keys, values and terminal ends establish independent map geometry.
pub proof fn partition(entries: Seq<Entry>, key: KeyLayout, child: Layout,
    after: Seq<u8>, width: Option<u32>)
    requires map::uniform(entries, key.width, width), entries.len() <= u32::MAX,
        crate::layout::width_spec(child) == width,
        after.len() == map::size(entries, key.width, width.is_none()) <= u32::MAX,
        crate::words::le_word(after, 0) == entries.len(),
        map::written(after, entries, key.width, width, entries.len() as int),
    ensures crate::map_layout::map_shape(after, Layout::Map { key, value: &child }),
        forall|i: int| 0 <= i < entries.len() ==>
            crate::search::encoded_key(after, Layout::Map { key, value: &child }, i) == entries[i].key,
        forall|i: int| 0 <= i < entries.len() ==>
            after.subrange(crate::map_layout::value_span(after, Layout::Map { key, value: &child }, i as u32).start as int,
                crate::map_layout::value_span(after, Layout::Map { key, value: &child }, i as u32).end as int) == entries[i].value,
{
    hide(crate::layout::descriptor_valid); hide(crate::layout::width_spec);
    hide(crate::validation::schema_valid); hide(crate::validation::local_valid);
    hide(crate::validation::children_valid); hide(crate::validation::child_valid);
    hide(crate::search::lex_less); hide(crate::search::keys_canonical); hide(crate::search::keys_ordered);
    hide(map::entry_written); hide(crate::words::le_word);
    reveal(crate::layout_head::head_spec);
    let layout = Layout::Map { key, value: &child };
    assert(crate::map_layout::value_width(layout) == width);
    assert(crate::map_layout::key_width(layout) == key.width);
    if let Some(fixed) = width {
        map::fixed_extent(entries, fixed);
        assert(entries.len() * (key.width as int + fixed as int)
            == entries.len() * key.width + entries.len() * fixed) by(nonlinear_arith);
        assert(after.len() == 4 + entries.len() * crate::map_layout::entry_stride(layout));
        assert(crate::map_layout::map_shape(after, layout));
    } else {
        if entries.len() == 0 { reveal_with_fuel(map::extent, 1); }
        else {
            reveal(map::entry_written);
            assert(map::entry_written(after, entries, key.width, width, entries.len() as int - 1));
            map::regions(entries, key.width, width, entries.len() as int - 1);
            assert(after.subrange(map::key_start(entries.len() as int - 1, key.width, width),
                map::key_start(entries.len() as int - 1, key.width, width) + key.width) == entries[entries.len() as int - 1].key);
            assert(entries.take(entries.len() as int) =~= entries);
            assert(crate::words::le_word(after, map::key_start(entries.len() as int - 1, key.width, width) + key.width)
                == map::extent(entries));
        }
        assert forall|i: int| 0 <= i < entries.len() implies {
            let end = #[trigger] crate::words::le_word(after, 4 + i * crate::map_layout::entry_stride(layout) + key.width);
            0 <= end <= after.len() - crate::map_layout::payload_start(after, layout)
                && (i == 0 || crate::words::le_word(after, 4 + (i - 1) * crate::map_layout::entry_stride(layout) + key.width) <= end)
        } by {
            reveal(map::entry_written);
            assert(map::entry_written(after, entries, key.width, width, i));
            if i > 0 { assert(map::entry_written(after, entries, key.width, width, i - 1)); }
            reveal(map::entry_written);
        assert(map::entry_written(after, entries, key.width, width, i));
        map::regions(entries, key.width, width, i);
            assert(after.subrange(map::key_start(i, key.width, width), map::key_start(i, key.width, width) + key.width) == entries[i].key);
            if i > 0 { assert(after.subrange(map::key_start(i - 1, key.width, width), map::key_start(i - 1, key.width, width) + key.width) == entries[i - 1].key); }
            map::prefix(entries, i); map::prefix(entries, i + 1);
        }
        assert(crate::map_layout::payload_start(after, layout) == map::base(entries.len() as int, key.width, true));
        assert(entries.len() == 0 || crate::words::le_word(after, 4 + (entries.len() - 1) * crate::map_layout::entry_stride(layout) + key.width)
            == after.len() - crate::map_layout::payload_start(after, layout));
        assert(crate::map_layout::map_shape(after, layout));
    }
    assert(crate::map_layout::map_shape(after, layout));
    assert forall|i: int| 0 <= i < entries.len() implies
        crate::search::encoded_key(after, layout, i) == entries[i].key by {
        reveal(map::entry_written);
        assert(map::entry_written(after, entries, key.width, width, i));
        map::regions(entries, key.width, width, i);
    }
    assert forall|i: int| 0 <= i < entries.len() implies
        after.subrange(crate::map_layout::value_span(after, layout, i as u32).start as int,
            crate::map_layout::value_span(after, layout, i as u32).end as int) == entries[i].value by {
        reveal(map::entry_written);
        assert(map::entry_written(after, entries, key.width, width, i));
        map::regions(entries, key.width, width, i);
        if width.is_none() {
            if i == 0 { assert(entries.take(0) =~= Seq::<Entry>::empty()); }
            else { assert(map::entry_written(after, entries, key.width, width, i - 1));
                assert(crate::words::le_word(after, map::key_start(i - 1, key.width, width) + key.width)
                == map::extent(entries.take(i))); }
        }
    }
}

/// Exact regions and recursively proved values establish complete map validity.
pub proof fn complete(entries: Seq<Entry>, key: KeyLayout, child: Layout,
    after: Seq<u8>, width: Option<u32>, budget: nat)
    requires map::uniform(entries, key.width, width), entries.len() <= u32::MAX,
        crate::layout::width_spec(child) == width, keys(entries, key),
        after.len() == map::size(entries, key.width, width.is_none()) <= u32::MAX,
        crate::words::le_word(after, 0) == entries.len(),
        map::written(after, entries, key.width, width, entries.len() as int),
        1 < budget <= MAX_DEPTH,
        crate::layout::descriptor_valid(Layout::Map { key, value: &child }, budget),
        forall|i: int| 0 <= i < entries.len() ==>
            crate::validation::schema_valid(entries[i].value, child, (budget - 1) as nat, true),
    ensures crate::validation::schema_valid(after, Layout::Map { key, value: &child }, budget, true),
{
    hide(crate::validation::schema_valid); hide(crate::validation::child_valid);
    hide(crate::validation::children_valid); hide(crate::validation::local_valid);
    hide(crate::layout::descriptor_valid); hide(crate::search::lex_less);
    hide(crate::leaves::fixed_value_valid); hide(crate::utf8::utf8_valid);
    hide(crate::words::le_word); hide(crate::map_layout::map_shape);
    hide(crate::layout::width_spec); hide(map::written); hide(map::uniform);
    partition(entries, key, child, after, width);
    reveal(crate::layout_head::head_spec);
    let layout = Layout::Map { key, value: &child };
    crate::encoding_metadata::descriptor_budget(layout, budget, MAX_DEPTH as nat);
    assert(crate::search::keys_canonical(after, layout, entries.len() as int));
    assert forall|i: int, j: int| 0 <= i < j < entries.len() implies
        crate::search::lex_less(#[trigger] crate::search::encoded_key(after, layout, i),
            #[trigger] crate::search::encoded_key(after, layout, j)) by {
        ordered(entries, key, i, j);
    }
    assert forall|i: int| 0 <= i < crate::validation::child_count(after, layout) implies
        #[trigger] crate::validation::child_valid(after, layout, i, (budget - 1) as nat, true) by {
        assert(0 <= i < entries.len());
        assert(crate::validation::child_span(after, layout, i) == crate::map_layout::value_span(after, layout, i as u32));
        assert(crate::validation::child_layout(layout, i) == child);
        assert(crate::validation::schema_valid(entries[i].value, child, (budget - 1) as nat, true));
        assert(after.subrange(crate::map_layout::value_span(after, layout, i as u32).start as int,
            crate::map_layout::value_span(after, layout, i as u32).end as int) == entries[i].value);
        crate::validation::child_definition(after, layout, i, (budget - 1) as nat, true);
    }
    crate::preservation::children_from_all(after, layout, 0, (budget - 1) as nat, true);
    reveal(crate::validation::local_valid);
    assert(crate::validation::local_valid(after, layout, true));
    assert(crate::validation::children_valid(after, layout, 0, (budget - 1) as nat, true));
    reveal_with_fuel(crate::validation::schema_valid, 2);
    reveal_with_fuel(crate::validation::children_valid, 1);
    assert(crate::validation::schema_valid(after, layout, budget, true));
    assert(layout == (Layout::Map { key, value: &child }));
}

/// A payload byte belongs to one value, even when adjacent values are empty.
pub proof fn payload_index(entries: Seq<Entry>, offset: int) -> (index: int)
    requires 0 <= offset < map::extent(entries),
    ensures 0 <= index < entries.len(),
        map::extent(entries.take(index)) <= offset < map::extent(entries.take(index)) + entries[index].value.len(),
    decreases entries.len(),
{
    reveal_with_fuel(map::extent, 2);
    assert(entries.len() > 0);
    if offset < entries[0].value.len() {
        assert(entries.take(0) =~= Seq::<Entry>::empty());
        0
    } else {
        let index = payload_index(entries.drop_first(), offset - entries[0].value.len());
        assert(entries.take(index + 1) =~= seq![entries[0]] + entries.drop_first().take(index));
        map::extent_concat(seq![entries[0]], entries.drop_first().take(index));
        index + 1
    }
}

/// Every byte is written: equal projected entries give equal complete output.
pub proof fn unique(entries: Seq<Entry>, key: u32, width: Option<u32>, left: Seq<u8>, right: Seq<u8>)
    requires map::uniform(entries, key, width), entries.len() <= u32::MAX,
        left.len() == map::size(entries, key, width.is_none()) <= u32::MAX, right.len() == left.len(),
        crate::words::le_word(left, 0) == entries.len(), crate::words::le_word(right, 0) == entries.len(),
        map::written(left, entries, key, width, entries.len() as int),
        map::written(right, entries, key, width, entries.len() as int),
    ensures left == right,
{
    hide(crate::words::le_word);
    crate::encoding_list_valid::word_bytes(left, 0, entries.len() as u32);
    crate::encoding_list_valid::word_bytes(right, 0, entries.len() as u32);
    let stride = key as int + if width.is_some() { width.unwrap() as int } else { 4int };
    if let Some(fixed) = width {
        map::fixed_extent(entries, fixed);
        assert(entries.len() * (key as int + fixed as int) == entries.len() * key + entries.len() * fixed) by(nonlinear_arith);
    }
    assert forall|offset: int| 0 <= offset < left.len() implies left[offset] == right[offset] by {
        if offset < 4 { }
        else if width.is_some() || offset < map::base(entries.len() as int, key, true) {
            let index = (offset - 4) / stride;
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(offset - 4, stride);
            vstd::arithmetic::div_mod::lemma_remainder(offset - 4, stride);
            assert(0 <= index < entries.len()) by(nonlinear_arith)
                requires offset >= 4, 0 < stride, offset - 4 < entries.len() * stride,
                    offset - 4 == stride * index + (offset - 4) % stride,
                    0 <= (offset - 4) % stride < stride;
            map::regions(entries, key, width, index);
            assert(map::entry_written(left, entries, key, width, index));
            assert(map::entry_written(right, entries, key, width, index));
            let start = map::key_start(index, key, width);
            assert(start <= offset < start + stride) by(nonlinear_arith)
                requires start == 4 + index * stride, offset - 4 == stride * index + (offset - 4) % stride,
                    0 <= (offset - 4) % stride < stride;
            if offset < start + key {
                assert(left[offset] == entries[index].key[offset - start]);
                assert(right[offset] == entries[index].key[offset - start]);
            } else if width.is_none() {
                let end = map::extent(entries.take(index + 1)) as u32;
                crate::encoding_list_valid::word_bytes(left, start + key, end);
                crate::encoding_list_valid::word_bytes(right, start + key, end);
            } else {
                assert(left[offset] == entries[index].value[offset - start - key]);
                assert(right[offset] == entries[index].value[offset - start - key]);
            }
        } else {
            let payload = map::base(entries.len() as int, key, true);
            let index = payload_index(entries, offset - payload);
            map::regions(entries, key, width, index);
            assert(map::entry_written(left, entries, key, width, index));
            assert(map::entry_written(right, entries, key, width, index));
            let start = map::value_start(entries, index, key, width);
            assert(left[offset] == entries[index].value[offset - start]);
            assert(right[offset] == entries[index].value[offset - start]);
        }
    }
    assert(left =~= right);
}

/// Arbitrary fixed maps have canonical ordered keys and exact interleaved values.
pub proof fn encoder_fixed_map_valid<E: Entries>(source: &E, key: KeyLayout, child: Layout,
    after: Seq<u8>, width: u32, budget: nat)
    requires source.trusted_input(), map::uniform(source.entries(), key.width, Some(width)),
        source.entries().len() <= u32::MAX, crate::layout::width_spec(child) == Some(width),
        keys(source.entries(), key),
        after.len() == map::size(source.entries(), key.width, false) <= u32::MAX,
        crate::words::le_word(after, 0) == source.entries().len(),
        map::written(after, source.entries(), key.width, Some(width), source.entries().len() as int),
        1 < budget <= MAX_DEPTH,
        crate::layout::descriptor_valid(Layout::Map { key, value: &child }, budget),
        forall|i: int| 0 <= i < source.entries().len() ==>
            crate::validation::schema_valid(source.entries()[i].value, child, (budget - 1) as nat, true),
    ensures crate::validation::schema_valid(after, Layout::Map { key, value: &child }, budget, true),
        forall|other: Seq<u8>| other.len() == after.len()
            && crate::words::le_word(other, 0) == source.entries().len()
            && #[trigger] map::written(other, source.entries(), key.width, Some(width), source.entries().len() as int) ==> other == after,
{
    complete(source.entries(), key, child, after, Some(width), budget);
    assert forall|other: Seq<u8>| other.len() == after.len()
        && crate::words::le_word(other, 0) == source.entries().len()
        && #[trigger] map::written(other, source.entries(), key.width, Some(width), source.entries().len() as int) implies other == after by {
        unique(source.entries(), key.width, Some(width), after, other);
    }
}

/// Arbitrary indexed maps write every end, including the terminal empty value.
pub proof fn encoder_indexed_map_valid<E: Entries>(source: &E, key: KeyLayout, child: Layout,
    after: Seq<u8>, budget: nat)
    requires source.trusted_input(), map::uniform(source.entries(), key.width, None),
        source.entries().len() <= u32::MAX, crate::layout::width_spec(child).is_none(),
        keys(source.entries(), key),
        after.len() == map::size(source.entries(), key.width, true) <= u32::MAX,
        crate::words::le_word(after, 0) == source.entries().len(),
        map::written(after, source.entries(), key.width, None, source.entries().len() as int),
        1 < budget <= MAX_DEPTH,
        crate::layout::descriptor_valid(Layout::Map { key, value: &child }, budget),
        forall|i: int| 0 <= i < source.entries().len() ==>
            crate::validation::schema_valid(source.entries()[i].value, child, (budget - 1) as nat, true),
    ensures crate::validation::schema_valid(after, Layout::Map { key, value: &child }, budget, true),
        forall|other: Seq<u8>| other.len() == after.len()
            && crate::words::le_word(other, 0) == source.entries().len()
            && #[trigger] map::written(other, source.entries(), key.width, None, source.entries().len() as int) ==> other == after,
{
    complete(source.entries(), key, child, after, None, budget);
    assert forall|other: Seq<u8>| other.len() == after.len()
        && crate::words::le_word(other, 0) == source.entries().len()
        && #[trigger] map::written(other, source.entries(), key.width, None, source.entries().len() as int) implies other == after by {
        unique(source.entries(), key.width, None, after, other);
    }
}

/// Successful prefix observations reach one entry with its exact running total.
pub open spec fn reaches<E: Entries>(source: &E, fixed: Option<u32>, index: int, total: u32, stop: int, at: u32) -> bool
    decreases stop - index,
{
    if index > stop { false }
    else if index == stop { total == at }
    else { map::step(source, index, total, fixed).is_ok()
        && reaches(source, fixed, index + 1, map::step(source, index, total, fixed).unwrap(), stop, at) }
}

/// A reached error wins over every later comparison and value measurement.
pub proof fn reached_fault<E: Entries>(source: &E, fixed: Option<u32>, index: int, total: u32, stop: int, at: u32)
    requires 0 <= index <= stop < source.entries().len(), reaches(source, fixed, index, total, stop, at),
        map::step(source, stop, at, fixed).is_err(),
    ensures map::fold(source, index, total, fixed) == map::step(source, stop, at, fixed),
    decreases stop - index,
{
    reveal_with_fuel(map::fold, 2); reveal_with_fuel(reaches, 2);
    if index < stop { reached_fault(source, fixed, index + 1, map::step(source, index, total, fixed).unwrap(), stop, at); }
}

/// The actual driver's result is its retained count/order/value observation.
///
/// Recursion checks a value before advancing to the next key. Therefore a
/// preceding value error is preserved even when a later key is out of order.
pub proof fn encoder_map_first_fault<E: Entries>(source: &E, key: u32, indexed: bool,
    fixed: Option<u32>, result: crate::Result<u32>, repeat: crate::Result<u32>)
    requires source.trusted_input(), result == map::measured(source, key, indexed, fixed),
        repeat == map::measured(source, key, indexed, fixed),
    ensures result == repeat,
        result.is_err() ==> result.unwrap_err() == repeat.unwrap_err(),
        forall|stop: int, at: u32| 0 <= stop < source.entries().len() <= u32::MAX
            && key as int + (if indexed { 4int } else { 0int }) <= u32::MAX
            && map::base(source.entries().len() as int, key, indexed) <= u32::MAX
            && #[trigger] reaches(source, fixed, 0, map::base(source.entries().len() as int, key, indexed) as u32, stop, at)
            && map::step(source, stop, at, fixed).is_err() ==>
                result == map::step(source, stop, at, fixed),
{
    assert forall|stop: int, at: u32| 0 <= stop < source.entries().len() <= u32::MAX
        && key as int + (if indexed { 4int } else { 0int }) <= u32::MAX
        && map::base(source.entries().len() as int, key, indexed) <= u32::MAX
        && #[trigger] reaches(source, fixed, 0, map::base(source.entries().len() as int, key, indexed) as u32, stop, at)
        && map::step(source, stop, at, fixed).is_err() implies
            result == map::step(source, stop, at, fixed) by {
        reached_fault(source, fixed, 0, map::base(source.entries().len() as int, key, indexed) as u32, stop, at);
    }
}
/// Two real primitive entries exercise the fixed-map root without premises.
pub(crate) fn fixed_map_witness()
    ensures true,
{
    use crate::encoding_product::Source;
    proof { reveal(crate::layout_head::head_spec); reveal_with_fuel(map::extent, 3); }
    proof { reveal_with_fuel(map::fold, 4); }
    let source = crate::encoding_map_fixed_inputs::FixedInputs { values: &[([0], [1]), ([255], [3])] };
    let first = source.value(0); let last = source.value(1);
    proof {
        crate::encoding_leaf::encoder_leaf_valid(&first, first.bytes(), 1);
        crate::encoding_leaf::encoder_leaf_valid(&last, last.bytes(), 1);
    }
    proof {
        assert(crate::search::lex_witness(source.entries()[0].key, source.entries()[1].key, 0));
        assert(crate::search::lex_less(source.entries()[0].key, source.entries()[1].key));
    }
    let measured = map::measure(&source, 1, false, Some(1));
    assert(measured == Ok(8));
    let mut bytes = [19u8; 10];
    map::encode(&source, &mut bytes[1..9], 1, Some(1), false);
    proof {
        reveal_with_fuel(crate::layout::descriptor_valid, 3);
        encoder_fixed_map_valid(&source, KeyLayout { width: 1, domain: KeyDomain::Raw },
            Layout::Leaf(crate::Leaf::Raw(1)), bytes@.subrange(1, 9), 1, 2);
    }
    assert(bytes[0] == 19 && bytes[9] == 19);
    proof { reveal_with_fuel(crate::layout::descriptor_valid, 3); }
    let empty = crate::encoding_map_fixed_inputs::FixedInputs { values: &[] };
    let mut zero = [19u8; 4];
    map::encode(&empty, &mut zero, 1, Some(1), true);
    proof { encoder_fixed_map_valid(&empty, KeyLayout { width: 1, domain: KeyDomain::Raw },
        Layout::Leaf(crate::Leaf::Raw(1)), zero@, 1, 2); }
}

/// Real text and terminal empty values exercise the indexed-map root.
pub(crate) fn indexed_map_witness()
    ensures true,
{
    use crate::encoding_product::Source;
    proof {
        reveal(crate::layout_head::head_spec); reveal_strlit(""); reveal_strlit("a");
        reveal_with_fuel(vstd::utf8::encode_utf8, 3); reveal_with_fuel(map::extent, 4);
    }
    let source = crate::encoding_map_text_inputs::TextInputs { values: &[([0], "a"), ([255], "")] };
    let first = source.value(0); let last = source.value(1);
    proof {
        crate::encoding_text::encoder_string_valid(&first, first.bytes(), 1);
        crate::encoding_text::encoder_string_valid(&last, last.bytes(), 1);
    }
    proof {
        assert(crate::search::lex_witness(source.entries()[0].key, source.entries()[1].key, 0));
        assert(crate::search::lex_less(source.entries()[0].key, source.entries()[1].key));
    }
    proof { reveal_with_fuel(map::fold, 4); }
    let measured = map::measure(&source, 1, true, None);
    assert(measured == Ok(15));
    let mut bytes = [19u8; 17];
    map::encode(&source, &mut bytes[1..16], 1, None, false);
    proof { reveal_with_fuel(crate::layout::descriptor_valid, 3); encoder_indexed_map_valid(&source, KeyLayout { width: 1, domain: KeyDomain::Raw }, Layout::Str,
        bytes@.subrange(1, 16), 2); }
    assert(bytes[0] == 19 && bytes[16] == 19);
    proof { reveal_with_fuel(crate::layout::descriptor_valid, 3); }
    let empty = crate::encoding_map_text_inputs::TextInputs { values: &[] };
    let mut zero = [19u8; 4];
    map::encode(&empty, &mut zero, 1, None, true);
    proof { encoder_indexed_map_valid(&empty, KeyLayout { width: 1, domain: KeyDomain::Raw }, Layout::Str, zero@, 2); }
}

/// Actual repeated measurements retain sorted, duplicate and descending results.
pub(crate) fn map_first_fault_witness()
    ensures true,
{
    proof { reveal_with_fuel(map::fold, 4); }
    let source = crate::encoding_map_fixed_inputs::FixedInputs { values: &[([255], [1]), ([0], [3])] };
    let first = map::measure(&source, 1, false, Some(1));
    let repeat = map::measure(&source, 1, false, Some(1));
    proof { encoder_map_first_fault(&source, 1, false, Some(1), first, repeat); }
    assert(first == Err(crate::Error::Fault { kind: crate::ErrorKind::UnsortedKeys, offset: u32::MAX }));
    let duplicate = crate::encoding_map_fixed_inputs::FixedInputs { values: &[([1], [0]), ([1], [255])] };
    let first = map::measure(&duplicate, 1, false, None);
    let repeat = map::measure(&duplicate, 1, false, None);
    proof { encoder_map_first_fault(&duplicate, 1, false, None, first, repeat); }
    assert(first == Err(crate::Error::Fault { kind: crate::ErrorKind::DuplicateKey, offset: u32::MAX }));
}

}
