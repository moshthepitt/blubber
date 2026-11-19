//! Offset-only paths, exact resolution, checked planning, and ordered replacement.

#![cfg_attr(verus_only, allow(missing_docs))]

use crate::layout::fixed_width;
use crate::validation::{child, count, enter, Parts};
use crate::LayoutHead;
use crate::{
    checked_add, checked_delta, checked_mul, checked_u32, read_u32, validate_map_keys, Error,
    ErrorKind, Layout, Result, Span, MAX_DEPTH,
};
#[cfg(verus_only)]
use crate::{layout::width_spec, validation, words};
#[path = "indexed_insert.rs"]
mod indexed_insert;
#[cfg(verus_only)]
#[path = "nested.rs"]
mod nested;
use indexed_insert::IndexedInsert;
#[path = "indexed_remove.rs"]
mod indexed_remove;
use indexed_remove::IndexedSplice;
#[path = "fixed_map.rs"]
mod fixed_map;
use fixed_map::MapChange;
pub use fixed_map::MapEdit;
#[path = "indexed_map.rs"]
mod indexed_map;
use indexed_map::IndexedMap;
#[path = "prepared.rs"]
mod prepared;
pub use prepared::{resolve_account_edit, resolve_edit, Prepared, Resolved};
#[path = "list_patch.rs"]
mod list_patch;
pub use list_patch::{
    CheckedListApply, CheckedListMoved, CheckedListPlan, CheckedListPrepared, CheckedListReady,
};
#[cfg(any(kani, verus_only))]
#[path = "list_patch_models.rs"]
mod list_patch_models;
#[cfg(kani)]
pub use list_patch_models::{__list_insert_model, __list_splice_model};
#[cfg(any(kani, verus_only))]
#[path = "indexed_map_models.rs"]
mod indexed_map_models;
#[cfg(kani)]
pub use indexed_map_models::__map_patch_model;
#[path = "fixed_walk.rs"]
mod fixed_walk;
pub use fixed_walk::{
    fixed_chunk_base, fixed_chunk_end, fixed_list_payload, fixed_read_payload, fixed_slot_region,
    split_fixed_chunk, split_fixed_map_entry, split_fixed_slot,
};
#[path = "indexed_walk.rs"]
mod indexed_walk;
pub use indexed_walk::{
    indexed_coordinate, indexed_map_key, indexed_next_end, indexed_read_word, indexed_slot_region,
    indexed_string_window,
};
#[path = "tail_edit.rs"]
mod tail_edit;
pub use tail_edit::{account_used, prepare_tail_append, TailAppend};
#[path = "tail_remove.rs"]
mod tail_remove;
pub use tail_remove::{prepare_tail_remove, TailRemove};
#[path = "fixed_insert.rs"]
mod fixed_insert;
pub use fixed_insert::{prepare_fixed_insert, FixedInsert};
#[path = "subtree_write.rs"]
mod subtree_write;
pub use subtree_write::{prepare_byte_replacement, SubtreeWrite};
#[path = "ancestor_edit.rs"]
mod ancestor_edit;
pub use ancestor_edit::{
    AncestorApply, AncestorMoved, AncestorPlan, AncestorPrepared, AncestorReady,
};
#[path = "subtree_checked.rs"]
mod subtree_checked;
pub use subtree_checked::{
    prepare_checked_byte_replacement, CheckedSubtreeMoved, CheckedSubtreePlan, CheckedSubtreeReady,
    CheckedSubtreeWrite,
};
#[path = "string_edit.rs"]
mod string_edit;
#[cfg(verus_only)]
use string_edit::string_result;
pub use string_edit::{prepare_string_write, StringWrite};
#[cfg(verus_only)]
use tail_edit::tail_result;
#[path = "bulk_list.rs"]
mod bulk_list;
pub use bulk_list::{BulkLayout, BulkListPlan, BulkListSource, BulkSlots};
#[path = "bulk_list_pass.rs"]
mod bulk_list_pass;
pub use bulk_list_pass::{HeaderBulkLayout, HeaderBulkPlan, HeaderBulkSlots, HeaderBulkSource};
#[path = "rewrite.rs"]
mod rewrite;
pub use rewrite::{resolve_rewrite_tail, RewritePlan, RewriteStream, RewriteTail};
#[cfg(verus_only)]
#[path = "admission.rs"]
mod admission;
#[cfg(verus_only)]
#[path = "lazy_list.rs"]
mod lazy_list;
#[cfg(verus_only)]
#[path = "witnesses.rs"]
mod witnesses;

#[cfg(kani)]
pub use subtree_checked::__ancestor_pass_model;
use vstd::prelude::*;

verus! {

/// One projection from a parent encoding to an exact child encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// A struct field's declaration ordinal, before fixed-field packing.
    Field(u16),
    /// A list element or map value in physical order; never a map key.
    Index(u32),
    /// An option's present payload; selecting None returns `MissingValue`.
    Some,
}

/// Bounded projection path. The root consumes one of the depth units.
///
/// An overflowing push records a sticky error. Even if its returned error is
/// ignored, subsequent resolution fails instead of resolving a shorter path.
#[derive(Clone, Copy, Debug)]
pub struct InlinePath {
    steps: [Step; MAX_DEPTH],
    len: usize,
    overflow: bool,
}

/// Empty, bounded root selection.
pub closed spec fn path_empty(path: InlinePath) -> bool {
    path.len == 0 && !path.overflow
}

/// Exact path append or sticky depth failure.
pub closed spec fn path_push(before: InlinePath, after: InlinePath, step: Step, result: Result<()>) -> bool {
    after.len == before.len + if result.is_ok() { 1int } else { 0int }
        && match result {
            Ok(()) => !after.overflow && after.len < MAX_DEPTH
                && after.steps[before.len as int] == step
                && forall|i: int| 0 <= i < before.len ==> after.steps[i] == before.steps[i],
            Err(error) => after.overflow && after.steps == before.steps
                && error == (Error::Fault { kind: ErrorKind::DepthExceeded, offset: u32::MAX }),
        }
}

/// Exact declaration selected by an unoverflowed, single-field path.
pub closed spec fn path_field(path: InlinePath) -> Option<u16> {
    if !path.overflow && path.len == 1 {
        match path.steps[0] { Step::Field(id) => Some(id), _ => None }
    } else { None }
}

/// Exact bounded selector, exhaustion or sticky depth error.
pub closed spec fn path_step(result: Result<Option<Step>>, path: InlinePath, index: usize) -> bool {
    match result {
        Ok(step) => !path.overflow && path.len < MAX_DEPTH
            && step == if index < path.len { Some(path.steps[index as int]) } else { None },
        Err(_) => path.overflow || path.len >= MAX_DEPTH,
    }
}

/// Exact capacity for appending one selector to a bounded path.
pub closed spec fn path_capacity(result: bool, path: InlinePath) -> bool {
    result == (!path.overflow && path.len < MAX_DEPTH - 1)
}

impl InlinePath {
    /// Creates a path selecting the root encoding.
    pub fn new() -> (result: Self)
        ensures path_empty(result),
    {
        Self { steps: [Step::Some; MAX_DEPTH], len: 0, overflow: false }
    }

    /// Builds a constant path to one root field, without runtime array setup.
    #[doc(hidden)]
    pub const fn root_field(declaration:u16)->(result:Self)
        ensures path_field(result)==Some(declaration),
    {Self{steps:[Step::Field(declaration);MAX_DEPTH],len:1,overflow:false}}

    /// Copies one selector, reporting exhaustion or a sticky depth failure.
    #[doc(hidden)]
    #[inline(always)]
    pub fn step(&self, index: usize) -> (result: Result<Option<Step>>)
        ensures path_step(result,*self,index),
        no_unwind
    {
        if self.overflow || self.len >= MAX_DEPTH {
            return Err(Error::new(ErrorKind::DepthExceeded, Error::NO_OFFSET));
        }
        if index >= self.len { return Ok(None); }
        Ok(Some(self.steps[index]))
    }

    /// Returns the declaration ordinal when this path selects one root field.
    #[doc(hidden)]
    #[inline(always)]
    pub fn direct_field(&self) -> (result: Option<u16>)
        ensures result == path_field(*self),
        no_unwind
    {
        if !self.overflow && self.len == 1 {
            match self.steps[0] { Step::Field(id) => Some(id), _ => None }
        } else { None }
    }

    /// Reports whether one additional selector fits the depth budget.
    #[doc(hidden)]
    #[inline(always)]
    pub fn can_push(&self) -> (result: bool)
        ensures path_capacity(result, *self),
        no_unwind
    {
        matches!(self.step(MAX_DEPTH - 2), Ok(None))
    }

    /// Appends a selector, or records `DepthExceeded` without changing the prefix.
    #[inline(always)]
    pub fn push(&mut self, step: Step) -> (result: Result<()>)
        ensures path_push(*old(self), *final(self), step, result),
    {
        if self.overflow || self.len >= MAX_DEPTH - 1 {
            self.overflow = true;
            return Err(ErrorKind::DepthExceeded.into());
        }
        self.steps[self.len] = step;
        self.len += 1;
        Ok(())
    }
}

impl Default for InlinePath {
    fn default() -> (result: Self)
        ensures path_empty(result),
    { Self::new() }
}

#[derive(Clone, Copy, Debug)]
struct PatchRun {
    offset: u32,
    stride: u32,
    count: u32,
}

#[derive(Clone, Copy, Debug)]
struct Node {
    payload_len: u32,
    region: Span,
    #[cfg(verus_only)]
    ghost layout: Layout,
    ordinal: u32,
    run: PatchRun,
}

#[derive(Clone, Copy, Debug)]
struct Trail {
    root: Layout,
    selected: Layout,
    local: bool,
    nodes: [Node; MAX_DEPTH],
    depth: usize,
}

spec fn empty_run() -> PatchRun { PatchRun { offset: 0, stride: 4, count: 0 } }

spec fn node_bytes(bytes: Seq<u8>, node: Node) -> Seq<u8> {
    bytes.subrange(node.region.start as int, node.region.end as int)
}

/// Independent correspondence between selectors and wire child ordinals.
spec fn selects(layout: Layout, step: Step, index: int) -> bool {
    match (layout.head(), step) {
        (LayoutHead::Struct { fixed, variable, .. }, Step::Field(id)) =>
            if 0 <= index < fixed.len() { fixed.view()[index].declaration == id }
            else { fixed.len() <= index < fixed.len() + variable.len()
                && variable.view()[index - fixed.len()].declaration == id },
        (LayoutHead::List { .. } | LayoutHead::Map { .. }, Step::Index(i)) => index == i,
        (LayoutHead::Option { .. }, Step::Some) => index == 0,
        _ => false,
    }
}

/// All and only stored ends at or after the selected child. Map terminal ends
/// are retained; struct and list terminal ends are implicit.
spec fn expected_run(bytes: Seq<u8>, layout: Layout, index: int) -> PatchRun {
    match layout.head() {
        LayoutHead::Struct { fixed_bytes, fixed, variable } =>
            if index < fixed.len() || index - fixed.len() >= variable.len() - 1 { empty_run() }
            else { PatchRun { offset: (fixed_bytes + 4 * (index - fixed.len())) as u32,
                stride: 4, count: (variable.len() - 1 - (index - fixed.len())) as u32 } },
        LayoutHead::List { element } => if width_spec(element).is_some()
                || index >= words::le_word(bytes, 0) - 1 { empty_run() }
            else { PatchRun { offset: (4 + 4 * index) as u32, stride: 4,
                count: (words::le_word(bytes, 0) - 1 - index) as u32 } },
        LayoutHead::Map { key, value } => if width_spec(value).is_some() { empty_run() }
            else { PatchRun { offset: (4 + index * (key.width + 4) + key.width) as u32,
                stride: (key.width + 4) as u32,
                count: (words::le_word(bytes, 0) - index) as u32 } },
        _ => empty_run(),
    }
}

spec fn run_within(run: PatchRun, lower: int, upper: int) -> bool {
    run.count == 0 || (run.stride >= 4 && lower <= run.offset
        && run.offset + (run.count - 1) * run.stride + 4 <= upper)
}

spec fn shifted(run: PatchRun, base: u32) -> PatchRun {
    if run.count == 0 { empty_run() }
    else { PatchRun { offset: (base + run.offset) as u32, ..run } }
}

struct Selection {
    payload_len: u32,
    span: Span,
    layout: Layout,
    ordinal: u32,
    run: PatchRun,
}

spec fn payload_extent(bytes: Seq<u8>, layout: Layout) -> int {
    match layout.head() {
        LayoutHead::Struct { .. } => bytes.len() - crate::struct_layout::payload_start(layout),
        LayoutHead::List { .. } => bytes.len() - crate::list_layout::payload_start(bytes, layout),
        LayoutHead::Map { .. } => bytes.len() - crate::map_layout::payload_start(bytes, layout),
        _ => 0,
    }
}

#[inline(always)]
fn part_payload(parts: &Parts, Ghost(bytes): Ghost<Seq<u8>>, _layout: &Layout) -> (result: u32)
    requires crate::edit_layout::parts_local(*parts, bytes, *_layout, false),
    ensures result == payload_extent(bytes, *_layout),
{
    match parts {
        Parts::Struct(p) => p.payload.end - p.payload.start,
        Parts::List(p) => p.payload.end - p.payload.start,
        Parts::Map(p) => p.payload.end - p.payload.start,
        _ => 0,
    }
}

spec fn selection_exact(s: Selection, bytes: Seq<u8>, layout: Layout, step: Step) -> bool {
    &&& crate::edit_layout::header_valid(bytes, layout)
    &&& 0 <= s.ordinal < validation::child_count(bytes, layout)
    &&& selects(layout, step, s.ordinal as int)
    &&& s.span == validation::child_span(bytes, layout, s.ordinal as int)
    &&& s.layout == validation::child_layout(layout, s.ordinal as int)
    &&& s.span.start <= s.span.end <= bytes.len()
    &&& s.run == expected_run(bytes, layout, s.ordinal as int)
    &&& s.payload_len == payload_extent(bytes, layout)
    &&& run_within(s.run, 0, s.span.start as int)
}

fn check_run_extent(run: PatchRun, limit: u32) -> (result: Result<()>)
    ensures result.is_ok() == run_within(run, 0, limit as int),
{
    if run.count > 0 {
        let tail = checked_mul(run.count as usize - 1, run.stride as usize)?;
        let end = checked_add(checked_add(run.offset as usize, tail)?, 4)?;
        if run.stride < 4 || end > limit as usize {
            return Err(ErrorKind::InvalidBoundary.into());
        }
    }
    Ok(())
}

#[inline(never)]
fn field_index(fixed: crate::Fields<'_>, variable: crate::Fields<'_>, id: u16) -> (result: Result<u32>)
    requires fixed.len() + variable.len() <= u16::MAX,
    ensures result.is_ok() ==> {
        let i = result.unwrap() as int;
        0 <= i < fixed.len() + variable.len() && if i < fixed.len() {
            fixed.view()[i].declaration == id
        } else { variable.view()[i - fixed.len()].declaration == id }
    },
{
    let nf=fixed.len();
    let total=nf+variable.len();
    let mut i = 0usize;
    while i < total
        invariant i <= total <= u16::MAX, nf==fixed.len(), total==nf+variable.len(),
        decreases fixed.len() + variable.len() - i,
    {
        let declaration = if i < nf { fixed.declaration(i) }
            else { variable.declaration(i - nf) };
        if declaration == id { return Ok(i as u32); }
        i += 1;
    }
    Err(ErrorKind::IndexOutOfBounds.into())
}

#[verifier::spinoff_prover]
#[verifier::rlimit(60)]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn ancestor_run(Ghost(bytes): Ghost<Seq<u8>>, layout: &Layout, parts: &Parts, index: u32) -> (result: Result<PatchRun>)
    requires crate::edit_layout::parts_local(*parts, bytes, *layout, false),
        index < validation::child_count(bytes, *layout),
    ensures result.is_ok() ==> result.unwrap() == expected_run(bytes, *layout, index as int),
{
    proof{reveal(crate::layout_head::head_spec);}
    let run = match parts {
        Parts::Struct(p)=>{
            let nf=match layout {
                Layout::Flat{checked}=>checked.header(3) as usize,
                _=>match layout.head(){LayoutHead::Struct{fixed,..}=>fixed.len(),_=>{assert(false);0}},
            };
            if (index as usize)<nf || index as usize-nf+1>=p.variables as usize {
                PatchRun{offset:0,stride:4,count:0}
            } else {let j=index as usize-nf;
                PatchRun{offset:checked_u32(p.directory.start as usize+4*j)?,stride:4,count:(p.variables as usize-1-j) as u32}
            }
        },
        Parts::List(p)=>{
            if p.element_width.is_some() || index+1>=p.count {PatchRun{offset:0,stride:4,count:0}}
            else{PatchRun{offset:checked_u32(4usize+4*index as usize)?,stride:4,count:p.count-1-index}}
        },
        Parts::Map(p)=>{
            let kw=p.key_width;
            if p.value_width.is_some(){PatchRun{offset:0,stride:4,count:0}}
            else{let stride=checked_u32(checked_add(kw as usize,4)?)?;
                PatchRun{offset:checked_u32(checked_add(checked_add(4,checked_mul(index as usize,stride as usize)?)?,kw as usize)?)?,stride,count:p.count-index}
            }
        },
        _=>PatchRun{offset:0,stride:4,count:0},
    };
    assert(run == expected_run(bytes, *layout, index as int));
    Ok(run)
}

#[verifier::spinoff_prover]
#[verifier::rlimit(60)]
#[cfg_attr(target_os = "solana", inline(never))]
fn select_child(bytes: &[u8], layout: &Layout, step: Step) -> (result: Result<Selection>)
    ensures result.is_ok() ==> selection_exact(result.unwrap(), bytes@, *layout, step),
{
    let parts = enter(bytes, layout, false)?;
    if matches!(parts, Parts::Map(_)) { validate_map_keys(bytes, layout)?; }
    let n = count(&parts, layout, Ghost(bytes@), false);
    let index=match (&parts,step) {
        (Parts::Struct(_),Step::Field(id))=>match layout {
            Layout::Flat{checked}=>{
                let index=match checked.declaration_index(id){Some(index)=>index as u32,None=>return Err(ErrorKind::IndexOutOfBounds.into())};
                proof{reveal(crate::layout_head::head_spec);}
                index
            },
            _=>match layout.head(){LayoutHead::Struct{fixed,variable,..}=>field_index(fixed,variable,id)?,_=>{assert(false);return Err(ErrorKind::InvalidSchema.into());}},
        },
        (Parts::List(_)|Parts::Map(_),Step::Index(i))=>{
            if i>=n{return Err(ErrorKind::IndexOutOfBounds.into());} i
        },
        (Parts::Option(_),Step::Some)=>{
            if n==0{return Err(ErrorKind::MissingValue.into());}0
        },
        _=>return Err(ErrorKind::InvalidSchema.into()),
    };
    assert(index < n);
    assert(selects(*layout, step, index as int));
    let (span, selected) = child(bytes, layout, &parts, index, false)?;
    let run = ancestor_run(Ghost(bytes@), layout, &parts, index)?;
    // Ancestor words must precede their child's payload, including empty children.
    check_run_extent(run, span.start)?;
    let payload_len = part_payload(&parts, Ghost(bytes@), layout);
    Ok(Selection { payload_len, span, layout: selected, ordinal: index, run })
}

#[verifier::spinoff_prover]
#[inline(always)]
fn select_flat_field(bytes: &[u8], checked: crate::CheckedLayout<'static>, id: u16) -> (result: Result<Selection>)
    requires checked.valid(), checked.view()[checked.root_index()] == 9,
    ensures result.is_ok() ==> selection_exact(result.unwrap(), bytes@, Layout::Flat { checked }, Step::Field(id))
        && matches!(result.unwrap().layout, Layout::Flat { .. }),
        validation::local_valid(bytes@, Layout::Flat { checked }, false)
            && id < checked.view()[checked.root_index()+3] + checked.view()[checked.root_index()+4] ==> result.is_ok(),
{
    checked.witness();
    let layout = Layout::Flat { checked };
    proof { reveal(crate::layout_head::head_spec); }
    let p = crate::struct_layout::partition_struct_header(bytes, &layout)?;
    let index = match checked.declaration_index(id) {
        Some(index) => index as u32, None => return Err(ErrorKind::IndexOutOfBounds.into()),
    };
    proof { reveal(crate::layout_head::head_spec); }
    let nf = checked.header(3);
    let payload_len = p.payload.end - p.payload.start;
    let (offset, child) = checked.packed_field(index as usize);
    if index < nf {
        proof {
            crate::edit_layout::fixed_bounds(crate::Fields::Flat { layout: &checked, variable: false }.view(),
                crate::Fields::Flat { layout: &checked, variable: true }.view(), checked.view()[checked.root_index()+1],
                0, 0, 0, MAX_DEPTH as nat);
        }
        proof { crate::layout::certified_width(child); }
        let width = child.header(1);
        proof {
            let field = crate::Fields::Flat { layout: &checked, variable: false }.view()[index as int];
            assert(field.offset == offset);
            assert(field.layout == (Layout::Flat { checked: child }));
            assert(width_spec(field.layout) == Some(width));
            assert(offset as int + width <= p.fixed.end);
        }
        let span = Span { start: offset, end: offset + width };
        return Ok(Selection { payload_len, span, layout: Layout::Flat { checked: child }, ordinal: index,
            run: PatchRun { offset: 0, stride: 4, count: 0 } });
    }
    let j = index - nf;
    let span = crate::edit_layout::struct_variable_field_local(bytes, &layout, &p, j as u16)?;
    let run = if j + 1 >= p.variables as u32 { PatchRun { offset: 0, stride: 4, count: 0 } }
        else { PatchRun { offset: p.directory.start + 4*j, stride: 4, count: p.variables as u32 - 1 - j } };
    proof { assert(run_within(run, 0, span.start as int)); }
    Ok(Selection { payload_len, span, layout: Layout::Flat { checked: child }, ordinal: index, run })
}

#[verifier::spinoff_prover]
#[inline(always)]
fn select_flat_index(bytes: &[u8], checked: crate::CheckedLayout<'static>, index: u32) -> (result: Result<Selection>)
    requires checked.valid(), 7 <= checked.view()[checked.root_index()] <= 8,
    ensures result.is_ok() ==> selection_exact(result.unwrap(), bytes@, Layout::Flat { checked }, Step::Index(index))
        && matches!(result.unwrap().layout, Layout::Flat { .. }),
        validation::local_valid(bytes@, Layout::Flat { checked }, false)
            && index < words::le_word(bytes@, 0) ==> result.is_ok(),
{
    checked.witness();
    let layout = Layout::Flat { checked };
    proof { reveal(crate::layout_head::head_spec); }
    if checked.header(0) == 7 {
        let p = crate::list_layout::partition_list_header(bytes, &layout)?;
        if index >= p.count { return Err(ErrorKind::IndexOutOfBounds.into()); }
        let span = crate::edit_layout::list_element_local(bytes, &layout, &p, index)?;
        let run = if p.element_width.is_some() || index + 1 >= p.count { PatchRun { offset: 0, stride: 4, count: 0 } }
            else { PatchRun { offset: checked_u32(4usize + 4*index as usize)?, stride: 4, count: p.count - 1 - index } };
        proof { assert(run_within(run, 0, span.start as int)); }
        let child = checked.only_child();
        return Ok(Selection { payload_len: p.payload.end - p.payload.start, span,
            layout: Layout::Flat { checked: child }, ordinal: index, run });
    }
    let p = crate::map_layout::partition_map_header(bytes, &layout)?;
    if index >= p.count { return Err(ErrorKind::IndexOutOfBounds.into()); }
    let span = crate::edit_layout::map_value_local(bytes, &layout, &p, index)?;
    let run = if p.value_width.is_some() { PatchRun { offset: 0, stride: 4, count: 0 } }
        else {
            proof {
                assert(p.key_width as int + 4 <= u32::MAX) by(nonlinear_arith)
                    requires p.count > 0, 4 + p.count as int * (p.key_width as int + 4) <= bytes.len(),
                        bytes.len() <= u32::MAX, p.key_width >= 0;
            }
            let stride = checked_u32(checked_add(p.key_width as usize, 4)?)?;
            proof {
                assert(0 <= index as int * stride
                    && 4 + index as int * stride + p.key_width + 4 <= p.payload.start) by(nonlinear_arith)
                    requires index < p.count, stride == p.key_width + 4,
                        p.payload.start == 4 + p.count as int * stride, stride > 0;
            }
            PatchRun { offset: checked_u32(checked_add(checked_add(4,
                checked_mul(index as usize, stride as usize)?)?, p.key_width as usize)?)?,
                stride, count: p.count - index }
        };
    proof {
        if p.value_width.is_none() {
            assert(run.offset + (run.count - 1) * run.stride + 4 == p.payload.start) by(nonlinear_arith)
                requires run.offset == 4 + index as int * run.stride + p.key_width,
                    run.count == p.count - index, run.stride == p.key_width + 4,
                    p.payload.start == 4 + p.count as int * run.stride;
        }
        assert(run_within(run, 0, span.start as int));
    }
    let child = checked.only_child();
    Ok(Selection { payload_len: p.payload.end - p.payload.start, span,
        layout: Layout::Flat { checked: child }, ordinal: index, run })
}

#[verifier::spinoff_prover]
#[inline(always)]
fn select_flat_some(bytes: &[u8], checked: crate::CheckedLayout<'static>) -> (result: Result<Selection>)
    requires checked.valid(), checked.view()[checked.root_index()] == 6,
    ensures result.is_ok() ==> selection_exact(result.unwrap(), bytes@, Layout::Flat { checked }, Step::Some)
        && matches!(result.unwrap().layout, Layout::Flat { .. }),
        validation::local_valid(bytes@, Layout::Flat { checked }, false)
            && bytes.len() > 0 && bytes@[0] == 1 ==> result.is_ok(),
{
    checked.witness();
    let layout = Layout::Flat { checked };
    proof { reveal(crate::layout_head::head_spec); }
    let span = match crate::partition_option(bytes, &layout)? {
        Some(span) => span, None => return Err(ErrorKind::MissingValue.into()),
    };
    proof { reveal(crate::layout_head::head_spec); }
    let child = checked.only_child();
    Ok(Selection { payload_len: 0, span, layout: Layout::Flat { checked: child }, ordinal: 0,
        run: PatchRun { offset: 0, stride: 4, count: 0 } })
}

#[verifier::spinoff_prover]
#[cfg_attr(target_os = "solana", inline(never))]
fn select_child_local(bytes: &[u8], layout: &Layout, step: Step) -> (result: Result<Selection>)
    ensures result.is_ok() ==> selection_exact(result.unwrap(), bytes@, *layout, step),
{
    if let Layout::Flat { checked } = layout {
        match step {
            Step::Index(index) if checked.header(0) == 7 || checked.header(0) == 8 => return select_flat_index(bytes, *checked, index),
            Step::Some if checked.header(0) == 6 => return select_flat_some(bytes, *checked),
            _ => (),
        }
    }

    if let (Layout::Flat { checked }, Step::Field(id)) = (layout, step) {
        if checked.header(0) == 9 { return select_flat_field(bytes, *checked, id); }
    }

    let parts = crate::edit_layout::enter_local(bytes, layout, false)?;
    let n = crate::edit_layout::count_local(&parts, layout, Ghost(bytes@), false);
    let index=match (&parts,step) {
        (Parts::Struct(_),Step::Field(id))=>match layout {
            Layout::Flat{checked}=>{
                let index=match checked.declaration_index(id){Some(index)=>index as u32,None=>return Err(ErrorKind::IndexOutOfBounds.into())};
                proof{reveal(crate::layout_head::head_spec);}
                index
            },
            _=>match layout.head(){LayoutHead::Struct{fixed,variable,..}=>field_index(fixed,variable,id)?,_=>{assert(false);return Err(ErrorKind::InvalidSchema.into());}},
        },
        (Parts::List(_)|Parts::Map(_),Step::Index(i))=>{
            if i>=n{return Err(ErrorKind::IndexOutOfBounds.into());} i
        },
        (Parts::Option(_),Step::Some)=>{
            if n==0{return Err(ErrorKind::MissingValue.into());}0
        },
        _=>return Err(ErrorKind::InvalidSchema.into()),
    };
    assert(index < n);
    assert(selects(*layout, step, index as int));
    let (span, selected) = crate::edit_layout::child_local(bytes, layout, &parts, index, false)?;
    let run = ancestor_run(Ghost(bytes@), layout, &parts, index)?;
    // Ancestor words must precede their child's payload, including empty children.
    check_run_extent(run, span.start)?;
    let payload_len = part_payload(&parts, Ghost(bytes@), layout);
    Ok(Selection { payload_len, span, layout: selected, ordinal: index, run })
}

spec fn edge_valid(bytes: Seq<u8>, node: Node, step: Step, next: Node) -> bool {
    let s = validation::child_span(node_bytes(bytes, node), node.layout, node.ordinal as int);
    &&& crate::edit_layout::header_valid(node_bytes(bytes, node), node.layout)
    &&& 0 <= node.ordinal < validation::child_count(node_bytes(bytes, node), node.layout)
    &&& selects(node.layout, step, node.ordinal as int)
    &&& next.layout == validation::child_layout(node.layout, node.ordinal as int)
    &&& next.region.start == node.region.start + s.start
    &&& next.region.end == node.region.start + s.end
    &&& node.region.start <= next.region.start <= next.region.end <= node.region.end
    &&& node.payload_len == payload_extent(node_bytes(bytes, node), node.layout)
    &&& node.run == shifted(expected_run(node_bytes(bytes, node), node.layout,
        node.ordinal as int), node.region.start)
    &&& run_within(node.run, node.region.start as int, next.region.start as int)
}

spec fn trail_valid(bytes: Seq<u8>, root: Layout, path: InlinePath, trail: Trail, used: u32) -> bool {
    &&& !path.overflow && path.len < MAX_DEPTH && trail.depth == path.len
    &&& trail.nodes[0].region == (Span { start: 12, end: used })
    &&& trail.root == root
    &&& trail.nodes[0].layout == root
    &&& trail.selected == trail.nodes[trail.depth as int].layout
    &&& forall|i: int| #![trigger trail.nodes[i]] 0 <= i <= trail.depth ==> {
        let node = trail.nodes[i];
        &&& 12 <= node.region.start <= node.region.end <= used <= bytes.len()
        &&& node.region.start <= trail.nodes[trail.depth as int].region.start
        &&& trail.nodes[trail.depth as int].region.end <= node.region.end
    }
    &&& forall|i: int| 0 <= i < trail.depth
        ==> edge_valid(bytes, trail.nodes[i], path.steps[i], trail.nodes[i + 1])
    &&& crate::edit_layout::header_valid(node_bytes(bytes, trail.nodes[trail.depth as int]),
        trail.nodes[trail.depth as int].layout)
}

fn envelope(bytes: &[u8], used: u32) -> (result: Result<()>)
    ensures result.is_ok() ==> 12 <= used <= bytes.len() <= u32::MAX
        && words::le_word(bytes@, 8) == used,
{
    checked_u32(bytes.len())?;
    let stored = read_u32(bytes, 8)?;
    if used < 12 || used as usize > bytes.len() || stored != used {
        return Err(Error::new(ErrorKind::UsedLength, 8));
    }
    Ok(())
}

#[verifier::spinoff_prover]
fn trace(bytes: &[u8], used: u32, layout: &Layout, path: &InlinePath) -> (result: Result<Trail>)
    requires 12 <= used <= bytes.len() <= u32::MAX,
    ensures result.is_ok() ==> trail_valid(bytes@, *layout, *path, result.unwrap(), used),
{
    crate::check_layout(layout, MAX_DEPTH)?;
    if path.overflow || path.len >= MAX_DEPTH { return Err(ErrorKind::DepthExceeded.into()); }
    let node = Node { payload_len: 0, region: Span { start: 12, end: used }, #[cfg(verus_only)] layout: *layout, ordinal: 0,
        run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let empty = Node { payload_len: 0, region: Span { start: 0, end: 0 }, #[cfg(verus_only)] layout: Layout::Str,
        ordinal: 0, run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let mut nodes = [empty; MAX_DEPTH];
    nodes[0] = node;
    let mut active = *layout;
    let mut depth = 0usize;
    while depth < path.len
        invariant
            !path.overflow, depth <= path.len < MAX_DEPTH,
            nodes[0].region == (Span { start: 12, end: used }), nodes[0].layout == *layout,
            active == nodes[depth as int].layout,
            12 <= used <= bytes.len() <= u32::MAX,
            forall|i: int| #![trigger nodes[i]] 0 <= i <= depth ==> {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= nodes[depth as int].region.start
                &&& nodes[depth as int].region.end <= n.region.end
            },
            forall|i: int| 0 <= i < depth
                ==> edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]),
        decreases path.len - depth,
    {
        let parent = nodes[depth];
        let local = &bytes[parent.region.start as usize..parent.region.end as usize];
        let selected = select_child(local, &active, path.steps[depth])
            .map_err(|e: Error| e.rebase(parent.region.start))?;
        let run = if selected.run.count == 0 { PatchRun { offset: 0, stride: 4, count: 0 } }
            else { PatchRun { offset: ((parent.region.start as u64) + (selected.run.offset as u64)) as u32, ..selected.run } };
        let next = Node { payload_len: 0, region: Span { start: ((parent.region.start as u64) + (selected.span.start as u64)) as u32,
                end: ((parent.region.start as u64) + (selected.span.end as u64)) as u32 }, #[cfg(verus_only)] layout: selected.layout, ordinal: 0,
            run: PatchRun { offset: 0, stride: 4, count: 0 } };
        let ghost previous = nodes@;
        nodes[depth] = Node { payload_len: selected.payload_len, ordinal: selected.ordinal, run, ..parent };
        nodes[depth + 1] = next;
        proof {
            assert(edge_valid(bytes@, nodes[depth as int], path.steps[depth as int], nodes[depth as int + 1]));
            assert forall|i: int| 0 <= i < depth implies
                edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]) by {
                assert(edge_valid(bytes@, previous[i], path.steps[i], previous[i + 1]));
                if i + 1 == depth {
                    assert(nodes[i + 1].region == previous[i + 1].region);
                    assert(nodes[i + 1].layout == previous[i + 1].layout);
                }
            }
            assert forall|i: int| #![trigger nodes[i]] 0 <= i <= depth + 1 implies {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= next.region.start
                &&& next.region.end <= n.region.end
            } by {
                if i <= depth {
                    assert(previous[i].region == nodes[i].region);
                    assert(previous[i].region.start <= parent.region.start);
                    assert(parent.region.end <= previous[i].region.end);
                }
            }
        }
        active = selected.layout;
        depth += 1;
    }
    let target = nodes[depth];
    let local = &bytes[target.region.start as usize..target.region.end as usize];
    let parts=enter(local, &active, false).map_err(|e: Error| e.rebase(target.region.start))?;
    if matches!(parts, Parts::Map(_)) {
        validate_map_keys(local, &active).map_err(|e: Error| e.rebase(target.region.start))?;
    }
    Ok(Trail { root: *layout, selected: active, local: false, nodes, depth })
}

#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn trace_local(bytes: &[u8], used: u32, layout: &Layout, path: &InlinePath) -> (result: Result<Trail>)
    requires 12 <= used <= bytes.len() <= u32::MAX,
    ensures result.is_ok() ==> trail_valid(bytes@, *layout, *path, result.unwrap(), used),
{
    if let Layout::Flat { checked } = layout { checked.witness(); return trace_flat(bytes, used, *checked, path); }
    crate::check_layout(layout, MAX_DEPTH)?;
    if path.overflow || path.len >= MAX_DEPTH { return Err(ErrorKind::DepthExceeded.into()); }
    let node = Node { payload_len: 0, region: Span { start: 12, end: used }, #[cfg(verus_only)] layout: *layout, ordinal: 0,
        run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let empty = Node { payload_len: 0, region: Span { start: 0, end: 0 }, #[cfg(verus_only)] layout: Layout::Str,
        ordinal: 0, run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let mut nodes = [empty; MAX_DEPTH];
    nodes[0] = node;
    let mut active = *layout;
    let mut depth = 0usize;
    while depth < path.len
        invariant
            !path.overflow, depth <= path.len < MAX_DEPTH,
            nodes[0].region == (Span { start: 12, end: used }), nodes[0].layout == *layout,
            active == nodes[depth as int].layout,
            12 <= used <= bytes.len() <= u32::MAX,
            forall|i: int| #![trigger nodes[i]] 0 <= i <= depth ==> {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= nodes[depth as int].region.start
                &&& nodes[depth as int].region.end <= n.region.end
            },
            forall|i: int| 0 <= i < depth
                ==> edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]),
        decreases path.len - depth,
    {
        let parent = nodes[depth];
        let local = &bytes[parent.region.start as usize..parent.region.end as usize];
        let selected = select_child_local(local, &active, path.steps[depth])
            .map_err(|e: Error| e.rebase(parent.region.start))?;
        let run = if selected.run.count == 0 { PatchRun { offset: 0, stride: 4, count: 0 } }
            else { PatchRun { offset: ((parent.region.start as u64) + (selected.run.offset as u64)) as u32, ..selected.run } };
        let next = Node { payload_len: 0, region: Span { start: ((parent.region.start as u64) + (selected.span.start as u64)) as u32,
                end: ((parent.region.start as u64) + (selected.span.end as u64)) as u32 }, #[cfg(verus_only)] layout: selected.layout, ordinal: 0,
            run: PatchRun { offset: 0, stride: 4, count: 0 } };
        let ghost previous = nodes@;
        nodes[depth] = Node { payload_len: selected.payload_len, ordinal: selected.ordinal, run, ..parent };
        nodes[depth + 1] = next;
        proof {
            assert(edge_valid(bytes@, nodes[depth as int], path.steps[depth as int], nodes[depth as int + 1]));
            assert forall|i: int| 0 <= i < depth implies
                edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]) by {
                assert(edge_valid(bytes@, previous[i], path.steps[i], previous[i + 1]));
                if i + 1 == depth {
                    assert(nodes[i + 1].region == previous[i + 1].region);
                    assert(nodes[i + 1].layout == previous[i + 1].layout);
                }
            }
            assert forall|i: int| #![trigger nodes[i]] 0 <= i <= depth + 1 implies {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= next.region.start
                &&& next.region.end <= n.region.end
            } by {
                if i <= depth {
                    assert(previous[i].region == nodes[i].region);
                    assert(previous[i].region.start <= parent.region.start);
                    assert(parent.region.end <= previous[i].region.end);
                }
            }
        }
        active = selected.layout;
        depth += 1;
    }
    let target = nodes[depth];
    let local = &bytes[target.region.start as usize..target.region.end as usize];
    let _parts=crate::edit_layout::enter_local(local, &active, false).map_err(|e: Error| e.rebase(target.region.start))?;

    Ok(Trail { root: *layout, selected: active, local: true, nodes, depth })
}

#[verifier::spinoff_prover]
#[inline(always)]
fn select_checked_child(bytes: &[u8], checked: crate::CheckedLayout<'static>, step: Step) -> (result: Result<Selection>)
    requires checked.valid(),
    ensures result.is_ok() ==> selection_exact(result.unwrap(), bytes@, Layout::Flat { checked }, step)
        && matches!(result.unwrap().layout, Layout::Flat { .. }),
{
    match step {
        Step::Field(id) if checked.header(0) == 9 => select_flat_field(bytes, checked, id),
        Step::Index(index) if checked.header(0) == 7 || checked.header(0) == 8 => select_flat_index(bytes, checked, index),
        Step::Some if checked.header(0) == 6 => select_flat_some(bytes, checked),
        _ => Err(ErrorKind::InvalidSchema.into()),
    }
}

#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn trace_flat(bytes: &[u8], used: u32, checked: crate::CheckedLayout<'static>, path: &InlinePath) -> (result: Result<Trail>)
    requires 12 <= used <= bytes.len() <= u32::MAX, checked.valid(),
    ensures result.is_ok() ==> trail_valid(bytes@, (Layout::Flat { checked }), *path, result.unwrap(), used),
{
    checked.witness();
    if path.overflow || path.len >= MAX_DEPTH { return Err(ErrorKind::DepthExceeded.into()); }
    let node = Node { payload_len: 0, region: Span { start: 12, end: used }, #[cfg(verus_only)] layout: (Layout::Flat { checked }), ordinal: 0,
        run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let empty = Node { payload_len: 0, region: Span { start: 0, end: 0 }, #[cfg(verus_only)] layout: Layout::Str,
        ordinal: 0, run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let mut nodes = [empty; MAX_DEPTH];
    nodes[0] = node;
    let mut active = checked;
    let mut depth = 0usize;
    while depth < path.len
        invariant
            !path.overflow, depth <= path.len < MAX_DEPTH,
            nodes[0].region == (Span { start: 12, end: used }), nodes[0].layout == (Layout::Flat { checked }),
            (Layout::Flat { checked: active }) == nodes[depth as int].layout,
            12 <= used <= bytes.len() <= u32::MAX,
            forall|i: int| #![trigger nodes[i]] 0 <= i <= depth ==> {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= nodes[depth as int].region.start
                &&& nodes[depth as int].region.end <= n.region.end
            },
            forall|i: int| 0 <= i < depth
                ==> edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]),
        decreases path.len - depth,
    {
        let parent = nodes[depth];
        let local = &bytes[parent.region.start as usize..parent.region.end as usize];
        active.witness();
        let selected = select_checked_child(local, active, path.steps[depth])
            .map_err(|e: Error| e.rebase(parent.region.start))?;
        let run = if selected.run.count == 0 { PatchRun { offset: 0, stride: 4, count: 0 } }
            else { PatchRun { offset: ((parent.region.start as u64) + (selected.run.offset as u64)) as u32, ..selected.run } };
        let next = Node { payload_len: 0, region: Span { start: ((parent.region.start as u64) + (selected.span.start as u64)) as u32,
                end: ((parent.region.start as u64) + (selected.span.end as u64)) as u32 }, #[cfg(verus_only)] layout: selected.layout, ordinal: 0,
            run: PatchRun { offset: 0, stride: 4, count: 0 } };
        let ghost previous = nodes@;
        nodes[depth] = Node { payload_len: selected.payload_len, ordinal: selected.ordinal, run, ..parent };
        nodes[depth + 1] = next;
        proof {
            assert(edge_valid(bytes@, nodes[depth as int], path.steps[depth as int], nodes[depth as int + 1]));
            assert forall|i: int| 0 <= i < depth implies
                edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]) by {
                assert(edge_valid(bytes@, previous[i], path.steps[i], previous[i + 1]));
                if i + 1 == depth {
                    assert(nodes[i + 1].region == previous[i + 1].region);
                    assert(nodes[i + 1].layout == previous[i + 1].layout);
                }
            }
            assert forall|i: int| #![trigger nodes[i]] 0 <= i <= depth + 1 implies {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= next.region.start
                &&& next.region.end <= n.region.end
            } by {
                if i <= depth {
                    assert(previous[i].region == nodes[i].region);
                    assert(previous[i].region.start <= parent.region.start);
                    assert(parent.region.end <= previous[i].region.end);
                }
            }
        }
        active = match selected.layout { Layout::Flat { checked } => { checked.witness(); checked }, _ => { assert(false); return Err(ErrorKind::InvalidSchema.into()); } };
        depth += 1;
    }
    let target = nodes[depth];
    let local = &bytes[target.region.start as usize..target.region.end as usize];
    let _parts=crate::edit_layout::enter_local(local, &(Layout::Flat { checked: active }), false).map_err(|e: Error| e.rebase(target.region.start))?;

    Ok(Trail { root: (Layout::Flat { checked }), selected: Layout::Flat { checked: active }, local: true, nodes, depth })
}

/// Exact path resolution through checked, contained parent/child intervals.
pub closed spec fn resolve_path(result: Result<Span>, bytes: Seq<u8>, layout: Layout, path: InlinePath) -> bool {
    match result {
        Ok(span) => exists|t: Trail| trail_valid(bytes, layout, path, t, words::le_word(bytes, 8) as u32)
            && span == t.nodes[t.depth as int].region,
        Err(_) => true,
    }
}

/// Resolves an account-relative region, excluding envelope and spare bytes.
///
/// Checks the envelope, descriptor, every visited partition, and map key order.
/// Unvisited values and scalar/string domains are not semantically validated.
/// Child diagnostics are rebased once to account coordinates.
pub fn resolve(bytes: &[u8], layout: &Layout, path: &InlinePath) -> (result: Result<Span>)
    ensures resolve_path(result, bytes@, *layout, *path),
        result.is_ok() ==> 12 <= result.unwrap().start <= result.unwrap().end <= bytes.len(),
{
    let used = read_u32(bytes, 8)?;
    envelope(bytes, used)?;
    let trail = trace(bytes, used, layout, path)?;
    Ok(trail.nodes[trail.depth].region)
}

/// Resolves only headers and selected adjacent ends for a typed read or edit.
///
/// Untouched directories and key order are certified by full validation only.
#[doc(hidden)]
pub fn resolve_local(bytes: &[u8], layout: &Layout, path: &InlinePath) -> (result: Result<Span>)
    ensures resolve_path(result, bytes@, *layout, *path),
        result.is_ok() ==> 12 <= result.unwrap().start <= result.unwrap().end <= bytes.len(),
{
    if let Layout::Flat { checked } = layout { checked.witness(); return resolve_flat(bytes, *checked, path); }
    let used = read_u32(bytes, 8)?;
    envelope(bytes, used)?;
    crate::check_layout(layout, MAX_DEPTH)?;
    if path.overflow || path.len >= MAX_DEPTH { return Err(ErrorKind::DepthExceeded.into()); }
    let ghost node = Node { payload_len: 0, region: Span { start: 12, end: used }, #[cfg(verus_only)] layout: *layout, ordinal: 0,
        run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let ghost empty = Node { payload_len: 0, region: Span { start: 0, end: 0 }, #[cfg(verus_only)] layout: Layout::Str,
        ordinal: 0, run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let ghost mut nodes = [empty; MAX_DEPTH];
    proof { nodes = vstd::array::spec_array_update(nodes, 0, node); }
    let mut region = Span { start: 12, end: used };
    let mut active = *layout;
    let mut depth = 0usize;
    while depth < path.len
        invariant
            !path.overflow, depth <= path.len < MAX_DEPTH,
            nodes[0].region == (Span { start: 12, end: used }), nodes[0].layout == *layout,
            active == nodes[depth as int].layout,
            region == nodes[depth as int].region,
            12 <= used <= bytes.len() <= u32::MAX,
            forall|i: int| #![trigger nodes[i]] 0 <= i <= depth ==> {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= nodes[depth as int].region.start
                &&& nodes[depth as int].region.end <= n.region.end
            },
            forall|i: int| 0 <= i < depth
                ==> edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]),
        decreases path.len - depth,
    {
        let ghost parent = nodes[depth as int];
        let local = &bytes[region.start as usize..region.end as usize];
        let selected = select_child_local(local, &active, path.steps[depth])
            .map_err(|e: Error| e.rebase(region.start))?;
        let ghost run = if selected.run.count == 0 { PatchRun { offset: 0, stride: 4, count: 0 } }
            else { PatchRun { offset: (parent.region.start + selected.run.offset) as u32, ..selected.run } };
        let next_region = Span { start: ((region.start as u64) + (selected.span.start as u64)) as u32,
            end: ((region.start as u64) + (selected.span.end as u64)) as u32 };
        let ghost next = Node { payload_len: 0, region: next_region, #[cfg(verus_only)] layout: selected.layout, ordinal: 0,
            run: PatchRun { offset: 0, stride: 4, count: 0 } };
        let ghost previous = nodes@;
        proof { nodes = vstd::array::spec_array_update(nodes, depth as int, Node { payload_len: selected.payload_len, ordinal: selected.ordinal, run, ..parent });
        nodes = vstd::array::spec_array_update(nodes, depth as int + 1, next);
            assert(edge_valid(bytes@, nodes[depth as int], path.steps[depth as int], nodes[depth as int + 1]));
            assert forall|i: int| 0 <= i < depth implies
                edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]) by {
                assert(edge_valid(bytes@, previous[i], path.steps[i], previous[i + 1]));
                if i + 1 == depth {
                    assert(nodes[i + 1].region == previous[i + 1].region);
                    assert(nodes[i + 1].layout == previous[i + 1].layout);
                }
            }
            assert forall|i: int| #![trigger nodes[i]] 0 <= i <= depth + 1 implies {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= next.region.start
                &&& next.region.end <= n.region.end
            } by {
                if i <= depth {
                    assert(previous[i].region == nodes[i].region);
                    assert(previous[i].region.start <= parent.region.start);
                    assert(parent.region.end <= previous[i].region.end);
                }
            }
        }
        region = next_region;
        active = selected.layout;
        depth += 1;
    }
    let ghost target = nodes[depth as int];
    let local = &bytes[region.start as usize..region.end as usize];
    let _parts=crate::edit_layout::enter_local(local, &active, false).map_err(|e: Error| e.rebase(region.start))?;

    proof {
        let trail = Trail { root: *layout, selected: active, local: true, nodes, depth };
        assert(trail_valid(bytes@, *layout, *path, trail, used));
        assert(resolve_path(Ok(region), bytes@, *layout, *path));
    }
    Ok(region)
}

spec fn patch_safe_at(bytes: Seq<u8>, run: PatchRun, delta: i64, i: int) -> bool {
    let at = run.offset + i * run.stride;
    &&& 0 <= at && at + 4 <= bytes.len()
    &&& 0 <= words::le_word(bytes, at) + delta <= u32::MAX
}

spec fn run_safe(bytes: Seq<u8>, run: PatchRun, delta: i64) -> bool {
    forall|i: int| 0 <= i < run.count ==> #[trigger] patch_safe_at(bytes, run, delta, i)
}

proof fn run_index_bounds(run: PatchRun, limit: int, i: int)
    requires run_within(run, 0, limit), 0 <= i < run.count,
    ensures 0 <= run.offset + i * run.stride,
        run.offset + i * run.stride + 4 <= limit,
{
    assert(i * run.stride <= (run.count - 1) * run.stride) by(nonlinear_arith)
        requires 0 <= i <= run.count - 1, run.stride >= 4;
}

spec fn run_payload(bytes: Seq<u8>, run: PatchRun, payload_len: u32) -> bool {
    forall|i: int| 0 <= i < run.count ==> #[trigger] words::le_word(bytes, run.offset + i * run.stride) <= payload_len
}

#[verifier::spinoff_prover]
#[inline(always)]
fn check_contiguous_run(bytes: &[u8], run: PatchRun, delta: i64, payload_len: u32) -> (result: Result<()>)
    requires run.count > 0, run.stride == 4, run_within(run, 0, bytes.len() as int), bytes.len() <= u32::MAX,
        -(u32::MAX as int) <= delta <= u32::MAX,
    ensures result.is_ok() == (run_safe(bytes@, run, delta) && run_payload(bytes@, run, payload_len)),
{
    let length = run.count as usize * 4;
    let (_, suffix) = bytes.split_at(run.offset as usize);
    let (directory, _) = suffix.split_at(length);
    let mut remaining = directory;
    let ghost mut done = 0int;
    while remaining.len() >= 4
        invariant run.stride == 4, run_within(run, 0, bytes.len() as int), bytes.len() <= u32::MAX,
            -(u32::MAX as int) <= delta <= u32::MAX,
            directory@ == bytes@.subrange(run.offset as int, run.offset + 4 * run.count),
            length == 4 * run.count, 0 <= done <= run.count,
            remaining@ == directory@.subrange(4 * done, directory.len() as int),
            remaining.len() == 4 * (run.count - done),
            forall|j: int| 0 <= j < done ==> #[trigger] patch_safe_at(bytes@, run, delta, j),
            forall|j: int| 0 <= j < done ==> #[trigger] words::le_word(bytes@, run.offset + 4*j) <= payload_len,
        decreases remaining.len(),
    {
        let ghost at = (run.offset + 4 * done) as u32;
        let (word, next) = remaining.split_at(4);
        let old = crate::words::read_contained_word(word, 0);
        proof {
            assert(at == run.offset + 4 * done);
            assert(old == words::le_word(bytes@, at as int));
        }
        if old > payload_len {
            let at = (run.offset as usize + length - remaining.len()) as u32;
            return Err(Error::new(ErrorKind::InvalidBoundary, at));
        }
        let shifted = (old as i64).wrapping_add(delta);
        assert(shifted as int == old as int + delta);
        if shifted as u64 > u32::MAX as u64 {
            proof {
                assert((shifted as u64 <= u32::MAX as u64)
                    == (0 <= shifted && shifted <= u32::MAX as i64)) by(bit_vector);
                assert(!patch_safe_at(bytes@, run, delta, done));
            }
            let at = (run.offset as usize + length - remaining.len()) as u32;
            return Err(Error::new(ErrorKind::ArithmeticOverflow, at));
        }
        assert((shifted as u64 <= u32::MAX as u64)
            == (0 <= shifted && shifted <= u32::MAX as i64)) by(bit_vector);
        proof {
            assert(old == words::le_word(bytes@, at as int));
            assert(patch_safe_at(bytes@, run, delta, done));
            done = done + 1;
        }
        remaining = next;
    }
    assert(done == run.count);
    assert(run_safe(bytes@, run, delta));
    assert forall|j: int| 0 <= j < run.count implies #[trigger]
        words::le_word(bytes@, run.offset + j * run.stride) <= payload_len by {
        assert(run.offset + j * run.stride == run.offset + 4*j);
        assert(words::le_word(bytes@, run.offset + 4*j) <= payload_len);
    }
    Ok(())
}

// Contiguous runs keep their short inlined pass. Strided map directories use
// a separate frame so the enclosing splice does not spill loop coordinates.
#[cfg_attr(target_os = "solana", inline(always))]
fn check_run(bytes: &[u8], run: PatchRun, delta: i64, payload_len: u32) -> (result: Result<()>)
    requires run_within(run, 0, bytes.len() as int), bytes.len() <= u32::MAX,
    ensures result.is_ok() == (run_safe(bytes@, run, delta)
        && (delta != 0 ==> run_payload(bytes@, run, payload_len))),
{
    if run.stride == 4 || run.count == 0 || delta == 0 {
        check_run_impl(bytes, run, delta, payload_len)
    } else {
        check_strided_run(bytes, run, delta, payload_len)
    }
}

#[cfg_attr(target_os = "solana", inline(never))]
fn check_strided_run(bytes: &[u8], run: PatchRun, delta: i64, payload_len: u32) -> (result: Result<()>)
    requires run_within(run, 0, bytes.len() as int), bytes.len() <= u32::MAX,
    ensures result.is_ok() == (run_safe(bytes@, run, delta)
        && (delta != 0 ==> run_payload(bytes@, run, payload_len))),
{
    check_run_impl(bytes, run, delta, payload_len)
}

#[cfg_attr(target_os = "solana", inline(always))]
fn check_run_impl(bytes: &[u8], run: PatchRun, delta: i64, payload_len: u32) -> (result: Result<()>)
    requires run_within(run, 0, bytes.len() as int), bytes.len() <= u32::MAX,
    ensures result.is_ok() == (run_safe(bytes@, run, delta)
        && (delta != 0 ==> run_payload(bytes@, run, payload_len))),
{
    if delta == 0 || run.count == 0 {
        proof {
            assert forall|i: int| 0 <= i < run.count implies #[trigger] patch_safe_at(bytes@, run, delta, i) by {
                run_index_bounds(run, bytes.len() as int, i);
                let at = run.offset + i * run.stride;
                assert(0 <= words::le_word(bytes@, at) <= u32::MAX);
            }
        }
        return Ok(());
    }
    if delta < -(u32::MAX as i64) || delta > u32::MAX as i64 {
        proof {
            run_index_bounds(run, bytes.len() as int, 0);
            assert(!patch_safe_at(bytes@, run, delta, 0));
        }
        let old = crate::words::read_contained_word(bytes, run.offset as usize);
        return Err(Error::new(if old > payload_len { ErrorKind::InvalidBoundary }
            else { ErrorKind::ArithmeticOverflow }, run.offset));
    }
    if run.stride == 4 { return check_contiguous_run(bytes, run, delta, payload_len); }
    let mut i = 0u32;
    while i < run.count
        invariant i <= run.count, delta != 0,
            -(u32::MAX as int) <= delta <= u32::MAX,
 run_within(run, 0, bytes.len() as int), bytes.len() <= u32::MAX,
            forall|j: int| 0 <= j < i ==> #[trigger] patch_safe_at(bytes@, run, delta, j),
            forall|j: int| 0 <= j < i ==> #[trigger] words::le_word(bytes@, run.offset + j * run.stride) <= payload_len,
        decreases run.count - i,
    {
        proof { run_index_bounds(run, bytes.len() as int, i as int); }
        let at_usize = run.offset as usize + i as usize * run.stride as usize;
        let at = at_usize as u32;
        assert(at == run.offset + i * run.stride);
        let old = crate::words::read_contained_word(bytes, at_usize);
        if old > payload_len {
            proof {
                assert(old == words::le_word(bytes@, run.offset + i as int * run.stride));
                assert(!run_payload(bytes@, run, payload_len));
            }
            return Err(Error::new(ErrorKind::InvalidBoundary, at));
        }
        // The run delta bounds prove this signed sum fits i64 even on rejection.
        let shifted = (old as i64).wrapping_add(delta);
        assert(i64::MIN <= old as int + delta <= i64::MAX);
        assert(shifted as int == old as int + delta);
        if shifted as u64 > u32::MAX as u64 {
            proof {
                assert((shifted as u64 <= u32::MAX as u64)
                    == (0 <= shifted && shifted <= u32::MAX as i64)) by(bit_vector);
                assert(!patch_safe_at(bytes@, run, delta, i as int));
            }
            return Err(Error::new(ErrorKind::ArithmeticOverflow, at));
        }
        assert((shifted as u64 <= u32::MAX as u64)
            == (0 <= shifted && shifted <= u32::MAX as i64)) by(bit_vector);
        assert(0 <= shifted <= u32::MAX);
        assert(at == run.offset + i * run.stride);
        assert(0 <= words::le_word(bytes@, run.offset + i * run.stride) + delta <= u32::MAX);
        proof {
            assert(patch_safe_at(bytes@, run, delta, i as int));
            assert forall|j: int| 0 <= j < i + 1 implies #[trigger] patch_safe_at(bytes@, run, delta, j) by {
                if j < i { assert(patch_safe_at(bytes@, run, delta, j)); }
                else { assert(j == i); }
            }
        }
        i += 1;
    }
    assert(run_safe(bytes@, run, delta));
    Ok(())
}

/// A single list operation. Indices and counts retain the full wire domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListEdit {
    /// Inserts before an ordinal in `0..=count`; `count` appends.
    Insert(u32),
    /// Removes one existing element; its old bytes are exposed by the plan.
    Remove(u32),
    /// Replaces one existing element without changing the count.
    Replace(u32),
    /// Removes the last element, or does nothing for an empty list.
    Pop,
    /// Keeps at most this many elements. Larger lengths leave the list unchanged.
    Truncate(u32),
    /// Removes all elements in one splice.
    Clear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ListChange {
    operation: ListEdit,
    width: u32,
    old_count: u32,
    new_count: u32,
    first: u32,
    last: u32,
}

/// Mathematical ordinal interval deleted or replaced by the operation.
spec fn list_indices(operation: ListEdit, count: u32) -> (int, int, int) {
    match operation {
        ListEdit::Insert(i) => (i as int, i as int, count + 1),
        ListEdit::Remove(i) => (i as int, i + 1, count - 1),
        ListEdit::Replace(i) => (i as int, i + 1, count as int),
        ListEdit::Pop => (if count == 0 { 0int } else { count - 1 }, count as int,
            if count == 0 { 0int } else { count - 1 }),
        ListEdit::Truncate(n) => (if n < count { n as int } else { count as int }, count as int,
            if n < count { n as int } else { count as int }),
        ListEdit::Clear => (0int, count as int, 0int),
    }
}

spec fn list_input(operation: ListEdit, width: u32) -> u32 {
    match operation { ListEdit::Insert(_) | ListEdit::Replace(_) => width, _ => 0 }
}

spec fn list_checked(bytes: Seq<u8>, plan: Plan) -> bool {
    match plan.list {
        None => target(plan) == plan.trail.nodes[plan.trail.depth as int].region
            && (width_spec(plan.trail.nodes[plan.trail.depth as int].layout).is_some()
                ==> plan.replacement_len == width_spec(plan.trail.nodes[plan.trail.depth as int].layout).unwrap()),
        Some(change) => {
            let node = plan.trail.nodes[plan.trail.depth as int];
            let indices = list_indices(change.operation, change.old_count);
            &&& match node.layout.head() {
                LayoutHead::List { element } => width_spec(element) == Some(change.width),
                LayoutHead::Map { key, value } => width_spec(value).is_some()
                    && (change.old_count > 0 || change.new_count > 0 ==> change.width == key.width + width_spec(value).unwrap()),
                _ => false,
            }
            &&& change.old_count == words::le_word(bytes, node.region.start as int)
            &&& node.region.end == node.region.start + 4 + change.old_count * change.width
            &&& 0 <= indices.0 <= indices.1 <= change.old_count
            &&& 0 <= indices.2 <= u32::MAX
            &&& change.first == indices.0 && change.last == indices.1 && change.new_count == indices.2
            &&& plan.replacement_len == list_input(change.operation, change.width)
            &&& target(plan).start == node.region.start + 4 + change.first * change.width
            &&& target(plan).end == node.region.start + 4 + change.last * change.width
            &&& node.region.end + plan.delta == node.region.start + 4 + change.new_count * change.width
            &&& node.region.start + 4 <= node.region.end + plan.delta <= plan.new_used
            &&& change.new_count == change.old_count - (change.last - change.first)
                + if matches!(change.operation, ListEdit::Insert(_) | ListEdit::Replace(_)) { 1int } else { 0int }
        },
    }
}

/// Immutable, allocation-free replacement preflight. Not `Clone` or `Copy`.
///
/// Holds descriptors, integer coordinates, and one compressed end-word run per
/// ancestor. Map plans also borrow an external encoded query key. No account or
/// replacement-value bytes are borrowed; list/subtree plans have a static key.
/// All runs share the replacement's signed byte delta. Applying is a separate
/// operation that must recheck readiness after any storage relocation.
#[derive(Debug)]
pub struct Plan<'key> {
    key: &'key [u8],
    map: Option<MapChange>,
    variable_map: Option<IndexedMap>,
    region: Span,
    indexed: Option<IndexedInsert>,
    splice: Option<IndexedSplice>,
    list: Option<ListChange>,
    trail: Trail,
    path: InlinePath,
    discriminator: [u8; 8],
    old_used: u32,
    replacement_len: u32,
    delta: i64,
    new_used: u32,
    required: u32,
    storage_limit: u32,
}

/// Exact source and movement admission, before checking ancestor end values.
pub closed spec fn replacement_admitted(bytes: Seq<u8>, plan: Plan) -> bool {
    let target = target(plan);
    &&& trail_valid(bytes, plan.trail.root, plan.path, plan.trail, plan.old_used)
    &&& list_checked(bytes, plan)
    &&& fixed_map::map_checked(bytes, plan)
    &&& indexed_map::checked(bytes, plan)
    &&& indexed_insert::indexed_checked(bytes, plan)
    &&& indexed_remove::splice_checked(bytes, plan)
    &&& plan.trail.nodes[plan.trail.depth as int].region.start <= target.start <= target.end
        <= plan.trail.nodes[plan.trail.depth as int].region.end
    &&& 12 <= plan.old_used <= bytes.len() <= plan.storage_limit
    &&& words::le_word(bytes, 8) == plan.old_used
    &&& forall|j: int| #![trigger plan.discriminator[j]] 0 <= j < 8 ==> plan.discriminator[j] == bytes[j]
    &&& plan.delta == plan.replacement_len as int - (target.end - target.start)
    &&& plan.new_used == plan.old_used + plan.delta
    &&& 12 <= plan.new_used <= plan.required <= plan.storage_limit
    &&& plan.required == if plan.new_used > plan.old_used { plan.new_used } else { plan.old_used }
    &&& target.start + plan.replacement_len == target.end + plan.delta
    &&& target.start <= target.start + plan.replacement_len <= plan.new_used
}

/// Exact replacement/suffix arithmetic and checked ancestor word additions.
pub closed spec fn replacement_checked(bytes: Seq<u8>, plan: Plan) -> bool {
    let target = target(plan);
    &&& trail_valid(bytes, plan.trail.root, plan.path, plan.trail, plan.old_used)
    &&& list_checked(bytes, plan)
    &&& fixed_map::map_checked(bytes, plan)
    &&& indexed_map::checked(bytes, plan)
    &&& indexed_insert::indexed_checked(bytes, plan)
    &&& indexed_remove::splice_checked(bytes, plan)
    &&& plan.trail.nodes[plan.trail.depth as int].region.start <= target.start <= target.end
        <= plan.trail.nodes[plan.trail.depth as int].region.end
    &&& 12 <= plan.old_used <= bytes.len() <= plan.storage_limit
    &&& words::le_word(bytes, 8) == plan.old_used
    &&& forall|j: int| #![trigger plan.discriminator[j]] 0 <= j < 8 ==> plan.discriminator[j] == bytes[j]
    &&& plan.delta == plan.replacement_len as int - (target.end - target.start)
    &&& plan.new_used == plan.old_used + plan.delta
    &&& 12 <= plan.new_used <= plan.required <= plan.storage_limit
    &&& plan.required == if plan.new_used > plan.old_used { plan.new_used } else { plan.old_used }
    &&& target.start + plan.replacement_len == target.end + plan.delta
    &&& target.start <= target.start + plan.replacement_len <= plan.new_used
    &&& forall|i: int| 0 <= i < plan.trail.depth
        ==> run_safe(bytes, plan.trail.nodes[i].run, plan.delta)
            && (plan.delta != 0 ==> run_payload(bytes, plan.trail.nodes[i].run, plan.trail.nodes[i].payload_len))
}

spec fn input_size(plan: Plan) -> u32 {
    if plan.variable_map.is_some() { plan.variable_map.unwrap().input_len }
    else if plan.map.is_some() { fixed_map::input_len(plan) }
    else if let Some(change) = plan.splice { change.input_len }
    else { match plan.indexed { Some(change) => change.input_len, None => plan.replacement_len } }
}

spec fn destination_start(plan: Plan) -> u32 {
    if plan.variable_map.is_some() { indexed_map::destination(plan) }
    else if plan.map.is_some() { fixed_map::value_start(plan) }
    else if let Some(change) = plan.splice { (change.payload - change.table_shrink + change.offset) as u32 }
    else { match plan.indexed {
        Some(change) => (change.payload + change.table_growth + change.offset) as u32,
        None => plan.region.start,
    } }
}

/// Indexed insertion preserves the sequence and synthesizes every stored end.
pub closed spec fn indexed_insert_refines(apply: Apply, after: Seq<u8>) -> bool {
    apply.plan.indexed.is_some() ==> indexed_insert::insert_result(apply.original@, apply.bytes@, after, apply.plan)
        && indexed_insert::sequence_insert(apply.original@, apply.bytes@, after, apply.plan)
}

/// The plan captures exactly the requested root, path, length, and capacity policy.
pub closed spec fn planned_for(plan: Plan, used: u32, layout: Layout, path: InlinePath,
    replacement_len: u32, storage_limit: u32) -> bool {
    plan.old_used == used && plan.trail.root == layout && plan.path == path
        && input_size(plan) == replacement_len && plan.storage_limit == storage_limit
}

/// Distinguishes a complete subtree replacement from a collection edit.
pub closed spec fn subtree_plan(plan: Plan) -> bool {
    plan.indexed.is_none() && plan.list.is_none() && plan.splice.is_none() && plan.map.is_none() && plan.variable_map.is_none()
}

/// Plans one exact subtree replacement and at most one root suffix move.
///
/// `used_len` must match the envelope; `storage_limit` is the physical hard cap
/// and must include current storage. Fixed targets require their exact width.
/// Only replacement length is known here; its encoder must validate its content
/// before a later apply begins. Every return preserves the complete buffer.
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
pub fn plan_replace(bytes: &[u8], used_len: u32, layout: &Layout, path: &InlinePath,
    replacement_len: u32, storage_limit: u32) -> (result: Result<Plan<'static>>)
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && subtree_plan(result.unwrap())
        && planned_for(result.unwrap(), used_len, *layout, *path, replacement_len, storage_limit),
{
    envelope(bytes, used_len)?;
    if bytes.len() > storage_limit as usize { return Err(ErrorKind::Capacity.into()); }
    let trail = trace(bytes, used_len, layout, path)?;
    plan_in_trail(bytes, used_len, path, &trail, replacement_len, storage_limit)
}

#[verifier::spinoff_prover]
fn resolve_flat(bytes: &[u8], checked: crate::CheckedLayout<'static>, path: &InlinePath) -> (result: Result<Span>)
    requires checked.valid(),
    ensures resolve_path(result, bytes@, (Layout::Flat { checked }), *path),
        result.is_ok() ==> 12 <= result.unwrap().start <= result.unwrap().end <= bytes.len(),
{
    let used = read_u32(bytes, 8)?;
    envelope(bytes, used)?;
    checked.witness();
    if path.overflow || path.len >= MAX_DEPTH { return Err(ErrorKind::DepthExceeded.into()); }
    let ghost node = Node { payload_len: 0, region: Span { start: 12, end: used }, #[cfg(verus_only)] layout: (Layout::Flat { checked }), ordinal: 0,
        run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let ghost empty = Node { payload_len: 0, region: Span { start: 0, end: 0 }, #[cfg(verus_only)] layout: Layout::Str,
        ordinal: 0, run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let ghost mut nodes = [empty; MAX_DEPTH];
    proof { nodes = vstd::array::spec_array_update(nodes, 0, node); }
    let mut region = Span { start: 12, end: used };
    let mut active = checked;
    let mut depth = 0usize;
    while depth < path.len
        invariant
            !path.overflow, depth <= path.len < MAX_DEPTH,
            nodes[0].region == (Span { start: 12, end: used }), nodes[0].layout == (Layout::Flat { checked }),
            (Layout::Flat { checked: active }) == nodes[depth as int].layout,
            region == nodes[depth as int].region,
            12 <= used <= bytes.len() <= u32::MAX,
            forall|i: int| #![trigger nodes[i]] 0 <= i <= depth ==> {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= nodes[depth as int].region.start
                &&& nodes[depth as int].region.end <= n.region.end
            },
            forall|i: int| 0 <= i < depth
                ==> edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]),
        decreases path.len - depth,
    {
        let ghost parent = nodes[depth as int];
        let local = &bytes[region.start as usize..region.end as usize];
        active.witness();
        let selected = select_checked_child(local, active, path.steps[depth])
            .map_err(|e: Error| e.rebase(region.start))?;
        let ghost run = if selected.run.count == 0 { PatchRun { offset: 0, stride: 4, count: 0 } }
            else { PatchRun { offset: (parent.region.start + selected.run.offset) as u32, ..selected.run } };
        let next_region = Span { start: ((region.start as u64) + (selected.span.start as u64)) as u32,
            end: ((region.start as u64) + (selected.span.end as u64)) as u32 };
        let ghost next = Node { payload_len: 0, region: next_region, #[cfg(verus_only)] layout: selected.layout, ordinal: 0,
            run: PatchRun { offset: 0, stride: 4, count: 0 } };
        let ghost previous = nodes@;
        proof { nodes = vstd::array::spec_array_update(nodes, depth as int, Node { payload_len: selected.payload_len, ordinal: selected.ordinal, run, ..parent });
        nodes = vstd::array::spec_array_update(nodes, depth as int + 1, next);
            assert(edge_valid(bytes@, nodes[depth as int], path.steps[depth as int], nodes[depth as int + 1]));
            assert forall|i: int| 0 <= i < depth implies
                edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]) by {
                assert(edge_valid(bytes@, previous[i], path.steps[i], previous[i + 1]));
                if i + 1 == depth {
                    assert(nodes[i + 1].region == previous[i + 1].region);
                    assert(nodes[i + 1].layout == previous[i + 1].layout);
                }
            }
            assert forall|i: int| #![trigger nodes[i]] 0 <= i <= depth + 1 implies {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= next.region.start
                &&& next.region.end <= n.region.end
            } by {
                if i <= depth {
                    assert(previous[i].region == nodes[i].region);
                    assert(previous[i].region.start <= parent.region.start);
                    assert(parent.region.end <= previous[i].region.end);
                }
            }
        }
        region = next_region;
        active = match selected.layout { Layout::Flat { checked } => { checked.witness(); checked }, _ => { assert(false); return Err(ErrorKind::InvalidSchema.into()); } };
        depth += 1;
    }
    let ghost target = nodes[depth as int];
    let local = &bytes[region.start as usize..region.end as usize];
    let _parts=crate::edit_layout::enter_local(local, &(Layout::Flat { checked: active }), false).map_err(|e: Error| e.rebase(region.start))?;

    proof {
        let trail = Trail { root: (Layout::Flat { checked }), selected: Layout::Flat { checked: active }, local: true, nodes, depth };
        assert(trail_valid(bytes@, (Layout::Flat { checked }), *path, trail, used));
        assert(resolve_path(Ok(region), bytes@, (Layout::Flat { checked }), *path));
    }
    Ok(region)
}


#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn plan_in_trail(bytes: &[u8], used_len: u32, path: &InlinePath, trail: &Trail,
    replacement_len: u32, storage_limit: u32) -> (result: Result<Plan<'static>>)
    requires trail_valid(bytes@, trail.root, *path, *trail, used_len),
        12 <= used_len <= bytes.len() <= storage_limit, bytes.len() <= u32::MAX,
        words::le_word(bytes@, 8) == used_len,
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap()) && subtree_plan(result.unwrap())
        && result.unwrap().trail == *trail
        && planned_for(result.unwrap(), used_len, trail.root, *path, replacement_len, storage_limit),
        admission::subtree_ready(bytes@, *trail, replacement_len, storage_limit)
            ==> result == Ok(admission::subtree_plan_for(bytes@, *path, *trail, replacement_len, storage_limit)),
{
    let ghost layout = trail.root;
    let target = trail.nodes[trail.depth];
    if let Some(width) = fixed_width(&trail.selected) {
        if replacement_len != width { return Err(ErrorKind::InvalidBoundary.into()); }
    }
    let delta = replacement_len as i64 - (target.region.end - target.region.start) as i64;
    let new_used = checked_delta(used_len, delta)?;
    let required = if new_used > used_len { new_used } else { used_len };
    if required > storage_limit { return Err(ErrorKind::Capacity.into()); }
    let mut i = 0usize;
    while i < trail.depth
        invariant i <= trail.depth < MAX_DEPTH,
            trail_valid(bytes@, layout, *path, *trail, used_len),
            bytes.len() <= u32::MAX,
            12 <= used_len <= bytes.len() <= storage_limit,
            words::le_word(bytes@, 8) == used_len,
            target == trail.nodes[trail.depth as int],
            delta == replacement_len as int - (target.region.end - target.region.start),
            new_used == used_len as int + delta,
            required == if new_used > used_len { new_used } else { used_len },
            forall|j: int| 0 <= j < i ==> run_safe(bytes@, trail.nodes[j].run, delta)
                && (delta != 0 ==> run_payload(bytes@, trail.nodes[j].run, trail.nodes[j].payload_len)),
        decreases trail.depth - i,
    {
        assert(edge_valid(bytes@, trail.nodes[i as int], path.steps[i as int], trail.nodes[i as int + 1]));
        assert(run_within(trail.nodes[i as int].run, 0, bytes.len() as int));
        check_run(bytes, trail.nodes[i].run, delta, trail.nodes[i].payload_len)?;
        i += 1;
    }
    Ok(Plan { key: &[], map: None, variable_map: None, region: target.region, indexed: None, splice: None, list: None, trail: *trail, path: *path,
        discriminator: [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]],
        old_used: used_len, replacement_len, delta, new_used, required, storage_limit })
}

/// Arithmetic projection for collection planners, without unfolding traversal.
proof fn replacement_extent(bytes: Seq<u8>, plan: Plan)
    requires replacement_admitted(bytes,plan),
    ensures 12 <= plan.region.start <= plan.region.end <= plan.old_used <= bytes.len() <= u32::MAX,
        plan.delta == plan.replacement_len as int - (plan.region.end - plan.region.start),
        plan.new_used == plan.old_used + plan.delta,
        plan.required == if plan.new_used > plan.old_used { plan.new_used } else { plan.old_used },
{ }

spec fn with_insert(plan: Plan, change: IndexedInsert) -> Plan {
    Plan { indexed: Some(change), ..plan }
}

#[verifier::spinoff_prover]
#[inline(always)]
fn attach_insert(plan: &mut Plan, change: IndexedInsert, Ghost(bytes): Ghost<Seq<u8>>)
    requires replacement_checked(bytes, *old(plan)), subtree_plan(*old(plan)),
        indexed_insert::indexed_checked(bytes, with_insert(*old(plan), change)),
    ensures *final(plan) == with_insert(*old(plan), change), replacement_checked(bytes, *final(plan)),
{
    plan.indexed = Some(change);
}

spec fn with_splice(plan: Plan, change: IndexedSplice) -> Plan {
    Plan { splice: Some(change), ..plan }
}

#[verifier::spinoff_prover]
#[inline(always)]
fn attach_splice(plan: &mut Plan, change: IndexedSplice, Ghost(bytes): Ghost<Seq<u8>>)
    requires replacement_checked(bytes, *old(plan)), subtree_plan(*old(plan)),
        indexed_remove::splice_checked(bytes, with_splice(*old(plan), change)),
    ensures *final(plan) == with_splice(*old(plan), change), replacement_checked(bytes, *final(plan)),
{
    plan.splice = Some(change);
}

/// The captured list operation, including a successful empty fixed-list pop.
pub closed spec fn planned_list_for(plan: Plan, operation: ListEdit) -> bool {
    if let Some(change) = plan.splice { operation == change.operation }
    else { match plan.indexed {
        Some(change) => operation == ListEdit::Insert(change.index),
        None => plan.list.is_some() && plan.list.unwrap().operation == operation,
    } }
}

/// Plans a list edit with checked local and ancestor geometry.
///
/// The path selects the list. Fixed insert/replace take exactly one element's
/// width; indexed insert/replace accept any input length, including zero.
/// All other operations take zero input bytes. Bounds errors use
/// `IndexOutOfBounds`; truncate beyond the count and pop on empty are no-ops.
/// Copy any removed value from [`Plan::removed_span`] before beginning apply.
/// New content must be validated before apply, as for [`plan_replace`].
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
pub fn plan_list_edit(bytes: &[u8], used_len: u32, layout: &Layout, path: &InlinePath,
    operation: ListEdit, input_len: u32, storage_limit: u32) -> (result: Result<Plan<'static>>)
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used_len, *layout, *path, input_len, storage_limit)
        && planned_list_for(result.unwrap(), operation),
{
    envelope(bytes, used_len)?;
    if bytes.len() > storage_limit as usize { return Err(ErrorKind::Capacity.into()); }
    let trail = trace(bytes, used_len, layout, path)?;
    list_in_trail(bytes, used_len, layout, path, trail, operation, input_len, storage_limit)
}

#[allow(clippy::too_many_arguments)]
#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn list_in_trail(bytes: &[u8], used_len: u32, layout: &Layout, path: &InlinePath,
    trail: Trail, operation: ListEdit, input_len: u32, storage_limit: u32)
    -> (result: Result<Plan<'static>>)
    requires trail_valid(bytes@, *layout, *path, trail, used_len),
        12 <= used_len <= bytes.len() <= storage_limit <= u32::MAX,
        words::le_word(bytes@, 8) == used_len,
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used_len, *layout, *path, input_len, storage_limit)
        && planned_list_for(result.unwrap(), operation),
{
    let element = if let Layout::Flat {checked}=trail.selected {
        proof {reveal(crate::layout_head::head_spec);}
        if checked.header(0)!=7 {return Err(ErrorKind::InvalidSchema.into());}
        Layout::Flat {checked:checked.only_child()}
    } else {match trail.selected.head() {
        LayoutHead::List {element}=>element,
        _=>return Err(ErrorKind::InvalidSchema.into()),
    }};
    let width = match fixed_width(&element) {
        Some(width) => width,
        None => return if matches!(operation, ListEdit::Insert(_)) {
            indexed_insert::plan_insert(bytes, used_len, path, trail, operation, input_len, storage_limit)
        } else { indexed_remove::plan_splice(bytes, used_len, path, trail, operation, input_len, storage_limit) },
    };
    plan_entries(bytes, used_len, layout, path, trail, operation, input_len, width, storage_limit)
}

#[verifier::spinoff_prover]
#[allow(clippy::too_many_arguments)] // Shared checked gateway inputs plus fixed-entry geometry.
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn plan_entries(bytes: &[u8], used_len: u32, _layout: &Layout, path: &InlinePath,
    trail: Trail, operation: ListEdit, input_len: u32, width: u32, storage_limit: u32)
    -> (result: Result<Plan<'static>>)
    requires trail_valid(bytes@, *_layout, *path, trail, used_len),
        12 <= used_len <= bytes.len() <= storage_limit <= u32::MAX,
        words::le_word(bytes@, 8) == used_len,
        match trail.nodes[trail.depth as int].layout.head() {
            LayoutHead::List { element } => width_spec(element) == Some(width),
            LayoutHead::Map { key, value } => width_spec(value).is_some()
                && (words::le_word(bytes@, trail.nodes[trail.depth as int].region.start as int) > 0
                    || !matches!(operation, ListEdit::Clear | ListEdit::Truncate(_))
                    ==> width == key.width + width_spec(value).unwrap()),
            _ => false,
        },
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used_len, *_layout, *path, input_len, storage_limit)
        && planned_list_for(result.unwrap(), operation)
        && result.unwrap().trail == trail && result.unwrap().map.is_none()
        && result.unwrap().list.is_some() && result.unwrap().variable_map.is_none()
        && result.unwrap().indexed.is_none() && result.unwrap().splice.is_none(),
        admission::entries_ready(bytes@, trail, operation, input_len, width, storage_limit)
            ==> result == Ok(admission::entries_plan_for(bytes@, *path, trail, operation, input_len, width, storage_limit)),
{
    let node = trail.nodes[trail.depth];
    let count = read_u32(bytes, node.region.start)?;
    proof {
        indexed_insert::word_in_slice(bytes@, node.region.start as int, node.region.end as int, 0);
        let local = node_bytes(bytes@, node);
        assert(crate::edit_layout::header_valid(local, trail.selected));
        assert(local.len() == node.region.end - node.region.start);
        assert(words::le_word(local, 0) == count);
        match trail.selected.head() {
            LayoutHead::List { .. } => { assert(crate::list_layout::header_shape(local, trail.selected));
                assert(local.len() == 4 + count as int * width);
                assert(count == 0 ==> local.len() == 4) by(nonlinear_arith)
                    requires local.len() == 4 + count as int * width;
            },
            LayoutHead::Map { .. } => {
                assert(crate::map_layout::header_shape(local, trail.selected));
                assert(local.len() == 4 + count as int * crate::map_layout::entry_stride(trail.selected));
                assert(count == 0 ==> local.len() == 4) by(nonlinear_arith)
                    requires local.len() == 4 + count as int * crate::map_layout::entry_stride(trail.selected);
                assert(count > 0 ==> width == crate::map_layout::entry_stride(trail.selected));
            },
            _ => {},
        }
        assert(node.region.end == node.region.start + 4 + count as int * width) by(nonlinear_arith)
            requires count == 0 ==> local.len() == 4,
                count > 0 ==> local.len() == 4 + count as int * width,
                local.len() == node.region.end - node.region.start;
    }
    let (first, last, new_count) = match operation {
        ListEdit::Insert(i) => {
            if i > count { return Err(ErrorKind::IndexOutOfBounds.into()); }
            let n = checked_delta(count, 1)?;
            (i, i, n)
        },
        ListEdit::Remove(i) | ListEdit::Replace(i) => {
            if i >= count { return Err(ErrorKind::IndexOutOfBounds.into()); }
            (i, i + 1, if matches!(operation, ListEdit::Remove(_)) { count - 1 } else { count })
        },
        ListEdit::Pop => (if count == 0 { 0u32 } else { count - 1 }, count,
            if count == 0 { 0u32 } else { count - 1 }),
        ListEdit::Truncate(n) => {
            let kept = if n < count { n } else { count };
            (kept, count, kept)
        },
        ListEdit::Clear => (0u32, count, 0u32),
    };
    let expected_input = if matches!(operation, ListEdit::Insert(_) | ListEdit::Replace(_)) { width } else { 0u32 };
    if input_len != expected_input { return Err(ErrorKind::InvalidBoundary.into()); }
    proof {
        assert(first as int * width <= count as int * width) by(nonlinear_arith)
            requires first <= count;
        assert(last as int * width <= count as int * width) by(nonlinear_arith)
            requires last <= count;
    }
    let start = (node.region.start as u64 + 4 + first as u64 * width as u64) as u32;
    let end = (node.region.start as u64 + 4 + last as u64 * width as u64) as u32;
    proof {
        assert(first as int * width <= last as int * width) by(nonlinear_arith)
            requires first <= last;
    }
    let delta = input_len as i64 - (end - start) as i64;
    proof {
        let indices = list_indices(operation, count);
        assert(delta == input_len as int - (indices.1 - indices.0) * width) by(nonlinear_arith)
            requires first == indices.0, last == indices.1,
                start == node.region.start + 4 + first as int * width,
                end == node.region.start + 4 + last as int * width,
                delta == input_len as int - (end - start);
    }
    let new_used = checked_delta(used_len, delta)?;
    let required = if new_used > used_len { new_used } else { used_len };
    if required > storage_limit { return Err(ErrorKind::Capacity.into()); }
    let mut i = 0usize;
    while i < trail.depth
        invariant i <= trail.depth < MAX_DEPTH,
            trail_valid(bytes@, *_layout, *path, trail, used_len), bytes.len() <= u32::MAX,
            12 <= used_len <= bytes.len() <= storage_limit <= u32::MAX,
            words::le_word(bytes@, 8) == used_len,
            count == words::le_word(bytes@, trail.nodes[trail.depth as int].region.start as int),
            delta == input_len as int - (list_indices(operation, count).1 - list_indices(operation, count).0) * width,
            new_used == used_len as int + delta,
            required == if new_used > used_len { new_used } else { used_len },
            forall|j: int| 0 <= j < i ==> run_safe(bytes@, trail.nodes[j].run, delta)
                && (delta != 0 ==> run_payload(bytes@, trail.nodes[j].run, trail.nodes[j].payload_len)),
        decreases trail.depth - i,
    {
        assert(edge_valid(bytes@, trail.nodes[i as int], path.steps[i as int], trail.nodes[i as int + 1]));
        assert(run_within(trail.nodes[i as int].run, 0, bytes.len() as int));
        check_run(bytes, trail.nodes[i].run, delta, trail.nodes[i].payload_len)?;
        i += 1;
    }
    let change = ListChange { operation, width, old_count: count, new_count, first, last };
    let plan = Plan { key: &[], map: None, variable_map: None, region: Span { start, end }, indexed: None, splice: None, list: Some(change), trail, path: *path,
        discriminator: [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]],
        old_used: used_len, replacement_len: input_len, delta, new_used, required, storage_limit };
    proof {
        assert(node.region.end + delta == node.region.start + 4 + new_count * width) by(nonlinear_arith)
            requires node.region.end == node.region.start + 4 + count * width,
                start == node.region.start + 4 + first * width, end == node.region.start + 4 + last * width,
                delta == input_len as int - (end - start),
                new_count == count - (last - first)
                    + if matches!(operation, ListEdit::Insert(_) | ListEdit::Replace(_)) { 1int } else { 0int },
                input_len == if matches!(operation, ListEdit::Insert(_) | ListEdit::Replace(_)) { width } else { 0u32 };
    }
    assert(list_checked(bytes@, plan));
    Ok(plan)
}

/// Plans a sorted fixed-value or indexed-value map edit.
///
/// `Insert` upserts, `Replace` requires presence, absent `Remove` succeeds, and
/// `Clear` requires an empty query. Insert/replace take the value's encoded
/// length; other operations take zero. The caller validates new value bytes
/// before apply. Stored keys and all affected ends are checked during preflight.
/// Plans borrow only the external query. Apply exposes value bytes, preserving
/// every retained key; copy any old value from `removed_span()` before apply.
#[allow(clippy::too_many_arguments)]
#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
pub fn plan_map_edit<'key>(bytes: &[u8], used_len: u32, layout: &Layout, path: &InlinePath,
    operation: MapEdit, key: &'key [u8], input: u32, storage_limit: u32) -> (result: Result<Plan<'key>>)
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used_len, *layout, *path, input, storage_limit)
        && planned_map(result.unwrap(), operation, key@, input)
        && previous_map_exact(bytes@, result.unwrap()),
{
    hide(replacement_checked); hide(planned_for);
    hide(indexed_map::previous_exact);
    envelope(bytes, used_len)?;
    if bytes.len() > storage_limit as usize { return Err(ErrorKind::Capacity.into()); }
    let trail = trace(bytes, used_len, layout, path)?;
    map_in_trail(bytes, used_len, layout, path, trail, operation, key, input, storage_limit, None)
}

#[allow(clippy::too_many_arguments)]
#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn map_in_trail<'key>(bytes: &[u8], used_len: u32, layout: &Layout, path: &InlinePath,
    trail: Trail, operation: MapEdit, key: &'key [u8], input: u32, storage_limit: u32, hint: Option<crate::Search>)
    -> (result: Result<Plan<'key>>)
    requires trail_valid(bytes@, *layout, *path, trail, used_len),
        12 <= used_len <= bytes.len() <= storage_limit <= u32::MAX,
        words::le_word(bytes@, 8) == used_len,
    ensures result.is_ok() ==> replacement_checked(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used_len, *layout, *path, input, storage_limit)
        && planned_map(result.unwrap(), operation, key@, input)
        && previous_map_exact(bytes@, result.unwrap()),
{
    let selected=trail.selected;
    let value = if let Layout::Flat {checked}=selected {
        proof {reveal(crate::layout_head::head_spec);}
        if checked.header(0)!=8 {return Err(ErrorKind::InvalidSchema.into());}
        Layout::Flat {checked:checked.only_child()}
    } else {match selected.head() {
        LayoutHead::Map{value,..}=>value,
        _=>return Err(ErrorKind::InvalidSchema.into()),
    }};
    if fixed_width(&value).is_some() {
            fixed_map::map_in_resolved(bytes, used_len, layout, path, trail, operation, key, input, storage_limit, hint)
        } else { indexed_map::map_in_resolved(bytes, used_len, layout, path, trail, operation, key, input, storage_limit, hint) }
}

/// Selects the fixed-value representation without exposing plan fields.
pub closed spec fn fixed_map_plan(plan: Plan) -> bool { plan.variable_map.is_none() }

/// Captured map operation, query, input length, and representation.
pub closed spec fn planned_map(plan: Plan, operation: MapEdit, key: Seq<u8>, input: u32) -> bool {
    if plan.variable_map.is_some() { indexed_map::planned(plan, operation, key, input) }
    else { fixed_map::planned_map_for(plan, operation, key, input) }
}

/// The returned old interval is the selected value, excluding the immutable key.
pub closed spec fn previous_map_exact(bytes: Seq<u8>, plan: Plan) -> bool {
    if plan.variable_map.is_some() { indexed_map::previous_exact(bytes, plan) }
    else { fixed_map::map_previous_exact(bytes, plan) }
}

/// Exact optional old element interval for remove/pop; other operations return None.
pub closed spec fn removed_exact(plan: Plan, result: Option<Span>) -> bool {
    result == if plan.variable_map.is_some() { indexed_map::removed(plan) }
    else if plan.map.is_some() { fixed_map::removed(plan) }
    else if let Some(change) = plan.splice {
        if matches!(change.operation, ListEdit::Remove(_) | ListEdit::Pop) && change.first < change.last {
            Some(Span { start: (change.payload + change.offset) as u32, end: (change.payload + change.end_offset) as u32 })
        } else { None }
    } else { match plan.list {
        Some(change) => if matches!(change.operation, ListEdit::Remove(_) | ListEdit::Pop)
                && change.first < change.last { Some(target(plan)) } else { None },
        None => None,
    } }
}

spec fn geometry_equal(a: Plan, b: Plan) -> bool {
    &&& a.old_used == b.old_used && a.new_used == b.new_used && a.required == b.required
    &&& a.delta == b.delta && a.trail.depth == b.trail.depth
    &&& a.discriminator == b.discriminator
    &&& a.map == b.map && a.variable_map == b.variable_map && a.key@ == b.key@
    &&& a.region == b.region && a.list == b.list && a.indexed == b.indexed && a.splice == b.splice
    &&& forall|i: int| 0 <= i <= a.trail.depth ==> {
        &&& a.trail.nodes[i].region == b.trail.nodes[i].region
        &&& a.trail.nodes[i].ordinal == b.trail.nodes[i].ordinal
        &&& a.trail.nodes[i].run == b.trail.nodes[i].run
    }
}

fn same_list(a: Option<ListChange>, b: Option<ListChange>) -> (result: bool)
    ensures result == (a == b),
{
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            let operation = match (a.operation, b.operation) {
                (ListEdit::Insert(i), ListEdit::Insert(j))
                | (ListEdit::Remove(i), ListEdit::Remove(j))
                | (ListEdit::Replace(i), ListEdit::Replace(j))
                | (ListEdit::Truncate(i), ListEdit::Truncate(j)) => i == j,
                (ListEdit::Pop, ListEdit::Pop) | (ListEdit::Clear, ListEdit::Clear) => true,
                _ => false,
            };
            operation && a.width == b.width && a.old_count == b.old_count && a.new_count == b.new_count
                && a.first == b.first && a.last == b.last
        },
        _ => false,
    }
}

fn same_geometry(a: &Plan, b: &Plan) -> (result: bool)
    requires a.trail.depth < MAX_DEPTH, b.trail.depth < MAX_DEPTH,
    ensures result == geometry_equal(*a, *b),
{
    if a.old_used != b.old_used || a.new_used != b.new_used || a.required != b.required
        || a.delta != b.delta || a.trail.depth != b.trail.depth
        || a.region.start != b.region.start || a.region.end != b.region.end
        || !fixed_map::same_map(a.map, b.map)
        || !indexed_map::same_change(a.variable_map, b.variable_map)
        || !matches!(crate::compare_keys(a.key, b.key), core::cmp::Ordering::Equal)
        || !same_list(a.list, b.list) || !indexed_insert::same_indexed(a.indexed, b.indexed)
        || !indexed_remove::same_splice(a.splice, b.splice) { return false; }
    let mut j = 0usize;
    while j < 8
        invariant j <= 8,
            a.old_used == b.old_used, a.new_used == b.new_used, a.required == b.required,
            a.delta == b.delta, a.trail.depth == b.trail.depth < MAX_DEPTH,
            a.map == b.map, a.variable_map == b.variable_map, a.key@ == b.key@,
            a.region == b.region, a.list == b.list, a.indexed == b.indexed, a.splice == b.splice,
            forall|k: int| 0 <= k < j ==> a.discriminator[k] == b.discriminator[k],
        decreases 8 - j,
    {
        if a.discriminator[j] != b.discriminator[j] { return false; }
        j += 1;
    }
    assert(a.discriminator@ =~= b.discriminator@);
    let mut i = 0usize;
    while i <= a.trail.depth
        invariant i <= a.trail.depth + 1,
            a.old_used == b.old_used, a.new_used == b.new_used, a.required == b.required,
            a.delta == b.delta, a.trail.depth == b.trail.depth < MAX_DEPTH,
            a.map == b.map, a.variable_map == b.variable_map, a.key@ == b.key@,
            a.region == b.region, a.list == b.list, a.indexed == b.indexed, a.splice == b.splice,
            a.discriminator == b.discriminator,
            forall|k: int| 0 <= k < i ==> a.trail.nodes[k].region == b.trail.nodes[k].region
                && a.trail.nodes[k].ordinal == b.trail.nodes[k].ordinal
                && a.trail.nodes[k].run == b.trail.nodes[k].run,
        decreases a.trail.depth + 1 - i,
    {
        let x = a.trail.nodes[i];
        let y = b.trail.nodes[i];
        if x.region.start != y.region.start || x.region.end != y.region.end || x.ordinal != y.ordinal
            || x.run.offset != y.run.offset || x.run.stride != y.run.stride || x.run.count != y.run.count {
            return false;
        }
        i += 1;
    }
    true
}

/// Required source/destination storage extent.
pub closed spec fn required_len(plan: Plan) -> u32 { plan.required }

/// Final logical extent.
pub closed spec fn used_after(plan: Plan) -> u32 { plan.new_used }

/// A compatible fresh plan exists for the current storage and captured facts.
pub closed spec fn readiness(result: Result<()>, bytes: Seq<u8>, plan: Plan) -> bool {
    match result {
        Ok(()) => exists|fresh: Plan| replacement_checked(bytes, fresh) && geometry_equal(plan, fresh)
            && planned_for(fresh, plan.old_used, plan.trail.root, plan.path,
                input_size(plan), plan.storage_limit) && bytes.len() >= plan.required,
        Err(error) => error == (Error::Fault { kind: ErrorKind::InvalidPlan, offset: u32::MAX }),
    }
}

impl<'key> Plan<'key> {
    /// Minimum physical length covering both old source and new destination.
    pub fn required_storage_len(&self) -> (result: u32)
        ensures result == required_len(*self),
    { self.required }

    /// Used extent to publish after completing the replacement and all patches.
    pub fn new_used_len(&self) -> (result: u32)
        ensures result == used_after(*self),
    { self.new_used }

    /// Original value bytes for list remove/pop or a present map remove/upsert/replace.
    ///
    /// Copy/decode these account coordinates before `begin_apply`. Absence returns `None`.
    /// No byte reference is retained.
    pub fn removed_span(&self) -> (result: Option<Span>)
        ensures removed_exact(*self, result),
    {
        if self.variable_map.is_some() { return indexed_map::removed_span(self); }
        if self.map.is_some() { return fixed_map::removed_span(self); }
        if let Some(change) = self.splice {
            return if matches!(change.operation, ListEdit::Remove(_) | ListEdit::Pop) && change.first < change.last {
                Some(Span { start: (change.payload as u64 + change.offset as u64) as u32,
                    end: (change.payload as u64 + change.end_offset as u64) as u32 })
            } else { None };
        }
        match self.list {
            Some(change) => if matches!(change.operation, ListEdit::Remove(_) | ListEdit::Pop)
                    && change.first < change.last { Some(self.region) } else { None },
            None => None,
        }
    }

    /// Rechecks envelope identity, exact path geometry, capacity, and every patch.
    ///
    /// Returns `InvalidPlan` before any write when these facts are incompatible.
    /// Relocation, zero extension, spare changes, and content changes preserving
    /// compatible geometry are allowed. This is structural readiness, not a
    /// snapshot comparison of unrelated payload bytes.
    pub fn check_ready(&self, bytes: &[u8]) -> (result: Result<()>)
        ensures readiness(result, bytes@, *self),
    {
        self.ready_plan(bytes)?;
        Ok(())
    }

    fn ready_plan(&self, bytes: &[u8]) -> (result: Result<Plan<'key>>)
        ensures match result {
            Ok(fresh) => replacement_checked(bytes@, fresh) && bytes.len() >= fresh.required
                && geometry_equal(*self, fresh)
                && planned_for(fresh, self.old_used, self.trail.root, self.path,
                    input_size(*self), self.storage_limit)
                && readiness(Ok(()), bytes@, *self),
            Err(error) => readiness(Err(error), bytes@, *self),
        },
    {
        if self.trail.depth >= MAX_DEPTH || bytes.len() < self.required as usize { return Err(ErrorKind::InvalidPlan.into()); }
        let root = self.trail.root;
        let replanned = if let Some(change) = self.variable_map {
            indexed_map::plan_map(bytes, self.old_used, &root, &self.path,
                change.change.operation, self.key, change.input_len, self.storage_limit)
        } else if let Some(change) = self.map {
            fixed_map::plan_map_edit(bytes, self.old_used, &root, &self.path,
                change.operation, self.key, fixed_map::input_size_exec(self), self.storage_limit)
        } else if let Some(change) = self.splice {
            plan_list_edit(bytes, self.old_used, &root, &self.path, change.operation, change.input_len, self.storage_limit)
        } else if let Some(change) = self.indexed {
            plan_list_edit(bytes, self.old_used, &root, &self.path, ListEdit::Insert(change.index), change.input_len, self.storage_limit)
        } else { match self.list {
            None => plan_replace(bytes, self.old_used, &root, &self.path, self.replacement_len, self.storage_limit),
            Some(change) => plan_list_edit(bytes, self.old_used, &root, &self.path,
                change.operation, self.replacement_len, self.storage_limit),
        } };
        let fresh = match replanned {
            Ok(fresh) => fresh,
            Err(_) => return Err(ErrorKind::InvalidPlan.into()),
        };
        if !same_geometry(self, &fresh) { return Err(ErrorKind::InvalidPlan.into()); }
        Ok(fresh)
    }
}

spec fn run_at(run: PatchRun, k: int) -> int { run.offset + k * run.stride }

spec fn run_changed(run: PatchRun, done: int, at: int) -> bool {
    exists|k: int| 0 <= k < done && #[trigger] run_at(run, k) <= at < run_at(run, k) + 4
}

spec fn patch_delta(before: Seq<u8>, after: Seq<u8>, run: PatchRun, delta: i64, done: int) -> bool {
    forall|k: int| 0 <= k < done ==> #[trigger] words::encode_exact(after,
        run_at(run, k), (words::le_word(before, run_at(run, k)) + delta) as u32)
}

spec fn patch_frame(before: Seq<u8>, after: Seq<u8>, run: PatchRun, done: int) -> bool {
    before.len() == after.len() && forall|i: int| 0 <= i < before.len() && !run_changed(run, done, i)
        ==> after[i] == before[i]
}

proof fn run_disjoint(run: PatchRun, i: int, j: int)
    requires run.stride >= 4, 0 <= i < j,
    ensures run_at(run, i) + 4 <= run_at(run, j),
{
    assert(run.offset + i * run.stride + 4 <= run.offset + j * run.stride) by(nonlinear_arith)
        requires 0 <= i < j, run.stride >= 4;
}

/// Extending an encoded prefix preserves source keys and advances every end.
#[verifier::spinoff_prover]
pub proof fn directory_suffix_induction(before: Seq<u8>, prefix: Seq<u8>, entry: Seq<u8>,
    key: int, stride: int, delta: int, done: int)
    requires 0 <= key, key + 4 == stride, 0 <= done,
        (done + 1) * stride <= before.len(), prefix.len() == done * stride, entry.len() == stride,
        words::le_word(entry, key) == words::le_word(before, done * stride + key) + delta,
        forall|k: int| 0 <= k < key ==> #[trigger] entry[k] == before[done * stride + k],
        forall|i: int| 0 <= i < done ==> #[trigger] words::le_word(prefix, i * stride + key)
            == words::le_word(before, i * stride + key) + delta,
        forall|i: int, k: int| 0 <= i < done && 0 <= k < key ==>
            #[trigger] prefix[i * stride + k] == before[i * stride + k],
    ensures forall|i: int| 0 <= i < done + 1 ==> #[trigger] words::le_word(prefix + entry, i * stride + key)
            == words::le_word(before, i * stride + key) + delta,
        forall|i: int, k: int| 0 <= i < done + 1 && 0 <= k < key ==>
            #[trigger] (prefix + entry)[i * stride + k] == before[i * stride + k],
{
    let next = prefix + entry;
    assert forall|i: int| 0 <= i < done + 1 implies #[trigger] words::le_word(next, i * stride + key)
        == words::le_word(before, i * stride + key) + delta by {
        if i < done {
            assert(i * stride + key + 4 <= prefix.len()) by(nonlinear_arith)
                requires key + 4 == stride, i < done, prefix.len() == done * stride, stride > 0;
            assert(words::le_word(prefix, i * stride + key) == words::le_word(before, i * stride + key) + delta);
            words::word_subrange(next, 0, prefix.len() as int, i * stride + key);
        } else {
            assert(i * stride == prefix.len());
        }
    }
    assert forall|i: int, k: int| 0 <= i < done + 1 && 0 <= k < key implies
        #[trigger] next[i * stride + k] == before[i * stride + k] by {
        if i < done {
            assert(i * stride + k < prefix.len()) by(nonlinear_arith)
                requires i < done, k < key, key < stride, prefix.len() == done * stride, stride > 0;
        }
    }
}

#[cfg(verus_only)]
proof fn directory_suffix_induction_witness()
{
    directory_suffix_induction(seq![1u8,4,0,0,0,2,9,0,0,0],
        seq![1u8,5,0,0,0],seq![2u8,10,0,0,0],1,5,1,1);
}

/// Applies a uniform delta to exact key/end records after complete admission.
///
/// The caller supplies `_count` complete records with `key + 4 == stride` and
/// proves every adjusted end fits u32. Each key and the slice length are
/// preserved. This kernel is private to production; Kani exports it separately.
#[verifier::spinoff_prover]
#[inline(always)]
#[verifier::rlimit(120)]
pub fn patch_directory_suffix(bytes: &mut [u8], key: usize, stride: usize, delta: i64, _count: usize)
    requires key + 4 == stride, 0 <= _count, old(bytes).len() == _count * stride,
        old(bytes).len() <= u32::MAX,
        forall|i: int| 0 <= i < _count ==> 0 <= #[trigger] words::le_word(old(bytes)@, i * stride + key) + delta <= u32::MAX,
    ensures final(bytes).len() == old(bytes).len(),
        forall|i: int| 0 <= i < _count ==> #[trigger] words::le_word(final(bytes)@, i * stride + key)
            == words::le_word(old(bytes)@, i * stride + key) + delta,
        forall|i: int, k: int| 0 <= i < _count && 0 <= k < key ==>
            #[trigger] final(bytes)@[i * stride + k] == old(bytes)@[i * stride + k],
    no_unwind
{
    let ghost before = bytes@;
    let ghost future = final(bytes)@;
    let ghost mut prefix = Seq::<u8>::empty();
    let ghost mut done = 0int;
    {
        let mut rest = &mut *bytes;
        while !rest.is_empty()
            invariant key + 4 == stride, 0 <= done <= _count,
                before.len() == _count * stride <= u32::MAX,
                prefix.len() <= before.len(), prefix.len() + rest.len() == before.len(),
                prefix.len() == done * stride, rest.len() == (_count - done) * stride,
                rest@ == before.subrange(prefix.len() as int, before.len() as int),
                future == prefix + final(rest)@,
                forall|i: int| 0 <= i < _count ==> 0 <= #[trigger] words::le_word(before, i * stride + key) + delta <= u32::MAX,
                forall|i: int| 0 <= i < done ==> #[trigger] words::le_word(prefix, i * stride + key)
                    == words::le_word(before, i * stride + key) + delta,
                forall|i: int, k: int| 0 <= i < done && 0 <= k < key ==>
                    #[trigger] prefix[i * stride + k] == before[i * stride + k],
            decreases rest.len(),
        {
            proof {
                assert(done < _count && stride <= rest.len()) by(nonlinear_arith)
                    requires rest.len() > 0, rest.len() == (_count - done) * stride, stride > 0, done <= _count;
                words::word_subrange(before, prefix.len() as int, before.len() as int, key as int);
            }
            let ghost offset = prefix.len();
            let ghost length = rest.len();
            let (entry, tail) = rest.split_at_mut(stride);
            proof {
                vstd::slice::mut_ref_slice_len_eq(&entry);
                assert(entry.len() == stride);
                assert(entry@ =~= before.subrange(offset as int, offset + stride));
                words::word_subrange(before, offset as int, offset + stride, key as int);
            }
            let value = crate::words::read_contained_word(entry, key);
            let end = crate::arithmetic::apply_checked_delta(value, delta);
            crate::words::write_contained_word(entry, key, end);
            proof {
                assert(entry.len() == stride);
                assert(tail.len() == length - stride);
                let next = prefix + entry@;
                assert((done + 1) * stride == prefix.len() + stride) by(nonlinear_arith)
                    requires prefix.len() == done * stride;
                assert(tail.len() == (_count - (done + 1)) * stride) by(nonlinear_arith)
                    requires tail.len() == length - stride, length == (_count - done) * stride;
                directory_suffix_induction(before, prefix, entry@, key as int, stride as int, delta as int, done);
                prefix = next;
                done = done + 1;
                assert(prefix.len() == done * stride);
                assert(tail.len() == (_count - done) * stride);
                assert(tail@ =~= before.subrange(prefix.len() as int, before.len() as int));
            }
            rest = tail;
        }
        proof {
            assert(done == _count) by(nonlinear_arith)
                requires rest.len() == (_count - done) * stride, rest.len() == 0, stride > 0;
            assert(future =~= prefix);
        }
    }
}

#[verifier::spinoff_prover]
#[inline(always)]
fn patch_contiguous_words(bytes: &mut [u8], _count: u32, delta: i64)
    requires old(bytes).len() == 4 * _count, old(bytes).len() <= u32::MAX,
        run_safe(old(bytes)@, PatchRun { offset: 0, stride: 4, count: _count }, delta),
    ensures patch_delta(old(bytes)@, final(bytes)@, PatchRun { offset: 0, stride: 4, count: _count }, delta, _count as int),
        patch_frame(old(bytes)@, final(bytes)@, PatchRun { offset: 0, stride: 4, count: _count }, _count as int),
    no_unwind
{
    let ghost before = bytes@;
    let ghost run = PatchRun { offset: 0, stride: 4, count: _count };
    let mut at = 0usize;
    while bytes.len() - at >= 4
        invariant at <= bytes.len() == 4 * _count, bytes.len() == before.len() <= u32::MAX,
            at % 4 == 0, run.stride == 4, run.offset == 0, run.count == _count,
            run_safe(before, run, delta),
            patch_delta(before, bytes@, run, delta, at as int / 4),
            patch_frame(before, bytes@, run, at as int / 4),
        decreases bytes.len() - at,
    {
        let ghost i = at as int / 4;
        proof {
            assert(at == run_at(run, i));
            assert forall|p: int| at <= p < at + 4 implies !run_changed(run, i, p) by {
                if run_changed(run, i, p) {
                    let k = choose|k: int| 0 <= k < i && #[trigger] run_at(run, k) <= p < run_at(run, k) + 4;
                    run_disjoint(run, k, i);
                }
            }
            assert(words::le_word(bytes@, at as int) == words::le_word(before, at as int));
            assert(patch_safe_at(before, run, delta, i));
        }
        let value = crate::words::read_contained_word(bytes, at);
        let new = crate::arithmetic::apply_checked_delta(value, delta);
        let ghost previous = bytes@;
        crate::words::write_contained_word(bytes, at, new);
        proof {
            assert forall|k: int| 0 <= k < i + 1 implies #[trigger] words::encode_exact(bytes@,
                run_at(run, k), (words::le_word(before, run_at(run, k)) + delta) as u32) by {
                if k < i {
                    run_disjoint(run, k, i);
                    assert(words::encode_exact(previous, run_at(run, k),
                        (words::le_word(before, run_at(run, k)) + delta) as u32));
                }
            }
            assert forall|p: int| 0 <= p < before.len() && !run_changed(run, i + 1, p)
                implies bytes@[p] == before[p] by {
                if at <= p < at + 4 { assert(run_changed(run, i + 1, p)); }
                if run_changed(run, i, p) {
                    let k = choose|k: int| 0 <= k < i && #[trigger] run_at(run, k) <= p < run_at(run, k) + 4;
                    assert(run_changed(run, i + 1, p));
                }
            }
        }
        at += 4;
    }
    assert(at == bytes.len());
}

#[verifier::spinoff_prover]
#[inline(always)]
fn patch_contiguous_run(bytes: &mut [u8], run: PatchRun, delta: i64)
    requires run.count > 0, run.stride == 4, run_within(run, 0, old(bytes).len() as int),
        old(bytes).len() <= u32::MAX, run_safe(old(bytes)@, run, delta),
    ensures patch_delta(old(bytes)@, final(bytes)@, run, delta, run.count as int),
        patch_frame(old(bytes)@, final(bytes)@, run, run.count as int),
    no_unwind
{
    let ghost before = bytes@;
    let length = run.count as usize * 4;
    let ghost local_run = PatchRun { offset: 0, stride: 4, count: run.count };
    let ghost local_before = before.subrange(run.offset as int, run.offset + length);
    let ghost mut patched = Seq::<u8>::empty();
    {
        let directory = borrow_destination(bytes, run.offset, length as u32);
        proof {
            assert forall|i: int| 0 <= i < run.count implies #[trigger] patch_safe_at(directory@, local_run, delta, i) by {
                indexed_insert::word_in_slice(before, run.offset as int, run.offset + length, 4*i);
                assert(patch_safe_at(before, run, delta, i));
            }
        }
        patch_contiguous_words(directory, run.count, delta);
        proof {
            assert(patch_delta(local_before, directory@, local_run, delta, run.count as int));
            patched = directory@;
        }
    }
    proof {
        assert(patch_delta(local_before, patched, local_run, delta, run.count as int));
        assert(bytes@.subrange(run.offset as int, run.offset + length) == patched);
        assert forall|i: int| 0 <= i < run.count implies #[trigger] words::encode_exact(bytes@,
            run_at(run, i), (words::le_word(before, run_at(run, i)) + delta) as u32) by {
            indexed_insert::word_in_slice(before, run.offset as int, run.offset + length, 4*i);
            assert(patch_safe_at(before, run, delta, i));
            assert(words::encode_exact(patched, run_at(local_run, i), (words::le_word(local_before, run_at(local_run, i)) + delta) as u32));
            assert(words::encode_exact(patched, 4*i, (words::le_word(before, run_at(run, i)) + delta) as u32));
        }
        assert forall|p: int| 0 <= p < bytes.len() && !run_changed(run, run.count as int, p)
            implies bytes@[p] == before[p] by {
            if run.offset <= p < run.offset + length {
                let i = (p - run.offset) / 4;
                assert(0 <= i < run.count);
                assert(run_at(run, i) <= p < run_at(run, i) + 4);
                assert(run_changed(run, run.count as int, p));
            }
        }
    }
}

#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn patch_run(bytes: &mut [u8], run: PatchRun, delta: i64)
    requires run_within(run, 0, old(bytes).len() as int), old(bytes).len() <= u32::MAX,
        run_safe(old(bytes)@, run, delta),
    ensures patch_delta(old(bytes)@, final(bytes)@, run, delta, run.count as int),
        patch_frame(old(bytes)@, final(bytes)@, run, run.count as int),
    no_unwind
{
    if run.stride == 4 && run.count > 0 {
        patch_contiguous_run(bytes, run, delta);
        return;
    }

    let ghost before = bytes@;
    let mut i = 0u32;
    while i < run.count
        invariant i <= run.count, run_within(run, 0, bytes.len() as int), bytes.len() <= u32::MAX,
            run_safe(before, run, delta),
            patch_delta(before, bytes@, run, delta, i as int),
            patch_frame(before, bytes@, run, i as int),
        decreases run.count - i,
    {
        proof { run_index_bounds(run, bytes.len() as int, i as int); }
        let at_usize = run.offset as usize + i as usize * run.stride as usize;
        let ghost at = at_usize as u32;
        assert(at == run_at(run, i as int));
        proof {
            assert forall|p: int| at <= p < at + 4 implies !run_changed(run, i as int, p) by {
                if run_changed(run, i as int, p) {
                    let k = choose|k: int| 0 <= k < i && #[trigger] run_at(run, k) <= p < run_at(run, k) + 4;
                    run_disjoint(run, k, i as int);
                }
            }
            assert(words::le_word(bytes@, at as int) == words::le_word(before, at as int));
        }
        assert(patch_safe_at(before, run, delta, i as int));
        let value = crate::words::read_contained_word(bytes, at_usize);
        assert(0 <= value as int + delta <= u32::MAX);
        let new = crate::arithmetic::apply_checked_delta(value, delta);
        let ghost previous = bytes@;
        crate::words::write_contained_word(bytes, at_usize, new);
        proof {
            assert forall|k: int| 0 <= k < i + 1 implies #[trigger] words::encode_exact(bytes@,
                run_at(run, k), (words::le_word(before, run_at(run, k)) + delta) as u32) by {
                if k < i {
                    run_disjoint(run, k, i as int);
                    assert(words::encode_exact(previous, run_at(run, k),
                        (words::le_word(before, run_at(run, k)) + delta) as u32));
                }
            }
            assert forall|p: int| 0 <= p < before.len() && !run_changed(run, i as int + 1, p)
                implies bytes@[p] == before[p] by {
                if at <= p < at + 4 { assert(run_changed(run, i as int + 1, p)); }
                if run_changed(run, i as int, p) {
                    let k = choose|k: int| 0 <= k < i && #[trigger] run_at(run, k) <= p < run_at(run, k) + 4;
                    assert(run_changed(run, i as int + 1, p));
                }
            }
        }
        i += 1;
    }
}

/// Mathematical address of one strided directory end.
pub open spec fn directory_word_at(first: u32, stride: u32, index: int) -> int {
    first + index * stride
}

/// Exact uniform directory delta and complete physical buffer frame.
pub open spec fn directory_delta(before: Seq<u8>, after: Seq<u8>, first: u32,
    stride: u32, count: u32, delta: i64) -> bool {
    &&& before.len() == after.len()
    &&& forall|i: int| 0 <= i < count ==> #[trigger] words::le_word(after, directory_word_at(first, stride, i))
        == words::le_word(before, directory_word_at(first, stride, i)) + delta
    &&& forall|p: int| 0 <= p < before.len()
        && !(exists|i: int| 0 <= i < count && #[trigger] directory_word_at(first, stride, i) <= p < directory_word_at(first, stride, i) + 4)
        ==> #[trigger] after[p] == before[p]
}

/// Applies one signed delta to a contiguous or strided run of directory ends.
///
/// Checks the complete range and every changed end before writing. Nonempty
/// runs require `stride >= 4`; nonzero deltas also require each old end to be
/// within `payload_len`. Every error preserves the whole physical buffer.
/// Keys between strided ends and all bytes outside the run remain unchanged.
/// The caller remains responsible for joins with neighboring, unpatched ends.
#[doc(hidden)]
#[verifier::spinoff_prover]
pub fn adjust_directory(bytes: &mut [u8], first: u32, stride: u32, count: u32,
    delta: i64, payload_len: u32) -> (result: Result<()>)
    ensures result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> directory_delta(old(bytes)@, final(bytes)@, first, stride, count, delta),
{
    let length = checked_u32(bytes.len())?;
    let run = PatchRun { offset: first, stride, count };
    check_run_extent(run, length)?;
    // This standalone gateway has no enclosing splice frame to protect.
    check_run_impl(bytes, run, delta, payload_len)?;
    let ghost before = bytes@;
    patch_run(bytes, run, delta);
    proof {
        assert forall|i: int| 0 <= i < count implies #[trigger] words::le_word(bytes@, directory_word_at(first, stride, i))
            == words::le_word(before, directory_word_at(first, stride, i)) + delta by {
            assert(patch_safe_at(before, run, delta, i));
            assert(words::encode_exact(bytes@, run_at(run, i),
                (words::le_word(before, run_at(run, i)) + delta) as u32));
        }
        assert forall|p: int| 0 <= p < before.len()
            && !(exists|i: int| 0 <= i < count && #[trigger] directory_word_at(first, stride, i) <= p < directory_word_at(first, stride, i) + 4)
            implies #[trigger] bytes@[p] == before[p] by {
            if run_changed(run, count as int, p) {
                let i = choose|i: int| 0 <= i < count && #[trigger] run_at(run, i) <= p < run_at(run, i) + 4;
                assert(directory_word_at(first, stride, i) == run_at(run, i));
                assert(exists|i: int| 0 <= i < count && #[trigger] directory_word_at(first, stride, i) <= p < directory_word_at(first, stride, i) + 4);
            }
        }
    }
    Ok(())
}

spec fn ancestor_changed(plan: Plan, done: int, at: int) -> bool {
    exists|j: int| 0 <= j < done && run_changed(plan.trail.nodes[j].run,
        plan.trail.nodes[j].run.count as int, at)
}

spec fn ancestors_delta(before: Seq<u8>, after: Seq<u8>, plan: Plan, done: int) -> bool {
    forall|j: int| 0 <= j < done ==> #[trigger] patch_delta(before, after,
        plan.trail.nodes[j].run, plan.delta, plan.trail.nodes[j].run.count as int)
}

spec fn ancestors_frame(before: Seq<u8>, after: Seq<u8>, plan: Plan, done: int) -> bool {
    before.len() == after.len() && forall|p: int| 0 <= p < before.len() && !ancestor_changed(plan, done, p)
        ==> after[p] == before[p]
}

proof fn trail_order(bytes: Seq<u8>, plan: Plan, i: int, j: int)
    requires replacement_checked(bytes, plan), 0 <= i <= j <= plan.trail.depth,
    ensures plan.trail.nodes[i].region.start <= plan.trail.nodes[j].region.start,
    decreases j - i,
{
    if i < j {
        trail_order(bytes, plan, i, j - 1);
        assert(edge_valid(bytes, plan.trail.nodes[j - 1], plan.path.steps[j - 1], plan.trail.nodes[j]));
    }
}

spec fn patch_geometry(original: Seq<u8>, plan: Plan) -> bool {
    &&& plan.trail.depth < MAX_DEPTH
    &&& target(plan).start <= original.len() <= u32::MAX
    &&& forall|j: int| 0 <= j < plan.trail.depth ==> {
        &&& run_within(plan.trail.nodes[j].run, plan.trail.nodes[j].region.start as int,
            plan.trail.nodes[j + 1].region.start as int)
        &&& run_safe(original, plan.trail.nodes[j].run, plan.delta)
        &&& 12 <= plan.trail.nodes[j].region.start
        &&& plan.trail.nodes[j + 1].region.start <= target(plan).start <= original.len() <= u32::MAX
    }
    &&& forall|i: int, j: int| 0 <= i <= j <= plan.trail.depth
        ==> plan.trail.nodes[i].region.start <= plan.trail.nodes[j].region.start
}

proof fn replacement_geometry(original: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan),
    ensures patch_geometry(original, plan),
{
    assert forall|i: int, j: int| 0 <= i <= j <= plan.trail.depth
        implies plan.trail.nodes[i].region.start <= plan.trail.nodes[j].region.start by {
        trail_order(original, plan, i, j);
    }
    assert forall|j: int| 0 <= j < plan.trail.depth implies {
        &&& run_within(plan.trail.nodes[j].run, plan.trail.nodes[j].region.start as int,
            plan.trail.nodes[j + 1].region.start as int)
        &&& run_safe(original, plan.trail.nodes[j].run, plan.delta)
        &&& 12 <= plan.trail.nodes[j].region.start
        &&& plan.trail.nodes[j + 1].region.start <= target(plan).start <= original.len() <= u32::MAX
    } by {
        assert(edge_valid(original, plan.trail.nodes[j], plan.path.steps[j], plan.trail.nodes[j + 1]));
    }
}

proof fn ancestor_separation(bytes: Seq<u8>, plan: Plan, i: int, j: int, p: int, q: int)
    requires patch_geometry(bytes, plan), 0 <= i < j < plan.trail.depth,
        run_changed(plan.trail.nodes[i].run, plan.trail.nodes[i].run.count as int, p),
        run_changed(plan.trail.nodes[j].run, plan.trail.nodes[j].run.count as int, q),
    ensures p < q,
{
    let a = plan.trail.nodes[i].run;
    let b = plan.trail.nodes[j].run;
    let k = choose|k: int| 0 <= k < a.count && #[trigger] run_at(a, k) <= p < run_at(a, k) + 4;
    let l = choose|l: int| 0 <= l < b.count && #[trigger] run_at(b, l) <= q < run_at(b, l) + 4;
    run_index_bounds(a, plan.trail.nodes[i + 1].region.start as int, k);
    assert(b.offset >= plan.trail.nodes[j].region.start);
}

#[verifier::spinoff_prover]
fn patch_ancestors(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>)
    requires patch_geometry(original, *plan), bytes.len() == original.len(),
        forall|p: int| 0 <= p < target(*plan).start ==> bytes@[p] == original[p],
    ensures ancestors_delta(original, final(bytes)@, *plan, plan.trail.depth as int),
        ancestors_frame(old(bytes)@, final(bytes)@, *plan, plan.trail.depth as int),
    no_unwind
{
    let ghost before = bytes@;
    let mut i = 0usize;
    while i < plan.trail.depth
        invariant i <= plan.trail.depth < MAX_DEPTH, patch_geometry(original, *plan),
            bytes.len() == original.len(), before.len() == original.len(),
            forall|p: int| 0 <= p < target(*plan).start ==> before[p] == original[p],
            ancestors_delta(original, bytes@, *plan, i as int),
            ancestors_frame(before, bytes@, *plan, i as int),
        decreases plan.trail.depth - i,
    {
        let run = plan.trail.nodes[i].run;
        proof {
            assert forall|p: int| run_changed(run, run.count as int, p)
                implies bytes@[p] == original[p] by {
                let k = choose|k: int| 0 <= k < run.count && #[trigger] run_at(run, k) <= p < run_at(run, k) + 4;
                run_index_bounds(run, plan.trail.nodes[i as int + 1].region.start as int, k);
                if ancestor_changed(*plan, i as int, p) {
                    let j = choose|j: int| 0 <= j < i && #[trigger] run_changed(plan.trail.nodes[j].run, plan.trail.nodes[j].run.count as int, p);
                    ancestor_separation(original, *plan, j, i as int, p, p);
                }
            }
            assert forall|k: int| 0 <= k < run.count implies #[trigger] patch_safe_at(bytes@, run, plan.delta, k) by {
                run_index_bounds(run, plan.trail.nodes[i as int + 1].region.start as int, k);
                assert forall|p: int| run_at(run, k) <= p < run_at(run, k) + 4
                    implies bytes@[p] == original[p] by {
                    assert(run_changed(run, run.count as int, p));
                    if ancestor_changed(*plan, i as int, p) {
                        let j = choose|j: int| 0 <= j < i && #[trigger] run_changed(plan.trail.nodes[j].run, plan.trail.nodes[j].run.count as int, p);
                        ancestor_separation(original, *plan, j, i as int, p, p);
                    }
                }
                assert(words::le_word(bytes@, run_at(run, k)) == words::le_word(original, run_at(run, k)));
                assert(patch_safe_at(original, run, plan.delta, k));
            }
        }
        let ghost previous = bytes@;
        patch_run(bytes, run, plan.delta);
        proof {
            assert forall|j: int| 0 <= j < i + 1 implies #[trigger] patch_delta(original, bytes@,
                plan.trail.nodes[j].run, plan.delta, plan.trail.nodes[j].run.count as int) by {
                let other = plan.trail.nodes[j].run;
                assert forall|k: int| 0 <= k < other.count implies #[trigger] words::encode_exact(bytes@,
                    run_at(other, k), (words::le_word(original, run_at(other, k)) + plan.delta) as u32) by {
                    assert(patch_safe_at(original, other, plan.delta, k));
                    if j < i {
                        assert(patch_delta(original, previous, other, plan.delta, other.count as int));
                        assert(words::encode_exact(previous, run_at(other, k),
                            (words::le_word(original, run_at(other, k)) + plan.delta) as u32));
                        assert forall|p: int| run_at(other, k) <= p < run_at(other, k) + 4
                            implies !run_changed(run, run.count as int, p) by {
                            assert(run_changed(other, other.count as int, p));
                            if run_changed(run, run.count as int, p) { ancestor_separation(original, *plan, j, i as int, p, p); }
                        }
                        assert(bytes@[run_at(other, k)] == previous[run_at(other, k)]);
                        assert(bytes@[run_at(other, k) + 1] == previous[run_at(other, k) + 1]);
                        assert(bytes@[run_at(other, k) + 2] == previous[run_at(other, k) + 2]);
                        assert(bytes@[run_at(other, k) + 3] == previous[run_at(other, k) + 3]);
                    } else {
                        assert(j == i);
                        assert(patch_delta(previous, bytes@, run, plan.delta, run.count as int));
                        assert(words::le_word(previous, run_at(other, k)) == words::le_word(original, run_at(other, k)));
                        assert(words::encode_exact(bytes@, run_at(run, k),
                            (words::le_word(previous, run_at(run, k)) + plan.delta) as u32));
                    }
                }
            }
            assert forall|p: int| 0 <= p < before.len() && !ancestor_changed(*plan, i as int + 1, p)
                implies bytes@[p] == before[p] by {
                if run_changed(run, run.count as int, p) { assert(ancestor_changed(*plan, i as int + 1, p)); }
                if ancestor_changed(*plan, i as int, p) {
                    let j = choose|j: int| 0 <= j < i && #[trigger] run_changed(plan.trail.nodes[j].run, plan.trail.nodes[j].run.count as int, p);
                    assert(ancestor_changed(*plan, i as int + 1, p));
                }
            }
        }
        i += 1;
    }
}

proof fn ancestor_range(original: Seq<u8>, plan: Plan, p: int)
    requires patch_geometry(original, plan), ancestor_changed(plan, plan.trail.depth as int, p),
    ensures 12 <= p < target(plan).start,
{
    let j = choose|j: int| 0 <= j < plan.trail.depth && #[trigger] run_changed(plan.trail.nodes[j].run,
        plan.trail.nodes[j].run.count as int, p);
    let run = plan.trail.nodes[j].run;
    let k = choose|k: int| 0 <= k < run.count && #[trigger] run_at(run, k) <= p < run_at(run, k) + 4;
    run_index_bounds(run, plan.trail.nodes[j + 1].region.start as int, k);
}

spec fn count_changed(plan: Plan, at: int) -> bool {
    plan.list.is_some() && plan.list.unwrap().new_count != plan.list.unwrap().old_count
        && plan.trail.nodes[plan.trail.depth as int].region.start <= at
            < plan.trail.nodes[plan.trail.depth as int].region.start + 4
}

spec fn count_published(after: Seq<u8>, plan: Plan) -> bool {
    plan.list.is_some() ==> words::le_word(after,
        plan.trail.nodes[plan.trail.depth as int].region.start as int) == plan.list.unwrap().new_count
}

spec fn replacement_result(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    &&& after.len() == original.len() && encoded.len() == original.len()
    &&& plan.required <= original.len()
    &&& words::le_word(after, 8) == plan.new_used
    &&& count_published(after, plan)
    &&& ancestors_delta(original, after, plan, plan.trail.depth as int)
    &&& forall|p: int| #![trigger after[p]] 0 <= p < after.len() ==> {
        &&& (p < 8 || plan.required <= p) ==> after[p] == original[p]
        &&& (12 <= p < target(plan).start && (plan.delta == 0 || !ancestor_changed(plan, plan.trail.depth as int, p))
                && !count_changed(plan, p))
            ==> after[p] == original[p]
        &&& target(plan).start <= p < target(plan).start + plan.replacement_len ==> after[p] == encoded[p]
        &&& target(plan).start + plan.replacement_len <= p < plan.new_used ==> after[p] == original[p - plan.delta]
        &&& plan.new_used <= p < plan.old_used ==> after[p] == 0
    }
}

proof fn ancestors_unchanged(original: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires patch_geometry(original, plan), plan.delta == 0,
        after.len() == original.len(),
        forall|p: int| 0 <= p < target(plan).start ==> after[p] == original[p],
    ensures ancestors_delta(original, after, plan, plan.trail.depth as int),
{
    assert forall|j: int| 0 <= j < plan.trail.depth implies #[trigger] patch_delta(original, after,
        plan.trail.nodes[j].run, 0, plan.trail.nodes[j].run.count as int) by {
        let run = plan.trail.nodes[j].run;
        assert forall|k: int| 0 <= k < run.count implies #[trigger] words::encode_exact(after,
            run_at(run, k), words::le_word(original, run_at(run, k)) as u32) by {
            run_index_bounds(run, plan.trail.nodes[j + 1].region.start as int, k);
            words::word_roundtrip(original, run_at(run, k));
        }
    }
}

proof fn ancestors_reframe(original: Seq<u8>, before: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires patch_geometry(original, plan), ancestors_delta(original, before, plan, plan.trail.depth as int),
        forall|p: int| ancestor_changed(plan, plan.trail.depth as int, p) ==> after[p] == before[p],
    ensures ancestors_delta(original, after, plan, plan.trail.depth as int),
{
    assert forall|j: int| 0 <= j < plan.trail.depth implies #[trigger] patch_delta(original, after,
        plan.trail.nodes[j].run, plan.delta, plan.trail.nodes[j].run.count as int) by {
        let run = plan.trail.nodes[j].run;
        assert(patch_delta(original, before, run, plan.delta, run.count as int));
        assert forall|k: int| 0 <= k < run.count implies #[trigger] words::encode_exact(after,
            run_at(run, k), (words::le_word(original, run_at(run, k)) + plan.delta) as u32) by {
            assert(words::encode_exact(before, run_at(run, k),
                (words::le_word(original, run_at(run, k)) + plan.delta) as u32));
            assert forall|p: int| run_at(run, k) <= p < run_at(run, k) + 4
                implies after[p] == before[p] by {
                assert(run_changed(run, run.count as int, p));
                assert(ancestor_changed(plan, plan.trail.depth as int, p));
                ancestor_range(original, plan, p);
            }
        }
    }
}

/// Clears one short block through safe constant-index stores.
#[inline(always)]
fn zero_8(bytes: &mut [u8])
    requires old(bytes).len() == 8,
    ensures final(bytes).len() == 8, forall|p: int| 0 <= p < 8 ==> final(bytes)@[p] == 0,
    no_unwind
{
    bytes[0] = 0;
    bytes[1] = 0;
    bytes[2] = 0;
    bytes[3] = 0;
    bytes[4] = 0;
    bytes[5] = 0;
    bytes[6] = 0;
    bytes[7] = 0;
    assert forall|p: int| 0 <= p < 8 implies bytes@[p] == 0 by { }
}

/// Clears one fixed block through safe constant-index stores.
#[inline(always)]
fn zero_32(bytes: &mut [u8])
    requires old(bytes).len() == 32,
    ensures final(bytes).len() == 32, forall|p: int| 0 <= p < 32 ==> final(bytes)@[p] == 0,
    no_unwind
{
    bytes[0] = 0;
    bytes[1] = 0;
    bytes[2] = 0;
    bytes[3] = 0;
    bytes[4] = 0;
    bytes[5] = 0;
    bytes[6] = 0;
    bytes[7] = 0;
    bytes[8] = 0;
    bytes[9] = 0;
    bytes[10] = 0;
    bytes[11] = 0;
    bytes[12] = 0;
    bytes[13] = 0;
    bytes[14] = 0;
    bytes[15] = 0;
    bytes[16] = 0;
    bytes[17] = 0;
    bytes[18] = 0;
    bytes[19] = 0;
    bytes[20] = 0;
    bytes[21] = 0;
    bytes[22] = 0;
    bytes[23] = 0;
    bytes[24] = 0;
    bytes[25] = 0;
    bytes[26] = 0;
    bytes[27] = 0;
    bytes[28] = 0;
    bytes[29] = 0;
    bytes[30] = 0;
    bytes[31] = 0;
    assert forall|p: int| 0 <= p < 32 implies bytes@[p] == 0 by { }
}

/// Clears one larger block through safe constant-index stores.
#[inline(always)]
fn zero_128(bytes: &mut [u8])
    requires old(bytes).len() == 128,
    ensures final(bytes).len() == 128, forall|p: int| 0 <= p < 128 ==> final(bytes)@[p] == 0,
    no_unwind
{
    let destination = borrow_destination(bytes, 0, 32);
    zero_32(destination);
    assert forall|p: int| 0 <= p < 32 implies bytes@[p] == 0 by {
        assert(bytes@.subrange(0, 32)[p - 0] == 0);
    }
    let destination = borrow_destination(bytes, 32, 32);
    zero_32(destination);
    assert forall|p: int| 32 <= p < 64 implies bytes@[p] == 0 by {
        assert(bytes@.subrange(32, 64)[p - 32] == 0);
    }
    let destination = borrow_destination(bytes, 64, 32);
    zero_32(destination);
    assert forall|p: int| 64 <= p < 96 implies bytes@[p] == 0 by {
        assert(bytes@.subrange(64, 96)[p - 64] == 0);
    }
    let destination = borrow_destination(bytes, 96, 32);
    zero_32(destination);
    assert forall|p: int| 96 <= p < 128 implies bytes@[p] == 0 by {
        assert(bytes@.subrange(96, 128)[p - 96] == 0);
    }
    assert forall|p: int| 0 <= p < 128 implies bytes@[p] == 0 by {
        if p < 32 { } else if p < 64 { } else if p < 96 { }
    }
}

#[inline(always)]
fn clear_tail(bytes: &mut [u8], start: u32, end: u32)
    requires start <= old(bytes).len(), end <= old(bytes).len(),
    ensures final(bytes).len() == old(bytes).len(),
        forall|p: int| 0 <= p < old(bytes).len() ==> final(bytes)@[p]
            == if start <= p < end { 0u8 } else { old(bytes)@[p] },
    no_unwind
{
    if start >= end { return; }
    let ghost before = bytes@;
    let mut at = start as usize;
    let end = end as usize;
    while at < end && end - at >= 128
        invariant start <= at, at <= end || at == start,
            start <= bytes.len(), end <= bytes.len(), bytes.len() == before.len(),
            forall|p: int| 0 <= p < bytes.len() ==> bytes@[p] == if start <= p < at { 0u8 } else { before[p] },
        decreases if at < end { end - at } else { 0int },
    {
        let next = at + 128;
        let (_, suffix) = bytes.split_at_mut(at);
        let (destination, _) = suffix.split_at_mut(128);
        zero_128(destination);
        assert forall|p: int| at <= p < next implies bytes@[p] == 0 by {
            assert(bytes@.subrange(at as int, next as int)[p - at] == 0);
        }
        at = next;
    }
    while at < end && end - at >= 32
        invariant start <= at, at <= end || at == start,
            start <= bytes.len(), end <= bytes.len(), bytes.len() == before.len(),
            forall|p: int| 0 <= p < bytes.len() ==> bytes@[p] == if start <= p < at { 0u8 } else { before[p] },
        decreases if at < end { end - at } else { 0int },
    {
        let next = at + 32;
        let (_, suffix) = bytes.split_at_mut(at);
        let (destination, _) = suffix.split_at_mut(32);
        zero_32(destination);
        assert forall|p: int| at <= p < next implies bytes@[p] == 0 by {
            assert(bytes@.subrange(at as int, next as int)[p - at] == 0);
        }
        at = next;
    }
    while at < end && end - at >= 8
        invariant start <= at, at <= end || at == start,
            start <= bytes.len(), end <= bytes.len(), bytes.len() == before.len(),
            forall|p: int| 0 <= p < bytes.len() ==> bytes@[p] == if start <= p < at { 0u8 } else { before[p] },
        decreases if at < end { end - at } else { 0int },
    {
        let next = at + 8;
        let (_, suffix) = bytes.split_at_mut(at);
        let (destination, _) = suffix.split_at_mut(8);
        zero_8(destination);
        assert forall|p: int| at <= p < next implies bytes@[p] == 0 by {
            assert(bytes@.subrange(at as int, next as int)[p - at] == 0);
        }
        at = next;
    }
    while at < end
        invariant start <= at, at <= end || at == start,
            start <= bytes.len(), end <= bytes.len(), bytes.len() == before.len(),
            forall|p: int| 0 <= p < bytes.len() ==> bytes@[p] == if start <= p < at { 0u8 } else { before[p] },
        decreases if at < end { end - at } else { 0int },
    {
        bytes[at] = 0;
        at += 1;
    }
}

#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn finish_replacement(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>)
    requires replacement_checked(original, *plan), payload_frame(original, old(bytes)@, *plan),
        plan.required <= old(bytes).len(),
    ensures replacement_result(original, old(bytes)@, final(bytes)@, *plan),
    no_unwind
{
    let ghost encoded = bytes@;
    proof { replacement_geometry(original, *plan); }
    if plan.delta != 0 {
        patch_ancestors(bytes, plan, Ghost(original));
    } else {
        proof { ancestors_unchanged(original, bytes@, *plan); }
    }
    let ghost patched = bytes@;
    if let Some(change) = plan.list {
        if change.new_count != change.old_count {
            let at = plan.trail.nodes[plan.trail.depth].region.start;
            match crate::write_u32(bytes, at, change.new_count) { Ok(()) => (), Err(_) => panic!("checked list count") };
        }
    }
    let ghost counted = bytes@;
    if plan.delta < 0 {
        clear_tail(bytes, plan.new_used, plan.old_used);
    }
    if plan.delta != 0 {
        match crate::write_u32(bytes, 8, plan.new_used) { Ok(()) => (), Err(_) => panic!("checked used length") };
    }
    proof {
        assert forall|p: int| ancestor_changed(*plan, plan.trail.depth as int, p)
            implies bytes@[p] == patched[p] by {
            ancestor_range(original, *plan, p);
            let j = choose|j: int| 0 <= j < plan.trail.depth && #[trigger] run_changed(plan.trail.nodes[j].run,
                plan.trail.nodes[j].run.count as int, p);
            let k = choose|k: int| 0 <= k < plan.trail.nodes[j].run.count
                && #[trigger] run_at(plan.trail.nodes[j].run, k) <= p < run_at(plan.trail.nodes[j].run, k) + 4;
            run_index_bounds(plan.trail.nodes[j].run, plan.trail.nodes[j + 1].region.start as int, k);
            trail_order(original, *plan, j + 1, plan.trail.depth as int);
        }
        ancestors_reframe(original, patched, bytes@, *plan);
    }
    let ghost published = bytes@;
    assert(words::le_word(published, 8) == plan.new_used);
    proof {
        assert forall|p: int| ancestor_changed(*plan, plan.trail.depth as int, p)
            implies bytes@[p] == published[p] by { ancestor_range(original, *plan, p); }
        ancestors_reframe(original, published, bytes@, *plan);
        assert(count_published(published, *plan));
        assert(count_published(bytes@, *plan));
        assert(words::le_word(bytes@, 8) == plan.new_used);
        assert forall|p: int| #![trigger bytes@[p]] 0 <= p < bytes.len() implies {
            &&& (p < 8 || plan.required <= p) ==> bytes@[p] == original[p]
            &&& (12 <= p < target(*plan).start && (plan.delta == 0 || !ancestor_changed(*plan, plan.trail.depth as int, p))
                    && !count_changed(*plan, p))
                ==> bytes@[p] == original[p]
            &&& target(*plan).start <= p < target(*plan).start + plan.replacement_len ==> bytes@[p] == encoded[p]
            &&& target(*plan).start + plan.replacement_len <= p < plan.new_used ==> bytes@[p] == original[p - plan.delta]
            &&& plan.new_used <= p < plan.old_used ==> bytes@[p] == 0
        } by {
            if ancestor_changed(*plan, plan.trail.depth as int, p) { ancestor_range(original, *plan, p); }
        }
        assert(replacement_result(original, encoded, bytes@, *plan));
    }
}

/// Independent sequence interpretation of fixed-width element bytes.
spec fn fixed_items(bytes: Seq<u8>, payload: int, count: int, width: int) -> Seq<Seq<u8>> {
    Seq::new(count as nat, |i: int| bytes.subrange(payload + i * width, payload + (i + 1) * width))
}

/// Ordinary sequence operations, without wire offsets or production selectors.
spec fn edited_items(items: Seq<Seq<u8>>, input: Seq<u8>, operation: ListEdit) -> Seq<Seq<u8>> {
    match operation {
        ListEdit::Insert(i) => items.subrange(0, i as int) + seq![input] + items.subrange(i as int, items.len() as int),
        ListEdit::Remove(i) => items.subrange(0, i as int) + items.subrange(i + 1, items.len() as int),
        ListEdit::Replace(i) => items.update(i as int, input),
        ListEdit::Pop => if items.len() == 0 { items } else { items.subrange(0, items.len() as int - 1) },
        ListEdit::Truncate(n) => items.subrange(0, if n < items.len() { n as int } else { items.len() as int }),
        ListEdit::Clear => seq![],
    }
}

spec fn list_model_refines(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    plan.list.is_some() ==> {
        let change = plan.list.unwrap();
        let base = plan.trail.nodes[plan.trail.depth as int].region.start as int;
        let items = fixed_items(original, base + 4, change.old_count as int, change.width as int);
        let input = encoded.subrange(target(plan).start as int, target(plan).start + plan.replacement_len);
        &&& count_published(after, plan)
        &&& fixed_items(after, base + 4, change.new_count as int, change.width as int)
            == edited_items(items, input, change.operation)
    }
}

spec fn list_payload(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    plan.list.is_some() ==> {
        let node = plan.trail.nodes[plan.trail.depth as int];
        forall|p: int| node.region.start + 4 <= p < node.region.end + plan.delta ==> #[trigger] after[p]
            == if p < target(plan).start { original[p] }
                else if p < target(plan).start + plan.replacement_len { encoded[p] }
                else { original[p - plan.delta] }
    }
}

#[verifier::spinoff_prover]
proof fn list_payload_refinement(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), replacement_result(original, encoded, after, plan),
    ensures list_payload(original, encoded, after, plan),
{
    replacement_geometry(original, plan);
    assert(list_checked(original, plan));
    if plan.list.is_some() {
        assert(plan.trail.nodes[plan.trail.depth as int].region.end + plan.delta <= plan.new_used);
        let base = plan.trail.nodes[plan.trail.depth as int].region.start as int;
        assert forall|p: int| base + 4 <= p < plan.trail.nodes[plan.trail.depth as int].region.end + plan.delta
            implies #[trigger] after[p] == if p < target(plan).start { original[p] }
                else if p < target(plan).start + plan.replacement_len { encoded[p] }
                else { original[p - plan.delta] } by {
            if ancestor_changed(plan, plan.trail.depth as int, p) {
                let j = choose|j: int| 0 <= j < plan.trail.depth && #[trigger] run_changed(plan.trail.nodes[j].run,
                    plan.trail.nodes[j].run.count as int, p);
                let k = choose|k: int| 0 <= k < plan.trail.nodes[j].run.count
                    && #[trigger] run_at(plan.trail.nodes[j].run, k) <= p < run_at(plan.trail.nodes[j].run, k) + 4;
                run_index_bounds(plan.trail.nodes[j].run, plan.trail.nodes[j + 1].region.start as int, k);
                trail_order(original, plan, j + 1, plan.trail.depth as int);
            }
        }
    }
}

/// Sequence refinement needs only a byte splice, ordinal arithmetic and extents.
/// It has no dependency on the descriptor parser, planner or apply implementation.
#[verifier::spinoff_prover]
proof fn sequence_refinement(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    payload: int, change: ListChange, start: int, input_len: int, delta: int)
    requires
        0 <= payload, original.len() == encoded.len(), original.len() == after.len(),
        payload + change.old_count * change.width <= original.len(),
        payload + change.new_count * change.width <= after.len(),
        0 <= change.first <= change.last <= change.old_count,
        (change.first as int, change.last as int, change.new_count as int)
            == list_indices(change.operation, change.old_count),
        start == payload + change.first * change.width,
        input_len == list_input(change.operation, change.width),
        delta == input_len - (change.last - change.first) * change.width,
        change.new_count == change.old_count - (change.last - change.first)
            + if matches!(change.operation, ListEdit::Insert(_) | ListEdit::Replace(_)) { 1int } else { 0int },
        forall|p: int| payload <= p < payload + change.new_count * change.width ==> #[trigger] after[p]
            == if p < start { original[p] } else if p < start + input_len { encoded[p] } else { original[p - delta] },
    ensures fixed_items(after, payload, change.new_count as int, change.width as int)
        == edited_items(fixed_items(original, payload, change.old_count as int, change.width as int),
            encoded.subrange(start, start + input_len), change.operation),
{
    let width = change.width as int;
    let input = encoded.subrange(start, start + input_len);
    let old_items = fixed_items(original, payload, change.old_count as int, width);
    let expected = edited_items(old_items, input, change.operation);
    let actual = fixed_items(after, payload, change.new_count as int, width);
    let inserted = if matches!(change.operation, ListEdit::Insert(_) | ListEdit::Replace(_)) { 1int } else { 0int };
    assert(delta == (inserted - (change.last - change.first)) * width) by(nonlinear_arith)
        requires delta == input_len - (change.last - change.first) * width,
            input_len == inserted * width;
    assert(expected.len() == change.new_count);
    assert forall|i: int| 0 <= i < change.new_count implies actual[i] == expected[i] by {
        let source = if i < change.first { i } else { i + (change.last - change.first) - inserted };
        assert(expected[i] == if inserted == 1 && i == change.first { input } else { old_items[source] });
        assert(0 <= source < change.old_count || (inserted == 1 && i == change.first));
        assert(payload <= payload + i * width <= payload + (i + 1) * width
            <= payload + change.new_count * width) by(nonlinear_arith)
            requires 0 <= i < change.new_count, width >= 0;
        if !(inserted == 1 && i == change.first) {
            assert(payload <= payload + source * width <= payload + (source + 1) * width
                <= payload + change.old_count * width) by(nonlinear_arith)
                requires 0 <= source < change.old_count, width >= 0;
        } else {
            assert(start + input_len <= after.len()) by(nonlinear_arith)
                requires start == payload + change.first * width, input_len == width,
                    change.first < change.new_count, payload + change.new_count * width <= after.len(), width >= 0;
        }
        assert((i + 1) * width - i * width == width) by(nonlinear_arith);
        assert(actual[i] == after.subrange(payload + i * width, payload + (i + 1) * width));
        if !(inserted == 1 && i == change.first) {
            assert((source + 1) * width - source * width == width) by(nonlinear_arith);
            assert(old_items[source] == original.subrange(payload + source * width, payload + (source + 1) * width));
        }
        assert(actual[i].len() == width);
        assert(expected[i].len() == width);
        assert forall|k: int| 0 <= k < width implies actual[i][k] == expected[i][k] by {
            let at = payload + i * width + k;
            assert(payload <= at < payload + change.new_count * width) by(nonlinear_arith)
                requires at == payload + i * width + k, 0 <= i < change.new_count, 0 <= k < width;
            if i < change.first {
                assert(at < start) by(nonlinear_arith)
                    requires at == payload + i * width + k, 0 <= k < width,
                        i < change.first, start == payload + change.first * width;
            } else if inserted == 1 && i == change.first {
                assert(start <= at < start + input_len);
            } else {
                assert(start + input_len <= at) by(nonlinear_arith)
                    requires at == payload + i * width + k, 0 <= k < width,
                        start == payload + change.first * width, input_len == inserted * width,
                        i >= change.first + inserted;
                assert(at - delta == payload + source * width + k) by(nonlinear_arith)
                    requires at == payload + i * width + k,
                        source == i + (change.last - change.first) - inserted,
                        delta == (inserted - (change.last - change.first)) * width;
            }
        }
        assert(actual[i] =~= expected[i]);
    }
    assert(actual =~= expected);
}

#[verifier::spinoff_prover]
proof fn list_refinement(original: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), replacement_result(original, encoded, after, plan),
    ensures list_model_refines(original, encoded, after, plan),
{
    assert(list_checked(original, plan));
    list_payload_refinement(original, encoded, after, plan);
    if let Some(change) = plan.list {
        let payload = plan.trail.nodes[plan.trail.depth as int].region.start + 4;
        assert(plan.delta == plan.replacement_len as int - (change.last - change.first) * change.width) by(nonlinear_arith)
            requires plan.delta == plan.replacement_len as int - (target(plan).end - target(plan).start),
                target(plan).end == payload + change.last * change.width,
                target(plan).start == payload + change.first * change.width;
        sequence_refinement(original, encoded, after, payload, change, target(plan).start as int,
            plan.replacement_len as int, plan.delta as int);
    }
}

/// Insert agrees exactly with sequence insertion, including the append ordinal.
pub closed spec fn list_insert_refines(apply: Apply, after: Seq<u8>) -> bool {
    &&& (apply.plan.list.is_some() && matches!(apply.plan.list.unwrap().operation, ListEdit::Insert(_))
        ==> list_model_refines(apply.original@, apply.bytes@, after, apply.plan))
    &&& indexed_insert_refines(apply, after)
}

/// Remove/pop agree with sequence deletion; empty pop retains the empty sequence.
pub closed spec fn list_remove_refines(apply: Apply, after: Seq<u8>) -> bool {
    &&& indexed_splice_refines(apply, after)
    &&& (apply.plan.list.is_some() && matches!(apply.plan.list.unwrap().operation, ListEdit::Remove(_) | ListEdit::Pop)
        ==> list_model_refines(apply.original@, apply.bytes@, after, apply.plan)
    )
}

/// Replacement agrees with a sequence update and preserves cardinality.
pub closed spec fn list_replace_refines(apply: Apply, after: Seq<u8>) -> bool {
    &&& indexed_splice_refines(apply, after)
    &&& (apply.plan.list.is_some() && matches!(apply.plan.list.unwrap().operation, ListEdit::Replace(_))
        ==> list_model_refines(apply.original@, apply.bytes@, after, apply.plan)
    )
}

/// Truncation agrees with taking the requested bounded prefix.
pub closed spec fn list_truncate_refines(apply: Apply, after: Seq<u8>) -> bool {
    &&& indexed_splice_refines(apply, after)
    &&& (apply.plan.list.is_some() && matches!(apply.plan.list.unwrap().operation, ListEdit::Truncate(_))
        ==> list_model_refines(apply.original@, apply.bytes@, after, apply.plan)
    )
}

/// Clear publishes a zero count and the empty element sequence.
pub closed spec fn list_clear_refines(apply: Apply, after: Seq<u8>) -> bool {
    &&& indexed_splice_refines(apply, after)
    &&& (apply.plan.list.is_some() && matches!(apply.plan.list.unwrap().operation, ListEdit::Clear)
        ==> list_model_refines(apply.original@, apply.bytes@, after, apply.plan)
    )
}

/// Indexed edits agree with ordinary sequence operations and exact byte frames.
pub closed spec fn indexed_splice_refines(apply: Apply, after: Seq<u8>) -> bool {
    apply.plan.splice.is_some() ==> indexed_remove::splice_result(apply.original@, apply.bytes@, after, apply.plan)
        && indexed_remove::sequence_splice(apply.original@, apply.bytes@, after, apply.plan)
}

/// Insertion/upsert refines finite-map insertion with unique sorted keys.
pub closed spec fn map_insert_refines(apply: Apply, after: Seq<u8>) -> bool {
    (apply.plan.map.is_some() && matches!(apply.plan.map.unwrap().operation, MapEdit::Insert)
        ==> fixed_map::map_result(apply.original@, apply.bytes@, after, apply.plan))
    && (apply.plan.variable_map.is_some() && matches!(apply.plan.variable_map.unwrap().change.operation, MapEdit::Insert)
        ==> indexed_map::map_result(apply.original@, apply.bytes@, after, apply.plan))
}

/// Replacement refines finite-map update and preserves every key.
pub closed spec fn map_replace_refines(apply: Apply, after: Seq<u8>) -> bool {
    (apply.plan.map.is_some() && matches!(apply.plan.map.unwrap().operation, MapEdit::Replace)
        ==> fixed_map::map_result(apply.original@, apply.bytes@, after, apply.plan))
    && (apply.plan.variable_map.is_some() && matches!(apply.plan.variable_map.unwrap().change.operation, MapEdit::Replace)
        ==> indexed_map::map_result(apply.original@, apply.bytes@, after, apply.plan))
}

/// Removal refines finite-map deletion, including absent-key no-ops.
pub closed spec fn map_remove_refines(apply: Apply, after: Seq<u8>) -> bool {
    (apply.plan.map.is_some() && matches!(apply.plan.map.unwrap().operation, MapEdit::Remove)
        ==> fixed_map::map_result(apply.original@, apply.bytes@, after, apply.plan))
    && (apply.plan.variable_map.is_some() && matches!(apply.plan.variable_map.unwrap().change.operation, MapEdit::Remove)
        ==> indexed_map::map_result(apply.original@, apply.bytes@, after, apply.plan))
}

/// Clear refines the empty finite map in one checked splice.
pub closed spec fn map_clear_refines(apply: Apply, after: Seq<u8>) -> bool {
    (apply.plan.map.is_some() && matches!(apply.plan.map.unwrap().operation, MapEdit::Clear)
        ==> fixed_map::map_result(apply.original@, apply.bytes@, after, apply.plan))
    && (apply.plan.variable_map.is_some() && matches!(apply.plan.variable_map.unwrap().change.operation, MapEdit::Clear)
        ==> indexed_map::map_result(apply.original@, apply.bytes@, after, apply.plan))
}

/// Mutable storage retained by the apply, for its final-state contract.
pub closed spec fn apply_storage<'a>(apply: Apply<'a, '_>) -> &'a mut [u8] { apply.bytes }

/// The apply is ready for movement (0), destination (1), or finish (2).
pub closed spec fn apply_phase(apply: Apply, phase: u8) -> bool { apply.phase == phase }

/// Exact replacement bytes, original suffix, boundary deltas and cleared tail.
pub closed spec fn replace_refines(apply: Apply, after: Seq<u8>) -> bool {
    if apply.plan.variable_map.is_some() { indexed_map::edit_result(apply.original@, apply.bytes@, after, apply.plan) }
    else if apply.plan.splice.is_some() { indexed_remove::splice_result(apply.original@, apply.bytes@, after, apply.plan) }
    else if apply.plan.indexed.is_some() { indexed_insert::insert_result(apply.original@, apply.bytes@, after, apply.plan) }
    else { replacement_result(apply.original@, apply.bytes@, after, apply.plan) }
}

/// Exclusive, ordered replacement application over the current storage.
///
/// Call `move_regions`, fill `destination` exactly once, then consume `finish`.
/// Dropping or forgetting an unfinished apply can leave an incomplete encoding;
/// the storage owner must retain its poison flag until `finish` returns.
#[must_use = "complete the replacement before clearing storage poison"]
pub struct Apply<'a, 'key> {
    bytes: &'a mut [u8],
    plan: Plan<'key>,
    // 0: ready; 1: moved; 2: destination issued; 3: internal field transition.
    // A transition cannot escape a normally returning method.
    phase: u8,
    original: Ghost<Seq<u8>>,
}

spec fn target(plan: Plan) -> Span { plan.region }

/// Exact unchanged prefix, relocated original suffix, and untouched spare.
spec fn payload_frame(before: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    &&& before.len() == after.len()
    &&& forall|i: int| 0 <= i < before.len() ==> {
        &&& (i < target(plan).start || plan.required <= i) ==> after[i] == before[i]
        &&& target(plan).start + plan.replacement_len <= i < plan.new_used
            ==> after[i] == before[i - plan.delta]
    }
}

/// Ordered apply transition, preserving the captured plan and byte frame.
pub closed spec fn replace_progress(before: Apply, after: Apply, phase: u8) -> bool {
    after.phase == phase && after.plan == before.plan && after.original == before.original
        && before.phase + 1 == phase
        && payload_frame(before.original@, after.bytes@, before.plan)
}

/// One exact original-source suffix copy, or no byte changes when no move is needed.
pub closed spec fn replace_movement(before: Apply, after: Apply) -> bool {
    planned_movement(before.original@, before.bytes@, after.bytes@, before.plan)
}

spec fn planned_movement(original: Seq<u8>, before: Seq<u8>, after: Seq<u8>, plan: Plan) -> bool {
    if plan.variable_map.is_some() { indexed_map::movement(original, after, plan) }
    else if plan.splice.is_some() { indexed_remove::movement(original, after, plan) }
    else if plan.indexed.is_some() { indexed_insert::movement(original, after, plan) } else {
    let src = Span { start: target(plan).end, end: plan.old_used };
    let dst = (target(plan).end + plan.delta) as u32;
    if plan.delta != 0 && src.start < src.end {
        if plan.map.is_none() {
            crate::moves::move_old_source(before, after, src, dst)
                && crate::moves::move_frame(before, after, src, dst)
        } else { fixed_map::key_written(after, plan)
            && forall|p: int| 0 <= p < before.len() && !fixed_map::key_byte(plan, p) ==>
                after[p] == if dst <= p < dst + src.end - src.start { before[src.start + p - dst] } else { before[p] } }
    } else if plan.map.is_none() { after == before } else {
        after.len() == before.len() && fixed_map::key_written(after, plan)
            && forall|p: int| #![trigger after[p]] 0 <= p < before.len() && !fixed_map::key_byte(plan, p)
                ==> after[p] == before[p]
    }
    }
}

/// The encoder receives exactly the measured replacement length.
pub closed spec fn destination_exact(apply: Apply, len: usize) -> bool {
    len == input_size(apply.plan)
}

/// The returned borrow reads and changes precisely the selected replacement.
pub closed spec fn destination_region(before: Apply, after: Apply, initial: Seq<u8>, encoded: Seq<u8>) -> bool {
    let start = destination_start(before.plan) as int;
    let end = start + input_size(before.plan);
    initial == before.bytes@.subrange(start, end)
        && after.bytes@.subrange(start, end) == encoded
}

#[inline(always)]
fn borrow_destination(bytes: &mut [u8], start: u32, len: u32) -> (result: &mut [u8])
    requires start as int + len <= old(bytes).len(),
    ensures result@ == old(bytes)@.subrange(start as int, start as int + len),
        final(bytes).len() == old(bytes).len(), result.len() == len,
        forall|lo: int, hi: int| 0 <= lo <= hi <= start ==>
            #[trigger] final(bytes)@.subrange(lo, hi) == old(bytes)@.subrange(lo, hi),
        final(bytes)@.subrange(start as int, start as int + len) == final(result)@,
        forall|p: int| 0 <= p < old(bytes).len() && !(start <= p < start as int + len)
            ==> final(bytes)@[p] == old(bytes)@[p],
    no_unwind
{
    let (prefix, suffix) = bytes.split_at_mut(start as usize);
    let (destination, tail) = suffix.split_at_mut(len as usize);
    proof { vstd::slice::mut_ref_slice_len_eq(&destination); }
    let _ = prefix;
    let _ = tail;
    destination
}

/// A successful readiness check starts before any destructive write.
pub closed spec fn apply_started(result: Result<Apply>, requested: Plan, before: Seq<u8>) -> bool {
    result.is_ok() ==> {
        let apply = result.unwrap();
        &&& apply.phase == 0 && apply.original@ == before
        &&& geometry_equal(requested, apply.plan)
        &&& planned_for(apply.plan, requested.old_used, requested.trail.root,
            requested.path, input_size(requested), requested.storage_limit)
    }
}

/// Every preflight rejection is InvalidPlan and preserves the entire buffer.
pub closed spec fn replace_failure(result: Result<Apply>, before: Seq<u8>, after: Seq<u8>) -> bool {
    result.is_err() ==> result.err().unwrap() == (Error::Fault { kind: ErrorKind::InvalidPlan, offset: u32::MAX })
        && after == before
}

// A direct field reborrow avoids moving the wide reference through mem::swap.
// Verus permits a mutable field loan from an invariant type at a call boundary.
#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn move_planned_regions(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>)
    requires replacement_checked(original, *plan), old(bytes)@ == original,
        plan.required <= old(bytes).len(),
    ensures payload_frame(original, final(bytes)@, *plan),
        planned_movement(original, original, final(bytes)@, *plan),
    no_unwind
{
    let region = plan.region;
    if plan.variable_map.is_some() {
        indexed_map::move_map(bytes, plan);
    } else if plan.splice.is_some() {
        indexed_remove::move_splice(bytes, plan);
    } else if plan.indexed.is_some() {
        indexed_insert::move_insert(bytes, plan);
    } else if plan.delta != 0 && region.end < plan.old_used {
        let dst = (region.end as i64 + plan.delta) as u32;
        match crate::move_bytes(bytes, Span { start: region.end, end: plan.old_used }, dst) { Ok(value) => value, Err(_) => panic!("checked edit operation") };
    }
    proof { fixed_map::occupied_key(original, *plan); }
    let ghost moved = bytes@;
    fixed_map::write_key(bytes, plan, Ghost(original));
    proof {
        if plan.map.is_none() { assert(bytes@ =~= moved); }
    }
}

impl<'a, 'key> Apply<'a, 'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        &&& replacement_checked(self.original@, self.plan)
        &&& self.phase <= 3
        &&& self.phase != 3 ==> {
            &&& self.plan.required <= self.bytes.len()
            &&& self.bytes.len() == self.original@.len()
            &&& if self.phase == 0 { self.bytes@ == self.original@ }
                else { payload_frame(self.original@, self.bytes@, self.plan)
                    && (self.plan.indexed.is_some() ==> indexed_insert::movement(self.original@, self.bytes@, self.plan))
                    && (self.plan.splice.is_some() ==> indexed_remove::movement(self.original@, self.bytes@, self.plan))
                    && (self.plan.variable_map.is_some() ==> indexed_map::movement(self.original@, self.bytes@, self.plan))
                    && fixed_map::key_written(self.bytes@, self.plan) }
        }
    }

    /// Moves the root suffix at most once, then an indexed prefix and directory.
    ///
    /// Empty or unchanged regions perform no copy. Panics before writing
    /// if movement has already run or the destination has already been issued.
    #[verifier::spinoff_prover]
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
    pub fn move_regions(&mut self)
        requires apply_phase(*old(self), 0),
        ensures replace_progress(*old(self), *final(self), 1), replace_movement(*old(self), *final(self)),
            apply_phase(*final(self), 1),
        no_unwind
    {
        proof { use_type_invariant(&*self); }
        if self.phase != 0 { vstd::pervasive::__call_panic(&["replacement movement must run exactly once"]); }
        self.phase = 3;
        move_planned_regions(self.bytes, &self.plan, self.original);
        self.phase = 1;
    }

    /// Returns the exact replacement region, once, after movement.
    ///
    /// Fill the entire returned slice before calling `finish`. Content validity
    /// belongs to the prevalidated encoder; this gateway checks only geometry.
    /// Panics before writing for duplicate or out-of-order use.
    #[verifier::spinoff_prover]
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
    pub fn destination(&mut self) -> (result: &mut [u8])
        requires apply_phase(*old(self), 1),
        ensures replace_progress(*old(self), *final(self), 2), apply_phase(*final(self), 2),
            destination_exact(*old(self), result.len()),
            destination_region(*old(self), *final(self), result@, final(result)@),
        no_unwind
    {
        proof { use_type_invariant(&*self); }
        if self.phase != 1 { vstd::pervasive::__call_panic(&["replacement destination requires completed movement"]); }
        self.phase = 3;
        let (start, len) = if let Some(change) = self.plan.variable_map {
            (indexed_map::destination_exec(&self.plan), change.input_len)
        } else if self.plan.map.is_some() {
            (fixed_map::value_start_exec(&self.plan), fixed_map::input_size_exec(&self.plan))
        } else if let Some(change) = self.plan.splice {
            (change.payload - change.table_shrink + change.offset, change.input_len)
        } else { match self.plan.indexed {
            Some(change) => (change.payload + change.table_growth + change.offset, change.input_len),
            None => (self.plan.region.start, self.plan.replacement_len),
        } };
        proof {
            if self.plan.variable_map.is_some() {
                indexed_map::movement_framed(self.original@, self.bytes@, self.plan);
            }
            if self.plan.indexed.is_some() {
                indexed_insert::movement_framed(self.original@, self.bytes@, self.plan);
            }
            if self.plan.splice.is_some() {
                indexed_remove::movement_framed(self.original@, self.bytes@, self.plan);
            }
        }
        let destination = borrow_destination(self.bytes, start, len);
        self.phase = 2;
        destination
    }

    /// Publishes local directory/count changes, ancestor ends, and used length.
    ///
    /// Consumes the apply. Equal-length replacements leave metadata untouched.
    /// Panics before writing if the destination has not been issued. The caller
    /// may clear storage poison only after this method returns normally.
    #[verifier::spinoff_prover]
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
    pub fn finish(self)
        requires apply_phase(self, 2),
        ensures replace_refines(self, final(apply_storage(self))@),
            list_insert_refines(self, final(apply_storage(self))@),
            list_remove_refines(self, final(apply_storage(self))@),
            list_replace_refines(self, final(apply_storage(self))@),
            list_truncate_refines(self, final(apply_storage(self))@),
            list_clear_refines(self, final(apply_storage(self))@),
            indexed_insert_refines(self, final(apply_storage(self))@),
            indexed_splice_refines(self, final(apply_storage(self))@),
            map_insert_refines(self, final(apply_storage(self))@),
            map_replace_refines(self, final(apply_storage(self))@),
            map_remove_refines(self, final(apply_storage(self))@),
            map_clear_refines(self, final(apply_storage(self))@),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        if self.phase != 2 { vstd::pervasive::__call_panic(&["replacement finish requires an encoded destination"]); }
        let bytes = self.bytes;
        let plan = self.plan;
        let original = self.original;
        let ghost encoded = bytes@;
        if plan.variable_map.is_some() {
            indexed_map::finish_map(bytes, &plan, original);
        } else if plan.splice.is_some() {
            indexed_remove::finish_splice(bytes, &plan, original);
            proof { indexed_remove::sequence_refinement(original@, encoded, bytes@, plan); }
        } else if plan.indexed.is_some() {
            indexed_insert::finish_insert(bytes, &plan, original);
            proof { indexed_insert::sequence_refinement(original@, encoded, bytes@, plan); }
        } else {
            finish_replacement(bytes, &plan, original);
            proof { list_refinement(original@, encoded, bytes@, plan);
                fixed_map::refinement(original@, encoded, bytes@, plan); }
        }
    }
}

/// Checks readiness and acquires exclusive storage without changing any byte.
///
/// A stale plan or insufficient storage returns `InvalidPlan`. The caller must
/// validate replacement content before entering the destructive phase.
pub fn begin_apply<'a, 'key>(bytes: &'a mut [u8], plan: Plan<'key>) -> (result: Result<Apply<'a, 'key>>)
    ensures replace_failure(result, old(bytes)@, final(bytes)@),
        apply_started(result, plan, old(bytes)@),
        result.is_ok() ==> apply_phase(result.unwrap(), 0),
{
    let fresh = plan.ready_plan(bytes)?;
    let original = Ghost(bytes@);
    Ok(Apply { bytes, plan: fresh, phase: 0, original })
}

/// Valid replacement bytes preserve the complete root through the actual splice.
#[verifier::spinoff_prover]
proof fn subtree_replace_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), subtree_plan(plan),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
            plan.trail.selected, (MAX_DEPTH - plan.trail.depth) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    let n = plan.trail.nodes[plan.trail.depth as int];
    assert(plan.region == n.region);
    assert(nested::node_after(after, plan, plan.trail.depth as int) =~=
        encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len));
    nested::lift_preserves(before, encoded, after, plan, MAX_DEPTH as nat, true, 0);
}

/// The verified bulk layout is the actual replacement selected by the outer plan.
spec fn bulk_result(before: Seq<u8>, initial: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, c: BulkListPlan, ends: Seq<int>) -> bool {
    &&& subtree_plan(plan)
    &&& plan.replacement_len == c.len && plan.region.end - plan.region.start == c.old_len
    &&& initial.subrange(0, c.old_len as int) == before.subrange(plan.region.start as int, plan.region.end as int)
    &&& bulk_list::layout_result(initial, encoded.subrange(plan.region.start as int,
        plan.region.start + plan.replacement_len), c, ends)
    &&& replacement_result(before, encoded, after, plan)
    &&& matches!(plan.trail.selected.head(), LayoutHead::List { .. })
    &&& c.indexed == crate::list_layout::element_width(plan.trail.selected).is_none()
    &&& !c.indexed ==> crate::list_layout::element_width(plan.trail.selected) == Some(c.width)
}

/// Bulk insertion derives whole-list validity from old children and new slots.
#[verifier::spinoff_prover]
proof fn bulk_list_insert_preserves(before: Seq<u8>, initial: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, c: BulkListPlan, ends: Seq<int>)
    requires replacement_checked(before, plan), plan.required <= before.len(),
        bulk_result(before, initial, encoded, after, plan, c, ends),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        bulk_list::inputs_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
            c, ends, plan.trail.selected, (MAX_DEPTH - plan.trail.depth) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    replacement_extent(before, plan);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    bulk_list::selected_preserves(initial, encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
        c, ends, plan.trail.selected, (MAX_DEPTH - plan.trail.depth) as nat, true);
    assert(nested::node_after(after, plan, plan.trail.depth as int) =~=
        encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len));
    nested::lift_preserves(before, encoded, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Bulk extension is insertion at the checked original terminal boundary.
#[verifier::spinoff_prover]
proof fn bulk_list_extend_preserves(before: Seq<u8>, initial: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, c: BulkListPlan, ends: Seq<int>)
    requires replacement_checked(before, plan), plan.required <= before.len(), c.index == c.count,
        bulk_result(before, initial, encoded, after, plan, c, ends),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        bulk_list::inputs_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
            c, ends, plan.trail.selected, (MAX_DEPTH - plan.trail.depth) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    hide(replacement_checked); hide(bulk_result);
    bulk_list_insert_preserves(before, initial, encoded, after, plan, c, ends);
}

/// Indexed list insert preserves the complete root at every checked depth.
#[verifier::spinoff_prover]
proof fn indexed_list_insert_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.indexed.is_some(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        crate::list_layout::element_width(plan.trail.selected).is_none(),
        indexed_insert::insert_result(before, encoded, after, plan), plan.required <= before.len(),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(destination_start(plan) as int,
            destination_start(plan) + plan.indexed.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    indexed_insert::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    indexed_insert::replacement_frame(before, encoded, after, plan);
    nested::lift_preserves(before, after, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Indexed list insert preserves the complete root at every checked depth.
#[verifier::spinoff_prover]
proof fn indexed_list_push_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.indexed.is_some(), plan.indexed.unwrap().index == plan.indexed.unwrap().count,
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        crate::list_layout::element_width(plan.trail.selected).is_none(),
        indexed_insert::insert_result(before, encoded, after, plan), plan.required <= before.len(),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(destination_start(plan) as int,
            destination_start(plan) + plan.indexed.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    indexed_insert::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    indexed_insert::replacement_frame(before, encoded, after, plan);
    nested::lift_preserves(before, after, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Indexed list remove preserves the complete root at every checked depth.
#[verifier::spinoff_prover]
proof fn indexed_list_remove_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.splice.is_some(), matches!(plan.splice.unwrap().operation, ListEdit::Remove(_) | ListEdit::Pop),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        crate::list_layout::element_width(plan.trail.selected).is_none(),
        indexed_remove::splice_result(before, encoded, after, plan), plan.required <= before.len(),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.splice.unwrap().operation, ListEdit::Replace(_)) ==> validation::schema_valid(encoded.subrange(destination_start(plan) as int,
            destination_start(plan) + plan.splice.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    indexed_remove::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    indexed_remove::replacement_frame(before, encoded, after, plan);
    nested::lift_preserves(before, after, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Indexed list replace preserves the complete root at every checked depth.
#[verifier::spinoff_prover]
proof fn indexed_list_replace_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.splice.is_some(), matches!(plan.splice.unwrap().operation, ListEdit::Replace(_)),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        crate::list_layout::element_width(plan.trail.selected).is_none(),
        indexed_remove::splice_result(before, encoded, after, plan), plan.required <= before.len(),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.splice.unwrap().operation, ListEdit::Replace(_)) ==> validation::schema_valid(encoded.subrange(destination_start(plan) as int,
            destination_start(plan) + plan.splice.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    indexed_remove::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    indexed_remove::replacement_frame(before, encoded, after, plan);
    nested::lift_preserves(before, after, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Indexed list truncate preserves the complete root at every checked depth.
#[verifier::spinoff_prover]
proof fn indexed_list_truncate_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.splice.is_some(), matches!(plan.splice.unwrap().operation, ListEdit::Truncate(_)),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        crate::list_layout::element_width(plan.trail.selected).is_none(),
        indexed_remove::splice_result(before, encoded, after, plan), plan.required <= before.len(),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.splice.unwrap().operation, ListEdit::Replace(_)) ==> validation::schema_valid(encoded.subrange(destination_start(plan) as int,
            destination_start(plan) + plan.splice.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    indexed_remove::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    indexed_remove::replacement_frame(before, encoded, after, plan);
    nested::lift_preserves(before, after, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Indexed list clear preserves the complete root at every checked depth.
#[verifier::spinoff_prover]
proof fn indexed_list_clear_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.splice.is_some(), matches!(plan.splice.unwrap().operation, ListEdit::Clear),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        crate::list_layout::element_width(plan.trail.selected).is_none(),
        indexed_remove::splice_result(before, encoded, after, plan), plan.required <= before.len(),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.splice.unwrap().operation, ListEdit::Replace(_)) ==> validation::schema_valid(encoded.subrange(destination_start(plan) as int,
            destination_start(plan) + plan.splice.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    indexed_remove::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    indexed_remove::replacement_frame(before, encoded, after, plan);
    nested::lift_preserves(before, after, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Indexed map insert preserves sorted keys, recursive values and the root.
#[verifier::spinoff_prover]
proof fn indexed_map_insert_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.variable_map.is_some(),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Insert),
        indexed_map::edit_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        indexed_map::writes(plan.variable_map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(indexed_map::destination(plan) as int,
                indexed_map::destination(plan) + plan.variable_map.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    indexed_map::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    indexed_map::replacement_frame(before, encoded, after, plan);
    nested::lift_preserves(before, after, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Indexed map replace preserves sorted keys, recursive values and the root.
#[verifier::spinoff_prover]
proof fn indexed_map_replace_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.variable_map.is_some(),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Replace),
        indexed_map::edit_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        indexed_map::writes(plan.variable_map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(indexed_map::destination(plan) as int,
                indexed_map::destination(plan) + plan.variable_map.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    indexed_map::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    indexed_map::replacement_frame(before, encoded, after, plan);
    nested::lift_preserves(before, after, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Indexed map remove preserves sorted keys, recursive values and the root.
#[verifier::spinoff_prover]
proof fn indexed_map_remove_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.variable_map.is_some(),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Remove),
        indexed_map::edit_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        indexed_map::writes(plan.variable_map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(indexed_map::destination(plan) as int,
                indexed_map::destination(plan) + plan.variable_map.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    indexed_map::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    indexed_map::replacement_frame(before, encoded, after, plan);
    nested::lift_preserves(before, after, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Indexed map clear preserves sorted keys, recursive values and the root.
#[verifier::spinoff_prover]
proof fn indexed_map_clear_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.variable_map.is_some(),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Clear),
        indexed_map::edit_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        indexed_map::writes(plan.variable_map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(indexed_map::destination(plan) as int,
                indexed_map::destination(plan) + plan.variable_map.unwrap().input_len),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    indexed_map::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    indexed_map::replacement_frame(before, encoded, after, plan);
    nested::lift_preserves(before, after, after, plan, MAX_DEPTH as nat, true, 0);
}


/// The specialized fixed-map insertion composes its exact generic refinement.
#[verifier::spinoff_prover]
proof fn fixed_insert_preserves(loan: FixedInsert, after: Seq<u8>, plan: Plan)
    requires fixed_insert::insertion_plan(loan, plan),
        replacement_checked(fixed_insert::insertion_original(loan), plan),
        fixed_insert::fixed_insert_finished(loan, after), fixed_insert::insertion_input_valid(loan, plan),
        validation::schema_valid(fixed_insert::insertion_original(loan).subrange(12, plan.old_used as int),
            plan.trail.root, MAX_DEPTH as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    fixed_insert::insertion_refines_plan(loan, after, plan);
    fixed_map_insert_preserves(fixed_insert::insertion_original(loan), after, after, plan);
}

/// A fixed-width overwrite composes the generic nested replacement theorem.
#[verifier::spinoff_prover]
proof fn fixed_write_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), subtree_plan(plan), width_spec(plan.trail.selected).is_some(),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
            plan.trail.selected, (MAX_DEPTH - plan.trail.depth) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    subtree_replace_preserves(before, encoded, after, plan);
}

/// Valid UTF-8 string bytes compose the generic nested replacement theorem.
#[verifier::spinoff_prover]
proof fn string_set_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), subtree_plan(plan), matches!(plan.trail.selected.head(), LayoutHead::Str),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
            plan.trail.selected, (MAX_DEPTH - plan.trail.depth) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    subtree_replace_preserves(before, encoded, after, plan);
}

/// The compact subtree loan's exact result inherits generic root preservation.
#[verifier::spinoff_prover]
proof fn subtree_write_preserves(write: SubtreeWrite, after: Seq<u8>)
    requires subtree_write::write_checked(write), subtree_write::write_phase(write, 1),
        subtree_write::write_finished(write, after),
        validation::schema_valid(subtree_write::write_original(write).subrange(12,
            subtree_write::write_plan(write).old_used as int),
            subtree_write::write_plan(write).trail.root, MAX_DEPTH as nat, true),
        validation::schema_valid(subtree_write::write_storage(write)@.subrange(
            subtree_write::write_plan(write).region.start as int,
            subtree_write::write_plan(write).region.start + subtree_write::write_plan(write).replacement_len), subtree_write::write_plan(write).trail.selected,
            (MAX_DEPTH - subtree_write::write_plan(write).trail.depth) as nat, true),
    ensures validation::schema_valid(after.subrange(12, subtree_write::write_plan(write).new_used as int),
        subtree_write::write_plan(write).trail.root, MAX_DEPTH as nat, true),
{
    subtree_write::checked_plan(write, after);
    subtree_replace_preserves(subtree_write::write_original(write), subtree_write::write_storage(write)@,
        after, subtree_write::write_plan(write));
}

/// The production splice frames every ancestor and preserves sibling encodings.
#[verifier::spinoff_prover]
proof fn nested_splice_frame(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), subtree_plan(plan),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
            plan.trail.selected, (MAX_DEPTH - plan.trail.depth) as nat, true),
    ensures crate::preservation::framed_path(before.subrange(12, plan.old_used as int),
        after.subrange(12, plan.new_used as int), plan.trail.root, nested::ordinals(plan, 0), true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    let n = plan.trail.nodes[plan.trail.depth as int];
    assert(plan.region == n.region);
    assert(nested::node_after(after, plan, plan.trail.depth as int) =~=
        encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len));
    nested::lift_preserves(before, encoded, after, plan, MAX_DEPTH as nat, true, 0);
}

/// The public inventory query composes the complete production splice frame.
#[verifier::spinoff_prover]
proof fn nested_frame(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), subtree_plan(plan),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
            plan.trail.selected, (MAX_DEPTH - plan.trail.depth) as nat, true),
    ensures crate::preservation::framed_path(before.subrange(12, plan.old_used as int),
        after.subrange(12, plan.new_used as int), plan.trail.root, nested::ordinals(plan, 0), true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    nested_splice_frame(before, encoded, after, plan);
}

/// Production splices equal the independent recursive ancestor reconstruction.
#[verifier::spinoff_prover]
proof fn nested_refines(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan,
    budget: nat, semantic: bool)
    requires replacement_checked(before, plan), subtree_plan(plan),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int),
            plan.trail.root, budget, semantic),
    ensures after.subrange(12, plan.new_used as int) == nested::nested_apply(
        before.subrange(12, plan.old_used as int), plan.trail.root, nested::ordinals(plan, 0),
        encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len)),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    nested::production_nested(before, encoded, after, plan, budget, semantic, 0);
    let last = plan.trail.nodes[plan.trail.depth as int];
    assert(plan.region == last.region);
    assert(nested::node_after(after, plan, plan.trail.depth as int) =~=
        encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len));
}

/// Removing an absent fixed-map key has the identity byte result.
#[verifier::spinoff_prover]
proof fn missing_fixed_remove_identity(before: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.map.is_some(),
        matches!(plan.map.unwrap().operation, MapEdit::Remove), !plan.map.unwrap().found,
    ensures replacement_result(before, before, before, plan),
{
    replacement_geometry(before, plan);
    replacement_extent(before, plan);
    assert(plan.list.is_some());
    let change = plan.list.unwrap();
    assert(change.operation == ListEdit::Truncate(change.old_count));
    assert(change.first == change.last && change.new_count == change.old_count);
    assert(target(plan).start == target(plan).end);
    assert(plan.delta == 0 && plan.new_used == plan.old_used);
    ancestors_unchanged(before, before, plan);
}

/// Replacing the entire root publishes the validated input's exact encoding.
#[verifier::spinoff_prover]
proof fn root_subtree_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, budget: nat, semantic: bool)
    requires replacement_checked(before, plan), subtree_plan(plan), plan.trail.depth == 0,
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int),
            plan.trail.root, budget, semantic),
        validation::schema_valid(encoded.subrange(12, plan.new_used as int),
            plan.trail.root, budget, semantic),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int),
        plan.trail.root, budget, semantic),
{
    reveal(subtree_plan);
    assert(plan.region == (Span { start: 12, end: plan.old_used }));
    assert(after.subrange(12, plan.new_used as int) =~=
        encoded.subrange(12, plan.new_used as int));
}

/// A missing fixed-map removal preserves the complete root at every depth.
#[verifier::spinoff_prover]
proof fn missing_fixed_remove_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, budget: nat, semantic: bool)
    requires replacement_checked(before, plan), plan.map.is_some(),
        matches!(plan.map.unwrap().operation, MapEdit::Remove), !plan.map.unwrap().found,
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int),
            plan.trail.root, budget, semantic),
    ensures after == before,
        validation::schema_valid(after.subrange(12, plan.new_used as int),
            plan.trail.root, budget, semantic),
{
    missing_fixed_remove_identity(before, plan);
    replacement_extent(before, plan);
    assert(plan.list.is_some());
    assert(plan.list.unwrap().new_count == plan.list.unwrap().old_count);
    assert(plan.region.start == plan.region.end && plan.delta == 0);
    words::word_roundtrip(before, 8);
    words::word_roundtrip(after, 8);
    assert forall|p: int| 0 <= p < before.len() implies #[trigger] after[p] == before[p] by {
        if 8 <= p < 12 { assert(p == 8 || p == 9 || p == 10 || p == 11); }
    }
    assert(after =~= before);
}

/// Sequence insertion/removal retains each old element's recursive validity.
#[verifier::spinoff_prover]
proof fn edited_items_valid(items: Seq<Seq<u8>>, input: Seq<u8>, operation: ListEdit,
    layout: Layout, budget: nat, semantic: bool)
    requires items.len() <= u32::MAX,
        0 <= list_indices(operation, items.len() as u32).0
            <= list_indices(operation, items.len() as u32).1 <= items.len(),
        forall|i: int| 0 <= i < items.len()
            ==> #[trigger] validation::schema_valid(items[i], layout, budget, semantic),
        matches!(operation, ListEdit::Insert(_) | ListEdit::Replace(_))
            ==> validation::schema_valid(input, layout, budget, semantic),
    ensures forall|i: int| 0 <= i < edited_items(items, input, operation).len()
        ==> #[trigger] validation::schema_valid(edited_items(items, input, operation)[i], layout, budget, semantic),
{
    let edited = edited_items(items, input, operation);
    assert forall|i: int| 0 <= i < edited.len() implies
        #[trigger] validation::schema_valid(edited[i], layout, budget, semantic) by {
        match operation {
            ListEdit::Insert(index) => {
                if i < index { assert(edited[i] == items[i]); }
                else if i == index { assert(edited[i] == input); }
                else { assert(edited[i] == items[i - 1]); }
            },
            ListEdit::Remove(index) => {
                if i < index { assert(edited[i] == items[i]); }
                else { assert(edited[i] == items[i + 1]); }
            },
            ListEdit::Replace(index) => {
                if i == index { assert(edited[i] == input); }
                else { assert(edited[i] == items[i]); }
            },
            ListEdit::Pop | ListEdit::Truncate(_) => { assert(edited[i] == items[i]); },
            ListEdit::Clear => { assert(false); },
        }
    }
}

/// Fixed-list reconstruction validates the complete edited list at any depth.
#[verifier::spinoff_prover]
proof fn fixed_list_selected_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, budget: nat, semantic: bool)
    requires replacement_checked(before, plan), plan.list.is_some(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(node_bytes(before, plan.trail.nodes[plan.trail.depth as int]),
            plan.trail.selected, budget, semantic),
        matches!(plan.list.unwrap().operation, ListEdit::Insert(_) | ListEdit::Replace(_))
            ==> validation::schema_valid(encoded.subrange(plan.region.start as int,
                plan.region.start + plan.replacement_len),
                validation::child_layout(plan.trail.selected, 0), (budget - 1) as nat, semantic),
    ensures validation::schema_valid(after.subrange(plan.trail.nodes[plan.trail.depth as int].region.start as int,
            plan.trail.nodes[plan.trail.depth as int].region.end + plan.delta),
        plan.trail.selected, budget, semantic),
{
    replacement_extent(before, plan);
    list_refinement(before, encoded, after, plan);
    let n = plan.trail.nodes[plan.trail.depth as int];
    let base = n.region.start as int;
    let end = n.region.end as int;
    let changed_end = n.region.end + plan.delta;
    let change = plan.list.unwrap();
    let layout = plan.trail.selected;
    let element = validation::child_layout(layout, 0);
    let old = node_bytes(before, plan.trail.nodes[plan.trail.depth as int]);
    let new = after.subrange(plan.trail.nodes[plan.trail.depth as int].region.start as int,
            plan.trail.nodes[plan.trail.depth as int].region.end + plan.delta);
    let items = fixed_items(before, base + 4, change.old_count as int, change.width as int);
    let actual = fixed_items(after, base + 4, change.new_count as int, change.width as int);
    let input = encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len);
    assert(changed_end == base + 4 + change.new_count * change.width);
    words::word_subrange(before, base, end, 0);
    words::word_subrange(after, base, changed_end, 0);
    crate::preservation::children_all(old, layout, 0, (budget - 1) as nat, semantic);
    assert forall|i: int| 0 <= i < items.len() implies
        #[trigger] validation::schema_valid(items[i], element, (budget - 1) as nat, semantic) by {
        assert(0 <= i < change.old_count <= u32::MAX);
        assert(base + 4 <= base + 4 + i * change.width <= base + 4 + (i + 1) * change.width <= end)
            by(nonlinear_arith)
            requires 0 <= i < change.old_count, 0 <= change.width,
                end == base + 4 + change.old_count * change.width;
        let span = validation::child_span(old, layout, i);
        assert(span.start == 4 + i * change.width);
        assert(span.end == 4 + (i + 1) * change.width);
        assert(items[i] =~= old.subrange(span.start as int, span.end as int));
        assert(validation::child_valid(old, layout, i, (budget - 1) as nat, semantic));
    }
    edited_items_valid(items, input, change.operation, element, (budget - 1) as nat, semantic);
    assert(validation::local_valid(new, layout, semantic));
    assert(actual == edited_items(items, input, change.operation));
    assert forall|i: int| 0 <= i < validation::child_count(new, layout) implies
        #[trigger] validation::child_valid(new, layout, i, (budget - 1) as nat, semantic) by {
        assert(0 <= i < change.new_count <= u32::MAX);
        assert(base + 4 <= base + 4 + i * change.width <= base + 4 + (i + 1) * change.width <= changed_end)
            by(nonlinear_arith)
            requires 0 <= i < change.new_count, 0 <= change.width,
                changed_end == base + 4 + change.new_count * change.width;
        let span = validation::child_span(new, layout, i);
        assert(actual[i] =~= new.subrange(span.start as int, span.end as int));
        assert(validation::schema_valid(actual[i], element, (budget - 1) as nat, semantic));
    }
    crate::preservation::children_all(new, layout, 0, (budget - 1) as nat, semantic);
}


/// A valid fixed-list edit lifts from its checked local bytes to the root.
#[verifier::spinoff_prover]
proof fn fixed_list_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.list.is_some(), plan.map.is_none(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.list.unwrap().operation, ListEdit::Insert(_) | ListEdit::Replace(_))
            ==> validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
                validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    fixed_list_selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    nested::lift_preserves(before, encoded, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Fixed-list insert preserves the complete root at any checked depth.
#[verifier::spinoff_prover]
proof fn fixed_list_insert_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.list.is_some(), plan.map.is_none(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        matches!(plan.list.unwrap().operation, ListEdit::Insert(_)), replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.list.unwrap().operation, ListEdit::Insert(_) | ListEdit::Replace(_))
            ==> validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
                validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    fixed_list_preserves(before, encoded, after, plan);
}

/// Fixed-list remove preserves the complete root at any checked depth.
#[verifier::spinoff_prover]
proof fn fixed_list_remove_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.list.is_some(), plan.map.is_none(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        matches!(plan.list.unwrap().operation, ListEdit::Remove(_) | ListEdit::Pop), replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.list.unwrap().operation, ListEdit::Insert(_) | ListEdit::Replace(_))
            ==> validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
                validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    fixed_list_preserves(before, encoded, after, plan);
}

/// Fixed-list push preserves the complete root at any checked depth.
#[verifier::spinoff_prover]
proof fn fixed_list_push_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.list.is_some(), plan.map.is_none(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        plan.list.unwrap().operation == ListEdit::Insert(plan.list.unwrap().old_count), replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.list.unwrap().operation, ListEdit::Insert(_) | ListEdit::Replace(_))
            ==> validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
                validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    fixed_list_preserves(before, encoded, after, plan);
}

/// Fixed-list replace preserves the complete root at any checked depth.
#[verifier::spinoff_prover]
proof fn fixed_list_replace_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.list.is_some(), plan.map.is_none(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        matches!(plan.list.unwrap().operation, ListEdit::Replace(_)), replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.list.unwrap().operation, ListEdit::Insert(_) | ListEdit::Replace(_))
            ==> validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
                validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    fixed_list_preserves(before, encoded, after, plan);
}

/// Fixed-list truncate preserves the complete root at any checked depth.
#[verifier::spinoff_prover]
proof fn fixed_list_truncate_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.list.is_some(), plan.map.is_none(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        matches!(plan.list.unwrap().operation, ListEdit::Truncate(_)), replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.list.unwrap().operation, ListEdit::Insert(_) | ListEdit::Replace(_))
            ==> validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
                validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    fixed_list_preserves(before, encoded, after, plan);
}

/// Fixed-list clear preserves the complete root at any checked depth.
#[verifier::spinoff_prover]
proof fn fixed_list_clear_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.list.is_some(), plan.map.is_none(),
        matches!(plan.trail.selected.head(), LayoutHead::List { .. }),
        matches!(plan.list.unwrap().operation, ListEdit::Clear), replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        matches!(plan.list.unwrap().operation, ListEdit::Insert(_) | ListEdit::Replace(_))
            ==> validation::schema_valid(encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len),
                validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    fixed_list_preserves(before, encoded, after, plan);
}

/// Valid sorted fixed-map edits lift through the actual ancestor splice.
#[verifier::spinoff_prover]
proof fn fixed_map_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.map.is_some(),
        replacement_result(before, encoded, after, plan), fixed_map::key_written(encoded, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        fixed_map::writes(plan.map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(fixed_map::value_start(plan) as int, fixed_map::value_start(plan) + fixed_map::input_len(plan)),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    nested::trail_schema(before, plan, MAX_DEPTH as nat, true, plan.trail.depth as int);
    fixed_map::selected_preserves(before, encoded, after, plan, (MAX_DEPTH - plan.trail.depth) as nat, true);
    nested::lift_preserves(before, encoded, after, plan, MAX_DEPTH as nat, true, 0);
}

/// Fixed-map insert preserves the complete root at any checked depth.
#[verifier::spinoff_prover]
proof fn fixed_map_insert_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.map.is_some(),
        matches!(plan.map.unwrap().operation, MapEdit::Insert),
        replacement_result(before, encoded, after, plan), fixed_map::key_written(encoded, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        fixed_map::writes(plan.map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(fixed_map::value_start(plan) as int, fixed_map::value_start(plan) + fixed_map::input_len(plan)),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    fixed_map_preserves(before, encoded, after, plan);
}

/// Fixed-map replace preserves the complete root at any checked depth.
#[verifier::spinoff_prover]
proof fn fixed_map_replace_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.map.is_some(),
        matches!(plan.map.unwrap().operation, MapEdit::Replace),
        replacement_result(before, encoded, after, plan), fixed_map::key_written(encoded, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        fixed_map::writes(plan.map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(fixed_map::value_start(plan) as int, fixed_map::value_start(plan) + fixed_map::input_len(plan)),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    fixed_map_preserves(before, encoded, after, plan);
}

/// Fixed-map remove preserves the complete root at any checked depth.
#[verifier::spinoff_prover]
proof fn fixed_map_remove_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.map.is_some(),
        matches!(plan.map.unwrap().operation, MapEdit::Remove),
        replacement_result(before, encoded, after, plan), fixed_map::key_written(encoded, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        fixed_map::writes(plan.map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(fixed_map::value_start(plan) as int, fixed_map::value_start(plan) + fixed_map::input_len(plan)),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    fixed_map_preserves(before, encoded, after, plan);
}

/// Fixed-map clear preserves the complete root at any checked depth.
#[verifier::spinoff_prover]
proof fn fixed_map_clear_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), plan.map.is_some(),
        matches!(plan.map.unwrap().operation, MapEdit::Clear),
        replacement_result(before, encoded, after, plan), fixed_map::key_written(encoded, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int), plan.trail.root, MAX_DEPTH as nat, true),
        fixed_map::writes(plan.map.unwrap()) ==> validation::schema_valid(
            encoded.subrange(fixed_map::value_start(plan) as int, fixed_map::value_start(plan) + fixed_map::input_len(plan)),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int), plan.trail.root, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid);
    hide(validation::local_valid);
    fixed_map_preserves(before, encoded, after, plan);
}

/// All fixed-list operations preserve a root list's complete recursive schema.
///
/// This theorem covers root selection only; composing an enclosing path needs
/// the ancestor partition and relocated-sibling lemmas separately.
#[verifier::spinoff_prover]
proof fn fixed_list_root_preserves(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    plan: Plan, budget: nat, semantic: bool)
    requires replacement_checked(before, plan), plan.list.is_some(), plan.trail.depth == 0,
        matches!(plan.trail.root.head(), LayoutHead::List { .. }),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(before.subrange(12, plan.old_used as int),
            plan.trail.root, budget, semantic),
        matches!(plan.list.unwrap().operation, ListEdit::Insert(_) | ListEdit::Replace(_))
            ==> validation::schema_valid(encoded.subrange(plan.region.start as int,
                plan.region.start + plan.replacement_len),
                validation::child_layout(plan.trail.root, 0), (budget - 1) as nat, semantic),
    ensures validation::schema_valid(after.subrange(12, plan.new_used as int),
        plan.trail.root, budget, semantic),
{
    replacement_extent(before, plan);
    list_refinement(before, encoded, after, plan);
    let change = plan.list.unwrap();
    let layout = plan.trail.root;
    let element = validation::child_layout(layout, 0);
    let old = before.subrange(12, plan.old_used as int);
    let new = after.subrange(12, plan.new_used as int);
    let items = fixed_items(before, 16, change.old_count as int, change.width as int);
    let actual = fixed_items(after, 16, change.new_count as int, change.width as int);
    let input = encoded.subrange(plan.region.start as int, plan.region.start + plan.replacement_len);
    assert(plan.trail.nodes[0].region == (Span { start: 12, end: plan.old_used }));
    assert(plan.new_used == 16 + change.new_count * change.width);
    words::word_subrange(before, 12, plan.old_used as int, 0);
    words::word_subrange(after, 12, plan.new_used as int, 0);
    crate::preservation::children_all(old, layout, 0, (budget - 1) as nat, semantic);
    assert forall|i: int| 0 <= i < items.len() implies
        #[trigger] validation::schema_valid(items[i], element, (budget - 1) as nat, semantic) by {
        assert(0 <= i < change.old_count <= u32::MAX);
        assert(16 <= 16 + i * change.width <= 16 + (i + 1) * change.width <= plan.old_used)
            by(nonlinear_arith)
            requires 0 <= i < change.old_count, 0 <= change.width,
                plan.old_used == 16 + change.old_count * change.width;
        let span = validation::child_span(old, layout, i);
        assert(span.start == 4 + i * change.width);
        assert(span.end == 4 + (i + 1) * change.width);
        assert(items[i] =~= old.subrange(span.start as int, span.end as int));
        assert(validation::child_valid(old, layout, i, (budget - 1) as nat, semantic));
    }
    edited_items_valid(items, input, change.operation, element, (budget - 1) as nat, semantic);
    assert(validation::local_valid(new, layout, semantic));
    assert(actual == edited_items(items, input, change.operation));
    assert forall|i: int| 0 <= i < validation::child_count(new, layout) implies
        #[trigger] validation::child_valid(new, layout, i, (budget - 1) as nat, semantic) by {
        assert(0 <= i < change.new_count <= u32::MAX);
        assert(16 <= 16 + i * change.width <= 16 + (i + 1) * change.width <= plan.new_used)
            by(nonlinear_arith)
            requires 0 <= i < change.new_count, 0 <= change.width,
                plan.new_used == 16 + change.new_count * change.width;
        let span = validation::child_span(new, layout, i);
        assert(actual[i] =~= new.subrange(span.start as int, span.end as int));
        assert(validation::schema_valid(actual[i], element, (budget - 1) as nat, semantic));
    }
    crate::preservation::children_all(new, layout, 0, (budget - 1) as nat, semantic);
}

/// A framed fixed-prefix overwrite has exactly the generic zero-delta result.
#[verifier::spinoff_prover]
proof fn fixed_overwrite_refines(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(before, plan), subtree_plan(plan), plan.delta == 0,
        crate::fixed_edit::overwrite_frame(before, after, plan.region),
        after.subrange(plan.region.start as int, plan.region.end as int)
            == encoded.subrange(plan.region.start as int, plan.region.end as int),
        encoded.len() == before.len(),
    ensures replacement_result(before, encoded, after, plan),
{
    replacement_geometry(before, plan);
    replacement_extent(before, plan);
    reveal(subtree_plan);
    ancestors_unchanged(before, after, plan);
    assert(words::le_word(after, 8) == plan.new_used);
    assert forall|i: int| plan.region.start <= i < plan.region.end implies #[trigger] after[i] == encoded[i] by {
        assert(after.subrange(plan.region.start as int, plan.region.end as int)[i - plan.region.start] == after[i]);
    }
}

/// Compact string publication refines any corresponding checked generic set.
#[verifier::spinoff_prover]
proof fn string_matches_generic(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan,
    start: u32, end: u32, used: u32, new_used: u32)
    requires replacement_checked(before, plan), subtree_plan(plan),
        string_result(before, encoded, after, start, end, used, new_used),
        plan.region.start == start, plan.region.end <= used, plan.old_used == used,
        plan.replacement_len == end - start, plan.new_used == new_used,
        start <= end, 12 <= used, new_used <= before.len(),
        plan.delta == 0 || (plan.region.end == used && forall|i: int| 0 <= i < plan.trail.depth
            ==> #[trigger] plan.trail.nodes[i].run.count == 0),
    ensures replacement_result(before, encoded, after, plan),
{
    replacement_geometry(before, plan);
    replacement_extent(before, plan);
    reveal(subtree_plan);
    assert forall|i: int| start <= i < end implies #[trigger] after[i] == encoded[i] by {
        assert(after.subrange(start as int, end as int)[i - start] == after[i]);
    }
    if plan.delta == 0 { ancestors_unchanged(before, after, plan); }
    else { assert(ancestors_delta(before, after, plan, plan.trail.depth as int)); }
}

/// Add the external key to the generic encoder snapshot; value bytes agree.
spec fn tail_encoded(encoded: Seq<u8>, used: u32, start: u32, key: Seq<u8>) -> Seq<u8> {
    encoded.subrange(0, used as int) + key + encoded.subrange(start as int, encoded.len() as int)
}

/// Exact key/value bytes of the encoder snapshot, independent of plan metadata.
#[verifier::spinoff_prover]
proof fn tail_encoding_matches(encoded: Seq<u8>, after: Seq<u8>, used: u32,
    start: u32, end: u32, key: Seq<u8>)
    requires encoded.len() == after.len(), used <= start <= end <= encoded.len(),
        start == used + key.len(), after.subrange(used as int, start as int) == key,
        after.subrange(start as int, end as int) == encoded.subrange(start as int, end as int),
    ensures tail_encoded(encoded, used, start, key).len() == encoded.len(),
        tail_encoded(encoded, used, start, key).subrange(used as int, start as int) == key,
        forall|p: int| used <= p < end ==> #[trigger] after[p] == tail_encoded(encoded, used, start, key)[p],
{
    let full = tail_encoded(encoded, used, start, key);
    assert(full.len() == encoded.len());
    assert(full.subrange(used as int, start as int) =~= key);
    assert forall|p: int| used <= p < end implies #[trigger] after[p] == full[p] by {
        if p < start {
            assert(after.subrange(used as int, start as int)[p - used] == after[p]);
            assert(full[p] == key[p - used]);
        } else {
            assert(after.subrange(start as int, end as int)[p - start] == after[p]);
            assert(encoded.subrange(start as int, end as int)[p - start] == encoded[p]);
            assert(full[p] == encoded[p]);
        }
    }
}

/// Compact tail publication has the generic insertion's exact byte result.
/// Existing list/map refinement theorems then give conditional model preservation.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn tail_matches_generic(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, plan: Plan,
    count_at: u32, count: u32, used: u32, start: u32, end: u32, key: Seq<u8>)
    requires replacement_checked(before, plan),
        tail_result(before, encoded, after, count_at, count, used, start, end, key),
        12 <= count_at, count_at + 4 <= used <= start <= end <= before.len(),
        start == used + key.len(), count < u32::MAX,
        plan.region == (Span { start: used, end: used }), plan.old_used == used,
        plan.new_used == end, plan.replacement_len == end - used,
        plan.indexed.is_none(), plan.splice.is_none(), plan.variable_map.is_none(),
        plan.list.is_some(), plan.list.unwrap().new_count == count + 1,
        plan.map.is_some() ==> key == plan.key@ && plan.map.unwrap().key_width == key.len(),
        plan.trail.nodes[plan.trail.depth as int].region.start == count_at,
        forall|i: int| 0 <= i < plan.trail.depth ==> #[trigger] plan.trail.nodes[i].run.count == 0,
    ensures replacement_result(before, tail_encoded(encoded, used, start, key), after, plan),
        fixed_map::key_written(tail_encoded(encoded, used, start, key), plan),
{
    hide(tail_encoded);
    hide(crate::search::lex_less);
    hide(crate::search::search_partition);
    hide(crate::search::search_found);
    hide(crate::search::keys_ordered);
    hide(ancestor_changed);
    hide(run_changed);
    hide(replacement_checked);
    hide(list_checked);
    hide(fixed_map::map_checked);
    assert(target(plan) == plan.region);
    assert(plan.trail.depth < MAX_DEPTH && plan.delta == end - used && plan.required == end
        && plan.list.unwrap().old_count == words::le_word(before, count_at as int)) by {
        reveal(replacement_checked);
        reveal(list_checked);
    }
    tail_encoding_matches(encoded, after, used, start, end, key);
    let full = tail_encoded(encoded, used, start, key);
    assert(full.len() == before.len());
    assert(fixed_map::key_written(full, plan));
    assert(words::le_word(after, count_at as int) == (count + 1) as u32);
    assert(count_published(after, plan));
    assert(words::le_word(after, 8) == plan.new_used);
    assert forall|j: int| 0 <= j < plan.trail.depth implies
        #[trigger] patch_delta(before, after, plan.trail.nodes[j].run, plan.delta,
            plan.trail.nodes[j].run.count as int) by {
        assert(plan.trail.nodes[j].run.count == 0);
    }
    assert(ancestors_delta(before, after, plan, plan.trail.depth as int));
    if plan.list.unwrap().old_count == plan.list.unwrap().new_count {
        words::word_roundtrip(before, count_at as int);
        assert(words::encode_exact(before, count_at as int, (count + 1) as u32));
        assert forall|p: int| count_at <= p < count_at + 4 implies #[trigger] after[p] == before[p] by {
            assert(p == count_at || p == count_at + 1 || p == count_at + 2 || p == count_at + 3);
        }
    }
    assert forall|p: int| 0 <= p < after.len() implies {
        &&& (p < 8 || plan.required <= p) ==> after[p] == before[p]
        &&& (12 <= p < target(plan).start && (plan.delta == 0 || !ancestor_changed(plan, plan.trail.depth as int, p))
                && !count_changed(plan, p)) ==> after[p] == before[p]
        &&& target(plan).start <= p < target(plan).start + plan.replacement_len ==> after[p] == full[p]
        &&& target(plan).start + plan.replacement_len <= p < plan.new_used ==> after[p] == before[p - plan.delta]
        &&& plan.new_used <= p < plan.old_used ==> after[p] == 0
    } by {
        if count_at <= p < count_at + 4 && !count_changed(plan, p) {
            assert(plan.list.unwrap().old_count == plan.list.unwrap().new_count);
        }
        if used <= p < end { assert(after[p] == full[p]); }
    }
    assert(replacement_result(before, full, after, plan));
}


/// Generated packed offsets construct a successful generic plan and preserve the root.
#[verifier::spinoff_prover]
proof fn fixed_admission_matches_generic(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    layout: Layout, path: InlinePath, used: u32, offset: u32, width: u32) -> (plan: Plan<'static>)
    requires 12 <= used <= before.len() <= u32::MAX, words::le_word(before, 8) == used,
        admission::fixed_prefix(layout, path, offset, width),
        crate::fixed_edit::overwrite_frame(before, after,
            Span { start: (12 + offset) as u32, end: (12 + offset + width) as u32 }),
        encoded.len() == before.len(),
        after.subrange(12 + offset as int, 12 + offset + width)
            == encoded.subrange(12 + offset as int, 12 + offset + width),
        validation::schema_valid(before.subrange(12, used as int), layout, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(12 + offset as int, 12 + offset + width),
            admission::prefix_layout(layout, path, path.len as nat), (MAX_DEPTH - path.len) as nat, true),
    ensures replacement_checked(before, plan), subtree_plan(plan),
        admission::subtree_ready(before, plan.trail, width, before.len() as u32),
        plan == admission::subtree_plan_for(before, path, plan.trail, width, before.len() as u32),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(after.subrange(12, used as int), layout, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    let trail = admission::fixed_trail(before, layout, path, used, offset, width);
    assert(admission::subtree_ready(before, trail, width, before.len() as u32));
    admission::subtree_checked(before, path, trail, width, before.len() as u32);
    let plan = admission::subtree_plan_for(before, path, trail, width, before.len() as u32);
    fixed_overwrite_refines(before, encoded, after, plan);
    fixed_write_preserves(before, encoded, after, plan);
    plan
}

/// String-loan admission derives its generic plan before composing root validity.
#[verifier::spinoff_prover]
proof fn string_admission_matches_generic(write: StringWrite, before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    layout: Layout, path: InlinePath, input_len: usize) -> (plan: Plan<'static>)
    requires string_edit::string_admitted(write, before, layout, path, input_len),
        ({ let (start, end, used, new_used) = string_edit::string_coordinates(write);
            string_result(before, encoded, after, start, end, used, new_used) }),
        validation::schema_valid(before.subrange(12, string_edit::string_coordinates(write).2 as int),
            layout, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(string_edit::string_coordinates(write).0 as int,
                string_edit::string_coordinates(write).1 as int),
            string_edit::string_selection(write, before, layout, path, input_len).layout, (MAX_DEPTH - 1) as nat, true),
    ensures replacement_checked(before, plan), subtree_plan(plan),
        admission::subtree_ready(before, plan.trail, input_len as u32, before.len() as u32),
        plan == admission::subtree_plan_for(before, path, plan.trail, input_len as u32, before.len() as u32),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(after.subrange(12, string_edit::string_coordinates(write).3 as int),
            layout, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    string_edit::admitted_facts(write, before, layout, path, input_len);
    let s = string_edit::string_selection(write, before, layout, path, input_len);
    assert(string_edit::string_selection_matches(write, before, layout, path, input_len, s));
    let (start, end, used, new_used) = string_edit::string_coordinates(write);
    admission::direct_checked(before, layout, path, used, s);
    let trail = admission::direct_trail(before, layout, path, used, s);
    assert(input_len <= u32::MAX);
    let delta = (input_len as int - (s.span.end - s.span.start)) as i64;
    admission::idle_runs(before, trail, path, used, delta);
    assert(admission::subtree_ready(before, trail, input_len as u32, before.len() as u32));
    admission::subtree_checked(before, path, trail, input_len as u32, before.len() as u32);
    let plan = admission::subtree_plan_for(before, path, trail, input_len as u32, before.len() as u32);
    string_matches_generic(before, encoded, after, plan, start, end, used, new_used);
    string_set_preserves(before, encoded, after, plan);
    plan
}


/// A compact fixed-list tail slot constructs the successful generic insertion.
#[verifier::spinoff_prover]
proof fn tail_list_admission_matches_generic(tail: TailAppend, before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    layout: Layout, path: InlinePath, width: u32) -> (plan: Plan<'static>)
    requires tail_edit::tail_admitted(tail, before, layout, path, width, None),
        ({ let (at, count, used, start, end) = tail_edit::tail_coordinates(tail);
            tail_result(before, encoded, after, at, count, used, start, end, Seq::empty()) }),
        validation::schema_valid(before.subrange(12, tail_edit::tail_coordinates(tail).2 as int),
            layout, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(tail_edit::tail_coordinates(tail).3 as int,
                tail_edit::tail_coordinates(tail).4 as int),
            validation::child_layout(tail_edit::tail_selection(tail, before, layout, path, width, None).layout, 0),
            (MAX_DEPTH - 2) as nat, true),
    ensures replacement_checked(before, plan),
        admission::entries_ready(before, plan.trail, ListEdit::Insert(tail_edit::tail_coordinates(tail).1),
            width, width, before.len() as u32),
        plan == admission::entries_plan_for(before, path, plan.trail,
            ListEdit::Insert(tail_edit::tail_coordinates(tail).1), width, width, before.len() as u32),
        replacement_result(before, encoded, after, plan),
        validation::schema_valid(after.subrange(12, tail_edit::tail_coordinates(tail).4 as int),
            layout, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    tail_edit::admitted_facts(tail, before, layout, path, width, None);
    let s = tail_edit::tail_selection(tail, before, layout, path, width, None);
    let (at, count, used, start, end) = tail_edit::tail_coordinates(tail);
    admission::direct_checked(before, layout, path, used, s);
    let trail = admission::direct_trail(before, layout, path, used, s);
    let operation = ListEdit::Insert(count);
    assert(start == used && end == used + width);
    {
        reveal(admission::entries_ready);
        assert(list_indices(operation,count)==(count as int,count as int,count as int+1));
        assert((list_indices(operation,count).1-list_indices(operation,count).0)*width==0) by(nonlinear_arith)
            requires list_indices(operation,count).1==list_indices(operation,count).0;
        assert forall|i:int| 0<=i<trail.depth implies run_safe(before,trail.nodes[i].run,width as i64)
            && (width!=0 ==> run_payload(before,trail.nodes[i].run,trail.nodes[i].payload_len)) by {
            assert(i==0);assert(trail.nodes[i].run.count==0);
        }
    }
    assert(admission::entries_ready(before, trail, operation, width, width, before.len() as u32));
    admission::entries_checked(before, path, trail, operation, width, width, before.len() as u32);
    let plan = admission::entries_plan_for(before, path, trail, operation, width, width, before.len() as u32);
    tail_matches_generic(before, encoded, after, plan, at, count, used, start, end, Seq::empty());
    assert(tail_encoded(encoded, used, start, Seq::empty()) =~= encoded);
    fixed_list_insert_preserves(before, encoded, after, plan);
    plan
}

/// An ordered-map tail admission constructs the generic searched insertion plan.
#[verifier::spinoff_prover]
proof fn tail_map_admission_matches_generic<'key>(tail: TailAppend, before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    layout: Layout, path: InlinePath, width: u32, query: &'key [u8]) -> (plan: Plan<'key>)
    requires tail_edit::tail_admitted(tail, before, layout, path, width, Some(query)),
        ({ let (at, count, used, start, end) = tail_edit::tail_coordinates(tail);
            tail_result(before, encoded, after, at, count, used, start, end, query@) }),
        validation::schema_valid(before.subrange(12, tail_edit::tail_coordinates(tail).2 as int),
            layout, MAX_DEPTH as nat, true),
        validation::schema_valid(encoded.subrange(tail_edit::tail_coordinates(tail).3 as int,
                tail_edit::tail_coordinates(tail).4 as int),
            validation::child_layout(tail_edit::tail_selection(tail, before, layout, path, width, Some(query)).layout, 0),
            (MAX_DEPTH - 2) as nat, true),
    ensures replacement_checked(before, plan), planned_map(plan, MapEdit::Insert, query@, width),
        admission::map_tail_ready(before, plan.trail, width, query, before.len() as u32),
        admission::entries_ready(before, plan.trail, ListEdit::Insert(tail_edit::tail_coordinates(tail).1),
            (width + query.len()) as u32, (width + query.len()) as u32, before.len() as u32),
        plan == admission::map_tail_plan(before, path, plan.trail, width, query, before.len() as u32),
        replacement_result(before, tail_encoded(encoded, tail_edit::tail_coordinates(tail).2,
            tail_edit::tail_coordinates(tail).3, query@), after, plan),
        validation::schema_valid(after.subrange(12, tail_edit::tail_coordinates(tail).4 as int),
            layout, MAX_DEPTH as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    hide(crate::search::lex_less); hide(crate::search::keys_ordered); hide(crate::search::keys_canonical);
    hide(crate::search::search_found); hide(crate::search::search_partition);
    tail_edit::admitted_facts(tail, before, layout, path, width, Some(query));
    let s = tail_edit::tail_selection(tail, before, layout, path, width, Some(query));
    let (at, count, used, start, end) = tail_edit::tail_coordinates(tail);
    admission::direct_checked(before, layout, path, used, s);
    let trail = Trail { local: false, ..admission::direct_trail(before, layout, path, used, s) };
    assert(validation::local_valid(node_bytes(before, trail.nodes[1]), s.layout, true));
    let stride = (width + query.len()) as u32;
    assert(width + query.len() <= u32::MAX);
    assert(admission::entries_ready(before, trail, ListEdit::Insert(count), stride, stride, before.len() as u32));
    assert(admission::map_tail_ready(before, trail, width, query, before.len() as u32));
    admission::map_tail_attachment(before, path, trail, width, query, before.len() as u32);
    let plan = admission::map_tail_plan(before, path, trail, width, query, before.len() as u32);
    assert(replacement_checked(before, plan));
    tail_matches_generic(before, encoded, after, plan, at, count, used, start, end, query@);
    let full = tail_encoded(encoded, used, start, query@);
    assert(full.subrange(start as int, end as int) =~= encoded.subrange(start as int, end as int));
    fixed_map_insert_preserves(before, full, after, plan);
    plan
}


/// Specialized front/middle insertion admission constructs the generic checked plan.
#[verifier::spinoff_prover]
proof fn fixed_insert_admission_matches_generic<'key>(loan: FixedInsert<'_, 'key>, before: Seq<u8>, root: Layout,
    path: InlinePath, width: u32, key: &'key [u8]) -> (plan: Plan<'key>)
    requires fixed_insert::fixed_insert_admitted(loan, before, root, path, width, key@),
        !path.overflow, path.len == 1,
        words::le_word(before, 8) == fixed_insert::insertion_coordinates(loan).0,
        validation::schema_valid(before.subrange(12, fixed_insert::insertion_coordinates(loan).0 as int), root, MAX_DEPTH as nat, true),
    ensures fixed_insert::insertion_plan(loan, plan), replacement_checked(before, plan),
        planned_map(plan, MapEdit::Insert, key@, width),
        admission::map_insert_ready(before, plan.trail, width, key, plan.map.unwrap().index, before.len() as u32),
        plan == admission::map_insert_plan(before, path, plan.trail, width, key, plan.map.unwrap().index, before.len() as u32),
        plan.trail.root == root, plan.new_used == fixed_insert::insertion_coordinates(loan).1,
{
    fixed_insert::admitted_plan(loan, before, root, path, width, key)
}

/// Admission and a valid exact-length encoder imply whole-root preservation.
#[verifier::spinoff_prover]
proof fn fixed_insert_admission_preserves<'key>(initial: FixedInsert<'_, 'key>, encoded: FixedInsert,
    before: Seq<u8>, after: Seq<u8>, root: Layout, path: InlinePath, width: u32, key: &'key [u8], slot: Seq<u8>)
    requires fixed_insert::fixed_insert_admitted(initial, before, root, path, width, key@),
        !path.overflow, path.len == 1,
        words::le_word(before, 8) == fixed_insert::insertion_coordinates(initial).0,
        validation::schema_valid(before.subrange(12, fixed_insert::insertion_coordinates(initial).0 as int), root, MAX_DEPTH as nat, true),
        fixed_insert::fixed_insert_loan(initial, encoded, slot, fixed_insert::insert_storage(encoded)@.subrange(
            fixed_insert::insertion_coordinates(encoded).2 as int, fixed_insert::insertion_coordinates(encoded).3 as int)),
        fixed_insert::fixed_insert_finished(encoded, after),
        validation::schema_valid(fixed_insert::insert_storage(encoded)@.subrange(
            fixed_insert::insertion_coordinates(encoded).2 as int, fixed_insert::insertion_coordinates(encoded).3 as int),
            validation::child_layout(fixed_insert::insertion_layout(initial), 0), (MAX_DEPTH - 2) as nat, true),
    ensures validation::schema_valid(after.subrange(12, fixed_insert::insertion_coordinates(initial).1 as int), root, MAX_DEPTH as nat, true),
{
    fixed_insert::admitted_preserves(initial, encoded, before, after, root, path, width, key, slot);
}

}
