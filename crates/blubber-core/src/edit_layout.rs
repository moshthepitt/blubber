//! Header and adjacent-boundary checks for immediate typed edits.

use crate::layout::fixed_width;
use crate::list_layout::partition_list_header;
use crate::map_layout::partition_map_header;
use crate::struct_layout::partition_struct_header;
use crate::validation::Parts;
use crate::{
    check_layout, fixed_region, partition_option, read_u32, ListPartition, MapPartition,
    StructPartition,
};
#[cfg(verus_only)]
use crate::{
    layout::{descriptor_fields, descriptor_valid, width_spec},
    leaves, list_layout, map_layout, struct_layout, validation, words,
};
use crate::{Error, ErrorKind, Layout, LayoutHead, Result, Span, MAX_DEPTH};
use vstd::prelude::*;

verus! {
/// Header admission does not certify directory order or untouched child bytes.
pub(crate) open spec fn header_valid(bytes: Seq<u8>, layout: Layout) -> bool {
    descriptor_valid(layout, MAX_DEPTH as nat) && match layout.head() {
        LayoutHead::Leaf(_) => bytes.len() <= u32::MAX && width_spec(layout) == Some(bytes.len() as u32),
        LayoutHead::Str => bytes.len() <= u32::MAX,
        LayoutHead::Struct { .. } => crate::struct_layout::header_shape(bytes, layout),
        LayoutHead::List { .. } => crate::list_layout::header_shape(bytes, layout),
        LayoutHead::Map { .. } => crate::map_layout::header_shape(bytes, layout),
        LayoutHead::Option { .. } => leaves::option_shape(bytes, layout),
    }
}

pub(crate) open spec fn parts_local(parts: Parts, bytes: Seq<u8>, layout: Layout, _semantic: bool) -> bool {
    header_valid(bytes, layout) && match parts {
        Parts::Done => matches!(layout.head(), LayoutHead::Leaf(_) | LayoutHead::Str),
        Parts::Struct(p) => struct_layout::header_iff(Ok(p), bytes, layout),
        Parts::List(p) => list_layout::header_iff(Ok(p), bytes, layout),
        Parts::Map(p) => map_layout::header_iff(Ok(p), bytes, layout),
        Parts::Option(p) => leaves::option_partition_iff(Ok(p), bytes, layout),
    }
}

/// Count arithmetic faults identify the source count word in a lazy header.
pub closed spec fn count_fault(error: Error) -> Error {
    match error {
        Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: u32::MAX } =>
            Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: 0 },
        other => other,
    }
}
#[cold]
pub(crate) fn count_error(error: Error) -> (result: Error)
    ensures result == count_fault(error),
{
    match error {
        Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: Error::NO_OFFSET } =>
            Error::new(ErrorKind::ArithmeticOverflow, 0),
        other => other,
    }
}

#[inline(always)]
pub(crate) fn enter_local(bytes: &[u8], layout: &Layout, _semantic: bool) -> (result: Result<Parts>)
    ensures result.is_ok() ==> parts_local(result.unwrap(), bytes@, *layout, false),

{
    proof { reveal(crate::layout_head::head_spec); }
    check_layout(layout, MAX_DEPTH)?;
    let parts = match layout.tag() {
        1..=4 => {
            fixed_region(bytes, layout)?;
            Parts::Done
        },
        5 => {
            if bytes.len() > u32::MAX as usize {
                return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
            }
            Parts::Done
        },
        9 => Parts::Struct(partition_struct_header(bytes, layout)?),
        6 => Parts::Option(partition_option(bytes, layout)?),
        7 => {
            let partition = match partition_list_header(bytes, layout) {
                Ok(partition) => partition,
                Err(error) => return Err(count_error(error)),
            };
            Parts::List(partition)
        },
        8 => {
            let p = match partition_map_header(bytes, layout) {
                Ok(partition) => partition,
                Err(error) => return Err(count_error(error)),
            };
            Parts::Map(p)
        },
        _=>{assert(false);return Err(ErrorKind::InvalidSchema.into());},
    };
    Ok(parts)
}

#[inline(always)]
pub(crate) fn count_local(parts: &Parts, layout: &Layout, Ghost(bytes): Ghost<Seq<u8>>, _semantic: bool)
    -> (result: u32)
    requires parts_local(*parts, bytes, *layout, _semantic),
    ensures result == validation::child_count(bytes, *layout),
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

pub(crate) proof fn fixed_bounds(
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
#[inline(always)]
pub(crate) fn child_local(
    bytes: &[u8], layout: &Layout, parts: &Parts, index: u32, _semantic: bool,
) -> (result: Result<(Span, Layout)>)
    requires parts_local(*parts, bytes@, *layout, _semantic),
        index < validation::child_count(bytes@, *layout),
    ensures match result {
        Ok((span, child)) => span == validation::child_span(bytes@, *layout, index as int)
            && child == validation::child_layout(*layout, index as int)
            && span.start <= span.end <= bytes.len(),
        Err(_) => true,
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
                    let span=struct_variable_field_local(bytes,layout,p,ordinal)?;
                    return Ok((span,Layout::Flat{checked:checked.child(ordinal as usize,true)}));
                }
            },
            Parts::Option(Some(span))=>return Ok((*span,Layout::Flat{checked:checked.only_child()})),
            Parts::List(p)=>{
                let span=list_element_local(bytes,layout,p,index)?;
                return Ok((span,Layout::Flat{checked:checked.only_child()}));
            },
            Parts::Map(p)=>{
                let span=map_value_local(bytes,layout,p,index)?;
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
                let span = struct_variable_field_local(bytes, layout, p, ordinal)?;
                Ok((span, variable.get(ordinal as usize).layout))
            }
        },
        (LayoutHead::Option { value }, Parts::Option(Some(span))) => Ok((*span, value)),
        (LayoutHead::List { element }, Parts::List(p)) => {
            let span = list_element_local(bytes, layout, p, index)?;
            Ok((span, element))
        },
        (LayoutHead::Map { value, .. }, Parts::Map(p)) => {
            let span = map_value_local(bytes, layout, p, index)?;
            Ok((span, value))
        },
        _ => { assert(false); Err(ErrorKind::InvalidSchema.into()) },
    }
}

pub(crate) fn list_element_local(
    bytes: &[u8], _layout: &Layout, partition: &ListPartition, index: u32,
) -> (result: Result<Span>)
    requires
        crate::list_layout::header_iff(Ok(*partition), bytes@, *_layout),
        index < partition.count,
    ensures crate::list_layout::list_shape(bytes@, *_layout) ==> result.is_ok(),
        result.is_ok() ==> result.unwrap() == crate::list_layout::element_span(bytes@, *_layout, index)
        && result.unwrap().start <= result.unwrap().end <= bytes.len()
        && (partition.element_width.is_none() ==> partition.payload.start <= result.unwrap().start),

{
    hide(words::le_word);
    match partition.element_width {
        Some(width) => {
            proof {
                assert(bytes.len() == 4 + partition.count as int * width);
                assert(0 <= index as int * width
                    && 4 + index as int * width + width <= bytes.len()) by(nonlinear_arith)
                    requires 0 <= width, index < partition.count,
                        bytes.len() == 4 + partition.count as int * width;
                assert((index as int + 1) * width == index as int * width + width)
                    by(nonlinear_arith);
            }
            let start = 4 + index * width;
            let span = Span { start, end: start + width };
            Ok(span)
        },
        None => {
            let start = if index == 0 { 0u32 } else {
                read_u32(bytes, 4 + 4 * (index - 1))?
            };
            let end = if index + 1 == partition.count {
                partition.payload.end - partition.payload.start
            } else {
                read_u32(bytes, 4 + 4 * index)?
            };
            if start > end || end > partition.payload.end - partition.payload.start {
                return Err(ErrorKind::InvalidBoundary.into());
            }
            let span = Span { start: partition.payload.start + start, end: partition.payload.start + end };
            Ok(span)
        },
    }
}

pub(crate) fn map_value_local(
    bytes: &[u8], _layout: &Layout, partition: &MapPartition, index: u32,
) -> (result: Result<Span>)
    requires
        crate::map_layout::header_iff(Ok(*partition), bytes@, *_layout),
        index < partition.count,
    ensures crate::map_layout::map_shape(bytes@, *_layout) ==> result.is_ok(),
        result.is_ok() ==> result.unwrap() == crate::map_layout::value_span(bytes@, *_layout, index)
        && result.unwrap().start <= result.unwrap().end <= bytes.len()
        && (partition.value_width.is_none() ==> partition.payload.start <= result.unwrap().start),

{
    hide(words::le_word);
    let tail = partition.value_width.unwrap_or(4u32);
    proof {
        let stride = crate::map_layout::entry_stride(*_layout);
        assert(0 <= index as int * stride
            && 4 + index as int * stride + stride <= bytes.len()
            && stride <= u32::MAX) by(nonlinear_arith)
            requires index < partition.count, partition.count > 0, stride > 0,
                4 + partition.count as int * stride <= bytes.len(), bytes.len() <= u32::MAX;
    }
    let stride = partition.key_width + tail;
    let at = 4 + index * stride + partition.key_width;
    match partition.value_width {
        Some(width) => {
            assert(index as int * stride + partition.key_width + width == (index as int + 1) * stride) by(nonlinear_arith)
                requires stride == partition.key_width + width;
            let span = Span { start: at, end: at + width };
            Ok(span)
        },
        None => {
            let start = if index == 0 { 0u32 } else {
                read_u32(bytes, at - stride)?
            };
            proof {
                assert(at as int - stride == 4 + (index as int - 1) * stride + partition.key_width) by(nonlinear_arith)
                    requires at == 4 + index as int * stride + partition.key_width;
            }
            let end = read_u32(bytes, at)?;
            if start > end || end > partition.payload.end - partition.payload.start {
                return Err(Error::new(ErrorKind::InvalidBoundary, at));
            }
            let span = Span { start: partition.payload.start + start, end: partition.payload.start + end };
            Ok(span)
        },
    }
}

#[inline(always)]
pub(crate) fn struct_variable_field_local(
    bytes: &[u8], _layout: &Layout, partition: &StructPartition, index: u16,
) -> (result: Result<Span>)
    requires
        crate::struct_layout::header_iff(Ok(*partition), bytes@, *_layout),
        index < partition.variables,
    ensures crate::struct_layout::struct_shape(bytes@, *_layout) ==> result.is_ok(),
        result.is_ok() ==> result.unwrap() == crate::struct_layout::variable_span(bytes@, *_layout, index)
        && partition.payload.start <= result.unwrap().start <= result.unwrap().end <= bytes.len(),

{
    hide(words::le_word);
    let start = if index == 0 { 0u32 } else {
        read_u32(bytes, partition.directory.start + 4 * (index as u32 - 1))?
    };
    let end = if index as u32 + 1 == partition.variables as u32 {
        partition.payload.end - partition.payload.start
    } else {
        read_u32(bytes, partition.directory.start + 4 * index as u32)?
    };
    if start > end || end > partition.payload.end - partition.payload.start {
        return Err(ErrorKind::InvalidBoundary.into());
    }
    Ok(Span { start: partition.payload.start + start, end: partition.payload.start + end })
}

}
