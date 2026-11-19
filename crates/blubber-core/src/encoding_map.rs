//! Borrowed map traversal, ordered preflight and exact key/value loans.
#![allow(clippy::question_mark)]

#[cfg(verus_only)]
use crate::encoding_product as product;
use crate::encoding_product::Source;
use crate::{Error, ErrorKind, Result};
use core::cmp::Ordering;
use vstd::prelude::*;

verus! {

/// One projected map entry, without an allocated runtime mirror.
pub ghost struct Entry {
    /// Canonical key bytes supplied by the selected key type.
    pub key: Seq<u8>,
    /// Exact value bytes supplied by the child source.
    pub value: Seq<u8>,
}

/// Immutable key/value projections and retained key operations.
///
/// Library key implementations supply canonical bytes and pure Rust-order
/// agreement. A true handwritten key marker promises the same contract;
/// default-false keys retain all comparisons, writes and publication checks.
pub trait Entries {
    /// Borrowed value projection.
    type Child: Source;
    /// Declaration-order key/value byte views.
    spec fn entries(&self) -> Seq<Entry>;
    /// Stable child measurement observation.
    spec fn measurement(&self, index: int) -> Result<u32>;
    /// Stable adjacent comparison observation.
    spec fn comparison(&self, index: int) -> Ordering;
    /// Trusted projection, child and key implementer contracts.
    spec fn trusted_input(&self) -> bool;

    /// Reads the entry count without callbacks.
    fn count(&self) -> (count: usize)
        ensures count == self.entries().len(),
        no_unwind;

    /// Borrows one value without running its codec.
    fn value(&self, index: usize) -> (child: Self::Child)
        requires self.trusted_input(), index < self.entries().len(),
        ensures child.trusted_input(), child.bytes() == self.entries()[index as int].value,
            child.measurement() == self.measurement(index as int),
        no_unwind;

    /// Runs the original adjacent Rust key comparison.
    fn compare(&self, index: usize) -> (result: Ordering)
        requires self.trusted_input(), 0 < index < self.entries().len(),
        ensures result == self.comparison(index as int),
            crate::search::lex_total(result, self.entries()[index as int - 1].key,
                self.entries()[index as int].key),
        no_unwind;

    /// Writes the selected canonical key into its exact loan.
    fn write_key(&self, index: usize, destination: &mut [u8])
        requires self.trusted_input(), index < self.entries().len(),
            old(destination).len() == self.entries()[index as int].key.len(),
        ensures final(destination)@ == self.entries()[index as int].key,
        no_unwind;
}

/// Complete value payload extent for an arbitrary entry sequence.
pub open spec fn extent(entries: Seq<Entry>) -> nat
    decreases entries.len(),
{
    if entries.len() == 0 { 0 } else { entries[0].value.len() + extent(entries.drop_first()) }
}

/// Header and key/end bytes before the value extents are added.
pub open spec fn base(count: int, key: u32, indexed: bool) -> int {
    4 + count * (key as int + if indexed { 4int } else { 0int })
}

/// Complete wire extent for fixed or indexed entries.
pub open spec fn size(entries: Seq<Entry>, key: u32, indexed: bool) -> int {
    base(entries.len() as int, key, indexed) + extent(entries)
}

/// Uniform key width and optional fixed value width.
pub open spec fn uniform(entries: Seq<Entry>, key: u32, width: Option<u32>) -> bool {
    key > 0 && forall|i: int| 0 <= i < entries.len() ==>
        entries[i].key.len() == key
        && (width.is_some() ==> entries[i].value.len() == width.unwrap())
}

/// Value extents distribute over sequence concatenation.
pub proof fn extent_concat(left: Seq<Entry>, right: Seq<Entry>)
    ensures extent(left + right) == extent(left) + extent(right),
    decreases left.len(),
{
    if left.len() > 0 {
        assert((left + right).drop_first() =~= left.drop_first() + right);
        extent_concat(left.drop_first(), right);
    }
}

/// Prefix extents stay within the complete payload.
pub proof fn prefix(entries: Seq<Entry>, index: int)
    requires 0 <= index <= entries.len(),
    ensures extent(entries.take(index)) <= extent(entries),
        index < entries.len() ==> extent(entries.take(index + 1))
            == extent(entries.take(index)) + entries[index].value.len(),
{
    assert(entries =~= entries.take(index) + entries.skip(index));
    extent_concat(entries.take(index), entries.skip(index));
    if index < entries.len() {
        assert(entries.take(index + 1) =~= entries.take(index) + seq![entries[index]]);
        extent_concat(entries.take(index), seq![entries[index]]);
        reveal_with_fuel(extent, 2);
    }
}

/// Fixed value extents equal count times stride.
pub proof fn fixed_extent(entries: Seq<Entry>, width: u32)
    requires forall|i: int| 0 <= i < entries.len() ==> entries[i].value.len() == width,
    ensures extent(entries) == entries.len() * width,
    decreases entries.len(),
{
    if entries.len() > 0 {
        assert forall|i: int| 0 <= i < entries.drop_first().len() implies
            entries.drop_first()[i].value.len() == width by { }
        fixed_extent(entries.drop_first(), width);
        assert(entries.len() * width == width + (entries.len() - 1) * width) by(nonlinear_arith);
    }
}

/// Canonical key-order error, before the current value is measured.
pub open spec fn order_error(order: Ordering) -> Option<Error> {
    match order {
        Ordering::Less => None,
        Ordering::Equal => Some(Error::Fault { kind: ErrorKind::DuplicateKey, offset: u32::MAX }),
        Ordering::Greater => Some(Error::Fault { kind: ErrorKind::UnsortedKeys, offset: u32::MAX }),
    }
}

/// One comparison, value measurement and checked extent in retained order.
pub open spec fn step<E: Entries>(source: &E, index: int, total: u32, fixed: Option<u32>) -> Result<u32> {
    if index > 0 && order_error(source.comparison(index)).is_some() {
        Err(order_error(source.comparison(index)).unwrap())
    } else { match source.measurement(index) {
        Err(error) => Err(error),
        Ok(n) => if fixed.is_some() {
            if n != fixed.unwrap() { Err(Error::Fault { kind: ErrorKind::InvalidBoundary, offset: u32::MAX }) }
            else { Ok(total) }
        } else if total + n > u32::MAX { crate::encoding_list::overflow() }
        else { Ok((total + n) as u32) },
    } }
}

/// Retained left-to-right preflight observation, including cumulative overflow.
pub open spec fn fold<E: Entries>(source: &E, index: int, total: u32, fixed: Option<u32>) -> Result<u32>
    decreases source.entries().len() - index,
{
    if index >= source.entries().len() { Ok(total) }
    else { match step(source, index, total, fixed) {
        Err(error) => Err(error), Ok(next) => fold(source, index + 1, next, fixed),
    } }
}

/// Complete measurement observation, with count and directory errors first.
pub open spec fn measured<E: Entries>(source: &E, key: u32, indexed: bool, fixed: Option<u32>) -> Result<u32> {
    let count = source.entries().len();
    let start = base(count as int, key, indexed);
    if count > u32::MAX { crate::encoding_list::overflow() }
    else if count == 0 { Ok(4) }
    else if key as int + (if indexed { 4int } else { 0int }) > u32::MAX || start > u32::MAX {
        crate::encoding_list::overflow()
    } else { match fold(source, 0, start as u32, fixed) {
        Err(error) => Err(error),
        Ok(n) => if fixed.is_some() {
            if count * fixed.unwrap() + n > u32::MAX { crate::encoding_list::overflow() }
            else { Ok((count * fixed.unwrap() + n) as u32) }
        } else { Ok(n) },
    } }
}

/// Checks every adjacent key and child in the retained first-fault order.
#[inline(always)]
pub fn measure<E: Entries>(source: &E, key: u32, indexed: bool, fixed: Option<u32>) -> (result: Result<u32>)
    requires source.trusted_input(),
    ensures result == measured(source, key, indexed, fixed),
        result.is_ok() ==> result.unwrap() == size(source.entries(), key, indexed),
        result.is_ok() ==> forall|i: int| 0 < i < source.entries().len() ==>
            crate::search::lex_less((#[trigger] source.entries()[i - 1]).key, (#[trigger] source.entries()[i]).key),
    no_unwind
{
    let length = source.count();
    if length > u32::MAX as usize { return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)); }
    let count = length as u32;
    if count == 0 { return Ok(4); }
    let stride = match key.checked_add(if indexed { 4u32 } else { 0u32 }) {
        Some(n) => n, None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    let directory = match count.checked_mul(stride) {
        Some(n) => n, None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    let mut total = match directory.checked_add(4) {
        Some(n) => n, None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
    };
    let ghost start = total;
    proof { reveal_with_fuel(fold, 2); }
    let mut index = 0;
    while index < length
        invariant source.trusted_input(), length == source.entries().len(), length <= u32::MAX,
            length > 0, key as int + (if indexed { 4int } else { 0int }) <= u32::MAX,
            index <= length, start == base(length as int, key, indexed),
            fold(source, index as int, total, fixed) == fold(source, 0, start, fixed),
            total == start + if fixed.is_some() { 0int } else { extent(source.entries().take(index as int)) as int },
            fixed.is_some() ==> forall|i: int| 0 <= i < index ==> source.entries()[i].value.len() == fixed.unwrap(),
            forall|i: int| 0 < i < index ==>
                crate::search::lex_less((#[trigger] source.entries()[i - 1]).key, (#[trigger] source.entries()[i]).key),
        decreases length - index,
    {
        proof { reveal_with_fuel(fold, 2); prefix(source.entries(), index as int); }
        if index > 0 {
            let order = source.compare(index);
            match order {
                Ordering::Less => {},
                Ordering::Equal => return Err(Error::new(ErrorKind::DuplicateKey, Error::NO_OFFSET)),
                Ordering::Greater => return Err(Error::new(ErrorKind::UnsortedKeys, Error::NO_OFFSET)),
            }
        }
        proof { assert(index == 0 || order_error(source.comparison(index as int)).is_none()); }
        let child = source.value(index);
        let fresh = match child.measure() { Ok(n) => n, Err(error) => return Err(error) };
        if let Some(width) = fixed {
            if fresh != width { return Err(Error::new(ErrorKind::InvalidBoundary, Error::NO_OFFSET)); }
        } else {
            total = match total.checked_add(fresh) {
                Some(n) => n, None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
            };
        }
        index += 1;
    }
    proof { assert(source.entries().take(length as int) =~= source.entries()); }
    if let Some(width) = fixed {
        proof { fixed_extent(source.entries(), width); }
        let values = match count.checked_mul(width) {
            Some(n) => n, None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
        };
        total = match total.checked_add(values) {
            Some(n) => n, None => return Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET)),
        };
    }
    Ok(total)
}
/// Key coordinate for either physical map layout.
pub open spec fn key_start(index: int, key: u32, width: Option<u32>) -> int {
    4 + index * (key as int + if width.is_some() { width.unwrap() as int } else { 4int })
}

/// Value coordinate, including empty values in indexed payloads.
pub open spec fn value_start(entries: Seq<Entry>, index: int, key: u32, width: Option<u32>) -> int {
    if width.is_some() { key_start(index, key, width) + key }
    else { base(entries.len() as int, key, true) + extent(entries.take(index)) }
}

/// Exact key, value and optional cumulative end for one completed entry.
pub open spec fn entry_written(bytes: Seq<u8>, entries: Seq<Entry>, key: u32, width: Option<u32>, i: int) -> bool {
    let k = key_start(i, key, width);
    let v = value_start(entries, i, key, width);
    &&& bytes.subrange(k, k + key) == entries[i].key
    &&& bytes.subrange(v, v + entries[i].value.len()) == entries[i].value
    &&& width.is_none() ==> crate::words::le_word(bytes, k + key) == extent(entries.take(i + 1))
}

/// Exact completed entries, independent of destination history.
pub open spec fn written(bytes: Seq<u8>, entries: Seq<Entry>, key: u32, width: Option<u32>, count: int) -> bool {
    forall|i: int| 0 <= i < count ==> #[trigger] entry_written(bytes, entries, key, width, i)
}

/// Representable measurement gives exact, disjoint key/value/end intervals.
pub proof fn regions(entries: Seq<Entry>, key: u32, width: Option<u32>, i: int)
    requires uniform(entries, key, width), 0 <= i < entries.len(),
        size(entries, key, width.is_none()) <= u32::MAX,
    ensures 4 <= key_start(i, key, width),
        key_start(i, key, width) + key <= value_start(entries, i, key, width),
        value_start(entries, i, key, width) + entries[i].value.len()
            <= size(entries, key, width.is_none()),
        key_start(i + 1, key, width) == key_start(i, key, width) + key
            + if width.is_some() { width.unwrap() as int } else { 4int },
        width.is_none() ==> key_start(i, key, width) + key + 4 <= base(entries.len() as int, key, true),
        width.is_none() ==> value_start(entries, i + 1, key, width)
            == value_start(entries, i, key, width) + entries[i].value.len(),
{
    prefix(entries, i); prefix(entries, i + 1);
    if let Some(fixed) = width {
        fixed_extent(entries, fixed);
        assert(i * (key as int + fixed as int) == i * key + i * fixed) by(nonlinear_arith);
        assert((i + 1) * (key as int + fixed as int) == i * (key as int + fixed as int) + key + fixed) by(nonlinear_arith);
        assert(i * (key as int + fixed as int) + key + fixed <= entries.len() * (key as int + fixed as int)) by(nonlinear_arith)
            requires i < entries.len(), key > 0;
        assert(entries.len() * (key as int + fixed as int) == entries.len() * key + entries.len() * fixed) by(nonlinear_arith);
    } else {
        assert((i + 1) * (key as int + 4) == i * (key as int + 4) + key + 4) by(nonlinear_arith);
        assert(i * (key as int + 4) + key + 4 <= entries.len() * (key as int + 4)) by(nonlinear_arith)
            requires i < entries.len(), key > 0;
    }
}

/// Exact one-entry write and its complete surrounding byte frame.
pub open spec fn entry_write(before: Seq<u8>, entries: Seq<Entry>, key: u32,
    width: Option<u32>, index: int) -> Seq<u8> {
    let k = key_start(index, key, width);
    let v = value_start(entries, index, key, width);
    let keyed = product::splice(before, k, entries[index].key);
    let valued = product::splice(keyed, v, entries[index].value);
    if width.is_some() { valued }
    else { product::splice(valued, k + key, product::word(extent(entries.take(index + 1)) as int)) }
}

/// Writes one entry, retaining the borrowed/owned callback order.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn write_entry<E: Entries>(source: &E, destination: &mut [u8], index: usize,
    key_at: usize, value_at: usize, key: u32, width: Option<u32>, owned: bool, reuse_fixed: bool) -> (length: usize)
    requires source.trusted_input(), uniform(source.entries(), key, width),
        index < source.entries().len(),
        old(destination).len() == size(source.entries(), key, width.is_none()) <= u32::MAX,
        key_at == key_start(index as int, key, width),
        value_at == value_start(source.entries(), index as int, key, width),
    ensures length == source.entries()[index as int].value.len(),
        final(destination)@ == entry_write(old(destination)@, source.entries(), key, width, index as int),
    no_unwind
{
    proof { regions(source.entries(), key, width, index as int); }
    let ghost before = destination@;
    let child = source.value(index);
    let mut length = if let (true, Some(fixed)) = (reuse_fixed, width) { fixed as usize } else { 0usize };
    if !reuse_fixed && width.is_some() && !owned {
        length = match child.measure() { Ok(n) => n as usize, Err(error) => {
                if owned { panic!("previously measured owned map: {:?}", error); }
                else { panic!("previously measured map input: {:?}", error); }
            } };
        if let Some(fixed) = width {
            if fixed != 0 && length != fixed as usize { panic!("fresh fixed map width"); }
        }
    }
    let key_end = crate::words::add_bounded(key_at, key as usize);
    let (prefix, _) = destination.split_at_mut(key_end);
    let (_, slot) = prefix.split_at_mut(key_at);
    source.write_key(index, slot);
    proof { assert(destination@ == product::splice(before, key_at as int, source.entries()[index as int].key)); }
    let ghost keyed = destination@;
    if width.is_none() || (owned && !reuse_fixed) {
        length = match child.measure() { Ok(n) => n as usize, Err(error) => {
                if owned { panic!("previously measured owned map: {:?}", error); }
                else { panic!("previously measured map input: {:?}", error); }
            } };
    }
    let end = crate::words::add_bounded(value_at, length);
    let (prefix, _) = destination.split_at_mut(end);
    let (_, slot) = prefix.split_at_mut(value_at);
    child.encode(slot);
    proof { assert(destination@ == product::splice(keyed, value_at as int, child.bytes())); }
    if width.is_none() {
        // The cumulative end uses the value coordinate, without another traversal.
        let payload = 4 + source.count() * (key as usize + 4);
        let finish = (end - payload) as u32;
        let word = [finish as u8, (finish / 256) as u8, (finish / 65536) as u8, (finish / 16777216) as u8];
        let (prefix, _) = destination.split_at_mut(key_end + 4);
        let (_, slot) = prefix.split_at_mut(key_end);
        crate::encoding_leaf::encode_array(&word, slot);
    }
    length
}
/// One exact write preserves the completed entries and the count header.
pub proof fn progress(before: Seq<u8>, entries: Seq<Entry>, key: u32, width: Option<u32>, index: int)
    requires uniform(entries, key, width), 0 <= index < entries.len(),
        before.len() == size(entries, key, width.is_none()) <= u32::MAX,
        written(before, entries, key, width, index),
    ensures entry_write(before, entries, key, width, index).len() == before.len(),
        entry_write(before, entries, key, width, index).subrange(0, 4) == before.subrange(0, 4),
        written(entry_write(before, entries, key, width, index), entries, key, width, index + 1),
{
    regions(entries, key, width, index);
    let after = entry_write(before, entries, key, width, index);
    let k = key_start(index, key, width);
    let v = value_start(entries, index, key, width);
    assert(after.subrange(0, 4) =~= before.subrange(0, 4));
    if width.is_none() {
        crate::encoding_struct::encoded_word(extent(entries.take(index + 1)) as u32);
        let word = product::word(extent(entries.take(index + 1)) as int);
        assert(after.subrange(k + key, k + key + 4) =~= word);
        crate::words::word_subrange(after, k + key, k + key + 4, 0);
    }
    assert forall|j: int| 0 <= j < index + 1 implies
        #[trigger] entry_written(after, entries, key, width, j) by {
        let a = key_start(j, key, width);
        let b = value_start(entries, j, key, width);
        regions(entries, key, width, j);
        if j == index {
            assert(after.subrange(k, k + key) =~= entries[j].key);
            assert(after.subrange(v, v + entries[j].value.len()) =~= entries[j].value);
        } else {
            assert(entry_written(before, entries, key, width, j));
            if let Some(fixed) = width {
                assert(j * (key as int + fixed as int) + key + fixed <= index * (key as int + fixed as int)) by(nonlinear_arith)
                    requires j < index, key > 0;
            } else {
                assert(j * (key as int + 4) + key + 4 <= index * (key as int + 4)) by(nonlinear_arith)
                    requires j < index, key > 0;
                assert(entries.take(index) =~= entries.take(j + 1) + entries.subrange(j + 1, index));
                extent_concat(entries.take(j + 1), entries.subrange(j + 1, index));
                assert(value_start(entries, j, key, width) + entries[j].value.len() <= v);
                assert(after.subrange(a + key, a + key + 4) =~= before.subrange(a + key, a + key + 4));
                crate::words::word_subrange(after, a + key, a + key + 4, 0);
                crate::words::word_subrange(before, a + key, a + key + 4, 0);
            }
            assert(after.subrange(a, a + key) =~= before.subrange(a, a + key));
            assert(after.subrange(b, b + entries[j].value.len()) =~= before.subrange(b, b + entries[j].value.len()));
        }
    }
}

/// Writes all map entries into their exact retained key/value/end regions.
///
/// # Panics
/// The destination must have its previously measured extent and each projected
/// key/value must satisfy its implementer contract. Opaque callbacks keep their
/// original borrowed/owned ordering and full checked publication protocol.
#[inline(always)]
pub fn encode<E: Entries>(source: &E, destination: &mut [u8], key: u32, width: Option<u32>, owned: bool)
    requires source.trusted_input(), uniform(source.entries(), key, width),
        source.entries().len() <= u32::MAX,
        old(destination).len() == size(source.entries(), key, width.is_none()) <= u32::MAX,
    ensures final(destination).len() == old(destination).len(),
        crate::words::le_word(final(destination)@, 0) == source.entries().len(),
        written(final(destination)@, source.entries(), key, width, source.entries().len() as int),
    no_unwind
{ encode_with_replay(source, destination, key, width, owned, false); }

/// Writes a covered fixed collection using its already measured stride.
///
/// # Panics
/// The complete graph must satisfy the trusted replay contract and the exact
/// destination and uniform-width preconditions of [`encode`].
#[inline(always)]
pub fn encode_fixed<E: Entries>(source: &E, destination: &mut [u8], key: u32, width: u32)
    requires source.trusted_input(), uniform(source.entries(), key, Some(width)),
        source.entries().len() <= u32::MAX,
        old(destination).len() == size(source.entries(), key, false) <= u32::MAX,
    ensures final(destination).len() == old(destination).len(),
        crate::words::le_word(final(destination)@, 0) == source.entries().len(),
        written(final(destination)@, source.entries(), key, Some(width), source.entries().len() as int),
    no_unwind
{ encode_with_replay(source, destination, key, Some(width), false, true); }

#[inline(always)]
fn encode_with_replay<E: Entries>(source: &E, destination: &mut [u8], key: u32, width: Option<u32>, owned: bool, reuse_fixed: bool)
    requires source.trusted_input(), uniform(source.entries(), key, width),
        source.entries().len() <= u32::MAX,
        old(destination).len() == size(source.entries(), key, width.is_none()) <= u32::MAX,
    ensures final(destination).len() == old(destination).len(),
        crate::words::le_word(final(destination)@, 0) == source.entries().len(),
        written(final(destination)@, source.entries(), key, width, source.entries().len() as int),
    no_unwind
{
    let length = source.count();
    let count = length as u32;
    let word = [count as u8, (count / 256) as u8, (count / 65536) as u8, (count / 16777216) as u8];
    let (header, _) = destination.split_at_mut(4);
    crate::encoding_leaf::encode_array(&word, header);
    proof { crate::encoding_struct::encoded_word(count); crate::words::word_subrange(destination@, 0, 4, 0); }
    if length == 0 { return; }
    proof {
        if width.is_none() {
            assert(key as int + 4 <= length * (key as int + 4)) by(nonlinear_arith)
                requires length > 0, key > 0;
        }
    }
    if let Some(fixed) = width {
        if fixed > 0 && !owned {
            proof {
                fixed_extent(source.entries(), fixed);
                let stride = key as int + fixed as int;
                assert(length * key + length * fixed == length * stride) by(nonlinear_arith)
                    requires stride == key as int + fixed as int;
                assert(key as int + fixed as int <= length * (key as int + fixed as int)) by(nonlinear_arith)
                    requires length > 0, key > 0;
                vstd::arithmetic::div_mod::lemma_div_multiples_vanish(length as int, stride);
                assert(stride * length == length * stride) by(nonlinear_arith);
                assert(destination.len() - 4 == length * stride);
                assert((destination.len() as int - 4) / stride == length);
            }
            let stride = key as usize + fixed as usize;
            if (destination.len() - 4) / stride != length { panic!("measured map extent"); }
        }
    }
    proof { if let Some(fixed) = width { fixed_extent(source.entries(), fixed);
        assert(length * (key as int + fixed as int) == length * key + length * fixed) by(nonlinear_arith); } }
    let mut index = 0;
    let mut key_at = 4usize;
    let mut payload = if width.is_none() { 4 + length * (key as usize + 4) } else { 4 };
    while index < length
        invariant source.trusted_input(), uniform(source.entries(), key, width),
            length == source.entries().len(), length <= u32::MAX,
            index <= length, key_at == key_start(index as int, key, width),
            width.is_some() ==> size(source.entries(), key, false) == key_start(length as int, key, width),
            width.is_none() ==> payload == value_start(source.entries(), index as int, key, width),
            destination.len() == size(source.entries(), key, width.is_none()) <= u32::MAX,
            crate::words::le_word(destination@, 0) == length,
            written(destination@, source.entries(), key, width, index as int),
        decreases length - index,
    {
        proof { regions(source.entries(), key, width, index as int); }
        let value_at = if width.is_some() { key_at + key as usize } else { payload };
        let ghost before = destination@;
        let size = write_entry(source, destination, index, key_at, value_at, key, width, owned, reuse_fixed);
        proof {
            progress(before, source.entries(), key, width, index as int);
            crate::words::word_subrange(before, 0, 4, 0);
            crate::words::word_subrange(destination@, 0, 4, 0);
        }
        if width.is_some() { key_at = value_at + size; }
        else { key_at += key as usize + 4; payload += size; }
        index += 1;
    }
    proof { assert(source.entries().take(length as int) =~= source.entries()); }
    if width.is_some() {
        if key_at != destination.len() { panic!("destination must have measured length"); }
    } else {
        if payload != destination.len() { panic!("destination must have measured length"); }
    }
}

}
