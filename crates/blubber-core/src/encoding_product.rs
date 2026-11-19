//! Type-directed field traversal and exact struct destination loans.
//!
//! Sources are trusted typed projections. The library owns the traversal and
//! packing algorithms; a projection supplies only child codec operations.

// Explicit matches retain panic-free error propagation under Verus.
#![allow(clippy::question_mark)]

use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// Trusted codec and replay markers for a complete projected field graph.
#[derive(Clone, Copy)]
pub struct Markers {
    /// Exact schema-valid encoding contract.
    pub encoder: bool,
    /// Pure deterministic measurement and replay contract.
    pub replay: bool,
}

/// Combines adjacent graphs before any child codec operation runs.
pub const fn join(left: Markers, right: Markers) -> (result: Markers)
    ensures result.encoder == (left.encoder && right.encoder),
        result.replay == (left.replay && right.replay),
    no_unwind
{ Markers { encoder: left.encoder && right.encoder, replay: left.replay && right.replay } }

/// Selects the contract route entirely from immutable monomorphized constants.
pub const fn contract(markers: Markers) -> (result: bool)
    ensures result == (markers.encoder && markers.replay),
    no_unwind
{ markers.encoder && markers.replay }

/// Child operations supplied by a typed library projection.
///
/// A true-marker source promises the declared byte view and pure measurement.
/// Default-false sources retain their calls and checked publication protocol.
/// Selecting a marker does not verify a handwritten implementation.
pub trait Source {
    /// Trusted projection and child-codec premise.
    spec fn trusted_input(&self) -> bool;
    /// Exact child bytes, independent of the destination history.
    spec fn bytes(&self) -> Seq<u8>;
    /// Deterministic measurement observation, including the first child error.
    spec fn measurement(&self) -> Result<u32>;
    /// Descriptor selected by the same trusted child type.
    spec fn layout(&self) -> crate::Layout;
    /// Whether the child occupies the packed fixed prefix.
    spec fn fixed(&self) -> bool;

    /// Reads the child's statically selected storage kind.
    fn is_fixed(&self) -> (result: bool)
        ensures result == self.fixed(),
        no_unwind;

    /// Measures this child, preserving the existing codec call.
    fn measure(&self) -> (result: Result<u32>)
        requires self.trusted_input(),
        ensures result == self.measurement(),
            result.is_ok() == (self.bytes().len() <= u32::MAX),
            result.is_ok() ==> result.unwrap() == self.bytes().len(),
        no_unwind;

    /// Writes a child into its exact previously measured loan.
    fn encode(&self, destination: &mut [u8])
        requires self.trusted_input(), old(destination).len() == self.bytes().len(),
        ensures final(destination)@ == self.bytes(),
        no_unwind;
}

/// A projected leaf in a declaration-ordered field tree.
pub struct Field<S> {
    /// Borrowed typed child projection.
    pub source: S,
}

/// Balanced grouping of adjacent declaration-ordered fields.
///
/// Grouping adds no descriptor node or wire depth.
pub struct Pair<L, R> {
    /// Earlier declarations.
    pub left: L,
    /// Later declarations.
    pub right: R,
}

/// Checked cursor through a struct's three disjoint output regions.
#[derive(Clone, Copy)]
pub struct Cursor {
    /// Current fixed coordinate.
    pub fixed: usize,
    /// Current fixed limit coordinate.
    pub fixed_limit: usize,
    /// Current directory coordinate.
    pub directory: usize,
    /// Current directory limit coordinate.
    pub directory_limit: usize,
    /// Current payload coordinate.
    pub payload: usize,
    /// Current end coordinate.
    pub end: u32,
}

/// One field's abstract byte view and storage class.
pub ghost struct Item {
    /// Exact child bytes.
    pub bytes: Seq<u8>,
    /// Packed-prefix selection.
    pub fixed: bool,
    /// Descriptor selected by the trusted child type.
    pub layout: crate::Layout,
}

/// Cursor geometry before the next projected child.
pub open spec fn geometry(cursor: Cursor, length: int) -> bool {
    cursor.fixed <= cursor.fixed_limit <= cursor.directory <= cursor.directory_limit
        <= cursor.payload <= length <= u32::MAX
        && (cursor.directory_limit - cursor.directory) % 4 == 0
        && cursor.end <= cursor.payload
}

/// Cursor following one exact child loan.
pub open spec fn next(cursor: Cursor, item: Item) -> Cursor {
    if item.fixed {
        Cursor { fixed: (cursor.fixed + item.bytes.len()) as usize, ..cursor }
    } else {
        Cursor {
            payload: (cursor.payload + item.bytes.len()) as usize,
            end: (cursor.end + item.bytes.len()) as u32,
            directory: if cursor.directory < cursor.directory_limit {
                (cursor.directory + 4) as usize
            } else { cursor.directory },
            ..cursor
        }
    }
}

/// An exact field fits its selected region without arithmetic overflow.
pub open spec fn fits(cursor: Cursor, length: int, item: Item) -> bool {
    geometry(cursor, length) && item.bytes.len() <= u32::MAX
        && if item.fixed { cursor.fixed + item.bytes.len() <= cursor.fixed_limit }
        else { cursor.payload + item.bytes.len() <= length
            && cursor.end + item.bytes.len() <= u32::MAX }
}

/// Replaces one interval while preserving the surrounding bytes.
pub open spec fn splice(before: Seq<u8>, at: int, value: Seq<u8>) -> Seq<u8> {
    before.subrange(0, at) + value
        + before.subrange(at + value.len(), before.len() as int)
}

/// Canonical little-endian directory word.
pub open spec fn word(value: int) -> Seq<u8> {
    seq![value as u8, (value / 256) as u8, (value / 65536) as u8,
        (value / 16777216) as u8]
}

/// Complete writes of one child, including its nonterminal cumulative end.
pub open spec fn paint(before: Seq<u8>, cursor: Cursor, item: Item) -> Seq<u8> {
    if item.fixed { splice(before, cursor.fixed as int, item.bytes) }
    else {
        let body = splice(before, cursor.payload as int, item.bytes);
        if cursor.directory < cursor.directory_limit {
            splice(body, cursor.directory as int, word(cursor.end + item.bytes.len()))
        } else { body }
    }
}

/// Recursive field traversal specification, independent of tree grouping.
pub open spec fn fits_all(cursor: Cursor, length: int, items: Seq<Item>) -> bool
    decreases items.len(),
{
    items.len() == 0 || (fits(cursor, length, items[0])
        && fits_all(next(cursor, items[0]), length, items.drop_first()))
}

/// Cursor after a declaration-ordered sequence.
pub open spec fn advance(cursor: Cursor, items: Seq<Item>) -> Cursor
    decreases items.len(),
{
    if items.len() == 0 { cursor }
    else { advance(next(cursor, items[0]), items.drop_first()) }
}

/// Output after a declaration-ordered sequence of exact child writes.
pub open spec fn paint_all(before: Seq<u8>, cursor: Cursor, items: Seq<Item>) -> Seq<u8>
    decreases items.len(),
{
    if items.len() == 0 { before }
    else { paint_all(paint(before, cursor, items[0]), next(cursor, items[0]), items.drop_first()) }
}

/// Sum of field bytes, used by measurement without constructing byte views.
pub open spec fn extent(items: Seq<Item>) -> nat
    decreases items.len(),
{
    if items.len() == 0 { 0 } else { items[0].bytes.len() + extent(items.drop_first()) }
}

/// Library field traversal, instantiated by `Field` and balanced `Pair`.
pub trait Fields: crate::encoding::private::Sealed {
    /// Declaration-ordered child views.
    spec fn items(&self) -> Seq<Item>;
    /// Trusted projected child operations.
    spec fn trusted_input(&self) -> bool;
    /// Full left-to-right result, including the first child or extent error.
    spec fn measurement(&self, total: u32) -> Result<u32>;

    /// Accumulates measured extents in declaration order, preserving first errors.
    fn measure_from(&self, total: u32) -> (result: Result<u32>)
        requires self.trusted_input(),
        ensures result.is_ok() == (total + extent(self.items()) <= u32::MAX),
            result.is_ok() ==> result.unwrap() == total + extent(self.items()),
            result == self.measurement(total),
        no_unwind;

    /// Writes each child immediately after measuring it, with no staged input tree.
    fn write(&self, destination: &mut [u8], cursor: Cursor) -> (result: Cursor)
        requires self.trusted_input(), fits_all(cursor, old(destination).len() as int, self.items()),
        ensures final(destination).len() == old(destination).len(),
            result == advance(cursor, self.items()),
            final(destination)@ == paint_all(old(destination)@, cursor, self.items()),
        no_unwind;
}

impl<S: Source> crate::encoding::private::Sealed for Field<S> {}
impl<L: Fields, R: Fields> crate::encoding::private::Sealed for Pair<L, R> {}

impl<S: Source> Fields for Field<S> {
    open spec fn items(&self) -> Seq<Item> {
        seq![Item { bytes: self.source.bytes(), fixed: self.source.fixed(), layout: self.source.layout() }]
    }
    open spec fn trusted_input(&self) -> bool { self.source.trusted_input() }
    open spec fn measurement(&self, total: u32) -> Result<u32> {
        match self.source.measurement() {
            Err(error) => Err(error),
            Ok(size) => if total + size <= u32::MAX { Ok((total + size) as u32) }
                else { Err(Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: u32::MAX }) },
        }
    }

    #[inline(always)]
    fn measure_from(&self, total: u32) -> (result: Result<u32>)
        ensures result.is_ok() == (total + crate::encoding_product::extent(self.items()) <= u32::MAX),
            result.is_ok() ==> result.unwrap() == total + crate::encoding_product::extent(self.items()),
            result == self.measurement(total),
    {
        proof { reveal_with_fuel(extent, 2); }
        let size = match self.source.measure() {
            Ok(size) => size,
            Err(error) => return Err(error),
        };
        match total.checked_add(size) {
            Some(value) => Ok(value),
            None => Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
        }
    }

    #[inline(always)]
    fn write(&self, destination: &mut [u8], cursor: Cursor) -> (result: Cursor)
        ensures final(destination).len() == old(destination).len(),
            result == crate::encoding_product::advance(cursor, self.items()),
            final(destination)@ == crate::encoding_product::paint_all(old(destination)@, cursor, self.items()),
    {
        proof { reveal_with_fuel(fits_all, 2); }
        let size = match self.source.measure() {
            Ok(size) => size as usize,
            Err(error) => panic!("previously measured schema input: {:?}", error),
        };
        let mut result = cursor;
        let ghost before = destination@;
        let ghost item = self.items()[0];
        if self.source.is_fixed() {
            let (prefix, _) = destination.split_at_mut(cursor.fixed_limit);
            let (_, rest) = prefix.split_at_mut(cursor.fixed);
            let (child, _) = rest.split_at_mut(size);
            self.source.encode(child);
            result.fixed = crate::words::add_bounded(result.fixed, size);
        } else {
            let (_, rest) = destination.split_at_mut(cursor.payload);
            let (child, _) = rest.split_at_mut(size);
            self.source.encode(child);
            proof { assert(destination@ == splice(before, cursor.payload as int, item.bytes)); }
            result.payload = crate::words::add_bounded(result.payload, size);
            result.end = crate::words::add_bounded(result.end as usize, size) as u32;
            if cursor.directory < cursor.directory_limit {
                let encoded = [result.end as u8, (result.end / 256) as u8,
                    (result.end / 65536) as u8, (result.end / 16777216) as u8];
                let (_, rest) = destination.split_at_mut(cursor.directory);
                let (slot, _) = rest.split_at_mut(4);
                crate::encoding_leaf::encode_array(&encoded, slot);
                result.directory = crate::words::add_bounded(result.directory, 4);
            }
        }
        proof {
            reveal_with_fuel(fits_all, 2);
            reveal_with_fuel(advance, 2);
            reveal_with_fuel(paint_all, 2);
        }
        result
    }
}

/// Balanced grouping preserves declaration order, extents and every write.
pub proof fn grouping(before: Seq<u8>, cursor: Cursor, left: Seq<Item>, right: Seq<Item>)
    ensures extent(left + right) == extent(left) + extent(right),
        advance(cursor, left + right) == advance(advance(cursor, left), right),
        paint_all(before, cursor, left + right)
            == paint_all(paint_all(before, cursor, left), advance(cursor, left), right),
        fits_all(cursor, before.len() as int, left + right)
            ==> fits_all(cursor, before.len() as int, left)
                && fits_all(advance(cursor, left), before.len() as int, right),
    decreases left.len(),
{
    if left.len() > 0 {
        assert((left + right).drop_first() =~= left.drop_first() + right);
        grouping(paint(before, cursor, left[0]), next(cursor, left[0]), left.drop_first(), right);
    }
}



/// Runs the library field traversal with its generic exact-write contract.
///
/// # Panics
/// Each source must satisfy its projection contract and fit the destination's
/// three regions. Public schema callers establish these facts by preflight.
#[inline(always)]
pub fn write_fields<F: Fields>(source: &F, destination: &mut [u8], cursor: Cursor) -> (result: Cursor)
    requires source.trusted_input(), fits_all(cursor, old(destination).len() as int, source.items()),
    ensures final(destination).len() == old(destination).len(),
        result == advance(cursor, source.items()),
        final(destination)@ == paint_all(old(destination)@, cursor, source.items()),
    no_unwind
{ source.write(destination, cursor) }

/// Accumulates field measurements through the generic library contract.
#[inline(always)]
pub fn measure_fields<F: Fields>(source: &F, total: u32) -> (result: Result<u32>)
    requires source.trusted_input(),
    ensures result.is_ok() == (total + extent(source.items()) <= u32::MAX),
        result.is_ok() ==> result.unwrap() == total + extent(source.items()),
        result == source.measurement(total),
    no_unwind
{ source.measure_from(total) }

/// Initializes exact fixed, directory and payload regions.
///
/// # Panics
/// The fixed prefix and directory must fit the destination.
#[inline(always)]
pub fn cursor(length: usize, fixed: usize, directory: usize) -> (result: Cursor)
    requires fixed + directory <= length <= u32::MAX, directory % 4 == 0,
    ensures geometry(result, length as int), result.fixed == 0,
        result.fixed_limit == fixed, result.directory == fixed,
        result.directory_limit == fixed + directory,
        result.payload == fixed + directory, result.end == 0,
    no_unwind
{
    let start = match fixed.checked_add(directory) {
        Some(start) => start,
        None => panic!("struct header extent"),
    };
    assert!(start <= length);
    Cursor { fixed: 0, fixed_limit: fixed, directory: fixed,
        directory_limit: fixed + directory, payload: fixed + directory, end: 0 }
}

/// Rejects an inexact destination after all existing child callbacks complete.
#[inline(always)]
pub fn finish(cursor: Cursor, length: usize)
    requires cursor.fixed == cursor.fixed_limit,
        cursor.directory == cursor.directory_limit, cursor.payload == length,
    ensures true,
    no_unwind
{
    assert!(cursor.fixed == cursor.fixed_limit && cursor.directory == cursor.directory_limit
        && cursor.payload == length, "destination must have measured length");
}

}
