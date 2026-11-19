//! Scalar domains, composed fixed values, and exact option tag partitions.

use crate::layout::fixed_width;
#[cfg(verus_only)]
use crate::layout::{descriptor_fields, descriptor_valid, width_spec};
use crate::LayoutHead;
use crate::{check_layout, fixed_region, Error, ErrorKind, Layout, Leaf, Result, Span, MAX_DEPTH};
use vstd::prelude::*;

verus! {

/// Domain validity of an exact fixed value, independently of metadata checking.
pub open spec fn fixed_value_valid(bytes: Seq<u8>, layout: Layout, budget: nat) -> bool
    decreases budget, 1int, 0int,
{
    budget > 0 && bytes.len() <= u32::MAX
        && width_spec(layout) == Some(bytes.len() as u32)
        && match layout.head() {
            LayoutHead::Leaf(Leaf::Bool) => bytes.len() == 1 && bytes[0] <= 1,
            LayoutHead::Leaf(_) => true,
            LayoutHead::Struct { fixed_bytes: _, fixed, variable } => variable.len() == 0
                && fixed_fields_valid(bytes, fixed.view(), 0, budget),
            _ => false,
        }
}

/// Every remaining packed fixed child has valid bits in its exact region.
pub open spec fn fixed_fields_valid(
    bytes: Seq<u8>, fixed: Seq<crate::Field>, i: int, budget: nat,
) -> bool
    decreases budget, 0int, fixed.len() - i,
{
    if !(0 <= i <= fixed.len()) { false }
    else if i == fixed.len() { true }
    else {
        let field = fixed[i];
        match width_spec(field.layout) {
            Some(width) => budget > 0
                && 0 <= field.offset <= field.offset + width <= bytes.len()
                && fixed_value_valid(bytes.subrange(field.offset as int,
                    field.offset as int + width), field.layout, (budget - 1) as nat)
                && fixed_fields_valid(bytes, fixed, i + 1, budget),
            None => false,
        }
    }
}

/// Acceptance and rejection completeness for checked fixed scalar/struct values.
pub open spec fn fixed_valid_iff(result: Result<()>, bytes: Seq<u8>, layout: Layout) -> bool {
    result.is_ok() == (descriptor_valid(layout, MAX_DEPTH as nat)
        && fixed_value_valid(bytes, layout, MAX_DEPTH as nat))
}

// Metadata and the exact length have already been checked at the safe gateway.
// The decreasing node budget bounds recursion even for public descriptors.
fn validate_fixed_inner(bytes: &[u8], layout: &Layout, _budget: usize) -> (result: Result<()>)
    requires
        descriptor_valid(*layout, _budget as nat),
        bytes.len() <= u32::MAX,
        width_spec(*layout) == Some(bytes.len() as u32),
    ensures
        result.is_ok() == fixed_value_valid(bytes@, *layout, _budget as nat),
        result.is_err() ==> match result {
            Err(Error::Fault { kind: ErrorKind::InvalidBool, offset }) =>
                offset < bytes.len() && bytes@[offset as int] > 1,
            _ => false,
        },
    decreases _budget,
{
    if let Layout::Flat {checked}=layout {
        return validate_fixed_flat(bytes,*checked,_budget);
    }
    match layout.head() {
        LayoutHead::Leaf(Leaf::Bool) => {
            if bytes[0] > 1 {
                return Err(Error::new(ErrorKind::InvalidBool, 0));
            }
        },
        LayoutHead::Leaf(_) => {},
        LayoutHead::Struct { fixed_bytes: _fixed_bytes, fixed, variable: _variable } => {
            proof { assert(_variable.view() =~= Seq::<crate::Field>::empty()); }
            let mut i = 0usize;
            let mut offset = 0u32;
            while i < fixed.len()
                invariant
                    i <= fixed.len(), _budget > 0,
                    bytes.len() == _fixed_bytes,
                    fixed_value_valid(bytes@, *layout, _budget as nat)
                        == fixed_fields_valid(bytes@, fixed.view(), 0, _budget as nat),
                    descriptor_fields(fixed.view(), Seq::empty(), _fixed_bytes,
                        i as int, 0, offset as int, _budget as nat),
                    fixed_fields_valid(bytes@, fixed.view(), 0, _budget as nat)
                        == fixed_fields_valid(bytes@, fixed.view(), i as int, _budget as nat),
                decreases fixed.len() - i,
            {
                proof {
                    reveal_with_fuel(descriptor_fields, 2);
                    reveal_with_fuel(fixed_fields_valid, 2);
                }
                let field = &fixed.get(i);
                let width = match fixed_width(&field.layout) {
                    Some(width) => width,
                    None => { assert(false); return Err(ErrorKind::InvalidSchema.into()); },
                };
                let end = offset + width;
                // The packed metadata relation bounds this child's end by the
                // complete fixed extent; the proof below unfolds the suffix.
                proof { fixed_suffix_bound(fixed.view(), _fixed_bytes, i as int,
                    offset as int, _budget as nat); }
                let child = &bytes[offset as usize..end as usize];
                match validate_fixed_inner(child, &field.layout, _budget - 1) {
                    Ok(()) => {},
                    Err(error) => return Err(error.rebase(offset)),
                }
                offset = end;
                i += 1;
            }
        },
        _ => { assert(false); return Err(ErrorKind::InvalidSchema.into()); },
    }
    Ok(())
}

// Flat roots carry only integer metadata through the recursive byte walk.
fn validate_fixed_flat(bytes: &[u8], checked: crate::CheckedLayout<'static>, _budget: usize)
    -> (result: Result<()>)
    requires descriptor_valid(Layout::Flat {checked}, _budget as nat),
        bytes.len() <= u32::MAX,
        width_spec(Layout::Flat {checked}) == Some(bytes.len() as u32),
    ensures result.is_ok() == fixed_value_valid(bytes@, Layout::Flat {checked}, _budget as nat),
        result.is_err() ==> match result {
            Err(Error::Fault {kind:ErrorKind::InvalidBool,offset}) => offset < bytes.len() && bytes@[offset as int] > 1,
            _ => false,
        },
    decreases _budget,
{
    checked.witness();
    proof {reveal(crate::layout_head::head_spec);}
    let tag=checked.header(0);
    if tag != 9 {
        if tag == 3 && bytes[0] > 1 {return Err(Error::new(ErrorKind::InvalidBool,0));}
        return Ok(());
    }
    let _fixed_bytes=checked.header(1);
    let nf=checked.header(3) as usize;
    let ghost layout=Layout::Flat {checked};
    let ghost fixed=crate::Fields::Flat {layout:&checked,variable:false}.view();
    proof {
        assert(crate::Fields::Flat {layout:&checked,variable:true}.view() =~= Seq::<crate::Field>::empty());
    }
    let mut i=0usize;
    let mut offset=0u32;
    while i<nf
        invariant i<=nf, nf==fixed.len(), _budget>0,
            nf==checked.view()[checked.root_index()+3],
            checked.view()[checked.root_index()]==9,
            checked.view()[checked.root_index()+4]==0,
            fixed==(crate::Fields::Flat {layout:&checked,variable:false}).view(),
            _fixed_bytes==checked.view()[checked.root_index()+1],
            layout==(Layout::Flat {checked}),
            bytes.len()==_fixed_bytes,
            fixed_value_valid(bytes@,layout,_budget as nat)
                ==fixed_fields_valid(bytes@,fixed,0,_budget as nat),
            descriptor_fields(fixed,Seq::empty(),_fixed_bytes,i as int,0,offset as int,_budget as nat),
            fixed_fields_valid(bytes@,fixed,0,_budget as nat)
                ==fixed_fields_valid(bytes@,fixed,i as int,_budget as nat),
        decreases nf-i,
    {
        proof {
            reveal_with_fuel(descriptor_fields,2);
            reveal_with_fuel(fixed_fields_valid,2);
        }
        let (_,_field_offset,child)=checked.field(i,false);
        proof {crate::layout::certified_width(child);}
        let width=child.header(1);
        proof {
            assert(fixed[i as int].layout==Layout::Flat {checked:child});
            assert(_field_offset==offset);
            fixed_suffix_bound(fixed,_fixed_bytes,i as int,offset as int,_budget as nat);
        }
        let end=offset+width;
        match validate_fixed_flat(&bytes[offset as usize..end as usize],child,_budget-1) {
            Ok(())=>{},
            Err(error)=>return Err(error.rebase(offset)),
        }
        offset=end;
        i+=1;
    }
    Ok(())
}

/// A packed metadata suffix never extends past the complete fixed extent.
proof fn fixed_suffix_bound(
    fixed: Seq<crate::Field>, size: u32, i: int, offset: int, budget: nat,
)
    requires descriptor_fields(fixed, Seq::empty(), size, i, 0, offset, budget),
    ensures
        offset <= size,
        i < fixed.len() ==> offset + width_spec(fixed[i].layout).unwrap() <= size,
    decreases fixed.len() - i,
{
    reveal_with_fuel(descriptor_fields, 2);
    if i < fixed.len() {
        let width = width_spec(fixed[i].layout).unwrap();
        fixed_suffix_bound(fixed, size, i + 1, offset + width, budget);
    }
}

/// Checks the exact encoding and every leaf domain of a fixed-kind value.
///
/// Checks metadata and exact width as [`fixed_region`] does, then visits packed
/// fixed fields in wire order. All integer/raw bit patterns are valid; bool
/// accepts only zero or one. The first invalid bool reports its offset relative
/// to `bytes`, including inside nested fixed structs. Variable-kind descriptors
/// return `InvalidSchema`. Traversal is bounded by [`MAX_DEPTH`] and uses no heap.
pub fn validate_fixed(bytes: &[u8], layout: &Layout) -> (result: Result<()>)
    ensures fixed_valid_iff(result, bytes@, *layout),
        result.is_err() && *layout == Layout::Leaf(Leaf::Bool) && bytes@.len() == 1
            ==> result == Err(Error::Fault { kind: ErrorKind::InvalidBool, offset: 0 }),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    proof { reveal(crate::layout_head::head_spec); }
    fixed_region(bytes, layout)?;
    validate_fixed_inner(bytes, layout, MAX_DEPTH)
}

/// Exact tag/extent shape of an option, excluding its active child's bit domain.
pub open spec fn option_shape(bytes: Seq<u8>, layout: Layout) -> bool {
    match layout.head() {
        LayoutHead::Option { value } => 1 <= bytes.len() <= u32::MAX
            && (if bytes[0] == 0 { bytes.len() == 1 }
                else { bytes[0] == 1 && match width_spec(value) {
                    Some(width) => bytes.len() == 1 + width,
                    None => true,
                } }),
        _ => false,
    }
}

/// Option partition iff valid metadata and exact tag/extent, with exact suffix.
pub open spec fn option_partition_iff(
    result: Result<Option<Span>>, bytes: Seq<u8>, layout: Layout,
) -> bool {
    let valid = descriptor_valid(layout, MAX_DEPTH as nat) && option_shape(bytes, layout);
    match result {
        Ok(payload) => valid && payload == if bytes[0] == 0 { None }
            else { Some(Span { start: 1, end: bytes.len() as u32 }) },
        Err(_) => !valid,
    }
}

/// Checks an option's tag and returns its exact active payload, if present.
///
/// Metadata is checked first, then wire-length overflow, a missing tag, the tag
/// domain, and exact extent. None is exactly `[0]`. Some consumes the complete
/// suffix and checks its width when the child has fixed kind. An empty variable
/// payload is allowed here. Child domains and variable structure are checked by
/// their own validators. Faults identify the tag at zero, trailing None bytes
/// at one, or a fixed payload's length fault rebased by one.
pub fn partition_option(bytes: &[u8], layout: &Layout) -> (result: Result<Option<Span>>)
    ensures option_partition_iff(result, bytes@, *layout),
        result.is_err() ==> crate::error::fault_within(result.unwrap_err(), bytes@.len()),
{
    check_layout(layout, MAX_DEPTH)?;
    let value = if let Layout::Flat { checked } = layout {
        proof { reveal(crate::layout_head::head_spec); }
        if checked.header(0) != 6 { return Err(ErrorKind::InvalidSchema.into()); }
        Layout::Flat { checked: checked.only_child() }
    } else { match layout.head() {
        LayoutHead::Option { value } => value,
        _ => return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)),
    }};
    if bytes.len() > u32::MAX as usize {
        return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
    }
    if bytes.is_empty() {
        return Err(Error::new(ErrorKind::Truncated, 0));
    }
    match bytes[0] {
        0 => {
            if bytes.len() != 1 {
                return Err(Error::new(ErrorKind::InvalidBoundary, 1));
            }
            Ok(None)
        },
        1 => {
            if let Some(width) = fixed_width(&value) {
                let expected = match width.checked_add(1) {
                    Some(expected) => expected,
                    None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
                };
                if bytes.len() - 1 < width as usize {
                    return Err(Error::new(ErrorKind::Truncated, 1));
                }
                if bytes.len() - 1 != width as usize {
                    return Err(Error::new(ErrorKind::InvalidBoundary, expected));
                }
            }
            Ok(Some(Span { start: 1, end: bytes.len() as u32 }))
        },
        _ => Err(Error::new(ErrorKind::InvalidTag, 0)),
    }
}

}
