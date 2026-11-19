//! Schema descriptors and bounded, allocation-free metadata checks.

// Verus generates ghost accessors for enum variants; runtime items stay linted.
#![cfg_attr(verus_only, allow(missing_docs))]

use crate::LayoutHead;
use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// Scalar encoding and its leaf validity rule.
#[derive(Clone, Copy, Debug)]
pub enum Leaf {
    /// Unsigned little-endian integer, width 1, 2, 4, 8, or 16.
    Unsigned(u8),
    /// Signed two's-complement little-endian integer of a supported width.
    Signed(u8),
    /// One byte whose value must be zero or one.
    Bool,
    /// An arbitrary, positive number of raw bytes.
    Raw(u32),
}

/// Order-preserving map-key encoding domain.
#[derive(Clone, Copy, Debug)]
pub enum KeyDomain {
    /// Big-endian unsigned integer.
    Unsigned,
    /// Big-endian integer with its sign bit flipped.
    Signed,
    /// Zero or one in a single byte.
    Bool,
    /// Raw bytes ordered lexicographically.
    Raw,
}

/// Fixed-width map-key descriptor.
#[derive(Clone, Copy, Debug)]
pub struct KeyLayout {
    /// Positive encoded width in bytes.
    pub width: u32,
    /// Canonical encoding domain.
    pub domain: KeyDomain,
}

/// A fixed field's declaration ordinal and packed wire position.
#[derive(Clone, Copy, Debug)]
pub struct FixedField {
    /// Zero-based ordinal in the original declaration.
    pub declaration: u16,
    /// Exact child descriptor, which must have fixed kind.
    pub layout: &'static Layout,
    /// Offset relative to the struct's beginning.
    pub offset: u32,
}

/// A variable field in declaration order, which is also payload order.
#[derive(Clone, Copy, Debug)]
pub struct VariableField {
    /// Zero-based ordinal in the original declaration.
    pub declaration: u16,
    /// Exact child descriptor, which must have variable kind.
    pub layout: &'static Layout,
}

/// Byte-aligned format-1 schema algebra.
///
/// Public construction is checked by [`check_layout`]. A struct with no variable
/// fields has fixed kind; all strings, options, lists, and maps have variable kind.
#[derive(Clone, Copy, Debug)]
pub enum Layout {
    /// Code-constructed view of an exact certified integer table.
    Flat {
        /// Exact immutable table, root and admitted depth.
        checked: crate::CheckedLayout<'static>,
    },
    /// A fixed scalar or raw byte array.
    Leaf(Leaf),
    /// Fixed prefix, elided-end directory, then variable payload.
    Struct {
        /// Exact length of the packed fixed prefix.
        fixed_bytes: u32,
        /// Fixed fields, individually in declaration order.
        fixed: &'static [FixedField],
        /// Variable fields, individually in declaration order.
        variable: &'static [VariableField],
    },
    /// Raw UTF-8 in an exact parent-delimited region.
    Str,
    /// One tag followed by the active child's exact encoding.
    Option {
        /// Schema of a present value.
        value: &'static Layout,
    },
    /// Counted fixed-stride or indexed elements.
    List {
        /// Element schema.
        element: &'static Layout,
    },
    /// Sorted fixed-width keys with fixed-stride or indexed values.
    Map {
        /// Key encoding and width.
        key: KeyLayout,
        /// Value schema.
        value: &'static Layout,
    },
}

/// Supported integer widths in the wire format.
pub open spec fn numeric_width(width: int) -> bool {
    width == 1 || width == 2 || width == 4 || width == 8 || width == 16
}

/// Width classification of a descriptor head, before metadata validation.
pub open spec fn width_spec(layout: Layout) -> Option<u32> {
    match layout.head() {
        LayoutHead::Leaf(leaf) => Some(match leaf {
            Leaf::Unsigned(width) | Leaf::Signed(width) => width as u32,
            Leaf::Bool => 1u32,
            Leaf::Raw(width) => width,
        }),
        LayoutHead::Struct { fixed_bytes, fixed: _, variable } =>
            if variable.len() == 0 { Some(fixed_bytes) } else { None },
        _ => None,
    }
}

#[inline(always)]
pub(crate) fn fixed_width(layout: &Layout) -> (result: Option<u32>)
    ensures result == width_spec(*layout),
{
    if let Layout::Flat {checked}=layout {
        checked.witness();
        proof { certified_width(*checked); }
        let tag=checked.header(0);
        return if matches!(tag, 1..=4)
            || (tag==9 && checked.header(4)==0) {
            Some(checked.header(1))
        } else { None };
    }

    match layout.head() {
        LayoutHead::Leaf(leaf) => Some(match leaf {
            Leaf::Unsigned(width) | Leaf::Signed(width) => width as u32,
            Leaf::Bool => 1,
            Leaf::Raw(width) => width,
        }),
        LayoutHead::Struct { fixed_bytes, fixed: _, variable } =>
            if variable.is_empty() { Some(fixed_bytes) } else { None },
        _ => None,
    }
}

/// Metadata relation for the remaining declaration-ordered merge of two arrays.
///
/// The merge consumes every ordinal exactly once and accumulates packed fixed
/// widths. It describes metadata only, independently of all byte predicates.
pub open spec fn descriptor_fields(
    fixed: Seq<crate::Field>, variable: Seq<crate::Field>, fixed_bytes: u32,
    i: int, j: int, offset: int, budget: nat,
) -> bool
    decreases budget, 0int, fixed.len() + variable.len() - i - j,
{
    if !(0 <= i <= fixed.len() && 0 <= j <= variable.len()) { false }
    else if i == fixed.len() && j == variable.len() { offset == fixed_bytes }
    else if i < fixed.len()
        && (j == variable.len() || fixed[i].declaration <= variable[j].declaration) {
        let field = fixed[i];
        match width_spec(field.layout) {
            Some(width) => width > 0 && field.declaration == i + j
                && field.offset == offset && offset + width <= u32::MAX
                && budget > 0 && descriptor_valid(field.layout, (budget - 1) as nat)
                && descriptor_fields(fixed, variable, fixed_bytes,
                    i + 1, j, offset + width, budget),
            None => false,
        }
    } else {
        variable[j].declaration == i + j && width_spec(variable[j].layout).is_none()
            && budget > 0 && descriptor_valid(variable[j].layout, (budget - 1) as nat)
            && descriptor_fields(fixed, variable, fixed_bytes, i, j + 1, offset, budget)
    }
}

/// Exposes one merge transition while keeping recursive descriptor facts folded.
pub(crate) proof fn descriptor_fields_step(
    fixed: Seq<crate::Field>, variable: Seq<crate::Field>, fixed_bytes: u32,
    i: int, j: int, offset: int, budget: nat,
)
    ensures descriptor_fields(fixed, variable, fixed_bytes, i, j, offset, budget) == (
        if !(0 <= i <= fixed.len() && 0 <= j <= variable.len()) { false }
        else if i == fixed.len() && j == variable.len() { offset == fixed_bytes }
        else if i < fixed.len()
            && (j == variable.len() || fixed[i].declaration <= variable[j].declaration) {
            match width_spec(fixed[i].layout) {
                Some(width) => width > 0 && fixed[i].declaration == i + j
                    && fixed[i].offset == offset && offset + width <= u32::MAX
                    && budget > 0 && descriptor_valid(fixed[i].layout, (budget - 1) as nat)
                    && descriptor_fields(fixed, variable, fixed_bytes, i + 1, j, offset + width, budget),
                None => false,
            }
        } else {
            variable[j].declaration == i + j && width_spec(variable[j].layout).is_none()
                && budget > 0 && descriptor_valid(variable[j].layout, (budget - 1) as nat)
                && descriptor_fields(fixed, variable, fixed_bytes, i, j + 1, offset, budget)
        }),
{
    reveal_with_fuel(descriptor_fields, 2);
}

/// Complete descriptor validity with a finite root-to-leaf node budget.
pub open spec fn descriptor_valid(layout: Layout, budget: nat) -> bool
    decreases budget, 1int, 0int,
{
    0 < budget <= crate::MAX_DEPTH && match layout.head() {
        LayoutHead::Leaf(Leaf::Unsigned(width)) | LayoutHead::Leaf(Leaf::Signed(width)) =>
            numeric_width(width as int),
        LayoutHead::Leaf(Leaf::Raw(width)) => width > 0,
        LayoutHead::Leaf(Leaf::Bool) | LayoutHead::Str => true,
        LayoutHead::Option { value } | LayoutHead::Map { key: _, value } =>
            descriptor_valid(value, (budget - 1) as nat) && match layout.head() {
                LayoutHead::Map { key, value: _ } => match key.domain {
                    KeyDomain::Unsigned | KeyDomain::Signed => numeric_width(key.width as int),
                    KeyDomain::Bool => key.width == 1,
                    KeyDomain::Raw => key.width > 0,
                },
                _ => true,
            },
        LayoutHead::List { element } => descriptor_valid(element, (budget - 1) as nat),
        LayoutHead::Struct { fixed_bytes, fixed, variable } =>
            0 < fixed.len() + variable.len() <= 65535
            && (variable.len() > 0 || fixed_bytes > 0)
            && descriptor_fields(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0, budget),
    }
}

/// Certified integer widths agree with the descriptor head seen by byte proofs.
pub(crate) proof fn certified_width(checked: crate::CheckedLayout)
    requires checked.valid(),
    ensures width_spec(Layout::Flat { checked }) ==
        if crate::flat_layout::is_fixed(checked.view(), checked.root_index()) {
            Some(checked.view()[checked.root_index()+1])
        } else { None },
{
    checked.metadata();
    reveal(crate::layout_head::head_spec);
}

/// Local flat field rules compose into the existing recursive metadata relation.
proof fn certified_fields(
    checked: crate::CheckedLayout, i: int, j: int, offset: int, budget: nat,
)
    requires checked.valid(), checked.view()[checked.root_index()] == 9,
        0 < budget <= crate::MAX_DEPTH,
        checked.view()[checked.root_index()+2] <= budget,
        crate::flat_layout::struct_fields(checked.view(), checked.root_index(),
            checked.view()[checked.root_index()+5] as int,
            checked.view()[checked.root_index()+3] as int,
            checked.view()[checked.root_index()+4] as int,
            i,j,offset,checked.view()[checked.root_index()+2] as int),
    ensures descriptor_fields(
        crate::Fields::Flat { layout: &checked, variable: false }.view(),
        crate::Fields::Flat { layout: &checked, variable: true }.view(),
        checked.view()[checked.root_index()+1], i,j,offset,budget),
    decreases budget, 0int,
        checked.view()[checked.root_index()+3] + checked.view()[checked.root_index()+4] - i-j,
{
    let w = checked.view(); let r = checked.root_index();
    let nf = w[r+3] as int; let nv = w[r+4] as int; let start = w[r+5] as int;
    reveal_with_fuel(crate::flat_layout::struct_fields, 2);
    reveal_with_fuel(descriptor_fields, 2);
    if i < nf || j < nv {
        let fixed = i < nf && (j == nv || w[start+8*i+1] <= w[start+8*(nf+j)+1]);
        let index = if fixed { i } else { nf+j };
        let f = start+8*index;
        let child = w[f+3] as int;
        checked.retarget_valid(child);
        certified_width(checked.retarget(child));
        certified_descriptor(checked.retarget(child), (budget-1) as nat);
        if fixed {
            certified_fields(checked, i+1,j,offset+w[child+1],budget);
        } else {
            certified_fields(checked, i,j+1,offset,budget);
        }
    }
}

/// A valid exact table certificate establishes the recursive kernel relation.
pub(crate) proof fn certified_descriptor(checked: crate::CheckedLayout, budget: nat)
    requires checked.valid(), 0 < budget <= crate::MAX_DEPTH,
        checked.view()[checked.root_index()+2] <= budget,
    ensures descriptor_valid(Layout::Flat { checked }, budget),
    decreases budget, 1int, 0int,
{
    checked.metadata();
    reveal(crate::layout_head::head_spec);
    let w = checked.view(); let r = checked.root_index();
    match w[r] {
        6 | 7 | 8 => {
            let child = w[w[r+5] as int+3] as int;
            checked.retarget_valid(child);
            certified_descriptor(checked.retarget(child), (budget-1) as nat);
        },
        9 => { certified_fields(checked,0,0,0,budget); },
        _ => {},
    }
}

/// Checks all descriptor metadata without reading encoded bytes.
///
/// Rejects unsupported or zero widths, nonpacked offsets, missing/duplicate
/// declaration ordinals, kind mismatches, empty fixed structs, and paths longer
/// than `max_depth` (including cycles). Budgets above [`crate::MAX_DEPTH`] are
/// rejected to bound the call stack. Every rejection is `InvalidSchema` with
/// no byte offset. The root consumes one depth unit. Traversal uses no heap.
#[inline(always)]
pub fn check_layout(layout: &Layout, max_depth: usize) -> (result: Result<()>)
    ensures result.is_ok() == descriptor_valid(*layout, max_depth as nat),
        result.is_err() ==> result == Err(Error::Fault {
            kind: ErrorKind::InvalidSchema, offset: u32::MAX,
        }),
{
    if max_depth == crate::MAX_DEPTH {
        if let Layout::Flat { checked } = layout {
            checked.witness();
            proof { certified_descriptor(*checked, max_depth as nat); }
            return Ok(());
        }
    }
    check_layout_inner(layout, max_depth)
}

fn check_layout_inner(layout: &Layout, max_depth: usize) -> (result: Result<()>)
    ensures
        result.is_ok() == descriptor_valid(*layout, max_depth as nat),
        result.is_err() ==> result == Err(Error::Fault {
            kind: ErrorKind::InvalidSchema, offset: u32::MAX,
        }),
    decreases max_depth,
{
    if max_depth == 0 || max_depth > crate::MAX_DEPTH {
        return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET));
    }
    if max_depth == crate::MAX_DEPTH {
        if let Layout::Flat { checked } = layout {
            checked.witness();
            proof { certified_descriptor(*checked, max_depth as nat); }
            return Ok(());
        }
    }
    match layout.head() {
        LayoutHead::Leaf(leaf) => {
            let valid = match leaf {
                Leaf::Unsigned(width) | Leaf::Signed(width) =>
                    width == 1 || width == 2 || width == 4 || width == 8 || width == 16,
                Leaf::Bool => true,
                Leaf::Raw(width) => width > 0,
            };
            if !valid { return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)); }
        },
        LayoutHead::Str => {},
        LayoutHead::Option { value } => { check_layout_inner(&value, max_depth - 1)?; },
        LayoutHead::List { element } => { check_layout_inner(&element, max_depth - 1)?; },
        LayoutHead::Map { key, value } => {
            let valid = match key.domain {
                KeyDomain::Unsigned | KeyDomain::Signed =>
                    key.width == 1 || key.width == 2 || key.width == 4
                        || key.width == 8 || key.width == 16,
                KeyDomain::Bool => key.width == 1,
                KeyDomain::Raw => key.width > 0,
            };
            if !valid { return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)); }
            check_layout_inner(&value, max_depth - 1)?;
        },
        LayoutHead::Struct { fixed_bytes, fixed, variable } => {
            if fixed.len() > 65535 || variable.len() > 65535 - fixed.len()
                || fixed.len() + variable.len() == 0
                || (variable.is_empty() && fixed_bytes == 0) {
                return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET));
            }
            let mut i = 0usize;
            let mut j = 0usize;
            let mut offset = 0u32;
            while i < fixed.len() || j < variable.len()
                invariant
                    i <= fixed.len(), j <= variable.len(),
                    fixed.len() + variable.len() <= 65535,
                    max_depth > 0,
                    descriptor_valid(*layout, max_depth as nat)
                        == descriptor_fields(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0, max_depth as nat),
                    descriptor_fields(fixed.view(), variable.view(), fixed_bytes, 0, 0, 0, max_depth as nat)
                        == descriptor_fields(fixed.view(), variable.view(), fixed_bytes,
                            i as int, j as int, offset as int, max_depth as nat),
                decreases fixed.len() + variable.len() - i - j,
            {
                proof { reveal_with_fuel(descriptor_fields, 2); }
                let declaration = (i + j) as u16;
                if i < fixed.len()
                    && (j == variable.len() || fixed.get(i).declaration <= variable.get(j).declaration) {
                    let field = &fixed.get(i);
                    let width = match fixed_width(&field.layout) {
                        Some(width) => width,
                        None => return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)),
                    };
                    if width == 0 || field.declaration != declaration || field.offset != offset {
                        return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET));
                    }
                    let end = match offset.checked_add(width) {
                        Some(end) => end,
                        None => return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET)),
                    };
                    check_layout_inner(&field.layout, max_depth - 1)?;
                    offset = end;
                    i += 1;
                } else {
                    let field = &variable.get(j);
                    if field.declaration != declaration || fixed_width(&field.layout).is_some() {
                        return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET));
                    }
                    check_layout_inner(&field.layout, max_depth - 1)?;
                    j += 1;
                }
            }
            if offset != fixed_bytes {
                return Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET));
            }
        },
    }
    Ok(())
}

#[cfg(verus_only)]
impl vstd::std_specs::convert::FromSpecImpl<&Layout> for Layout {
    open spec fn obeys_from_spec() -> bool { true }
    open spec fn from_spec(source: &Layout) -> Self { *source }
}
impl From<&Layout> for Layout {
    fn from(source: &Layout) -> (result: Self)
        ensures result == *source,
    { *source }
}

}
