//! Const field metadata shared by typed encoders and descriptor construction.
#![cfg_attr(verus_only, allow(missing_docs))]

use crate::encoding_product::Markers;
use crate::{FixedField, Layout, VariableField};
use vstd::prelude::*;

verus! {

/// Resolves a child width from its kind, rejecting an absent fixed width.
pub const fn field_width(fixed: bool, width: Option<u32>) -> (result: u32)
    requires fixed ==> width.is_some(),
    ensures result == if fixed { width.unwrap() } else { 0u32 },
    no_unwind
{
    if fixed {
        match width {
            Some(width) => width,
            None => panic!("fixed field has no width"),
        }
    } else { 0 }
}

/// A larger descriptor budget admits every layout admitted by a smaller one.
pub proof fn descriptor_budget(layout: Layout, small: nat, large: nat)
    requires crate::layout::descriptor_valid(layout, small), small <= large <= crate::MAX_DEPTH,
    ensures crate::layout::descriptor_valid(layout, large),
    decreases small, 1int, 0int,
{
    match layout.head() {
        crate::LayoutHead::Option { value } | crate::LayoutHead::Map { value, .. } => {
            descriptor_budget(value, (small - 1) as nat, (large - 1) as nat);
        }
        crate::LayoutHead::List { element } => {
            descriptor_budget(element, (small - 1) as nat, (large - 1) as nat);
        }
        crate::LayoutHead::Struct { fixed_bytes, fixed, variable } => {
            fields_budget(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0, small, large);
        }
        _ => {},
    }
}

proof fn fields_budget(fixed: Seq<crate::Field>, variable: Seq<crate::Field>, bytes: u32,
    i: int, j: int, offset: int, small: nat, large: nat)
    requires crate::layout::descriptor_fields(fixed, variable, bytes, i, j, offset, small),
        small <= large <= crate::MAX_DEPTH,
    ensures crate::layout::descriptor_fields(fixed, variable, bytes, i, j, offset, large),
    decreases small, 0int, fixed.len() + variable.len() - i - j,
{
    reveal_with_fuel(crate::layout::descriptor_fields, 2);
    if i < fixed.len() && (j == variable.len() || fixed[i].declaration <= variable[j].declaration) {
        descriptor_budget(fixed[i].layout, (small - 1) as nat, (large - 1) as nat);
        fields_budget(fixed, variable, bytes, i + 1, j,
            offset + crate::layout::width_spec(fixed[i].layout).unwrap(), small, large);
    } else if j < variable.len() {
        descriptor_budget(variable[j].layout, (small - 1) as nat, (large - 1) as nat);
        fields_budget(fixed, variable, bytes, i, j + 1, offset, small, large);
    }
}

/// Metadata selected by the same field types used by projected encoders.
pub struct Metadata<W: 'static> {
    /// Child metadata or two adjacent field groups.
    pub node: Node<W>,
    /// Total declaration count.
    pub count: usize,
    /// Fixed declaration count.
    pub fixed_count: usize,
    /// Total packed fixed width.
    pub fixed_bytes: u32,
    /// Sum of child minima, excluding the parent directory.
    pub minimum: u32,
    /// Conjunction of every child encoder and replay marker.
    pub markers: Markers,
}

/// Grouping that adds no descriptor node.
pub enum Node<W: 'static> {
    /// One child schema and its identity view.
    Field {
        /// Independent schema descriptor.
        layout: &'static Layout,
        /// Canonical wire descriptor selected by the same child type.
        wire: &'static W,
        /// Whether the child is in the fixed prefix.
        fixed: bool,
    },
    /// Adjacent groups, in declaration order.
    Pair(&'static Metadata<W>, &'static Metadata<W>),
}

/// One child's metadata before packed offsets are assigned.
pub ghost struct Declaration<W: 'static> {
    /// Independent child layout.
    pub layout: Layout,
    /// Canonical child identity view.
    pub wire: &'static W,
    /// Packed fixed storage selection.
    pub fixed: bool,
    /// Packed width, zero for variable storage.
    pub width: nat,
    /// Minimum child extent.
    pub minimum: nat,
    /// Complete child graph markers.
    pub markers: Markers,
}

/// Flattened declaration order; internal groups are absent from this view.
pub open spec fn view<W: 'static>(meta: &Metadata<W>) -> Seq<Declaration<W>>
    decreases meta,
{
    match meta.node {
        Node::Field { layout, wire, fixed } => seq![Declaration {
            layout: *layout, wire, fixed, width: meta.fixed_bytes as nat,
            minimum: meta.minimum as nat, markers: meta.markers,
        }],
        Node::Pair(left, right) => view(left) + view(right),
    }
}

/// Number of fixed fields in a declaration prefix.
pub open spec fn count<W: 'static>(items: Seq<Declaration<W>>) -> nat
    decreases items.len(),
{
    if items.len() == 0 { 0 }
    else { count(items.drop_last()) + if items.last().fixed { 1nat } else { 0nat } }
}

/// Packed width of a declaration prefix.
pub open spec fn width<W: 'static>(items: Seq<Declaration<W>>) -> nat
    decreases items.len(),
{
    if items.len() == 0 { 0 } else { width(items.drop_last()) + items.last().width }
}

/// Sum of child minima in a declaration prefix.
pub open spec fn minimum<W: 'static>(items: Seq<Declaration<W>>) -> nat
    decreases items.len(),
{
    if items.len() == 0 { 0 } else { minimum(items.drop_last()) + items.last().minimum }
}

/// Cached summaries agree with the finite balanced tree.
pub open spec fn valid<W: 'static>(meta: &Metadata<W>) -> bool
    decreases meta,
{
    meta.fixed_count <= meta.count
        && meta.count == view(meta).len() && meta.fixed_count == count(view(meta))
        && meta.fixed_bytes == width(view(meta)) && meta.minimum == minimum(view(meta))
        && match meta.node {
            Node::Field { fixed, .. } => fixed || meta.fixed_bytes == 0,
            Node::Pair(left, right) => valid(left) && valid(right)
                && meta.count == left.count + right.count
                && meta.fixed_count == left.fixed_count + right.fixed_count
                && meta.fixed_bytes == left.fixed_bytes + right.fixed_bytes
                && meta.minimum == left.minimum + right.minimum
                && meta.markers.encoder == (left.markers.encoder && right.markers.encoder)
                && meta.markers.replay == (left.markers.replay && right.markers.replay),
        }
}

/// Prefix arithmetic and one-field extension, shared by all const builders.
pub proof fn prefix<W: 'static>(items: Seq<Declaration<W>>, index: int)
    requires 0 <= index < items.len(),
    ensures count(items.take(index + 1)) == count(items.take(index))
                + if items[index].fixed { 1nat } else { 0nat },
        width(items.take(index + 1)) == width(items.take(index)) + items[index].width,
        count(items.take(index)) <= index,
        count(items.take(index + 1)) <= index + 1,
        count(items.take(index)) <= count(items),
        count(items.take(index + 1)) <= count(items),
        index - count(items.take(index)) <= items.len() - count(items),
        index + 1 - count(items.take(index + 1)) <= items.len() - count(items),
        width(items.take(index)) <= width(items),
        width(items.take(index + 1)) <= width(items),
{
    assert(items.take(index + 1).drop_last() =~= items.take(index));
    assert(items.take(index + 1).last() == items[index]);
    stats_bounds(items.take(index));
    stats_bounds(items.take(index + 1));
    stats_bounds(items.skip(index));
    stats_bounds(items.skip(index + 1));
    assert(items =~= items.take(index) + items.skip(index));
    assert(items =~= items.take(index + 1) + items.skip(index + 1));
    sums(items.take(index), items.skip(index));
    sums(items.take(index + 1), items.skip(index + 1));
}

/// Prefix counts fit their declaration count.
pub proof fn stats_bounds<W: 'static>(items: Seq<Declaration<W>>)
    ensures count(items) <= items.len(),
    decreases items.len(),
{
    if items.len() > 0 { stats_bounds(items.drop_last()); }
}

/// Adjacent groups have additive summaries.
pub proof fn sums<W: 'static>(left: Seq<Declaration<W>>, right: Seq<Declaration<W>>)
    ensures count(left + right) == count(left) + count(right),
        width(left + right) == width(left) + width(right),
        minimum(left + right) == minimum(left) + minimum(right),
    decreases right.len(),
{
    if right.len() > 0 {
        assert((left + right).drop_last() =~= left + right.drop_last());
        sums(left, right.drop_last());
    }
}

/// Constructs the metadata of one trusted typed child projection.
#[inline(always)]
pub const fn leaf<W: 'static>(layout: &'static Layout, wire: &'static W,
    fixed: bool, bytes: u32, minimum: u32, markers: Markers) -> (result: Metadata<W>)
    requires fixed || bytes == 0,
    ensures valid(&result), result.count == 1, result.fixed_count == (if fixed { 1int } else { 0int }),
        result.fixed_bytes == bytes, result.minimum == minimum,
        view(&result)[0] == (Declaration { layout: *layout, wire, fixed,
            width: bytes as nat, minimum: minimum as nat, markers }),
        result.markers == markers,
    no_unwind
{
    proof { reveal_with_fuel(count, 2); reveal_with_fuel(width, 2); reveal_with_fuel(self::minimum, 2); }
    Metadata { node: Node::Field { layout, wire, fixed }, count: 1,
        fixed_count: if fixed { 1 } else { 0 }, fixed_bytes: bytes, minimum, markers }
}

/// Joins adjacent groups without adding wire depth or changing declaration order.
pub const fn pair<W: 'static>(left: &'static Metadata<W>, right: &'static Metadata<W>) -> (result: Metadata<W>)
    requires valid(left), valid(right), left.count + right.count <= usize::MAX,
        left.fixed_bytes + right.fixed_bytes <= u32::MAX,
        left.minimum + right.minimum <= u32::MAX,
    ensures valid(&result), view(&result) == view(left) + view(right),
    no_unwind
{
    proof { sums(view(left), view(right)); }
    assert!(left.count <= usize::MAX - right.count, "schema field count exceeds usize");
    assert!(left.fixed_bytes <= u32::MAX - right.fixed_bytes, "schema wire width exceeds u32");
    assert!(left.minimum <= u32::MAX - right.minimum, "schema minimum exceeds u32");
    Metadata { node: Node::Pair(left, right), count: left.count + right.count,
        fixed_count: left.fixed_count + right.fixed_count,
        fixed_bytes: left.fixed_bytes + right.fixed_bytes, minimum: left.minimum + right.minimum,
        markers: crate::encoding_product::join(left.markers, right.markers) }
}

/// Selects a field in declaration order without exposing internal groups.
pub const fn field<W: 'static>(meta: &Metadata<W>, index: usize) -> (result: &Metadata<W>)
    requires valid(meta), index < meta.count,
    ensures valid(result), result.count == 1, view(result)[0] == view(meta)[index as int],
        result.fixed_bytes == view(meta)[index as int].width,
        !view(meta)[index as int].fixed ==> result.fixed_bytes == 0,
        match result.node { Node::Field { layout, wire, fixed } =>
            *layout == view(meta)[index as int].layout && wire == view(meta)[index as int].wire
                && fixed == view(meta)[index as int].fixed,
            _ => false },
    decreases meta,
    no_unwind
{
    proof { reveal_with_fuel(valid, 2); reveal_with_fuel(view, 2); }
    match meta.node {
        Node::Field { .. } => {
            assert!(index == 0, "field ordinal outside representation");
            meta
        }
        Node::Pair(left, right) => {
            if index < left.count {
                let result = field(left, index);
                assert(view(meta)[index as int] == view(left)[index as int]);
                result
            } else {
                let result = field(right, index - left.count);
                assert(view(meta)[index as int] == view(right)[index as int - left.count]);
                result
            }
        }
    }
}

/// Canonical identity children in declaration order, from the shared type.
pub const fn wire<W: 'static, const N: usize>(meta: &Metadata<W>, fallback: &'static W) -> (result: [&'static W; N])
    requires valid(meta), N == meta.count,
    ensures forall|i: int| 0 <= i < N ==> result@[i] == view(meta)[i].wire,
{
    assert!(N == meta.count, "wire field count mismatch");
    let mut result = [fallback; N];
    let mut index = 0;
    while index < N
        invariant valid(meta), N == meta.count, index <= N,
            forall|i: int| 0 <= i < index ==> result@[i] == view(meta)[i].wire,
        decreases N - index,
    {
        if let Node::Field { wire, .. } = field(meta, index).node { result[index] = wire; }
        index += 1;
    }
    result
}

/// Fixed metadata agrees with each declaration and its exact prefix width.
pub open spec fn fixed_matches<W: 'static>(meta: &Metadata<W>, result: Seq<FixedField>, end: int) -> bool {
    forall|i: int| 0 <= i < end && view(meta)[i].fixed ==> {
        let slot = count(view(meta).take(i)) as int;
        &&& 0 <= slot < result.len()
        &&& result[slot].declaration == i
        &&& result[slot].offset == width(view(meta).take(i))
        &&& *result[slot].layout == view(meta)[i].layout
    }
}

/// Each initialized fixed slot points back to its unique declaration.
pub open spec fn fixed_inverse<W: 'static>(meta: &Metadata<W>, result: Seq<FixedField>, slots: int) -> bool {
    forall|slot: int| 0 <= slot < slots ==> {
        let i = #[trigger] result[slot].declaration as int;
        &&& i < meta.count
        &&& view(meta)[i].fixed
        &&& count(view(meta).take(i)) == slot
    }
}

/// Fixed metadata in packed wire order; unused slots support generic constants.
pub const fn fixed<W: 'static, const N: usize>(meta: &Metadata<W>) -> (result: [FixedField; N])
    requires valid(meta), meta.count <= u16::MAX, N >= meta.fixed_count,
    ensures fixed_matches(meta, result@, meta.count as int),
        fixed_inverse(meta, result@, meta.fixed_count as int),
{
    hide(valid); hide(view); hide(count); hide(width); hide(minimum);
    assert!(N >= meta.fixed_count, "fixed field count mismatch");
    let mut result = [FixedField { declaration: 0, offset: 0, layout: &Layout::Str }; N];
    let mut index = 0;
    let mut slot = 0;
    let mut offset = 0;
    proof {
        reveal(valid); reveal_with_fuel(count, 1); reveal_with_fuel(width, 1);
        assert(meta.count == view(meta).len());
        assert(meta.fixed_count == count(view(meta)));
        assert(meta.fixed_bytes == width(view(meta)));
        assert(count(view(meta).take(0)) == 0);
        assert(width(view(meta).take(0)) == 0);
    }
    while index < meta.count
        invariant valid(meta), meta.count <= u16::MAX, N >= meta.fixed_count,
            meta.count == view(meta).len(), meta.fixed_count == count(view(meta)),
            meta.fixed_bytes == width(view(meta)),
            index <= meta.count, slot == count(view(meta).take(index as int)),
            offset == width(view(meta).take(index as int)), fixed_matches(meta, result@, index as int),
            fixed_inverse(meta, result@, slot as int),
        decreases meta.count - index,
    {
        proof { prefix(view(meta), index as int); }
        let value = field(meta, index);
        if let Node::Field { layout, fixed: true, .. } = value.node {
            proof {
                assert forall|i: int| 0 <= i < index && view(meta)[i].fixed implies
                    count(view(meta).take(i)) < slot by {
                    prefix(view(meta).take(index as int), i);
                    assert(view(meta).take(index as int).take(i) =~= view(meta).take(i));
                    assert(view(meta).take(index as int)[i] == view(meta)[i]);
                }
            }
            result[slot] = FixedField { declaration: index as u16, offset, layout };
            slot += 1;
            offset += value.fixed_bytes;
        }
        index += 1;
    }
    proof { assert(view(meta).take(index as int) =~= view(meta)); }
    result
}

/// Variable metadata agrees with declarations after excluding fixed fields.
pub open spec fn variable_matches<W: 'static>(meta: &Metadata<W>, result: Seq<VariableField>, end: int) -> bool {
    forall|i: int| 0 <= i < end && !view(meta)[i].fixed ==> {
        let slot = i - count(view(meta).take(i));
        &&& 0 <= slot < result.len()
        &&& result[slot].declaration == i
        &&& *result[slot].layout == view(meta)[i].layout
    }
}

/// Each initialized variable slot points back to its unique declaration.
pub open spec fn variable_inverse<W: 'static>(meta: &Metadata<W>, result: Seq<VariableField>, slots: int) -> bool {
    forall|slot: int| 0 <= slot < slots ==> {
        let i = #[trigger] result[slot].declaration as int;
        &&& i < meta.count
        &&& !view(meta)[i].fixed
        &&& i - count(view(meta).take(i)) == slot
    }
}

/// Variable metadata in payload order, from the shared field representation.
pub const fn variable<W: 'static, const N: usize>(meta: &Metadata<W>) -> (result: [VariableField; N])
    requires valid(meta), meta.count <= u16::MAX, N >= meta.count - meta.fixed_count,
    ensures variable_matches(meta, result@, meta.count as int),
        variable_inverse(meta, result@, (meta.count - meta.fixed_count) as int),
{
    hide(valid); hide(view); hide(count); hide(width); hide(minimum);
    proof { assert(meta.fixed_count <= meta.count) by { reveal(valid); } }
    assert!(N >= meta.count - meta.fixed_count, "variable field count mismatch");
    let mut result = [VariableField { declaration: 0, layout: &Layout::Str }; N];
    let mut index = 0;
    let mut slot = 0;
    proof {
        reveal(valid); reveal_with_fuel(count, 1); reveal_with_fuel(width, 1);
        assert(meta.count == view(meta).len());
        assert(meta.fixed_count == count(view(meta)));
        assert(count(view(meta).take(0)) == 0);
    }
    while index < meta.count
        invariant valid(meta), meta.count <= u16::MAX, N >= meta.count - meta.fixed_count,
            meta.count == view(meta).len(), meta.fixed_count == count(view(meta)),
            index <= meta.count, slot == index - count(view(meta).take(index as int)),
            variable_matches(meta, result@, index as int),
            variable_inverse(meta, result@, slot as int),
        decreases meta.count - index,
    {
        proof { prefix(view(meta), index as int); }
        if let Node::Field { layout, fixed: false, .. } = field(meta, index).node {
            proof {
                assert forall|i: int| 0 <= i < index && !view(meta)[i].fixed implies
                    i - count(view(meta).take(i)) < slot by {
                    prefix(view(meta).take(index as int), i);
                    assert(view(meta).take(index as int).take(i) =~= view(meta).take(i));
                    assert(view(meta).take(index as int)[i] == view(meta)[i]);
                }
            }
            result[slot] = VariableField { declaration: index as u16, layout };
            slot += 1;
        }
        index += 1;
    }
    proof { assert(view(meta).take(index as int) =~= view(meta)); }
    result
}

/// Flat struct header and declaration ordinals, from the same field types.
pub const fn header<W: 'static, const N: usize>(meta: &Metadata<W>) -> (result: [u32; N])
    requires valid(meta), meta.count <= u16::MAX, N == 8 + 2 * meta.count,
    ensures result@[0] == 9, result@[1] == meta.fixed_bytes,
        result@[3] == meta.fixed_count, result@[4] == meta.count - meta.fixed_count,
        forall|i: int| 0 <= i < meta.count ==> result@[8 + 2*i] == i
            && (view(meta)[i].fixed ==> result@[9 + 2*i] == width(view(meta).take(i))),
{
    assert!(N == 8 + 2 * meta.count, "struct header length mismatch");
    let mut result = [0; N];
    result[0] = 9; result[1] = meta.fixed_bytes;
    result[3] = meta.fixed_count as u32; result[4] = (meta.count - meta.fixed_count) as u32;
    let mut index = 0;
    let mut offset = 0;
    while index < meta.count
        invariant valid(meta), meta.count <= u16::MAX, N == 8 + 2 * meta.count,
            index <= meta.count, offset == width(view(meta).take(index as int)),
            result@[0] == 9, result@[1] == meta.fixed_bytes,
            result@[3] == meta.fixed_count, result@[4] == meta.count - meta.fixed_count,
            forall|i: int| 0 <= i < index ==> result@[8 + 2*i] == i
                && (view(meta)[i].fixed ==> result@[9 + 2*i] == width(view(meta).take(i))),
        decreases meta.count - index,
    {
        proof { prefix(view(meta), index as int); }
        let value = field(meta, index);
        result[8 + 2 * index] = index as u32;
        if let Node::Field { fixed: true, .. } = value.node {
            result[9 + 2 * index] = offset;
            offset += value.fixed_bytes;
        }
        index += 1;
    }
    result
}

/// Native fixed boundaries are exact prefix widths of the shared representation.
pub const fn offsets<W: 'static, const N: usize>(meta: &Metadata<W>) -> (result: [u32; N])
    requires valid(meta), N == meta.count + 1, meta.count <= u16::MAX,
        meta.fixed_count == meta.count,
        forall|i: int| 0 <= i < meta.count ==> #[trigger] view(meta)[i].fixed && view(meta)[i].width > 0,
    ensures forall|i: int| 0 <= i < N ==> result@[i] == width(view(meta).take(i)),
{
    assert!(N == meta.count + 1, "fixed boundary count mismatch");
    assert!(meta.fixed_count == meta.count, "Fixed requires fixed-kind fields");
    let mut result = [0; N];
    let mut index = 0;
    while index < meta.count
        invariant valid(meta), N == meta.count + 1, meta.count <= u16::MAX,
            meta.fixed_count == meta.count, index <= meta.count,
            forall|i: int| 0 <= i < meta.count ==> #[trigger] view(meta)[i].fixed && view(meta)[i].width > 0,
            forall|i: int| 0 <= i <= index ==> result@[i] == width(view(meta).take(i)),
        decreases meta.count - index,
    {
        proof { prefix(view(meta), index as int); }
        let value = field(meta, index);
        assert!(value.fixed_bytes > 0, "Fixed field must have positive width");
        result[index + 1] = result[index] + value.fixed_bytes;
        index += 1;
    }
    result
}

/// Trusted leaf projections agree on type, storage class and fixed extent.
/// Values of equally typed fields may differ; field selection is not asserted.
pub open spec fn projected<W: 'static>(meta: &Metadata<W>, items: Seq<crate::encoding_product::Item>) -> bool {
    items.len() == meta.count && (forall|i: int| 0 <= i < items.len() ==> {
        let child = #[trigger] view(meta)[i];
        &&& items[i].layout == child.layout
        &&& items[i].fixed == child.fixed
        &&& child.fixed ==> items[i].bytes.len() == child.width
    })
}

/// Every variable metadata leaf contributes zero packed fixed width.
pub proof fn variable_widths<W: 'static>(meta: &Metadata<W>)
    requires valid(meta),
    ensures forall|i: int| 0 <= i < meta.count && !view(meta)[i].fixed ==> view(meta)[i].width == 0,
    decreases meta,
{
    reveal_with_fuel(valid, 2); reveal_with_fuel(view, 2);
    match meta.node {
        Node::Field { .. } => {},
        Node::Pair(left, right) => { variable_widths(left); variable_widths(right); },
    }
}

/// Type-directed metadata and measured values have identical packed summaries.
pub proof fn projected_sums<W: 'static>(meta: &Metadata<W>, items: Seq<crate::encoding_product::Item>, k: int)
    requires valid(meta), projected(meta, items), 0 <= k <= items.len(),
    ensures width(view(meta).take(k)) == crate::encoding_struct::fixed(items.take(k)),
        count(view(meta).take(k)) == k - crate::encoding_struct::variables(items.take(k)),
    decreases k,
{
    if k > 0 {
        projected_sums(meta, items, k - 1);
        variable_widths(meta);
        prefix(view(meta), k - 1);
        crate::encoding_struct::prefix(items, k - 1);
    }
}

/// Const builder outputs agree with every matching typed byte projection.
pub proof fn descriptor_agrees<W: 'static>(meta: &Metadata<W>, items: Seq<crate::encoding_product::Item>,
    fixed: &'static [FixedField], variable: &'static [VariableField])
    requires valid(meta), meta.count <= u16::MAX, projected(meta, items),
        fixed.len() == meta.fixed_count, variable.len() == meta.count - meta.fixed_count,
        fixed_matches(meta, fixed@, meta.count as int), fixed_inverse(meta, fixed@, meta.fixed_count as int),
        variable_matches(meta, variable@, meta.count as int),
        variable_inverse(meta, variable@, (meta.count - meta.fixed_count) as int),
        forall|i: int| 0 <= i < meta.count && view(meta)[i].fixed ==>
            crate::layout::width_spec(view(meta)[i].layout) == Some(view(meta)[i].width as u32)
                && view(meta)[i].width <= u32::MAX,
    ensures crate::encoding_struct::agrees(items, Layout::Struct { fixed_bytes: meta.fixed_bytes, fixed, variable }),
{
    hide(view); hide(valid); hide(width); hide(count);
    hide(crate::encoding_struct::fixed); hide(crate::encoding_struct::variables);
    assert(meta.count == view(meta).len() && meta.fixed_count == count(view(meta))
        && meta.fixed_bytes == width(view(meta))) by { reveal(valid); }
    projected_sums(meta, items, items.len() as int);
    assert(view(meta).take(items.len() as int) =~= view(meta));
    assert(items.take(items.len() as int) =~= items);
    reveal(crate::layout_head::head_spec);
    assert forall|slot: int| 0 <= slot < fixed.len() implies {
        let f = #[trigger] fixed@[slot];
        let d = f.declaration as int;
        &&& d < items.len() && items[d].fixed
        &&& slot == d - crate::encoding_struct::variables(items.take(d))
        &&& f.offset == crate::encoding_struct::fixed(items.take(d))
        &&& *f.layout == items[d].layout
        &&& crate::layout::width_spec(*f.layout) == Some(items[d].bytes.len() as u32)
        &&& items[d].bytes.len() <= u32::MAX
    } by {
        let d = fixed@[slot].declaration as int;
        projected_sums(meta, items, d);
    }
    assert forall|slot: int| 0 <= slot < variable.len() implies {
        let f = #[trigger] variable@[slot];
        let d = f.declaration as int;
        &&& d < items.len() && !items[d].fixed
        &&& slot == crate::encoding_struct::variables(items.take(d))
        &&& *f.layout == items[d].layout
    } by {
        let d = variable@[slot].declaration as int;
        projected_sums(meta, items, d);
    }
}
/// Child descriptor facts supplied by trusted library type metadata.
pub open spec fn child_contract<W: 'static>(meta: &Metadata<W>, budget: nat) -> bool {
    forall|i: int| 0 <= i < meta.count ==> {
        let child = #[trigger] view(meta)[i];
        &&& crate::layout::descriptor_valid(child.layout, (budget - 1) as nat)
        &&& if child.fixed { 0 < child.width <= u32::MAX
            && crate::layout::width_spec(child.layout) == Some(child.width as u32) }
            else { child.width == 0 && crate::layout::width_spec(child.layout).is_none() }
    }
}

proof fn merge_fields<W: 'static>(meta: &Metadata<W>, fixed: &'static [FixedField],
    variable: &'static [VariableField], budget: nat, k: int)
    requires valid(meta), meta.count <= u16::MAX, 1 < budget <= crate::MAX_DEPTH,
        child_contract(meta, budget), fixed.len() == meta.fixed_count,
        variable.len() == meta.count - meta.fixed_count,
        fixed_matches(meta, fixed@, meta.count as int), fixed_inverse(meta, fixed@, meta.fixed_count as int),
        variable_matches(meta, variable@, meta.count as int),
        variable_inverse(meta, variable@, (meta.count - meta.fixed_count) as int),
        0 <= k <= meta.count,
    ensures crate::layout::descriptor_fields(crate::layout_head::Fields::Fixed(fixed).view(),
        crate::layout_head::Fields::Variable(variable).view(), meta.fixed_bytes,
        count(view(meta).take(k)) as int, k - count(view(meta).take(k)), width(view(meta).take(k)) as int, budget),
    decreases meta.count - k,
{
    hide(valid); hide(view); hide(count); hide(width); hide(minimum);
    hide(crate::layout::descriptor_valid); hide(crate::layout::width_spec);
    hide(crate::layout::descriptor_fields);
    assert(meta.count == view(meta).len() && meta.fixed_count == count(view(meta))
        && meta.fixed_bytes == width(view(meta))) by { reveal(valid); }
    let items = view(meta);
    let i = count(items.take(k)) as int;
    let j = k - i;
    if k < meta.count {
        prefix(items, k);
        merge_fields(meta, fixed, variable, budget, k + 1);
        if items[k].fixed {
            assert(i < fixed.len());
            assert(fixed@[i].declaration == k && fixed@[i].offset == width(items.take(k))
                && *fixed@[i].layout == items[k].layout);
            assert(crate::layout::descriptor_valid(*fixed@[i].layout, (budget - 1) as nat));
            assert(crate::layout::width_spec(*fixed@[i].layout) == Some(items[k].width as u32));
            if j < variable.len() {
                let d = variable@[j].declaration as int;
                assert(!items[d].fixed && d - count(items.take(d)) == j);
                if d < k {
                    prefix(items.take(k), d);
                    assert(items.take(k).take(d) =~= items.take(d));
                    assert(items.take(k)[d] == items[d]);
                    assert(false);
                }
                assert(k < d);
            }
        } else {
            assert(j < variable.len());
            assert(variable@[j].declaration == k && *variable@[j].layout == items[k].layout);
            assert(crate::layout::descriptor_valid(*variable@[j].layout, (budget - 1) as nat));
            assert(crate::layout::width_spec(*variable@[j].layout).is_none());
            if i < fixed.len() {
                let d = fixed@[i].declaration as int;
                assert(items[d].fixed && count(items.take(d)) == i);
                if d < k {
                    prefix(items.take(k), d);
                    assert(items.take(k).take(d) =~= items.take(d));
                    assert(items.take(k)[d] == items[d]);
                    assert(false);
                }
                assert(k < d);
            }
        }
    } else {
        assert(items.take(k) =~= items);
    }
    let f = crate::layout_head::Fields::Fixed(fixed).view();
    let v = crate::layout_head::Fields::Variable(variable).view();
    assert(0 <= i <= f.len() && 0 <= j <= v.len());
    crate::layout::descriptor_fields_step(f, v, meta.fixed_bytes, i, j, width(items.take(k)) as int, budget);
    if k < meta.count {
        if items[k].fixed {
            assert(i < f.len() && (j == v.len() || f[i].declaration <= v[j].declaration));
            assert(f[i].declaration == i + j && f[i].offset == width(items.take(k)));
            assert(crate::layout::descriptor_fields(f, v, meta.fixed_bytes,
                i + 1, j, width(items.take(k)) + crate::layout::width_spec(f[i].layout).unwrap(), budget));
            assert(width(items.take(k)) + crate::layout::width_spec(f[i].layout).unwrap() <= u32::MAX);
            assert(crate::layout::width_spec(f[i].layout).unwrap() > 0);
            assert(crate::layout::descriptor_valid(f[i].layout, (budget - 1) as nat));
            assert(0 < budget);
            assert(!(i == f.len() && j == v.len()));
            assert(match crate::layout::width_spec(f[i].layout) {
                Some(w) => w > 0 && f[i].declaration == i + j
                    && f[i].offset == width(items.take(k))
                    && width(items.take(k)) + w <= u32::MAX
                    && crate::layout::descriptor_valid(f[i].layout, (budget - 1) as nat)
                    && crate::layout::descriptor_fields(f, v, meta.fixed_bytes, i + 1, j,
                        width(items.take(k)) + w, budget),
                None => false,
            });
            assert(crate::layout::descriptor_fields(f, v, meta.fixed_bytes, i, j, width(items.take(k)) as int, budget)) by {
                reveal_with_fuel(crate::layout::descriptor_fields, 3);
            }
        } else {
            assert(!(i < f.len() && (j == v.len() || f[i].declaration <= v[j].declaration)));
            assert(crate::layout::descriptor_fields(f, v, meta.fixed_bytes,
                i, j + 1, width(items.take(k)) as int, budget));
            assert(v[j].declaration == i + j);
            assert(crate::layout::width_spec(v[j].layout).is_none());
            assert(crate::layout::descriptor_valid(v[j].layout, (budget - 1) as nat));
            assert(0 < budget);
            assert(!(i == f.len() && j == v.len()));
            assert(v[j].declaration == i+j && crate::layout::width_spec(v[j].layout).is_none()
                && crate::layout::descriptor_valid(v[j].layout, (budget - 1) as nat)
                && crate::layout::descriptor_fields(f, v, meta.fixed_bytes, i, j+1, width(items.take(k)) as int, budget));
            assert(crate::layout::descriptor_fields(f, v, meta.fixed_bytes, i, j, width(items.take(k)) as int, budget)) by {
                reveal_with_fuel(crate::layout::descriptor_fields, 3);
            }
        }
    } else {
        assert(i == f.len() && j == v.len() && width(items.take(k)) == meta.fixed_bytes);
    }
    reveal_with_fuel(crate::layout::descriptor_fields, 2);
}

/// Shared field types determine packed descriptors and identity child order.
///
/// `fixed`, `variable` and `wire` are the outputs of the verified const builders.
/// Child facts are the existing Schema metadata contract at the typed projection
/// boundary; internal product grouping never consumes descriptor depth.
#[verifier::rlimit(240)]
pub proof fn encoder_descriptor_coherent<W: 'static>(meta: &Metadata<W>, fixed: &'static [FixedField],
    variable: &'static [VariableField], wire: Seq<&'static W>, budget: nat)
    requires valid(meta), 0 < meta.count <= u16::MAX, 1 < budget <= crate::MAX_DEPTH,
        child_contract(meta, budget), fixed.len() == meta.fixed_count,
        variable.len() == meta.count - meta.fixed_count,
        fixed_matches(meta, fixed@, meta.count as int), fixed_inverse(meta, fixed@, meta.fixed_count as int),
        variable_matches(meta, variable@, meta.count as int),
        variable_inverse(meta, variable@, (meta.count - meta.fixed_count) as int),
        wire.len() == meta.count,
        forall|i: int| 0 <= i < wire.len() ==> #[trigger] wire[i] == view(meta)[i].wire,
    ensures crate::layout::descriptor_valid(Layout::Struct { fixed_bytes: meta.fixed_bytes, fixed, variable }, budget),
        forall|i: int| 0 <= i < meta.count ==> #[trigger] wire[i] == view(meta)[i].wire,
        forall|items: Seq<crate::encoding_product::Item>| projected(meta, items) ==>
            crate::encoding_struct::agrees(items, Layout::Struct { fixed_bytes: meta.fixed_bytes, fixed, variable }),
{
    hide(projected); hide(crate::encoding_struct::agrees);
    assert forall|items: Seq<crate::encoding_product::Item>| projected(meta, items) implies
        crate::encoding_struct::agrees(items, Layout::Struct { fixed_bytes: meta.fixed_bytes, fixed, variable }) by {
        descriptor_agrees(meta, items, fixed, variable);
    }
    reveal(crate::layout_head::head_spec);
    merge_fields(meta, fixed, variable, budget, 0);
    if variable.len() == 0 {
        prefix(view(meta), 0);
        assert(view(meta)[0].fixed);
        assert(0 < view(meta)[0].width);
        assert(width(view(meta).take(1)) == view(meta)[0].width);
        assert(0 < meta.fixed_bytes);
    }
}

/// A fixed field and empty variable field instantiate all descriptor premises.
#[cfg(verus_only)]
pub(crate) fn descriptor_witness()
    ensures true,
{
    let ghost left = Metadata::<u8> { node: Node::Field {
        layout: &Layout::Leaf(crate::Leaf::Unsigned(4)), wire: &0u8, fixed: true },
        count: 1, fixed_count: 1, fixed_bytes: 4, minimum: 4,
        markers: Markers { encoder: true, replay: true } };
    let ghost right = Metadata::<u8> { node: Node::Field { layout: &Layout::Str, wire: &0u8, fixed: false },
        count: 1, fixed_count: 0, fixed_bytes: 0, minimum: 0,
        markers: Markers { encoder: true, replay: true } };
    let ghost meta = Metadata::<u8> { node: Node::Pair(&left, &right),
        count: 2, fixed_count: 1, fixed_bytes: 4, minimum: 4,
        markers: Markers { encoder: true, replay: true } };
    let ghost f = [FixedField { declaration: 0, offset: 0, layout: &Layout::Leaf(crate::Leaf::Unsigned(4)) }];
    let ghost v = [VariableField { declaration: 1, layout: &Layout::Str }];
    proof {
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(view, 3); reveal_with_fuel(valid, 3);
        reveal_with_fuel(count, 3); reveal_with_fuel(width, 3); reveal_with_fuel(minimum, 3);
        assert forall|i: int| 0 <= i < 2 implies view(&meta)[i].wire == &0u8 by { }
        encoder_descriptor_coherent(&meta, &f, &v, seq![&0u8, &0u8], 2);
        let inputs = seq![crate::encoding_product::Item { bytes: seq![0u8, 0, 0, 0], fixed: true,
            layout: Layout::Leaf(crate::Leaf::Unsigned(4)) },
            crate::encoding_product::Item { bytes: Seq::empty(), fixed: false, layout: Layout::Str }];
        assert(projected(&meta, inputs));
        assert(crate::encoding_struct::agrees(inputs, Layout::Struct { fixed_bytes: 4, fixed: &f, variable: &v }));
    }
}

/// An opaque child keeps every enclosing balanced product on the checked route.
///
/// This theorem reads only type metadata. Projected codec callbacks are not
/// invoked while selecting a route.
pub proof fn encoder_opacity_preserved<W: 'static>(meta: &Metadata<W>, index: int)
    requires valid(meta), 0 <= index < meta.count,
        !(view(meta)[index].markers.encoder && view(meta)[index].markers.replay),
    ensures !(meta.markers.encoder && meta.markers.replay),
    decreases meta,
{
    reveal_with_fuel(valid, 2);
    reveal_with_fuel(view, 2);
    match meta.node {
        Node::Field { .. } => {},
        Node::Pair(left, right) => {
            if index < left.count { encoder_opacity_preserved(left, index); }
            else { encoder_opacity_preserved(right, index - left.count); }
        }
    }
}

/// Concrete opaque child, true sibling and enclosing graph need no codec call.
#[cfg(verus_only)]
pub(crate) fn opacity_witness()
    ensures true,
{
    let ghost left = Metadata::<u8> { node: Node::Field { layout: &Layout::Str, wire: &0u8, fixed: false },
        count: 1, fixed_count: 0, fixed_bytes: 0, minimum: 0,
        markers: Markers { encoder: false, replay: true } };
    let ghost right = Metadata::<u8> { node: Node::Field { layout: &Layout::Str, wire: &0u8, fixed: false },
        count: 1, fixed_count: 0, fixed_bytes: 0, minimum: 0,
        markers: Markers { encoder: true, replay: true } };
    let ghost parent = Metadata::<u8> { node: Node::Pair(&left, &right),
        count: 2, fixed_count: 0, fixed_bytes: 0, minimum: 0,
        markers: Markers { encoder: false, replay: true } };
    proof {
        reveal_with_fuel(view, 3); reveal_with_fuel(valid, 3);
        reveal_with_fuel(count, 3); reveal_with_fuel(width, 3); reveal_with_fuel(minimum, 3);
        encoder_opacity_preserved(&parent, 0);
    }
}

}
