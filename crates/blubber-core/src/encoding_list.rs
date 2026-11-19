//! Borrowed list traversal, checked measurement and exact element loans.
#![allow(clippy::question_mark)]

#[cfg(verus_only)]
use crate::encoding_product::{self as product, Item};
use crate::encoding_product::{Cursor, Source};
use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// Immutable element projections, supplied without an intermediate collection.
///
/// Schema adapters select the child type and borrow the indexed initializer.
/// Their trusted projection contract supplies the same immutable byte view on
/// every call. Default-false codecs retain the checked publication protocol.
pub trait Elements {
    /// Type of one borrowed child projection.
    type Child: Source;
    /// Declaration-order child views.
    spec fn items(&self) -> Seq<Item>;
    /// Stable measurement observation for one projected child.
    spec fn measurement(&self, index: int) -> Result<u32>;
    /// Trusted projection and child codec contracts.
    spec fn trusted_input(&self) -> bool;

    /// Number of projected elements; no child callback runs.
    fn count(&self) -> (count: usize)
        ensures count == self.items().len(),
        no_unwind;

    /// Borrows one child without invoking its codec.
    fn at(&self, index: usize) -> (child: Self::Child)
        requires self.trusted_input(), index < self.items().len(),
        ensures child.trusted_input(), child.measurement() == self.measurement(index as int), child.bytes() == self.items()[index as int].bytes,
            child.fixed() == self.items()[index as int].fixed,
            child.layout() == self.items()[index as int].layout,
        no_unwind;
}

/// Stored-end bytes, with the zero/one-element terminal end elided.
pub open spec fn directory(count: int, indexed: bool) -> int {
    if indexed && count > 0 { 4 * (count - 1) } else { 0 }
}

/// Complete measured extent, including the count and stored ends.
pub open spec fn size(items: Seq<Item>, indexed: bool) -> int {
    4 + directory(items.len() as int, indexed) + product::extent(items)
}

/// Uniform storage class and fixed stride selected by the child schema.
pub open spec fn uniform(items: Seq<Item>, width: Option<u32>) -> bool {
    forall|i: int| 0 <= i < items.len() ==> {
        &&& items[i].fixed == width.is_some()
        &&& width.is_some() ==> items[i].bytes.len() == width.unwrap()
    }
}

/// Initial count, directory and element payload coordinates.
pub open spec fn initial(items: Seq<Item>, width: Option<u32>) -> Cursor {
    let payload = 4 + directory(items.len() as int, width.is_none());
    Cursor {
        fixed: 4, fixed_limit: if width.is_some() { size(items, false) as usize } else { 4 },
        directory: if width.is_some() { size(items, false) as usize } else { 4 },
        directory_limit: if width.is_some() { size(items, false) as usize } else { payload as usize },
        payload: if width.is_some() { size(items, false) as usize } else { payload as usize },
        end: 0,
    }
}

/// Exact list output, including all child bytes and cumulative ends.
pub open spec fn output(before: Seq<u8>, items: Seq<Item>, width: Option<u32>) -> Seq<u8> {
    product::paint_all(product::splice(before, 0, product::word(items.len() as int)),
        initial(items, width), items)
}

/// Additive byte extents do not depend on sequence grouping.
pub proof fn extent_concat(left: Seq<Item>, right: Seq<Item>)
    ensures product::extent(left + right) == product::extent(left) + product::extent(right),
    decreases left.len(),
{
    if left.len() > 0 {
        assert((left + right).drop_first() =~= left.drop_first() + right);
        extent_concat(left.drop_first(), right);
    }
}

/// Prefix sums stay bounded by the complete borrowed sequence.
pub proof fn extent_prefix(items: Seq<Item>, index: int)
    requires 0 <= index <= items.len(),
    ensures product::extent(items.take(index)) <= product::extent(items),
        index < items.len() ==> product::extent(items.take(index + 1))
            == product::extent(items.take(index)) + items[index].bytes.len(),
{
    assert(items =~= items.take(index) + items.skip(index));
    extent_concat(items.take(index), items.skip(index));
    if index < items.len() {
        assert(items.take(index + 1) =~= items.take(index) + seq![items[index]]);
        extent_concat(items.take(index), seq![items[index]]);
        reveal_with_fuel(product::extent, 2);
    }
}

/// Canonical arithmetic failure before a destination loan is created.
pub open spec fn overflow() -> Result<u32> {
    Err(Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: u32::MAX })
}

/// First child error or first overflowing cumulative extent in declaration order.
pub open spec fn fold<E: Elements>(source: &E, index: int, total: u32) -> Result<u32>
    decreases source.items().len() - index,
{
    if index >= source.items().len() { Ok(total) }
    else { match source.measurement(index) {
        Err(error) => Err(error),
        Ok(value) => if total + value > u32::MAX { overflow() }
            else { fold(source, index + 1, (total + value) as u32) },
    } }
}

/// Complete measurement observation, including count/directory rejection order.
pub open spec fn measured<E: Elements>(source: &E, indexed: bool, stride: Option<u32>) -> Result<u32> {
    let count = source.items().len();
    let base = 4 + directory(count as int, indexed);
    if count > u32::MAX { overflow() }
    else if stride.is_some() {
        if 4 + count * stride.unwrap() > u32::MAX { overflow() }
        else { Ok((4 + count * stride.unwrap()) as u32) }
    } else if base > u32::MAX { overflow() }
    else { fold(source, 0, base as u32) }
}

/// Checked list measurement, retaining every existing child call and first error.
///
/// `stride` selects the retained fixed-child width check. It changes arithmetic
/// placement only; it never elides child measurements.
#[inline(always)]
pub fn measure<E: Elements>(source: &E, indexed: bool, stride: Option<u32>) -> (result: Result<u32>)
    requires source.trusted_input(),
        stride.is_some() ==> !indexed && uniform(source.items(), stride),
    ensures result == measured(source, indexed, stride),
        result.is_ok() == (source.items().len() <= u32::MAX
            && size(source.items(), indexed) <= u32::MAX),
        result.is_ok() ==> result.unwrap() == size(source.items(), indexed),
    no_unwind
{
    let length = source.count();
    if length > u32::MAX as usize { return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)); }
    let count = length as u32;
    if let Some(width) = stride {
        let mut index = 0;
        while index < length
            invariant source.trusted_input(), length == source.items().len(),
                index <= length, uniform(source.items(), stride), stride == Some(width),
            decreases length - index,
        {
            let child = source.at(index);
            let measured = match child.measure() { Ok(n) => n, Err(e) => return Err(e) };
            if measured != width { return Err(Error::new(ErrorKind::InvalidBoundary, Error::NO_OFFSET)); }
            index += 1;
        }
        proof { uniform_extent(source.items(), width); }
        let payload = match count.checked_mul(width) {
            Some(n) => n, None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
        };
        return match payload.checked_add(4) {
            Some(n) => Ok(n), None => Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
        };
    }
    let words = if indexed && count > 0 { count - 1 } else { 0 };
    let ends = match words.checked_mul(4) {
        Some(n) => n, None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    let mut total = match ends.checked_add(4) {
        Some(n) => n, None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    proof { reveal_with_fuel(fold, 2); }
    let mut index = 0;
    while index < length
        invariant source.trusted_input(), length == source.items().len(), length <= u32::MAX,
            index <= length, fold(source, index as int, total) == measured(source, indexed, stride),
            total == 4 + directory(length as int, indexed)
                + product::extent(source.items().take(index as int)),
        decreases length - index,
    {
        proof {
            extent_prefix(source.items(), index as int);
            extent_prefix(source.items(), index as int + 1);
            reveal_with_fuel(fold, 2);
        }
        let child = source.at(index);
        let measured = match child.measure() { Ok(n) => n, Err(e) => return Err(e) };
        total = match total.checked_add(measured) {
            Some(n) => n, None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
        };
        index += 1;
    }
    proof { assert(source.items().take(length as int) =~= source.items()); }
    Ok(total)
}

/// A uniform fixed sequence has count times its child width.
pub proof fn uniform_extent(items: Seq<Item>, width: u32)
    requires uniform(items, Some(width)),
    ensures product::extent(items) == items.len() * width,
    decreases items.len(),
{
    if items.len() > 0 {
        assert(uniform(items.drop_first(), Some(width)));
        uniform_extent(items.drop_first(), width);
        assert(items.len() * width == width + (items.len() - 1) * width) by(nonlinear_arith);
    }
}

/// Writes one fresh measurement into its exact element interval.
#[inline(always)]
fn write_one<S: Source>(source: &S, destination: &mut [u8], cursor: Cursor, width: Option<u32>, check_width: bool, reuse_fixed: bool) -> (result: Cursor)
    requires source.trusted_input(), source.fixed() == width.is_some(),
        width.is_some() ==> source.bytes().len() == width.unwrap(),
        product::fits(cursor, old(destination).len() as int,
            Item { bytes: source.bytes(), fixed: source.fixed(), layout: source.layout() }),
    ensures final(destination).len() == old(destination).len(),
        result == product::next(cursor, Item { bytes: source.bytes(), fixed: source.fixed(), layout: source.layout() }),
        final(destination)@ == product::paint(old(destination)@, cursor,
            Item { bytes: source.bytes(), fixed: source.fixed(), layout: source.layout() }),
        product::geometry(result, final(destination).len() as int),
    no_unwind
{
    let size = if let (true, Some(fixed)) = (reuse_fixed, width) { fixed as usize } else { match source.measure() {
        Ok(n) => n as usize,
        Err(error) => {
            if check_width { panic!("previously measured list input: {:?}", error); }
            else { panic!("previously measured owned list: {:?}", error); }
        },
    } };
    let ghost before = destination@;
    let ghost item = Item { bytes: source.bytes(), fixed: source.fixed(), layout: source.layout() };
    let mut result = cursor;
    if let Some(fixed) = width {
        if check_width && fixed != 0 && size != fixed as usize { panic!("fresh fixed list width"); }
        let end = crate::words::add_bounded(cursor.fixed, size);
        let (prefix, _) = destination.split_at_mut(end);
        let (_, child) = prefix.split_at_mut(cursor.fixed);
        source.encode(child);
        result.fixed = crate::words::add_bounded(cursor.fixed, size);
    } else {
        let end = crate::words::add_bounded(cursor.payload, size);
        let (prefix, _) = destination.split_at_mut(end);
        let (_, child) = prefix.split_at_mut(cursor.payload);
        source.encode(child);
        proof { assert(destination@ == product::splice(before, cursor.payload as int, item.bytes)); }
        result.payload = crate::words::add_bounded(cursor.payload, size);
        result.end = crate::words::add_bounded(cursor.end as usize, size) as u32;
        if cursor.directory < cursor.directory_limit {
            let encoded = [result.end as u8, (result.end / 256) as u8,
                (result.end / 65536) as u8, (result.end / 16777216) as u8];
            let (_, rest) = destination.split_at_mut(cursor.directory);
            let (slot, _) = rest.split_at_mut(4);
            crate::encoding_leaf::encode_array(&encoded, slot);
            result.directory = crate::words::add_bounded(cursor.directory, 4);
        }
    }
    proof { crate::encoding_struct::paint_once(before, cursor, item); }
    result
}

/// Exact list writer for arbitrary borrowed child projections.
///
/// # Panics
/// The destination must have its previously measured extent. Each child must
/// satisfy its codec contract; opaque codecs retain the same fresh calls.
#[inline(always)]
pub fn encode<E: Elements>(source: &E, destination: &mut [u8], width: Option<u32>, check_width: bool)
    requires source.trusted_input(), uniform(source.items(), width),
        source.items().len() <= u32::MAX,
        old(destination).len() == size(source.items(), width.is_none()) <= u32::MAX,
    ensures final(destination)@ == output(old(destination)@, source.items(), width),
    no_unwind
{ encode_with_replay(source, destination, width, check_width, false); }

/// Writes a covered fixed collection using its already measured stride.
///
/// # Panics
/// The complete graph must satisfy the trusted replay contract and the exact
/// destination and uniform-width preconditions of [`encode`].
#[inline(always)]
pub fn encode_fixed<E: Elements>(source: &E, destination: &mut [u8], width: u32)
    requires source.trusted_input(), uniform(source.items(), Some(width)),
        source.items().len() <= u32::MAX,
        old(destination).len() == size(source.items(), false) <= u32::MAX,
    ensures final(destination)@ == output(old(destination)@, source.items(), Some(width)),
    no_unwind
{ encode_with_replay(source, destination, Some(width), false, true); }

#[inline(always)]
fn encode_with_replay<E: Elements>(source: &E, destination: &mut [u8], width: Option<u32>, check_width: bool, reuse_fixed: bool)
    requires source.trusted_input(), uniform(source.items(), width),
        source.items().len() <= u32::MAX,
        old(destination).len() == size(source.items(), width.is_none()) <= u32::MAX,
    ensures final(destination)@ == output(old(destination)@, source.items(), width),
    no_unwind
{
    let length = source.count();
    let count = length as u32;
    let encoded = [count as u8, (count / 256) as u8, (count / 65536) as u8, (count / 16777216) as u8];
    let (header, _) = destination.split_at_mut(4);
    crate::encoding_leaf::encode_array(&encoded, header);
    let ghost counted = destination@;
    proof {
        assert(counted == product::splice(old(destination)@, 0, product::word(length as int)));
        initial_fits(source.items(), width);
    }
    let ends = if width.is_none() { length.saturating_sub(1) * 4 } else { 0 };
    let payload = 4 + ends;
    if let Some(fixed) = width {
        if check_width && fixed != 0 {
            proof {
                uniform_extent(source.items(), fixed);
                vstd::arithmetic::div_mod::lemma_div_multiples_vanish(length as int, fixed as int);
                assert(fixed as int * length == length as int * fixed as int) by(nonlinear_arith);
                assert(destination.len() - 4 == fixed as int * length);
                assert((destination.len() as int - 4) / fixed as int == length);
            }
            if (destination.len() - 4) / fixed as usize != length { panic!("measured list extent"); }
        }
    }
    let mut cursor = Cursor {
        fixed: 4, fixed_limit: if width.is_some() { destination.len() } else { 4 },
        directory: if width.is_some() { destination.len() } else { 4 },
        directory_limit: if width.is_some() { destination.len() } else { payload },
        payload: if width.is_some() { destination.len() } else { payload }, end: 0,
    };
    proof {
        assert(cursor == initial(source.items(), width));
        assert(source.items().skip(0) =~= source.items());
    }
    let mut index = 0;
    while index < length
        invariant source.trusted_input(), uniform(source.items(), width),
            length == source.items().len(), index <= length,
            destination.len() == size(source.items(), width.is_none()) <= u32::MAX,
            cursor.fixed_limit == initial(source.items(), width).fixed_limit,
            product::geometry(cursor, destination.len() as int),
            product::advance(cursor, source.items().skip(index as int))
                == product::advance(initial(source.items(), width), source.items()),
            product::fits_all(cursor, destination.len() as int, source.items().skip(index as int)),
            product::paint_all(destination@, cursor, source.items().skip(index as int))
                == product::paint_all(counted, initial(source.items(), width), source.items()),
        decreases length - index,
    {
        let child = source.at(index);
        proof {
            assert(source.items().skip(index as int)[0] == source.items()[index as int]);
            reveal_with_fuel(product::fits_all, 2);
        }
        proof {
            reveal_with_fuel(product::advance, 2);
            reveal_with_fuel(product::paint_all, 2);
            assert(source.items().skip(index as int).drop_first() =~= source.items().skip(index as int + 1));
        }
        cursor = write_one(&child, destination, cursor, width, check_width, reuse_fixed);
        index += 1;
    }
    proof {
        initial_fits(source.items(), width);
        assert(source.items().skip(length as int) =~= Seq::<Item>::empty());
        reveal_with_fuel(product::advance, 1);
    }
    if cursor.fixed != cursor.fixed_limit || cursor.payload != destination.len() {
        panic!("destination must have measured length");
    }
}

/// Exact list geometry follows from representable measurement and uniform storage.
pub proof fn initial_fits(items: Seq<Item>, width: Option<u32>)
    requires uniform(items, width), size(items, width.is_none()) <= u32::MAX,
    ensures product::geometry(initial(items, width), size(items, width.is_none())),
        product::fits_all(initial(items, width), size(items, width.is_none()), items),
        product::advance(initial(items, width), items).fixed == initial(items, width).fixed_limit,
        product::advance(initial(items, width), items).payload == size(items, width.is_none()),
{
    uniform_sums(items, width);
    crate::encoding_struct::packing_fits(items, initial(items, width), size(items, width.is_none()));
}

/// Product summaries specialize to the list's uniform storage class.
pub proof fn uniform_sums(items: Seq<Item>, width: Option<u32>)
    requires uniform(items, width),
    ensures crate::encoding_struct::fixed(items) == if width.is_some() { product::extent(items) } else { 0nat },
        crate::encoding_struct::variable(items) == if width.is_some() { 0nat } else { product::extent(items) },
        crate::encoding_struct::variables(items) == if width.is_some() { 0nat } else { items.len() },
    decreases items.len(),
{
    if items.len() > 0 {
        assert(uniform(items.drop_first(), width));
        uniform_sums(items.drop_first(), width);
    }
}

}
