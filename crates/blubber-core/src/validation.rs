//! Complete schema validation with a bounded stack of offset-only frames.

use crate::layout::fixed_width;
#[cfg(verus_only)]
use crate::layout::{descriptor_fields, descriptor_valid, width_spec};
use crate::list_layout::list_element_at;
use crate::map_layout::map_value_at;
use crate::struct_layout::struct_variable_field_at;
use crate::words::add_bounded;
use crate::LayoutHead;
use crate::{
    check_layout, fixed_region, partition_list, partition_map, partition_option, partition_struct,
    validate_fixed, validate_utf8, Error, ErrorKind, Layout, ListPartition, MapPartition, Result,
    Span, StructPartition, MAX_DEPTH,
};
#[cfg(verus_only)]
use crate::{leaves, list_layout, map_layout, search, struct_layout, utf8, words};
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy)]
pub(crate) enum Parts {
    Done,
    Struct(StructPartition),
    List(ListPartition),
    Map(MapPartition),
    Option(Option<Span>),
}

#[derive(Clone, Copy)]
struct Frame {
    layout: Layout,
    region: Span,
    parts: Option<Parts>,
    next: u32,
}

/// Local shape and semantic domains, independent of the traversal algorithm.
pub open spec fn local_valid(bytes: Seq<u8>, layout: Layout, semantic: bool) -> bool {
    descriptor_valid(layout, MAX_DEPTH as nat) && match layout.head() {
        LayoutHead::Leaf(_) => bytes.len() <= u32::MAX
            && width_spec(layout) == Some(bytes.len() as u32)
            && (!semantic || leaves::fixed_value_valid(bytes, layout, MAX_DEPTH as nat)),
        LayoutHead::Str => bytes.len() <= u32::MAX && (!semantic || utf8::utf8_valid(bytes, 0)),
        LayoutHead::Struct { .. } => struct_layout::struct_shape(bytes, layout),
        LayoutHead::Option { .. } => leaves::option_shape(bytes, layout),
        LayoutHead::List { .. } => list_layout::list_shape(bytes, layout),
        LayoutHead::Map { .. } => map_layout::map_shape(bytes, layout)
            && (!semantic || (search::keys_canonical(bytes, layout, words::le_word(bytes, 0))
                && search::keys_ordered(bytes, layout, words::le_word(bytes, 0)))),
    }
}

/// Number of immediate data children, including fixed struct fields.
pub open spec fn child_count(bytes: Seq<u8>, layout: Layout) -> int {
    match layout.head() {
        LayoutHead::Struct { fixed, variable, .. } => fixed.len() + variable.len(),
        LayoutHead::Option { .. } => if bytes.len() > 0 && bytes[0] == 1 { 1int } else { 0int },
        LayoutHead::List { .. } | LayoutHead::Map { .. } => words::le_word(bytes, 0),
        _ => 0,
    }
}

/// Child schema in wire order: fixed fields, then variable fields.
pub open spec fn child_layout(layout: Layout, index: int) -> Layout {
    match layout.head() {
        LayoutHead::Struct { fixed, variable, .. } => if index < fixed.len() {
            fixed.view()[index].layout
        } else { variable.view()[index - fixed.len()].layout },
        LayoutHead::Option { value } | LayoutHead::Map { value, .. } => value,
        LayoutHead::List { element } => element,
        _ => Layout::Str,
    }
}

/// Declarative child interval in its immediate parent's exact slice.
pub open spec fn child_span(bytes: Seq<u8>, layout: Layout, index: int) -> Span {
    match layout.head() {
        LayoutHead::Struct { fixed, .. } => if index < fixed.len() {
            Span { start: fixed.view()[index].offset,
                end: (fixed.view()[index].offset + width_spec(fixed.view()[index].layout).unwrap()) as u32 }
        } else { struct_layout::variable_span(bytes, layout, (index - fixed.len()) as u16) },
        LayoutHead::Option { .. } => Span { start: 1, end: bytes.len() as u32 },
        LayoutHead::List { .. } => list_layout::element_span(bytes, layout, index as u32),
        LayoutHead::Map { .. } => map_layout::value_span(bytes, layout, index as u32),
        _ => Span { start: 0, end: 0 },
    }
}

/// One child satisfies its schema in the independently specified exact interval.
pub open spec fn child_valid(
    bytes: Seq<u8>, layout: Layout, index: int, budget: nat, semantic: bool,
) -> bool
    decreases budget, 1int, 0int,
{
    let span = child_span(bytes, layout, index);
    schema_valid(bytes.subrange(span.start as int, span.end as int),
        child_layout(layout, index), budget, semantic)
}

/// Exposes the exact byte interval of one independent child-validity query.
pub(crate) proof fn child_definition(bytes: Seq<u8>, layout: Layout, index: int, budget: nat, semantic: bool)
    ensures child_valid(bytes, layout, index, budget, semantic) == schema_valid(
        bytes.subrange(child_span(bytes, layout, index).start as int, child_span(bytes, layout, index).end as int),
        child_layout(layout, index), budget, semantic),
{
    reveal_with_fuel(child_valid, 2);
}

/// Every child in a suffix satisfies the independent recursive schema relation.
pub open spec fn children_valid(
    bytes: Seq<u8>, layout: Layout, next: int, budget: nat, semantic: bool,
) -> bool
    decreases budget, 2int, child_count(bytes, layout) - next,
{
    0 <= next <= child_count(bytes, layout) && (next == child_count(bytes, layout)
        || (child_valid(bytes, layout, next, budget, semantic)
            && children_valid(bytes, layout, next + 1, budget, semantic)))
}

pub(crate) proof fn children_split(bytes: Seq<u8>, layout: Layout, next: int, budget: nat, semantic: bool)
    requires 0 <= next < child_count(bytes, layout),
    ensures children_valid(bytes, layout, next, budget, semantic)
        == (child_valid(bytes, layout, next, budget, semantic)
            && children_valid(bytes, layout, next + 1, budget, semantic)),
{
    reveal_with_fuel(children_valid, 2);
}

/// Full finite encoding relation, including active-value depth and domains.
///
/// Structural mode omits bool, UTF-8 and key-domain/order checks. Both modes
/// retain exact extents, tags, canonical directory shape and checked metadata.
pub open spec fn schema_valid(
    bytes: Seq<u8>, layout: Layout, budget: nat, semantic: bool,
) -> bool
    decreases budget, 0int, 0int,
{
    budget > 0 && local_valid(bytes, layout, semantic)
        && children_valid(bytes, layout, 0, (budget - 1) as nat, semantic)
}

/// Structural acceptance and rejection completeness, including the depth cap.
pub open spec fn validate_layout_iff(
    result: Result<()>, bytes: Seq<u8>, layout: Layout, budget: usize, semantic: bool,
) -> bool {
    !semantic ==> (result.is_ok() == (budget <= MAX_DEPTH
        && schema_valid(bytes, layout, budget as nat, false)))
}

/// Structural rejection completeness; the traversal's work measure proves totality.
pub open spec fn validate_layout_total(
    result: Result<()>, bytes: Seq<u8>, layout: Layout, budget: usize, semantic: bool,
) -> bool {
    !semantic ==> (result.is_err() == !(budget <= MAX_DEPTH
        && schema_valid(bytes, layout, budget as nat, false)))
}

/// Complete semantic acceptance and rejection equivalence, including depth.
pub open spec fn validate_schema_iff(
    result: Result<()>, bytes: Seq<u8>, layout: Layout, budget: usize,
) -> bool {
    result.is_ok() == (budget <= MAX_DEPTH && schema_valid(bytes, layout, budget as nat, true))
}

/// Rejection completeness; loop decreases clauses additionally prove termination.
pub open spec fn validate_schema_total(
    result: Result<()>, bytes: Seq<u8>, layout: Layout, budget: usize,
) -> bool {
    result.is_err() == !(budget <= MAX_DEPTH && schema_valid(bytes, layout, budget as nat, true))
}

/// Local rejection evidence. Partition diagnostics are constrained only to the
/// supplied region; bool and UTF-8 diagnostics also retain their semantic kind
/// and position. This does not specify the first rejecting sibling.
#[verifier::opaque]
pub open spec fn local_fault(
    bytes: Seq<u8>, layout: Layout, semantic: bool, error: Error,
) -> bool {
    !local_valid(bytes, layout, semantic)
        && crate::error::fault_within(error, bytes.len())
        && (semantic && layout == Layout::Leaf(crate::Leaf::Bool) && bytes.len() == 1
            ==> error == (Error::Fault { kind: ErrorKind::InvalidBool, offset: 0 }))
        && (semantic && layout == Layout::Str && bytes.len() <= u32::MAX
            ==> utf8::utf8_iff(Err(error), bytes))
}

/// Mathematical coordinate translation, used only for contained diagnostics.
/// Sentinel faults have no coordinate to translate.
pub open spec fn absolute_fault(error: Error, base: int) -> Error {
    match error {
        Error::Fault { kind, offset } => if offset == u32::MAX { error }
            else { Error::Fault { kind, offset: (base + offset) as u32 } },
        Error::Program { .. } => error,
    }
}

/// A located diagnostic belongs to a rejecting node in its exact child interval.
/// This relation uses schema intervals, not traversal frames. It preserves the
/// local kind and adds each ancestor's interval start. Sibling precedence and
/// the kind of unlocated faults remain unspecified.
#[verifier::opaque]
pub open spec fn located_fault(
    bytes: Seq<u8>, layout: Layout, budget: nat, semantic: bool, error: Error,
) -> bool
    decreases budget,
{
    crate::error::fault_within(error, bytes.len()) && match error {
        Error::Fault { offset, .. } => offset < u32::MAX && budget > 0
            && (local_fault(bytes, layout, semantic, error)
                || (local_valid(bytes, layout, semantic) && exists|i: int, inner: Error|
                    #![trigger child_span(bytes, layout, i), absolute_fault(inner, child_span(bytes, layout, i).start as int)]
                {
                    let span = child_span(bytes, layout, i);
                    &&& 0 <= i < child_count(bytes, layout)
                    &&& span.start <= span.end <= bytes.len()
                    &&& located_fault(bytes.subrange(span.start as int, span.end as int),
                        child_layout(layout, i), (budget - 1) as nat, semantic, inner)
                    &&& error == absolute_fault(inner, span.start as int)
                })),
        _ => false,
    }
}

/// Unlocated fault kinds remain outside the composed coordinate contract.
pub open spec fn subtree_fault(
    bytes: Seq<u8>, layout: Layout, budget: nat, semantic: bool, error: Error,
) -> bool {
    crate::error::fault_within(error, bytes.len()) && match error {
        Error::Fault { offset, .. } => offset == u32::MAX
            || located_fault(bytes, layout, budget, semantic, error),
        _ => false,
    }
}

/// Composed diagnostic provenance and coordinates, without a first-fault claim.
pub open spec fn validate_fault(
    result: Result<()>, bytes: Seq<u8>, layout: Layout, budget: usize, semantic: bool,
) -> bool {
    match result {
        Ok(()) => true,
        Err(error) => subtree_fault(bytes, layout, budget as nat, semantic, error),
    }
}

// Hide the quantified coordinate continuation from the traversal's work proof.
// Its lemmas use only schema intervals and contained mathematical translations.
#[verifier::opaque]
spec fn faults_lift(
    bytes: Seq<u8>, root: Layout, depth: nat, region: Span, node: Layout,
    budget: nat, semantic: bool,
) -> bool {
    forall|error: Error| #[trigger] subtree_fault(
        bytes.subrange(region.start as int, region.end as int), node, budget, semantic, error)
        ==> subtree_fault(bytes, root, depth, semantic, absolute_fault(error, region.start as int))
}

#[verifier::opaque]
spec fn stack_faults(
    bytes: Seq<u8>, root: Layout, depth: nat, frames: Seq<Frame>, len: nat, semantic: bool,
) -> bool {
    forall|i: int| 0 <= i < len ==> #[trigger] faults_lift(bytes, root, depth,
        frames[i].region, frames[i].layout, (depth - i) as nat, semantic)
}

proof fn preserve_stack_faults(
    bytes: Seq<u8>, root: Layout, depth: nat, a: Seq<Frame>, b: Seq<Frame>,
    old_len: nat, new_len: nat, semantic: bool,
)
    requires
        old_len <= a.len(), new_len <= b.len(),
        stack_faults(bytes, root, depth, a, old_len, semantic),
        forall|i: int| 0 <= i < new_len && i < old_len ==>
            #[trigger] b[i].region == a[i].region && b[i].layout == a[i].layout,
        forall|i: int| old_len <= i < new_len ==> #[trigger] faults_lift(bytes, root, depth,
            b[i].region, b[i].layout, (depth - i) as nat, semantic),
    ensures stack_faults(bytes, root, depth, b, new_len, semantic),
{
    reveal(stack_faults);
}

proof fn lift_local_fault(
    bytes: Seq<u8>, root: Layout, depth: nat, region: Span, node: Layout,
    budget: nat, semantic: bool, error: Error,
)
    requires
        budget > 0,
        region.start <= region.end <= bytes.len(),
        faults_lift(bytes, root, depth, region, node, budget, semantic),
        local_fault(bytes.subrange(region.start as int, region.end as int), node, semantic, error),
    ensures subtree_fault(bytes, root, depth, semantic, absolute_fault(error, region.start as int)),
{
    assert(subtree_fault(bytes.subrange(region.start as int, region.end as int),
        node, budget, semantic, error)) by {
        reveal(local_fault);
        reveal(located_fault);
    }
    reveal(faults_lift);
}

#[verifier::spinoff_prover]
#[verifier::rlimit(480)]
proof fn lift_child_faults(
    bytes: Seq<u8>, root: Layout, depth: nat, region: Span, node: Layout,
    budget: nat, semantic: bool, index: int,
)
    requires
        region.start <= region.end <= bytes.len() <= u32::MAX,
        budget > 1,
        faults_lift(bytes, root, depth, region, node, budget, semantic),
        local_valid(bytes.subrange(region.start as int, region.end as int), node, semantic),
        0 <= index < child_count(bytes.subrange(region.start as int, region.end as int), node),
        ({ let span = child_span(bytes.subrange(region.start as int, region.end as int), node, index);
            span.start <= span.end <= region.end - region.start }),
    ensures ({
        let local = bytes.subrange(region.start as int, region.end as int);
        let span = child_span(local, node, index);
        faults_lift(bytes, root, depth,
            Span { start: (region.start + span.start) as u32, end: (region.start + span.end) as u32 },
            child_layout(node, index), (budget - 1) as nat, semantic)
    }),
{
    let local = bytes.subrange(region.start as int, region.end as int);
    let span = child_span(local, node, index);
    let child_region = Span { start: (region.start + span.start) as u32,
        end: (region.start + span.end) as u32 };
    let child_bytes = bytes.subrange(child_region.start as int, child_region.end as int);
    assert(child_bytes =~= local.subrange(span.start as int, span.end as int));
    reveal(faults_lift);
    assert forall|error: Error| subtree_fault(child_bytes, child_layout(node, index),
        (budget - 1) as nat, semantic, error)
        implies subtree_fault(bytes, root, depth, semantic,
            absolute_fault(error, child_region.start as int)) by {
        let parent_error = absolute_fault(error, span.start as int);
        assert(subtree_fault(local, node, budget, semantic, parent_error)) by {
            reveal(located_fault);
            if matches!(error, Error::Fault { offset, .. } if offset < u32::MAX) {
                assert(exists|i: int, inner: Error|
                    #![trigger child_span(local, node, i), absolute_fault(inner, child_span(local, node, i).start as int)]
                {
                    let s = child_span(local, node, i);
                    &&& 0 <= i < child_count(local, node)
                    &&& s.start <= s.end <= local.len()
                    &&& located_fault(local.subrange(s.start as int, s.end as int),
                        child_layout(node, i), (budget - 1) as nat, semantic, inner)
                    &&& parent_error == absolute_fault(inner, s.start as int)
                });
            }
        }
        assert(subtree_fault(bytes, root, depth, semantic,
            absolute_fault(parent_error, region.start as int)));
        assert(absolute_fault(parent_error, region.start as int)
            == absolute_fault(error, child_region.start as int));
    }
}

pub(crate) open spec fn parts_valid(parts: Parts, bytes: Seq<u8>, layout: Layout, semantic: bool) -> bool {
    local_valid(bytes, layout, semantic) && match parts {
        Parts::Done => matches!(layout.head(), LayoutHead::Leaf(_) | LayoutHead::Str),
        Parts::Struct(p) => struct_layout::partition_iff(Ok(p), bytes, layout),
        Parts::List(p) => list_layout::partition_iff(Ok(p), bytes, layout),
        Parts::Map(p) => map_layout::partition_iff(Ok(p), bytes, layout),
        Parts::Option(p) => leaves::option_partition_iff(Ok(p), bytes, layout),
    }
}

// Local checks run once per node. Map keys precede value traversal.
pub(crate) fn enter(bytes: &[u8], layout: &Layout, semantic: bool) -> (result: Result<Parts>)
    ensures
        match result {
            Ok(parts) => parts_valid(parts, bytes@, *layout, semantic),
            Err(error) => !local_valid(bytes@, *layout, semantic)
                && local_fault(bytes@, *layout, semantic, error),
        },
{
    proof { reveal(crate::layout_head::head_spec); }
    reveal(local_fault);
    check_layout(layout, MAX_DEPTH)?;
    let parts = match layout.tag() {
        1..=4 => {
            if semantic { validate_fixed(bytes, layout)?; }
            else { fixed_region(bytes, layout)?; }
            Parts::Done
        },
        5 => {
            if bytes.len() > u32::MAX as usize {
                return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
            }
            if semantic { validate_utf8(bytes)?; }
            Parts::Done
        },
        9 => Parts::Struct(partition_struct(bytes, layout)?),
        6 => Parts::Option(partition_option(bytes, layout)?),
        7 => Parts::List(partition_list(bytes, layout)?),
        8 => {
            let p = partition_map(bytes, layout)?;
            if semantic { crate::search::validate_partition_keys(bytes, layout, &p)?; }
            Parts::Map(p)
        },
        _=>{assert(false);return Err(ErrorKind::InvalidSchema.into());},
    };
    Ok(parts)
}

#[inline(always)]
pub(crate) fn count(parts: &Parts, layout: &Layout, Ghost(bytes): Ghost<Seq<u8>>, _semantic: bool)
    -> (result: u32)
    requires parts_valid(*parts, bytes, *layout, _semantic),
    ensures result == child_count(bytes, *layout),
{
    match parts {
        Parts::Done => 0,
        Parts::Struct(_) => if let Layout::Flat{checked}=layout {
            proof{reveal(crate::layout_head::head_spec);}
            checked.header(3)+checked.header(4)
        } else { match layout.head() {
            LayoutHead::Struct { fixed, variable, .. } => (fixed.len() + variable.len()) as u32,
            _ => { assert(false); 0 },
        }
        },
        Parts::List(p) => p.count,
        Parts::Map(p) => p.count,
        Parts::Option(p) => if p.is_some() { 1 } else { 0 },
    }
}

// Metadata entails containment for fixed children even in mixed structs.
proof fn fixed_bounds(
    fixed: Seq<crate::Field>, variable: Seq<crate::Field>, size: u32,
    i: int, j: int, offset: int, budget: nat,
)
    requires descriptor_fields(fixed, variable, size, i, j, offset, budget),
    ensures
        offset <= size,
        forall|k: int| #![trigger fixed[k]] i <= k < fixed.len() ==> {
            let f = fixed[k];
            width_spec(f.layout).is_some()
                && f.offset + width_spec(f.layout).unwrap() <= size
        },
    decreases fixed.len() + variable.len() - i - j,
{
    reveal_with_fuel(descriptor_fields, 2);
    if i < fixed.len() && (j == variable.len() || fixed[i].declaration <= variable[j].declaration) {
        let width = width_spec(fixed[i].layout).unwrap();
        fixed_bounds(fixed, variable, size, i + 1, j, offset + width, budget);
    } else if j < variable.len() {
        fixed_bounds(fixed, variable, size, i, j + 1, offset, budget);
    }
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
pub(crate) fn child(
    bytes: &[u8], layout: &Layout, parts: &Parts, index: u32, _semantic: bool,
) -> (result: Result<(Span, Layout)>)
    requires parts_valid(*parts, bytes@, *layout, _semantic),
        index < child_count(bytes@, *layout),
    ensures match result {
        Ok((span, child)) => span == child_span(bytes@, *layout, index as int)
            && child == child_layout(*layout, index as int)
            && span.start <= span.end <= bytes.len(),
        Err(_) => false,
    },
{
    if let Layout::Flat{checked}=layout {
        checked.witness();
        proof{reveal(crate::layout_head::head_spec);}
        match parts {
            Parts::Struct(p)=>{
                let nf=checked.header(3) as usize;
                if (index as usize)<nf {
                    proof{fixed_bounds(crate::Fields::Flat{layout:checked,variable:false}.view(),
                        crate::Fields::Flat{layout:checked,variable:true}.view(),checked.view()[checked.root_index()+1],0,0,0,MAX_DEPTH as nat);}
                    let (_,offset,child)=checked.field(index as usize,false);
                    proof{crate::layout::certified_width(child);}
                    let width=child.header(1);
                    return Ok((Span{start:offset,end:offset+width},Layout::Flat{checked:child}));
                } else {
                    let ordinal=(index as usize-nf) as u16;
                    let span=struct_variable_field_at(bytes,layout,p,ordinal)?;
                    return Ok((span,Layout::Flat{checked:checked.child(ordinal as usize,true)}));
                }
            },
            Parts::Option(Some(span))=>return Ok((*span,Layout::Flat{checked:checked.only_child()})),
            Parts::List(p)=>{
                let span=list_element_at(bytes,layout,p,index)?;
                return Ok((span,Layout::Flat{checked:checked.only_child()}));
            },
            Parts::Map(p)=>{
                let span=map_value_at(bytes,layout,p,index)?;
                return Ok((span,Layout::Flat{checked:checked.only_child()}));
            },
            _=>{},
        }
    }
    match (layout.head(), parts) {
        (LayoutHead::Struct { fixed, variable, fixed_bytes: _size }, Parts::Struct(p)) => {
            if (index as usize) < fixed.len() {
                proof { fixed_bounds(fixed.view(), variable.view(), _size, 0, 0, 0, MAX_DEPTH as nat); }
                let field = &fixed.get(index as usize);
                let width = match fixed_width(&field.layout) {
                    Some(width) => width,
                    None => { assert(false); return Err(ErrorKind::InvalidSchema.into()); },
                };
                Ok((Span { start: field.offset, end: field.offset + width }, field.layout))
            } else {
                let ordinal = (index as usize - fixed.len()) as u16;
                let span = struct_variable_field_at(bytes, layout, p, ordinal)?;
                Ok((span, variable.get(ordinal as usize).layout))
            }
        },
        (LayoutHead::Option { value }, Parts::Option(Some(span))) => Ok((*span, value)),
        (LayoutHead::List { element }, Parts::List(p)) => {
            let span = list_element_at(bytes, layout, p, index)?;
            Ok((span, element))
        },
        (LayoutHead::Map { value, .. }, Parts::Map(p)) => {
            let span = map_value_at(bytes, layout, p, index)?;
            Ok((span, value))
        },
        _ => { assert(false); Err(ErrorKind::InvalidSchema.into()) },
    }
}

// The table checker proves the flag recursively. These lemmas connect that
// metadata fact to the existing byte-level relation, including active depth.
#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
proof fn unrestricted_schema(bytes: Seq<u8>, checked: crate::CheckedLayout, budget: nat, semantic: bool)
    requires checked.valid(), crate::flat_layout::unrestricted(checked.view(),checked.root_index()),
        checked.view()[checked.root_index()+2] <= budget <= MAX_DEPTH,
        bytes.len() == checked.view()[checked.root_index()+1],
    ensures schema_valid(bytes,Layout::Flat{checked},budget,semantic),
        leaves::fixed_value_valid(bytes,Layout::Flat{checked},budget),
    decreases budget, 1int, 0int,
{
    checked.metadata();
    crate::layout::certified_descriptor(checked,budget);
    crate::layout::certified_descriptor(checked,MAX_DEPTH as nat);
    crate::layout::certified_width(checked);
    reveal(crate::layout_head::head_spec);
    let layout = Layout::Flat{checked};
    if checked.view()[checked.root_index()] == 9 {
        let fixed = crate::Fields::Flat{layout:&checked,variable:false}.view();
        let variable = crate::Fields::Flat{layout:&checked,variable:true}.view();
        assert(variable =~= Seq::<crate::Field>::empty());
        assert(struct_layout::struct_shape(bytes,layout));
        unrestricted_struct_suffix(bytes,checked,0,budget,semantic);
    } else {
        reveal_with_fuel(children_valid,2);
    }
    assert(local_valid(bytes,layout,semantic));
    assert(children_valid(bytes,layout,0,(budget-1) as nat,semantic));
}

#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
proof fn unrestricted_struct_suffix(bytes: Seq<u8>, checked: crate::CheckedLayout, i: int,
    budget: nat, semantic: bool)
    requires checked.valid(), checked.view()[checked.root_index()]==9,
        crate::flat_layout::unrestricted(checked.view(),checked.root_index()),
        checked.view()[checked.root_index()+2] <= budget <= MAX_DEPTH,
        bytes.len() == checked.view()[checked.root_index()+1],
        0 <= i <= checked.view()[checked.root_index()+3],
    ensures children_valid(bytes,Layout::Flat{checked},i,(budget-1) as nat,semantic),
        leaves::fixed_fields_valid(bytes,crate::Fields::Flat{layout:&checked,variable:false}.view(),i,budget),
    decreases budget, 0int, checked.view()[checked.root_index()+3]-i,
{
    reveal(crate::flat_layout::unrestricted_fields);
    checked.metadata();
    crate::layout::certified_descriptor(checked,budget);
    reveal(crate::layout_head::head_spec);
    let w=checked.view(); let r=checked.root_index();
    let fixed=crate::Fields::Flat{layout:&checked,variable:false}.view();
    let variable=crate::Fields::Flat{layout:&checked,variable:true}.view();
    assert(variable =~= Seq::<crate::Field>::empty());
    reveal_with_fuel(children_valid,2);
    reveal_with_fuel(leaves::fixed_fields_valid,2);
    if i<fixed.len() {
        fixed_bounds(fixed,variable,w[r+1],0,0,0,budget);
        checked.field_metadata(i,false);
        let child=w[w[r+5]+8*i+3] as int;
        checked.retarget_valid(child);
        let next=checked.retarget(child);
        crate::layout::certified_width(next);
        // The checked packed-field relation also bounds child metadata depth.
        unrestricted_field_depth(w,r,w[r+5] as int,w[r+3] as int,0,0,i);
        let span=child_span(bytes,Layout::Flat{checked},i);
        assert(span.start <= span.end <= bytes.len());
        unrestricted_schema(bytes.subrange(span.start as int,span.end as int),next,(budget-1) as nat,semantic);
        unrestricted_struct_suffix(bytes,checked,i+1,budget,semantic);
    }
}

proof fn unrestricted_field_depth(w:Seq<u32>,r:int,start:int,nf:int,i:int,offset:int,k:int)
    requires crate::flat_layout::struct_fields(w,r,start,nf,0,i,0,offset,w[r+2] as int),
        i<=k<nf,
    ensures w[w[start+8*k+3] as int+2] < w[r+2],
    decreases nf-i,
{
    reveal_with_fuel(crate::flat_layout::struct_fields,2);
    if i<k { unrestricted_field_depth(w,r,start,nf,i+1,offset+w[w[start+8*i+3] as int+1],k); }
}

#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
proof fn unrestricted_collection_suffix(bytes:Seq<u8>,layout:Layout,checked:crate::CheckedLayout,
    i:int,budget:nat,semantic:bool)
    requires local_valid(bytes,layout,semantic), checked.valid(),
        match layout.head() {
            LayoutHead::List{element}=>element==Layout::Flat{checked},
            LayoutHead::Map{value,..}=>value==Layout::Flat{checked},
            _=>false,
        },
        crate::flat_layout::unrestricted(checked.view(),checked.root_index()),
        checked.view()[checked.root_index()+2] < budget <= MAX_DEPTH,
        0<=i<=child_count(bytes,layout),
    ensures children_valid(bytes,layout,i,(budget-1) as nat,semantic),
    decreases child_count(bytes,layout)-i,
{
    checked.metadata();
    crate::layout::certified_width(checked);
    reveal(crate::layout_head::head_spec);
    assert(width_spec(Layout::Flat{checked})==Some(checked.view()[checked.root_index()+1]));
    reveal_with_fuel(children_valid,2);
    if i<child_count(bytes,layout) {
        let span=child_span(bytes,layout,i);
        let width=checked.view()[checked.root_index()+1] as int;
        let n=child_count(bytes,layout);
        let stride=match layout.head() {
            LayoutHead::Map{key,..}=>key.width as int+width,
            _=>width,
        };
        let extra=stride-width;
        assert(width>0 && stride>=width && bytes.len()==4+n*stride);
        assert(4+i*stride+extra+width<=bytes.len()) by(nonlinear_arith)
            requires 0<=i<n, width>0, stride>=width, extra==stride-width,
                bytes.len()==4+n*stride;
        assert(span.start==4+i*stride+extra);
        assert(4+(i+1)*stride <= bytes.len()) by(nonlinear_arith)
            requires 0<=i<n, bytes.len()==4+n*stride, stride>0;
        assert(4+(i+1)*stride == 4+i*stride+extra+width) by(nonlinear_arith)
            requires extra==stride-width;
        assert(span.end==span.start+width);
        assert(span.start<=span.end<=bytes.len());
        unrestricted_schema(bytes.subrange(span.start as int,span.end as int),checked,(budget-1) as nat,semantic);
        unrestricted_collection_suffix(bytes,layout,checked,i+1,budget,semantic);
    }
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
pub(crate) proof fn leaf_span_bounds(bytes:Seq<u8>,layout:Layout,i:int,semantic:bool)
    requires local_valid(bytes,layout,semantic),0<=i<child_count(bytes,layout),
        matches!(layout.head(),LayoutHead::List{..}|LayoutHead::Map{..}),
    ensures ({let span=child_span(bytes,layout,i);
        span.start<=span.end<=bytes.len()
        && (width_spec(child_layout(layout,i)).is_some() ==>
            span.end-span.start==width_spec(child_layout(layout,i)).unwrap())}),
{
    let n=child_count(bytes,layout);
    match layout.head() {
        LayoutHead::List{element}=>{
            if let Some(w)=width_spec(element) {
                assert(4+(i+1)*w<=bytes.len()) by(nonlinear_arith)
                    requires 0<=i<n,bytes.len()==4+n*w,w>=0;
                assert(4+(i+1)*w==4+i*w+w) by(nonlinear_arith);
            } else {
                let p=list_layout::payload_start(bytes,layout);
                if i>0 {assert(words::le_word(bytes,4+4*(i-1))<=bytes.len()-p);}
                if i+1<n {
                    assert(words::le_word(bytes,4+4*i)<=bytes.len()-p);
                    if i>0 {assert(words::le_word(bytes,4+4*(i-1))<=words::le_word(bytes,4+4*i));}
                }
            }
        },
        LayoutHead::Map{..}=>{
            let stride=map_layout::entry_stride(layout);
            let kw=map_layout::key_width(layout);
            if let Some(w)=map_layout::value_width(layout) {
                assert(4+(i+1)*stride<=bytes.len()) by(nonlinear_arith)
                    requires 0<=i<n,bytes.len()==4+n*stride,stride>=0;
                assert(4+(i+1)*stride==4+i*stride+kw+w) by(nonlinear_arith)
                    requires stride==kw+w;
            } else {
                let p=map_layout::payload_start(bytes,layout);
                assert(words::le_word(bytes,4+i*stride+kw)<=bytes.len()-p);
                if i>0 {
                    assert(words::le_word(bytes,4+(i-1)*stride+kw)<=words::le_word(bytes,4+i*stride+kw));
                }
            }
        },
        _=>{},
    }
}

#[inline(always)]
fn scan_leaf(bytes:&[u8],is_str:bool,is_bool:bool,semantic:bool,Ghost(layout):Ghost<Layout>)
    ->(result:Result<()>)
    requires descriptor_valid(layout,MAX_DEPTH as nat),bytes.len()<=u32::MAX,
        matches!(layout.head(),LayoutHead::Leaf(_)|LayoutHead::Str),
        is_str==matches!(layout.head(),LayoutHead::Str),
        is_bool==matches!(layout.head(),LayoutHead::Leaf(crate::Leaf::Bool)),
        !is_str ==> width_spec(layout)==Some(bytes.len() as u32),
    ensures result.is_ok()==local_valid(bytes@,layout,semantic),
        result.is_err() ==> local_fault(bytes@,layout,semantic,result.unwrap_err()),
{
    reveal(local_fault);
    reveal(crate::layout_head::head_spec);
    if semantic {
        if is_str {validate_utf8(bytes)?;}
        else if is_bool && bytes[0]>1 {return Err(Error::new(ErrorKind::InvalidBool,0));}
    }
    Ok(())
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn next_leaf_start(bytes:Seq<u8>,layout:Layout,i:int,semantic:bool)
    requires local_valid(bytes,layout,semantic),0<=i<i+1<child_count(bytes,layout),
        matches!(layout.head(),LayoutHead::List{..}|LayoutHead::Map{..}),
    ensures ({let a=child_span(bytes,layout,i);let b=child_span(bytes,layout,i+1);
        b.start==match layout.head() {
            LayoutHead::List{element}=>if width_spec(element).is_some() {a.start+width_spec(element).unwrap()}else{a.end as int},
            LayoutHead::Map{..}=>if map_layout::value_width(layout).is_some() {a.start+map_layout::entry_stride(layout)}else{a.end as int},
            _=>0int,
        }}),
{
    leaf_span_bounds(bytes,layout,i,semantic);
    leaf_span_bounds(bytes,layout,i+1,semantic);
    match layout.head() {
        LayoutHead::List{element}=>if let Some(w)=width_spec(element) {
            assert(4+(i+1)*w<=bytes.len()) by(nonlinear_arith)
                requires 0<=i<i+1<child_count(bytes,layout),bytes.len()==4+child_count(bytes,layout)*w,w>=0;
            assert(4+(i+1)*w==4+i*w+w) by(nonlinear_arith);
        },
        LayoutHead::Map{..}=>if map_layout::value_width(layout).is_some() {
            let stride=map_layout::entry_stride(layout);let key=map_layout::key_width(layout);
            assert(4+(i+1)*stride+key<=bytes.len()) by(nonlinear_arith)
                requires 0<=i<i+1<child_count(bytes,layout),bytes.len()==4+child_count(bytes,layout)*stride,0<=key<=stride;
            assert(4+(i+1)*stride+key==4+i*stride+key+stride) by(nonlinear_arith);
        },
        _=>{},
    }
}

#[derive(Clone,Copy)]
struct LeafGeometry {
    count:u32,
    payload:u32,
    width:Option<u32>,
    #[cfg(verus_only)]
    ghost key:u32,
    stride:usize,
    stored_last:bool,
}
#[derive(Clone,Copy)]
struct LeafPosition {index:u32,at:usize,cursor:usize}

#[verifier::opaque]
spec fn leaf_scan_state(bytes:Seq<u8>,layout:Layout,leaf:Layout,g:LeafGeometry,p:LeafPosition,semantic:bool)->bool {
    &&& local_valid(bytes,layout,semantic)
    &&& descriptor_valid(leaf,MAX_DEPTH as nat)
    &&& matches!(leaf.head(),LayoutHead::Leaf(_)|LayoutHead::Str)
    &&& 0<g.count==child_count(bytes,layout)
    &&& match layout.head() {
        LayoutHead::List{element}=>element==leaf && g.key==0 && !g.stored_last
            && g.payload==list_layout::payload_start(bytes,layout) && g.width==width_spec(element),
        LayoutHead::Map{value,..}=>value==leaf && g.key==map_layout::key_width(layout)
            && g.stored_last && g.payload==map_layout::payload_start(bytes,layout) && g.width==width_spec(value),
        _=>false,
    }
    &&& g.stride==g.key+g.width.unwrap_or(4)
    &&& p.index<=g.count
    &&& if p.index<g.count {
        &&& p.at==child_span(bytes,layout,p.index as int).start
        &&& (g.width.is_none() ==> p.cursor==4+g.key+p.index as int*g.stride)
    }else{true}
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn leaf_advance(bytes:Seq<u8>,layout:Layout,leaf:Layout,g:LeafGeometry,p:LeafPosition,span:Span,semantic:bool)
    requires leaf_scan_state(bytes,layout,leaf,g,p,semantic),p.index<g.count,
        span==child_span(bytes,layout,p.index as int),span.start<=span.end<=bytes.len(),
    ensures p.index+1<g.count ==> (
        (g.width.is_some() ==> p.at+g.stride<=usize::MAX)
        && (g.width.is_none() ==> p.cursor+g.stride<=usize::MAX)),
        leaf_scan_state(bytes,layout,leaf,g,LeafPosition{
            index:(p.index+1) as u32,
            at:if p.index+1<g.count {if g.width.is_none(){span.end as usize}else{(p.at+g.stride) as usize}}else{p.at},
            cursor:if p.index+1<g.count && g.width.is_none(){(p.cursor+g.stride) as usize}else{p.cursor},
        },semantic),
{
    hide(descriptor_valid);hide(search::lex_less);hide(search::keys_ordered);
    hide(search::keys_canonical);hide(leaves::fixed_value_valid);hide(utf8::utf8_valid);
    reveal(leaf_scan_state);
    if p.index+1<g.count {
        let i=p.index as int;
        next_leaf_start(bytes,layout,i,semantic);
        leaf_span_bounds(bytes,layout,i+1,semantic);
        let next=child_span(bytes,layout,i+1);
        assert(next.start==if g.width.is_none(){span.end as int}else{p.at+g.stride});
        if g.width.is_none() {
            assert(p.cursor+g.stride<=bytes.len()) by(nonlinear_arith)
                requires p.cursor==4+g.key+i*g.stride,i+1<g.count,g.stride==g.key+4,
                    g.payload==(if g.stored_last {4+g.count as int*g.stride}else{4+4*(g.count as int-1)}),
                    g.key==0 || g.stored_last,g.payload<=bytes.len();
            assert(4+g.key+(i+1)*g.stride==p.cursor+g.stride) by(nonlinear_arith)
                requires p.cursor==4+g.key+i*g.stride;
        }
    }
}

#[inline(always)]
#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
fn leaf_span_at(bytes:&[u8],g:&LeafGeometry,p:LeafPosition,Ghost(semantic):Ghost<bool>,
    Ghost(layout):Ghost<Layout>,Ghost(leaf):Ghost<Layout>)->(result:Span)
    requires leaf_scan_state(bytes@,layout,leaf,*g,p,semantic),p.index<g.count,
    ensures result==child_span(bytes@,layout,p.index as int),
        result.start<=result.end<=bytes.len(),
        local_valid(bytes@,layout,semantic),descriptor_valid(leaf,MAX_DEPTH as nat),
        child_layout(layout,p.index as int)==leaf,
        matches!(leaf.head(),LayoutHead::Leaf(_)|LayoutHead::Str),
        g.width==width_spec(leaf),
        g.width.is_some() ==> result.end-result.start==width_spec(leaf).unwrap(),
{
    hide(descriptor_valid);hide(search::lex_less);hide(search::keys_ordered);
    hide(search::keys_canonical);hide(leaves::fixed_value_valid);hide(utf8::utf8_valid);
    reveal(leaf_scan_state);
    let ghost span=child_span(bytes@,layout,p.index as int);
    let ghost i=p.index as int;
    let indexed=g.width.is_none();
    proof {
        leaf_span_bounds(bytes@,layout,i as int,semantic);
        assert(span.start==p.at);
        if indexed && (g.stored_last || i+1<g.count) {
            assert(p.cursor+4<=bytes.len()) by(nonlinear_arith)
                requires p.cursor==4+g.key+i*g.stride,i<g.count,g.stride==g.key+4,
                    g.payload==(if g.stored_last {4+g.count as int*g.stride}else{4+4*(g.count as int-1)}),
                    g.stored_last || i+1<g.count,g.key==0 || g.stored_last,g.payload<=bytes.len();
        }
    }
    let end=if indexed {
        if !g.stored_last && p.index+1==g.count {bytes.len()}
        else {add_bounded(g.payload as usize,crate::words::directory_word(bytes,p.cursor) as usize)}
    }else{add_bounded(p.at,g.width.unwrap() as usize)};
    proof {assert(end==span.end);assert(p.at<=end<=bytes.len());}
    Span{start:p.at as u32,end:end as u32}
}

#[inline(always)]
#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
fn stream_leaf_span(bytes:&[u8],g:&LeafGeometry,p:LeafPosition,Ghost(semantic):Ghost<bool>,
    Ghost(layout):Ghost<Layout>,Ghost(leaf):Ghost<Layout>)->(result:(Span,LeafPosition))
    requires leaf_scan_state(bytes@,layout,leaf,*g,p,semantic),p.index<g.count,
    ensures result.0==child_span(bytes@,layout,p.index as int),
        result.0.start<=result.0.end<=bytes.len(),
        result.1.index==p.index+1,
        leaf_scan_state(bytes@,layout,leaf,*g,result.1,semantic),
        local_valid(bytes@,layout,semantic),descriptor_valid(leaf,MAX_DEPTH as nat),
        child_layout(layout,p.index as int)==leaf,
        matches!(leaf.head(),LayoutHead::Leaf(_)|LayoutHead::Str),
        g.width==width_spec(leaf),
        g.width.is_some() ==> result.0.end-result.0.start==width_spec(leaf).unwrap(),
{
    let span=leaf_span_at(bytes,g,p,Ghost(semantic),Ghost(layout),Ghost(leaf));
    let next_index=p.index+1;
    proof {leaf_advance(bytes@,layout,leaf,*g,p,span,semantic);}
    let next=if next_index<g.count {
        LeafPosition{index:next_index,at:if g.width.is_none(){span.end as usize}else{add_bounded(p.at,g.stride)},
            cursor:if g.width.is_none(){add_bounded(p.cursor,g.stride)}else{p.cursor}}
    }else{LeafPosition{index:next_index,..p}};
    (span,next)
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn leaf_child_outcome(bytes:Seq<u8>,layout:Layout,leaf:Layout,i:int,span:Span,
    budget:nat,semantic:bool,result:Result<()>)
    requires local_valid(bytes,layout,semantic),0<=i<child_count(bytes,layout),
        budget>1,budget<=MAX_DEPTH,span==child_span(bytes,layout,i),span.start<=span.end<=bytes.len(),
        leaf==child_layout(layout,i),matches!(leaf.head(),LayoutHead::Leaf(_)|LayoutHead::Str),
        result.is_ok()==local_valid(bytes.subrange(span.start as int,span.end as int),leaf,semantic),
        result.is_err() ==> local_fault(bytes.subrange(span.start as int,span.end as int),leaf,semantic,result.unwrap_err()),
    ensures result.is_ok()==child_valid(bytes,layout,i,(budget-1) as nat,semantic),
        result.is_err() ==> subtree_fault(bytes,layout,budget,semantic,absolute_fault(result.unwrap_err(),span.start as int)),
{
    let region=bytes.subrange(span.start as int,span.end as int);
    reveal_with_fuel(children_valid,2);
    assert(child_count(region,leaf)==0);
    if result.is_err() {
        assert(bytes.subrange(0,bytes.len() as int) =~= bytes);
        assert(faults_lift(bytes,layout,budget,Span{start:0,end:bytes.len() as u32},layout,budget,semantic)) by {reveal(faults_lift);}
        lift_child_faults(bytes,layout,budget,Span{start:0,end:bytes.len() as u32},layout,budget,semantic,i);
        lift_local_fault(bytes,layout,budget,span,leaf,(budget-1) as nat,semantic,result.unwrap_err());
    }
}

#[verifier::opaque]
spec fn string_context(bytes:Seq<u8>,layout:Layout,leaf:Layout,payload:u32,count:u32,map:bool,key:usize,semantic:bool)->bool {
    &&& local_valid(bytes,layout,semantic)
    &&& descriptor_valid(leaf,MAX_DEPTH as nat)
    &&& matches!(leaf.head(),LayoutHead::Str)
    &&& count>0 && count==child_count(bytes,layout)
    &&& key==(if map {32usize}else{0usize})
    &&& match layout.head() {
        LayoutHead::List{element}=>!map && element==leaf && payload==list_layout::payload_start(bytes,layout),
        LayoutHead::Map{value,..}=>map && value==leaf && key==map_layout::key_width(layout)
            && payload==map_layout::payload_start(bytes,layout),
        _=>false,
    }
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn string_facts(bytes:Seq<u8>,layout:Layout,leaf:Layout,payload:u32,count:u32,map:bool,key:usize,semantic:bool)
    requires string_context(bytes,layout,leaf,payload,count,map,key,semantic),
    ensures local_valid(bytes,layout,semantic),descriptor_valid(leaf,MAX_DEPTH as nat),
        matches!(leaf.head(),LayoutHead::Str),count>0,count==child_count(bytes,layout),
        payload<=bytes.len()<=u32::MAX,
        payload==4+(if map {count as int}else{count as int-1})*(key+4),
        key==(if map {32usize}else{0usize}),
        forall|i:int| 0<=i<count ==> #[trigger] child_layout(layout,i)==leaf,
{
    hide(descriptor_valid);hide(search::keys_ordered);hide(search::keys_canonical);hide(search::lex_less);
    reveal(string_context);
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn string_interval(bytes:Seq<u8>,layout:Layout,leaf:Layout,payload:u32,count:u32,map:bool,key:usize,semantic:bool,i:int)
    requires string_context(bytes,layout,leaf,payload,count,map,key,semantic),0<=i<count,
    ensures ({let span=child_span(bytes,layout,i);
        span.start<=span.end<=bytes.len()
        && span.start==payload+if i==0 {0int}else{words::le_word(bytes,4+(i-1)*(key+4)+key)}
        && span.end==if !map && i+1==count {bytes.len() as int}else{payload+words::le_word(bytes,4+i*(key+4)+key)}
    }),
{
    hide(descriptor_valid);hide(search::keys_ordered);hide(search::keys_canonical);hide(search::lex_less);
    reveal(string_context);
    assert(width_spec(leaf).is_none());
    assert(i as u32==i);
    let span=child_span(bytes,layout,i);
    let previous=if i==0 {0int}else{words::le_word(bytes,4+(i-1)*(key+4)+key)};
    let end=if !map && i+1==count {bytes.len()-payload}else{words::le_word(bytes,4+i*(key+4)+key)};
    if map {
        assert(map_layout::value_width(layout).is_none());
        assert(map_layout::entry_stride(layout)==key+4);
        assert(end<=bytes.len()-payload);
        if i>0 {assert(previous<=bytes.len()-payload);}
    }else{
        assert(list_layout::element_width(layout).is_none());
        if i+1<count {assert(end<=bytes.len()-payload);}
        if i>0 {assert(previous<=bytes.len()-payload);}
    }
    assert(span.start==(payload+previous) as u32);
    assert(span.end==(payload+end) as u32);
    leaf_span_bounds(bytes,layout,i,semantic);
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn ascii_string_suffix(bytes:Seq<u8>,layout:Layout,leaf:Layout,payload:u32,count:u32,map:bool,key:usize,semantic:bool,budget:nat,i:int)
    requires string_context(bytes,layout,leaf,payload,count,map,key,semantic),
        1<budget<=MAX_DEPTH,0<=i<=count,
        semantic ==> forall|j:int| payload<=j<bytes.len() ==> #[trigger] bytes[j]<=0x7f,
    ensures children_valid(bytes,layout,i,(budget-1) as nat,semantic),
    decreases count-i,
{
    hide(children_valid);hide(schema_valid);hide(child_valid);hide(descriptor_valid);
    string_facts(bytes,layout,leaf,payload,count,map,key,semantic);
    if i<count {
        string_interval(bytes,layout,leaf,payload,count,map,key,semantic,i);
        let span=child_span(bytes,layout,i);
        assert(payload<=span.start);
        let region=bytes.subrange(span.start as int,span.end as int);
        if semantic {
            assert forall|j:int| 0<=j<region.len() implies #[trigger] region[j]<=0x7f by {
                assert(region[j]==bytes[span.start+j]);
            }
            utf8::ascii_valid(region);
        }
        assert(local_valid(region,leaf,semantic));
        leaf_child_outcome(bytes,layout,leaf,i,span,budget,semantic,Ok(()));
        ascii_string_suffix(bytes,layout,leaf,payload,count,map,key,semantic,budget,i+1);
        children_split(bytes,layout,i,(budget-1) as nat,semantic);
    } else {reveal_with_fuel(children_valid,1);}
}

// A strict parent has already checked every directory word. Walk slices here
// without carrying metadata or wire-sized cursors through each UTF-8 check.
#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
fn scan_strings<const MAP:bool,const KEY:usize,const SEM:bool>(bytes:&[u8],payload:u32,
    Ghost(layout):Ghost<Layout>,Ghost(leaf):Ghost<Layout>,Ghost(count):Ghost<u32>,Ghost(budget):Ghost<usize>)
    ->(result:Result<bool>)
    requires local_valid(bytes@,layout,SEM),descriptor_valid(leaf,MAX_DEPTH as nat),
        matches!(leaf.head(),LayoutHead::Str),count>0,1<budget<=MAX_DEPTH,
        count==child_count(bytes@,layout),KEY==if MAP {32usize}else{0usize},
        match layout.head() {
            LayoutHead::List{element}=>!MAP && element==leaf && payload==list_layout::payload_start(bytes@,layout),
            LayoutHead::Map{value,..}=>MAP && value==leaf && KEY==map_layout::key_width(layout)
                && payload==map_layout::payload_start(bytes@,layout),
            _=>false,
        },
    ensures match result {
        Ok(done)=>done && children_valid(bytes@,layout,0,(budget-1) as nat,SEM),
        Err(error)=>!children_valid(bytes@,layout,0,(budget-1) as nat,SEM)
            && subtree_fault(bytes@,layout,budget as nat,SEM,error),
    },
{
    hide(children_valid);hide(schema_valid);hide(child_valid);hide(descriptor_valid);
    hide(local_valid);
    hide(search::keys_ordered);hide(search::keys_canonical);hide(search::lex_less);hide(utf8::utf8_valid);
    assert(string_context(bytes@,layout,leaf,payload,count,MAP,KEY,SEM)) by {reveal(string_context);}
    proof {string_facts(bytes@,layout,leaf,payload,count,MAP,KEY,SEM);}
    if !SEM || crate::utf8::all_ascii(&bytes[payload as usize..]) {
        proof {
            assert forall|j:int| SEM && payload<=j<bytes.len() implies #[trigger] bytes@[j]<=0x7f by {
                if SEM {assert(bytes@.subrange(payload as int,bytes.len() as int)[j-payload]==bytes@[j]);}
            }
            ascii_string_suffix(bytes@,layout,leaf,payload,count,MAP,KEY,SEM,budget as nat,0);
        }
        return Ok(true);
    }
    let stride=KEY+4;
    let mut ends=&bytes[4..payload as usize];
    let mut data=&bytes[payload as usize..];
    let mut previous=0usize;
    let ghost stored=if MAP {count as int}else{count as int-1};
    let ghost mut i=0int;
    while ends.len()>=stride
        invariant local_valid(bytes@,layout,SEM),descriptor_valid(leaf,MAX_DEPTH as nat),
            matches!(leaf.head(),LayoutHead::Str),count>0,1<budget<=MAX_DEPTH,
            count==child_count(bytes@,layout),KEY==if MAP {32usize}else{0usize},
            string_context(bytes@,layout,leaf,payload,count,MAP,KEY,SEM),
            forall|j:int| 0<=j<count ==> #[trigger] child_layout(layout,j)==leaf,
            stride==KEY+4,stored==if MAP {count as int}else{count as int-1},0<=i<=stored,
            payload as int==4+stored*stride,payload<=bytes.len()<=u32::MAX,
            ends@==bytes@.subrange(4+i*stride,payload as int),ends.len()==(stored-i)*stride,
            previous==if i==0 {0int}else{words::le_word(bytes@,4+(i-1)*stride+KEY)},
            previous<=bytes.len()-payload,
            data@==bytes@.subrange(payload+previous,bytes.len() as int),
            data.len()==bytes.len()-payload-previous,
            children_valid(bytes@,layout,0,(budget-1) as nat,SEM)
                ==children_valid(bytes@,layout,i,(budget-1) as nat,SEM),
        decreases stored-i,
    {
        proof {
            assert(i<stored) by(nonlinear_arith)
                requires ends.len()==(stored-i)*stride,ends.len()>=stride,stride>0;
            assert(4+i*stride+stride<=payload) by(nonlinear_arith)
                requires 0<=i<stored,payload==4+stored*stride,stride>0;
        }
        let end=crate::words::directory_word(ends,KEY) as usize;
        proof {
            crate::words::word_subrange(bytes@,4+i*stride,payload as int,KEY as int);
            assert(end==words::le_word(bytes@,4+i*stride+KEY));
            string_interval(bytes@,layout,leaf,payload,count,MAP,KEY,SEM,i);
            let span=child_span(bytes@,layout,i);
            assert(span.start==payload+previous);
            assert(span.end==payload+end);
            assert(previous<=end<=bytes.len()-payload);
        }
        let len=crate::words::sub_bounded(end,previous);
        let region=&data[..len];
        let checked=scan_leaf(region,true,false,SEM,Ghost(leaf));
        proof {
            let span=child_span(bytes@,layout,i);
            assert(region@ =~= bytes@.subrange(span.start as int,span.end as int));
            leaf_child_outcome(bytes@,layout,leaf,i,span,budget as nat,SEM,checked);
            children_split(bytes@,layout,i,(budget-1) as nat,SEM);
        }
        if let Err(error)=checked {
            let at=add_bounded(payload as usize,previous) as u32;
            return Err(error.rebase(at));
        }
        proof {
            assert(4+(i+1)*stride==4+i*stride+stride) by(nonlinear_arith);
            assert((stored-i)*stride-stride==(stored-(i+1))*stride) by(nonlinear_arith);
            assert(ends@.subrange(stride as int,ends.len() as int)
                =~= bytes@.subrange(4+(i+1)*stride,payload as int));
        }
        ends=&ends[stride..];
        data=&data[len..];
        previous=end;
        proof {i=i+1;}
    }
    assert(i==stored);
    if !MAP {
        let checked=scan_leaf(data,true,false,SEM,Ghost(leaf));
        proof {
            let span=child_span(bytes@,layout,i);
            string_interval(bytes@,layout,leaf,payload,count,MAP,KEY,SEM,i);
            assert(span.start==payload+previous && span.end==bytes.len());
            assert(data@ =~= bytes@.subrange(span.start as int,span.end as int));
            leaf_child_outcome(bytes@,layout,leaf,i,span,budget as nat,SEM,checked);
            children_split(bytes@,layout,i,(budget-1) as nat,SEM);
            reveal_with_fuel(children_valid,1);
        }
        if let Err(error)=checked {
            let at=add_bounded(payload as usize,previous) as u32;
            return Err(error.rebase(at));
        }
    } else {proof {reveal_with_fuel(children_valid,1);}}
    Ok(true)
}

#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
fn scan_leaf_children(bytes:&[u8],layout:&Layout,parts:&Parts,budget:usize,semantic:bool)
    ->(result:Result<bool>)
    requires parts_valid(*parts,bytes@,*layout,semantic), 0<budget<=MAX_DEPTH,
    ensures match result {
        Ok(done)=>done ==> children_valid(bytes@,*layout,0,(budget-1) as nat,semantic),
        Err(error)=>!children_valid(bytes@,*layout,0,(budget-1) as nat,semantic)
            && subtree_fault(bytes@,*layout,budget as nat,semantic,error),
    },
{
    hide(children_valid);hide(schema_valid);hide(child_valid);hide(descriptor_valid);
    hide(search::lex_less);hide(search::keys_ordered);hide(search::keys_canonical);
    let leaf=match layout.head() {
        LayoutHead::List{element}=>element,
        LayoutHead::Map{value,..}=>value,
        _=>return Ok(false),
    };
    if leaf.tag()>5 || budget<=1 {return Ok(false);}
    let n=count(parts,layout,Ghost(bytes@),semantic);
    if n==0 {proof {reveal_with_fuel(children_valid,1);}return Ok(true);}
    proof {
        assert(matches!(leaf.head(),LayoutHead::Leaf(_)|LayoutHead::Str));
        reveal_with_fuel(descriptor_valid,2);
        assert(descriptor_valid(leaf,(MAX_DEPTH-1) as nat));
        assert(descriptor_valid(leaf,MAX_DEPTH as nat));
    }
    check_layout(&leaf,MAX_DEPTH)?;
    let is_str=matches!(leaf.head(),LayoutHead::Str);
    let is_bool=matches!(leaf.head(),LayoutHead::Leaf(crate::Leaf::Bool));
    if is_str {
        match parts {
            Parts::List(p)=>return if semantic {
                scan_strings::<false,0,true>(bytes,p.payload.start,Ghost(*layout),Ghost(leaf),Ghost(n),Ghost(budget))
            }else{
                scan_strings::<false,0,false>(bytes,p.payload.start,Ghost(*layout),Ghost(leaf),Ghost(n),Ghost(budget))
            },
            Parts::Map(p) if p.key_width==32 => {
                return if semantic {
                    scan_strings::<true,32,true>(bytes,p.payload.start,Ghost(*layout),Ghost(leaf),Ghost(n),Ghost(budget))
                }else{
                    scan_strings::<true,32,false>(bytes,p.payload.start,Ghost(*layout),Ghost(leaf),Ghost(n),Ghost(budget))
                };
            },
            _=>{},
        }
    }
    let (payload,width,key,stored_last)=match parts {
        Parts::List(p)=>(p.payload.start,p.element_width,0u32,false),
        Parts::Map(p)=>(p.payload.start,p.value_width,p.key_width,true),
        _=>{assert(false);return Ok(false);},
    };
    proof {
        let tail=width.unwrap_or(4) as int;
        if stored_last {
            assert(4+n as int*(key+tail)<=bytes.len());
            assert(4+key+tail<=bytes.len()) by(nonlinear_arith)
                requires n>0,4+n as int*(key+tail)<=bytes.len(),key+tail>0;
        }
        assert(key as int+tail<=usize::MAX);assert(4+key as int<=usize::MAX);
    }
    let g=LeafGeometry{count:n,payload,width,#[cfg(verus_only)] key,stride:add_bounded(key as usize,width.unwrap_or(4) as usize),stored_last};
    let mut position=LeafPosition{index:0,at:if width.is_none(){payload as usize}else{add_bounded(4,key as usize)},cursor:add_bounded(4,key as usize)};
    assert(leaf_scan_state(bytes@,*layout,leaf,g,position,semantic)) by {reveal(leaf_scan_state);}
    while position.index<n
        invariant budget>1,budget<=MAX_DEPTH,n==g.count,n==child_count(bytes@,*layout),
            leaf_scan_state(bytes@,*layout,leaf,g,position,semantic),
            is_str==matches!(leaf.head(),LayoutHead::Str),
            is_bool==matches!(leaf.head(),LayoutHead::Leaf(crate::Leaf::Bool)),
            children_valid(bytes@,*layout,0,(budget-1) as nat,semantic)
                ==children_valid(bytes@,*layout,position.index as int,(budget-1) as nat,semantic),
        decreases n-position.index,
    {
        let ghost index=position.index;
        let (span,next)=stream_leaf_span(bytes,&g,position,Ghost(semantic),Ghost(*layout),Ghost(leaf));
        let region=&bytes[span.start as usize..span.end as usize];
        let checked=scan_leaf(region,is_str,is_bool,semantic,Ghost(leaf));
        proof {
            leaf_child_outcome(bytes@,*layout,leaf,index as int,span,budget as nat,semantic,checked);
            children_split(bytes@,*layout,index as int,(budget-1) as nat,semantic);
        }
        match checked {
            Ok(())=>position=next,
            Err(error)=>return Err(error.rebase(span.start)),
        }
    }
    proof {reveal(leaf_scan_state);reveal_with_fuel(children_valid,1);}
    Ok(true)
}

// True means every child has already been validated without inspecting its
// unrestricted payload. False keeps the ordinary bounded traversal.
#[inline(always)]
fn skip_unrestricted(_bytes:&[u8],layout:&Layout,_parts:&Parts,budget:usize,_semantic:bool)
    ->(result:bool)
    requires parts_valid(*_parts,_bytes@,*layout,_semantic), 0<budget<=MAX_DEPTH,
    ensures result ==> children_valid(_bytes@,*layout,0,(budget-1) as nat,_semantic),
{
    if let Layout::Flat{checked}=layout {
        checked.witness();
        proof { reveal(crate::layout_head::head_spec); }
        let tag=checked.header(0);
        if tag==9 && checked.is_unrestricted() && checked.header(2) as usize<=budget {
            proof { unrestricted_struct_suffix(_bytes@,*checked,0,budget as nat,_semantic); }
            return true;
        }
        if tag==7 || tag==8 {
            let child=checked.only_child();
            if child.is_unrestricted() && (child.header(2) as usize)<budget {
                proof {
                    reveal(crate::layout_head::head_spec);
                    unrestricted_collection_suffix(_bytes@,*layout,child,0,budget as nat,_semantic);
                }
                return true;
            }
        }
    }
    false
}

spec fn frame_bytes(bytes: Seq<u8>, frame: Frame) -> Seq<u8> {
    bytes.subrange(frame.region.start as int, frame.region.end as int)
}

spec fn frame_valid(bytes: Seq<u8>, frame: Frame, semantic: bool) -> bool {
    frame.region.start <= frame.region.end <= bytes.len()
        && match frame.parts {
            None => frame.next == 0,
            Some(parts) => parts_valid(parts, frame_bytes(bytes, frame), frame.layout, semantic)
                && frame.next <= child_count(frame_bytes(bytes, frame), frame.layout),
        }
}

spec fn frame_work(bytes: Seq<u8>, frame: Frame, budget: nat, semantic: bool) -> bool {
    match frame.parts {
        None => schema_valid(frame_bytes(bytes, frame), frame.layout, budget, semantic),
        Some(_) => children_valid(frame_bytes(bytes, frame), frame.layout,
            frame.next as int, (budget - 1) as nat, semantic),
    }
}

spec fn pending(bytes: Seq<u8>, frames: Seq<Frame>, len: nat, budget: nat, semantic: bool) -> bool
    decreases len,
{
    if len == 0 { true } else {
        pending(bytes, frames, (len - 1) as nat, budget, semantic)
            && frame_work(bytes, frames[len - 1], (budget - len + 1) as nat, semantic)
    }
}

// A conservative finite work bound. Ghost arithmetic is erased; no fuel counter
// or exponential integer is stored or calculated at runtime.
spec fn weight(budget: nat) -> nat
    decreases budget,
{
    if budget == 0 { 1nat }
    else { (u32::MAX as nat + 1) * (weight((budget - 1) as nat) + 1) + 2 }
}

spec fn frame_weight(bytes: Seq<u8>, frame: Frame, budget: nat) -> nat {
    match frame.parts {
        None => weight(budget),
        Some(_) => ((child_count(frame_bytes(bytes, frame), frame.layout) - frame.next) as nat)
            * (weight((budget - 1) as nat) + 1) + 1,
    }
}

spec fn pending_weight(bytes: Seq<u8>, frames: Seq<Frame>, len: nat, budget: nat) -> nat
    decreases len,
{
    if len == 0 { 0nat } else {
        pending_weight(bytes, frames, (len - 1) as nat, budget)
            + frame_weight(bytes, frames[len - 1], (budget - len + 1) as nat)
    }
}

proof fn equal_prefix_weight(bytes: Seq<u8>, a: Seq<Frame>, b: Seq<Frame>, len: nat, budget: nat, semantic: bool)
    requires len <= a.len(), len <= b.len(), forall|i: int| 0 <= i < len ==> a[i] == b[i],
    ensures pending_weight(bytes, a, len, budget) == pending_weight(bytes, b, len, budget),
        pending(bytes, a, len, budget, semantic) == pending(bytes, b, len, budget, semantic),
    decreases len,
{
    if len > 0 { equal_prefix_weight(bytes, a, b, (len - 1) as nat, budget, semantic); }
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn entered_frame(bytes:Seq<u8>,frame:Frame,parts:Parts,next:u32,b:nat,semantic:bool)
    requires frame_valid(bytes,frame,semantic),frame.parts.is_none(), b>0,
        parts_valid(parts,frame_bytes(bytes,frame),frame.layout,semantic),
        next==0 || (next==child_count(frame_bytes(bytes,frame),frame.layout)
            && children_valid(frame_bytes(bytes,frame),frame.layout,0,(b-1) as nat,semantic)),
    ensures ({let updated=Frame{parts:Some(parts),next,..frame};
        frame_valid(bytes,updated,semantic)
        && frame_work(bytes,frame,b,semantic)==frame_work(bytes,updated,b,semantic)
        && frame_weight(bytes,updated,b)<frame_weight(bytes,frame,b)
    }),
{
    let region=frame_bytes(bytes,frame);
    let n=child_count(region,frame.layout);
    assert(0<=n<=u32::MAX);
    if next>0 { reveal_with_fuel(children_valid,1); }
    let updated=Frame{parts:Some(parts),next,..frame};
    assert(frame_work(bytes,frame,b,semantic)==frame_work(bytes,updated,b,semantic));
    assert(weight(b)==(u32::MAX as nat+1)*(weight((b-1) as nat)+1)+2);
    assert(frame_weight(bytes,updated,b)<frame_weight(bytes,frame,b)) by(nonlinear_arith)
        requires 0<=n<=u32::MAX, 0<=next<=n,
            frame_weight(bytes,updated,b)==(n-next)*(weight((b-1) as nat)+1)+1,
            frame_weight(bytes,frame,b)==(u32::MAX as nat+1)*(weight((b-1) as nat)+1)+2;
}

proof fn lift_subtree_fault(bytes:Seq<u8>,root:Layout,depth:nat,region:Span,node:Layout,
    budget:nat,semantic:bool,error:Error)
    requires faults_lift(bytes,root,depth,region,node,budget,semantic),
        subtree_fault(bytes.subrange(region.start as int,region.end as int),node,budget,semantic,error),
    ensures subtree_fault(bytes,root,depth,semantic,absolute_fault(error,region.start as int)),
{
    reveal(faults_lift);
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn wrapper_schema(bytes:Seq<u8>,checked:crate::CheckedLayout,budget:nat,semantic:bool)
    requires checked.valid(), bytes.len()<=u32::MAX, 0<budget<=MAX_DEPTH,
        checked.view()[checked.root_index()]==9,
        checked.view()[checked.root_index()+3]==0,
        checked.view()[checked.root_index()+4]==1,
    ensures local_valid(bytes,Layout::Flat{checked},semantic),
        schema_valid(bytes,Layout::Flat{checked},budget,semantic)
            == (budget>1 && schema_valid(bytes,
                child_layout(Layout::Flat{checked},0),(budget-1) as nat,semantic)),
        child_span(bytes,Layout::Flat{checked},0)==(Span{start:0,end:bytes.len() as u32}),
{
    checked.metadata();
    crate::layout::certified_descriptor(checked,MAX_DEPTH as nat);
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::flat_layout::struct_fields,2);
    assert(checked.view()[checked.root_index()+1]==0);
    reveal_with_fuel(children_valid,3);
    assert(bytes.subrange(0,bytes.len() as int) =~= bytes);
}

// A certified leaf needs only its exact width and restricted byte domain.
#[inline(always)]
#[verifier::spinoff_prover]
fn flat_leaf(bytes:&[u8],checked:crate::CheckedLayout<'static>,semantic:bool,Ghost(budget):Ghost<usize>)
    ->(result:Result<()>)
    requires bytes.len()<=u32::MAX,0<budget<=MAX_DEPTH,
        1<=checked.view()[checked.root_index()]<=5,
    ensures result.is_ok()==schema_valid(bytes@,Layout::Flat{checked},budget as nat,semantic),
        result.is_err() ==> subtree_fault(bytes@,Layout::Flat{checked},budget as nat,semantic,result.unwrap_err()),
{
    checked.witness();
    proof {
        crate::layout::certified_descriptor(checked,MAX_DEPTH as nat);
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(children_valid,2);
    }
    let ghost layout=Layout::Flat{checked};
    let tag=checked.header(0);
    if tag!=5 {
        let width=checked.header(1);
        proof {crate::layout::certified_width(checked);}
        if bytes.len()<width as usize {
            proof {reveal(local_fault);reveal(located_fault);}
            return Err(Error::new(ErrorKind::Truncated,0));
        }
        if bytes.len()!=width as usize {
            proof {reveal(local_fault);reveal(located_fault);}
            return Err(Error::new(ErrorKind::InvalidBoundary,width));
        }
    }
    let outcome=scan_leaf(bytes,tag==5,tag==3,semantic,Ghost(layout));
    if let Err(error)=outcome {
        proof {reveal(local_fault);reveal(located_fault);}
        return Err(error);
    }
    Ok(())
}

// Common leaf containers and transparent one-field wrappers do not need a
// traversal stack. Complex shapes continue through the bounded general walker.
#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
fn simple_flat(bytes:&[u8],checked:crate::CheckedLayout<'static>,budget:usize,semantic:bool)
    ->(result:Result<bool>)
    requires bytes.len()<=u32::MAX, 0<budget<=MAX_DEPTH,
    ensures match result {
        Ok(done)=>done ==> schema_valid(bytes@,Layout::Flat{checked},budget as nat,semantic),
        Err(error)=>!schema_valid(bytes@,Layout::Flat{checked},budget as nat,semantic)
            && subtree_fault(bytes@,Layout::Flat{checked},budget as nat,semantic,error),
    },
    decreases budget,
{
    checked.witness();
    proof {crate::layout::certified_descriptor(checked,MAX_DEPTH as nat);}
    let tag=checked.header(0);
    let layout=Layout::Flat{checked};
    if matches!(tag,1..=5) {
        flat_leaf(bytes,checked,semantic,Ghost(budget))?;
        return Ok(true);
    }
    if tag==9 && checked.header(3)==0 && checked.header(4)==1 {
        proof { wrapper_schema(bytes@,checked,budget as nat,semantic); }
        if budget==1 {return Err(ErrorKind::DepthExceeded.into());}
        let child=checked.child(0,true);
        proof {reveal(crate::layout_head::head_spec);}
        let result=simple_flat(bytes,child,budget-1,semantic);
        proof {
            if result.is_err() {
                assert(bytes@.subrange(0,bytes.len() as int) =~= bytes@);
                assert(faults_lift(bytes@,layout,budget as nat,Span{start:0,end:bytes.len() as u32},
                    layout,budget as nat,semantic)) by {reveal(faults_lift);}
                lift_child_faults(bytes@,layout,budget as nat,Span{start:0,end:bytes.len() as u32},
                    layout,budget as nat,semantic,0);
                lift_subtree_fault(bytes@,layout,budget as nat,Span{start:0,end:bytes.len() as u32},
                    Layout::Flat{checked:child},(budget-1) as nat,semantic,result.unwrap_err());
            }
        }
        return result;
    }
    if tag==7 {
        let child=checked.only_child();
        if child.is_unrestricted() && (child.header(2) as usize)<budget {
            proof {reveal(crate::layout_head::head_spec);}
            match partition_list(bytes,&layout) {
                Ok(_)=>{},
                Err(error)=>{
                    proof {reveal(local_fault);reveal(located_fault);}
                    return Err(error);
                },
            }
            proof {
                assert(local_valid(bytes@,layout,semantic));
                unrestricted_collection_suffix(bytes@,layout,child,0,budget as nat,semantic);
            }
            return Ok(true);
        }
    }
    if tag==9 {
        if !checked.is_unrestricted() || checked.header(2) as usize>budget {return Ok(false);}
    } else if tag==7 || tag==8 {
        let child=checked.only_child();
        if !child.is_unrestricted() && child.header(0)>5 {return Ok(false);}
        if budget<=1 {return Ok(false);}
    } else if tag>5 {return Ok(false);}
    let parts=match enter(bytes,&layout,semantic) {
        Ok(parts)=>parts,
        Err(error)=>{
            proof {reveal(located_fault);reveal(local_fault);}
            return Err(error);
        },
    };
    if tag<=5 {
        proof {reveal(crate::layout_head::head_spec);reveal_with_fuel(children_valid,2);}
        return Ok(true);
    }
    let done=if skip_unrestricted(bytes,&layout,&parts,budget,semantic) {true}
        else {scan_leaf_children(bytes,&layout,&parts,budget,semantic)?};
    Ok(done)
}

proof fn schema_children(bytes:Seq<u8>,layout:Layout,budget:nat,semantic:bool)
    requires budget>0,local_valid(bytes,layout,semantic),
    ensures schema_valid(bytes,layout,budget,semantic)
        ==children_valid(bytes,layout,0,(budget-1) as nat,semantic),
{
}

proof fn rejected_schema(bytes:Seq<u8>,layout:Layout,budget:nat,semantic:bool)
    requires budget==0 || !local_valid(bytes,layout,semantic),
    ensures !schema_valid(bytes,layout,budget,semantic),
{
}

proof fn selected_child_schema(bytes:Seq<u8>,layout:Layout,index:int,span:Span,child:Layout,budget:nat,semantic:bool)
    requires span==child_span(bytes,layout,index),child==child_layout(layout,index),
        span.start<=span.end<=bytes.len(),
    ensures child_valid(bytes,layout,index,budget,semantic)
        ==schema_valid(bytes.subrange(span.start as int,span.end as int),child,budget,semantic),
{
}

// Certified trees can use one bounded call frame per active node.
#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
fn validate_node(bytes:&[u8],layout:&Layout,budget:usize,semantic:bool)->(result:Result<()>)
    requires bytes.len()<=u32::MAX,0<budget<=MAX_DEPTH,
    ensures result.is_ok()==schema_valid(bytes@,*layout,budget as nat,semantic),
        result.is_err() ==> subtree_fault(bytes@,*layout,budget as nat,semantic,result.unwrap_err()),
    decreases budget,
{
    hide(search::lex_less);hide(search::keys_ordered);hide(search::keys_canonical);hide(utf8::utf8_valid);
    hide(schema_valid);
    if let Layout::Flat{checked}=layout {
        match simple_flat(bytes,*checked,budget,semantic) {
            Ok(true)=>return Ok(()),
            Ok(false)=>{},
            Err(error)=>return Err(error),
        }
    }
    let ghost root_region=Span{start:0,end:bytes.len() as u32};
    proof {
        assert(bytes@.subrange(0,bytes.len() as int) =~= bytes@);
        assert(faults_lift(bytes@,*layout,budget as nat,root_region,*layout,budget as nat,semantic)) by {reveal(faults_lift);}
    }
    let parts=match enter(bytes,layout,semantic) {
        Ok(parts)=>parts,
        Err(error)=>{
            proof {
                rejected_schema(bytes@,*layout,budget as nat,semantic);
                lift_local_fault(bytes@,*layout,budget as nat,root_region,*layout,budget as nat,semantic,error);
            }
            return Err(error);
        },
    };
    let n=count(&parts,layout,Ghost(bytes@),semantic);
    proof {schema_children(bytes@,*layout,budget as nat,semantic);}
    let mut i=0u32;
    while i<n
        invariant 0<budget<=MAX_DEPTH,bytes.len()<=u32::MAX,
            root_region.start==0,root_region.end==bytes.len(),
            faults_lift(bytes@,*layout,budget as nat,root_region,*layout,budget as nat,semantic),
            parts_valid(parts,bytes@,*layout,semantic),local_valid(bytes@,*layout,semantic),
            n==child_count(bytes@,*layout),i<=n,
            schema_valid(bytes@,*layout,budget as nat,semantic)
                ==children_valid(bytes@,*layout,0,(budget-1) as nat,semantic),
            children_valid(bytes@,*layout,0,(budget-1) as nat,semantic)
                ==children_valid(bytes@,*layout,i as int,(budget-1) as nat,semantic),
        decreases n-i,
    {
        let (span,node)=child(bytes,layout,&parts,i,semantic)?;
        proof {selected_child_schema(bytes@,*layout,i as int,span,node,(budget-1) as nat,semantic);}
        if budget==1 {
            proof {
                children_split(bytes@,*layout,i as int,0,semantic);
                rejected_schema(bytes@.subrange(span.start as int,span.end as int),node,0,semantic);
                assert(!child_valid(bytes@,*layout,i as int,0,semantic));
            }
            return Err(ErrorKind::DepthExceeded.into());
        }
        let candidate=&bytes[span.start as usize..span.end as usize];
        let outcome=validate_node(candidate,&node,budget-1,semantic);
        proof {
            assert(candidate@ =~= bytes@.subrange(span.start as int,span.end as int));
            children_split(bytes@,*layout,i as int,(budget-1) as nat,semantic);
        }
        if let Err(error)=outcome {
            proof {
                assert(!child_valid(bytes@,*layout,i as int,(budget-1) as nat,semantic));
                assert(bytes@.subrange(0,bytes.len() as int) =~= bytes@);
                lift_child_faults(bytes@,*layout,budget as nat,root_region,*layout,budget as nat,semantic,i as int);
                lift_subtree_fault(bytes@,*layout,budget as nat,span,node,(budget-1) as nat,semantic,error);
            }
            return Err(error.rebase(span.start));
        }
        assert(child_valid(bytes@,*layout,i as int,(budget-1) as nat,semantic));
        i+=1;
    }
    proof {reveal_with_fuel(children_valid,1);}
    Ok(())
}

// The arbitrary-layout stack invariant composes each arbitrary layout relation.
#[verifier::spinoff_prover]
#[verifier::rlimit(480)]
fn validate_stack(bytes:&[u8],layout:&Layout,max_depth:usize,semantic:bool)->(result:Result<()>)
    requires bytes.len()<=u32::MAX,0<max_depth<=MAX_DEPTH,
        descriptor_valid(*layout,MAX_DEPTH as nat),
    ensures result.is_ok()==schema_valid(bytes@,*layout,max_depth as nat,semantic),
        result.is_err() ==> subtree_fault(bytes@,*layout,max_depth as nat,semantic,result.unwrap_err()),
{
    let root = Frame { layout: *layout, region: Span { start: 0, end: bytes.len() as u32 },
        parts: None, next: 0 };
    let mut frames = [root; MAX_DEPTH];
    let mut len = 1usize;
    proof {
        assert(frame_bytes(bytes@, root) =~= bytes@);
        assert(faults_lift(bytes@, *layout, max_depth as nat, root.region,
            *layout, max_depth as nat, semantic)) by { reveal(faults_lift); }
        assert(stack_faults(bytes@, *layout, max_depth as nat, frames@, len as nat, semantic)) by {
            reveal(stack_faults);
        }
        reveal_with_fuel(pending, 2);
    }
    while len > 0
        invariant
            0 <= len <= max_depth <= MAX_DEPTH,
            max_depth > 0, bytes.len() <= u32::MAX,
            frames.len() == MAX_DEPTH,
            forall|i: int| 0 <= i < len ==> frame_valid(bytes@, frames@[i], semantic),
            stack_faults(bytes@, *layout, max_depth as nat, frames@, len as nat, semantic),
            schema_valid(bytes@, *layout, max_depth as nat, semantic)
                == pending(bytes@, frames@, len as nat, max_depth as nat, semantic),
        decreases pending_weight(bytes@, frames@, len as nat, max_depth as nat),
    {
        let ghost old_frames = frames@;
        let level = len - 1;
        let frame = frames[level];
        let region = &bytes[frame.region.start as usize..frame.region.end as usize];
        proof {
            assert(faults_lift(bytes@, *layout, max_depth as nat, frame.region,
                frame.layout, (max_depth - level) as nat, semantic)) by { reveal(stack_faults); }
        }
        match frame.parts {
            None => {
                let parts = match enter(region, &frame.layout, semantic) {
                    Ok(parts) => parts,
                    Err(error) => {
                        proof {
                            lift_local_fault(bytes@, *layout, max_depth as nat, frame.region,
                                frame.layout, (max_depth - level) as nat, semantic, error);
                        }
                        return Err(error.rebase(frame.region.start));
                    },
                };
                let skip = if skip_unrestricted(region,&frame.layout,&parts,max_depth-level,semantic) { true }
                    else { match scan_leaf_children(region,&frame.layout,&parts,max_depth-level,semantic) {
                        Ok(done)=>done,
                        Err(error)=>{
                            proof { lift_subtree_fault(bytes@,*layout,max_depth as nat,frame.region,
                                frame.layout,(max_depth-level) as nat,semantic,error); }
                            return Err(error.rebase(frame.region.start));
                        },
                    } };
                let next = if skip { count(&parts,&frame.layout,Ghost(region@),semantic) } else { 0 };
                let updated = Frame { parts: Some(parts), next, ..frame };
                frames[level] = updated;
                proof {
                    preserve_stack_faults(bytes@, *layout, max_depth as nat,
                        old_frames, frames@, len as nat, len as nat, semantic);
                    equal_prefix_weight(bytes@, old_frames, frames@, level as nat, max_depth as nat, semantic);
                    entered_frame(bytes@,frame,parts,next,(max_depth-level) as nat,semantic);
                    reveal_with_fuel(pending,2);
                    reveal_with_fuel(pending_weight, 2);
                }
            },
            Some(parts) => {
                let children = count(&parts, &frame.layout, Ghost(region@), semantic);
                if frame.next == children {
                    len -= 1;
                    proof {
                        preserve_stack_faults(bytes@, *layout, max_depth as nat,
                            old_frames, frames@, (len + 1) as nat, len as nat, semantic);
                    }
                } else {
                    if len == max_depth {
                        proof {
                            reveal_with_fuel(children_valid, 2);
                            assert(!children_valid(region@, frame.layout, frame.next as int, 0, semantic)) by {
                                let span = child_span(region@, frame.layout, frame.next as int);
                                assert(!schema_valid(region@.subrange(span.start as int, span.end as int),
                                    child_layout(frame.layout, frame.next as int), 0, semantic));
                                assert(!child_valid(region@, frame.layout, frame.next as int, 0, semantic));
                            }
                        }
                        return Err(Error::new(ErrorKind::DepthExceeded, Error::NO_OFFSET));
                    }
                    let (span, next_layout) = child(region, &frame.layout, &parts, frame.next, semantic)?;
                    let updated = Frame { next: frame.next + 1, ..frame };
                    frames[level] = updated;
                    let child = Frame { layout: next_layout,
                        region: Span { start: frame.region.start + span.start,
                            end: frame.region.start + span.end }, parts: None, next: 0 };
                    frames[len] = child;
                    len += 1;
                    proof {
                        assert(frame_bytes(bytes@, child) =~= region@.subrange(span.start as int, span.end as int));
                        lift_child_faults(bytes@, *layout, max_depth as nat, frame.region,
                            frame.layout, (max_depth - level) as nat, semantic, frame.next as int);
                        preserve_stack_faults(bytes@, *layout, max_depth as nat,
                            old_frames, frames@, (len - 1) as nat, len as nat, semantic);
                        equal_prefix_weight(bytes@, old_frames, frames@, level as nat, max_depth as nat, semantic);
                        children_split(region@, frame.layout, frame.next as int,
                            (max_depth - level - 1) as nat, semantic);
                        reveal_with_fuel(pending, 3);
                        assert(pending(bytes@, old_frames, (len - 1) as nat, max_depth as nat, semantic)
                            == pending(bytes@, frames@, len as nat, max_depth as nat, semantic));
                        let b = (max_depth - level) as nat;
                        assert(frame_weight(bytes@, updated, b) + weight((b - 1) as nat)
                            < frame_weight(bytes@, frame, b)) by(nonlinear_arith)
                            requires frame.next < children,
                                frame_weight(bytes@, updated, b) == (children - frame.next - 1)
                                    * (weight((b - 1) as nat) + 1) + 1,
                                frame_weight(bytes@, frame, b) == (children - frame.next)
                                    * (weight((b - 1) as nat) + 1) + 1;
                        reveal_with_fuel(pending_weight, 3);
                    }
                }
            },
        }
    }
    Ok(())
}

// The stack invariant composes every layout relation in one SMT query.
#[verifier::spinoff_prover]
#[verifier::rlimit(480)]
fn validate_tree(bytes: &[u8], layout: &Layout, max_depth: usize, semantic: bool) -> (result: Result<()>)
    ensures
        validate_layout_iff(result, bytes@, *layout, max_depth, semantic),
        validate_layout_total(result, bytes@, *layout, max_depth, semantic),
        semantic ==> validate_schema_iff(result, bytes@, *layout, max_depth),
        result.is_ok() == (max_depth <= MAX_DEPTH
            && schema_valid(bytes@, *layout, max_depth as nat, semantic)),
        semantic ==> validate_schema_total(result, bytes@, *layout, max_depth),
        validate_fault(result, bytes@, *layout, max_depth, semantic),
{
    check_layout(layout, MAX_DEPTH)?;
    if max_depth == 0 || max_depth > MAX_DEPTH {
        return Err(Error::new(ErrorKind::DepthExceeded, Error::NO_OFFSET));
    }
    if bytes.len() > u32::MAX as usize {
        return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
    }
    if let Layout::Flat{..}=layout {
        return validate_node(bytes,layout,max_depth,semantic);
    }
    validate_stack(bytes,layout,max_depth,semantic)
}

/// Validates an exact subtree, including all active children and map key order.
///
/// Bounds active data nodes by [`MAX_DEPTH`] without allocation. Certified flat
/// layouts use bounded recursion; arbitrary layouts use a fixed frame array.
/// The root consumes one depth unit. Metadata is checked at `MAX_DEPTH` before the requested
/// `max_depth` (which must be in `1..=MAX_DEPTH`) and wire length. An active
/// child beyond that budget returns `DepthExceeded` without a byte offset.
/// Inactive option payloads and empty collections consume no child depth.
/// Each node's partition is checked before its children; map keys are checked
/// before values. Struct children are visited in packed wire order. Child
/// faults are relative to this slice and rebased through checked parent intervals.
/// Validation preserves every byte and never forms a string reference.
pub fn validate(bytes: &[u8], layout: &Layout, max_depth: usize) -> (result: Result<()>)
    ensures validate_schema_iff(result, bytes@, *layout, max_depth), validate_schema_total(result, bytes@, *layout, max_depth),
        validate_fault(result, bytes@, *layout, max_depth, true),
{
    validate_tree(bytes, layout, max_depth, true)
}

}
