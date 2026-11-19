//! Header-admitted bulk layout with checked end transport.
#![cfg_attr(verus_only, allow(missing_docs))]
use super::*;
#[cfg(verus_only)]
use crate::list_layout;

verus! {

/// Checked bulk list coordinates, independent of the backing allocation.
#[derive(Clone, Copy)]
pub struct HeaderBulkPlan {
    pub(super) count: u32,
    pub(super) index: u32,
    pub(super) added: u32,
    pub(super) old_payload: u32,
    pub(super) payload: u32,
    pub(super) old_len: u32,
    pub(super) offset: u32,
    pub(super) input_len: u32,
    pub(super) indexed: bool,
    pub(super) width: u32,
    pub(super) len: u32,
}

spec fn legacy(c: HeaderBulkPlan) -> BulkListPlan {
    BulkListPlan { count: c.count, index: c.index, added: c.added, old_payload: c.old_payload,
        payload: c.payload, old_len: c.old_len, offset: c.offset, input_len: c.input_len,
        indexed: c.indexed, width: c.width, len: c.len }
}

pub closed spec fn geometry(bytes: Seq<u8>, c: HeaderBulkPlan) -> bool {
    &&& 4 <= c.old_payload <= c.payload <= c.old_len + (c.payload - c.old_payload) + c.input_len == bytes.len() <= u32::MAX
    &&& c.old_payload <= c.old_len
    &&& c.index <= c.count
    &&& c.count + c.added <= u32::MAX
    &&& c.count == words::le_word(bytes, 0)
    &&& c.offset <= c.old_len - c.old_payload
    &&& c.len == bytes.len()
    &&& c.added == 0 ==> c.input_len == 0
    &&& if c.indexed {
        &&& c.old_payload == 4 + 4 * indexed_remove::words_for(c.count as int)
        &&& c.payload == 4 + 4 * indexed_remove::words_for(c.count + c.added)
        &&& (c.count == 0 ==> c.old_len == 4)
        &&& c.offset == indexed_insert::boundary(bytes, 0, c.old_payload as int, c.old_len as int, c.count as int, c.index as int)
        } else {
        &&& c.old_payload == 4 && c.payload == 4
        &&& c.old_len == 4 + c.count * c.width
        &&& c.offset == c.index * c.width
        &&& c.input_len == c.added * c.width
    }
}

pub(super) open spec fn moved(before: Seq<u8>, after: Seq<u8>, c: HeaderBulkPlan) -> bool {
    let growth = c.payload - c.old_payload + c.input_len;
    &&& before.len() == after.len()
    &&& words::le_word(after, 0) == c.count
    &&& forall|p: int| c.payload <= p < c.payload + c.offset ==> #[trigger] after[p] == before[p - (c.payload - c.old_payload)]
    &&& forall|p: int| c.payload + c.offset + c.input_len <= p < after.len() ==> #[trigger] after[p] == before[p - growth]
    &&& c.indexed ==> {
        &&& forall|k: int| 1 <= k <= c.index && k < c.count ==> #[trigger] words::le_word(after, 4 * k) == words::le_word(before, 4 * k)
        &&& forall|k: int| c.index < k < c.count ==> #[trigger] words::le_word(after, 4 * (k + c.added)) == words::le_word(before, 4 * k) + c.input_len
        &&& c.added > 0 && c.index == c.count && c.count > 0 ==> words::le_word(after, 4 * c.count) == c.old_len - c.old_payload
    }
}

proof fn same_word(before: Seq<u8>, after: Seq<u8>, at: int)
    requires before.len() == after.len(), 0 <= at, at + 4 <= before.len(),
        forall|p: int| at <= p < at + 4 ==> after[p] == before[p],
    ensures words::le_word(after, at) == words::le_word(before, at),
{ }

/// Moves each retained payload range once and shifts old directory ends downward.
#[verifier::spinoff_prover]
#[inline(always)]
fn bulk_move(bytes: &mut [u8], c: HeaderBulkPlan, poisoned: &mut bool) -> (result: Result<()>)
    requires geometry(old(bytes)@, c),
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        result.is_ok() ==> moved(old(bytes)@, final(bytes)@, c),
        source_partition(old(bytes)@, c) ==> result.is_ok(),
    no_unwind
{
    hide(words::le_word);
    reveal(geometry);
    let ghost original = bytes@;
    *poisoned = true;
    let split = c.old_payload + c.offset;
    let table_growth = c.payload - c.old_payload;
    let growth = table_growth + c.input_len;
    if growth != 0 && split < c.old_len {
        match crate::move_bytes(bytes, Span { start: split, end: c.old_len }, split + growth) {
            Ok(()) => (), Err(error) => return Err(error),
        }
    }
    let ghost suffix_moved = bytes@;
    if table_growth != 0 && c.offset != 0 {
        match crate::move_bytes(bytes, Span { start: c.old_payload, end: split }, c.payload) {
            Ok(()) => (), Err(error) => return Err(error),
        }
    }
    let ghost payload_moved = bytes@;
    proof {
        same_word(original, bytes@, 0);
        assert forall|p: int| c.payload <= p < c.payload + c.offset implies
            #[trigger] bytes@[p] == original[p - table_growth] by { }
        assert forall|p: int| c.payload + c.offset + c.input_len <= p < bytes.len() implies
            #[trigger] bytes@[p] == original[p - growth] by { }
        if c.indexed {
            assert forall|k: int| 1 <= k < c.count implies
                #[trigger] words::le_word(bytes@, 4 * k) == words::le_word(original, 4 * k) by {
                same_word(original, bytes@, 4 * k);
            }
        }
    }
    if c.indexed && c.added != 0 {
        let mut boundary = c.count;
        let mut upper = c.old_len - c.old_payload;
        while boundary > c.index + 1
            invariant original == old(bytes)@, geometry(original, c), bytes.len() == original.len(), c.indexed, c.added > 0, *poisoned,
                c.index <= boundary <= c.count, c.index < boundary || c.index == c.count,
                forall|k: int| 1 <= k < boundary ==> #[trigger] words::le_word(bytes@, 4 * k) == words::le_word(original, 4 * k),
                forall|k: int| boundary <= k < c.count ==> #[trigger] words::le_word(bytes@, 4 * (k + c.added)) == words::le_word(original, 4 * k) + c.input_len,
                forall|p: int| c.payload <= p < bytes.len() ==> #[trigger] bytes@[p] == payload_moved[p],
                words::le_word(bytes@, 0) == c.count,
                upper == indexed_insert::boundary(original, 0, c.old_payload as int, c.old_len as int, c.count as int, boundary as int),
                0 <= upper <= c.old_len - c.old_payload,
            decreases boundary,
        {
            boundary -= 1;
            let at = boundary as usize * 4;
            let dest = (boundary as usize + c.added as usize) * 4;
            let ghost previous = bytes@;
            proof {
                if source_partition(original, c) {
                    ordered_boundary(original, c, c.index as int, boundary as int);
                    ordered_boundary(original, c, boundary as int, boundary as int + 1);
                    assert(c.offset <= words::le_word(previous, at as int) <= upper);
                    assert(words::le_word(previous, at as int) + c.input_len <= u32::MAX);
                }
            }
            let transported = crate::patch_pass::transport_contained(bytes, at, dest, c.offset, upper, c.input_len as i64, poisoned);
            upper = match transported { Ok(value) => value, Err(error) => {
                assert(!source_partition(original, c)); return Err(error);
            } };
            proof {
                same_word(previous, bytes@, 0);
                assert forall|k: int| 1 <= k < boundary implies
                    #[trigger] words::le_word(bytes@, 4 * k) == words::le_word(original, 4 * k) by {
                    same_word(previous, bytes@, 4 * k);
                }
                assert forall|k: int| boundary < k < c.count implies
                    #[trigger] words::le_word(bytes@, 4 * (k + c.added)) == words::le_word(original, 4 * k) + c.input_len by {
                    same_word(previous, bytes@, 4 * (k + c.added));
                }
            }
        }
        proof {
            assert forall|k: int| 1 <= k <= c.index && k < c.count implies
                #[trigger] words::le_word(bytes@, 4 * k) == words::le_word(original, 4 * k) by { }
            assert forall|k: int| c.index < k < c.count implies
                #[trigger] words::le_word(bytes@, 4 * (k + c.added)) == words::le_word(original, 4 * k) + c.input_len by { }
        }
        let ghost directory_done = bytes@;
        if c.index == c.count && c.count != 0 {
            match crate::write_u32(bytes, 4 * c.count, c.old_len - c.old_payload) {
                Ok(()) => (), Err(error) => return Err(error),
            }
            proof {
                same_word(directory_done, bytes@, 0);
                assert forall|k: int| 1 <= k < c.count implies
                    #[trigger] words::le_word(bytes@, 4 * k) == words::le_word(original, 4 * k) by {
                    same_word(directory_done, bytes@, 4 * k);
                }
            }
        }
    }
    Ok(())
}

/// Directory validity is a ghost premise of success and refinement, never admission.
pub closed spec fn source_partition(bytes: Seq<u8>, c: HeaderBulkPlan) -> bool {
    !c.indexed || (forall|k: int| 0 <= k < c.count ==> #[trigger] indexed_insert::boundary_step(bytes, 0,
        c.old_payload as int, c.old_len as int, c.count as int, k))
}

proof fn ordered_boundary(bytes: Seq<u8>, c: HeaderBulkPlan, i: int, j: int)
    requires geometry(bytes, c), c.indexed, source_partition(bytes, c), 0 <= i <= j <= c.count,
    ensures indexed_insert::boundary(bytes, 0, c.old_payload as int, c.old_len as int, c.count as int, i)
        <= indexed_insert::boundary(bytes, 0, c.old_payload as int, c.old_len as int, c.count as int, j),
    decreases j - i,
{
    reveal(source_partition);
    if i < j {
        ordered_boundary(bytes, c, i, j - 1);
        assert(indexed_insert::boundary_step(bytes, 0, c.old_payload as int, c.old_len as int, c.count as int, j - 1));
    }
}


/// A header-admitted plan matches the source prefix and grown extent.
/// Unvisited old ends carry no partition certificate.
pub closed spec fn prepared(bytes: Seq<u8>, c: HeaderBulkPlan) -> bool {
    &&& bytes.len() == c.old_len
    &&& c.old_len <= c.len <= u32::MAX
    &&& forall|extended: Seq<u8>| extended.len() == c.len
        && extended.subrange(0, c.old_len as int) == bytes ==> geometry(extended, c)
}

/// Header geometry retained during bulk input measurement; ends remain unchecked.
pub struct HeaderBulkSource<'a> {
    bytes: &'a [u8],
    layout: Layout,
    parts: crate::ListPartition,
}

impl<'a> HeaderBulkSource<'a> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        list_layout::header_iff(Ok(self.parts), self.bytes@, self.layout)
    }

    /// Checks header geometry before indexing or measuring user input.
    #[inline(always)]
    pub fn check(bytes: &'a [u8], layout: &Layout) -> (result: Result<Self>)
        ensures result.is_ok() ==> source_for(result.unwrap(), bytes@, *layout),
    {
        let parts = crate::list_layout::partition_list_header(bytes, layout)?;
        Ok(Self { bytes, layout: *layout, parts })
    }

    /// Original element count from the admitted header.
    #[inline(always)]
    pub fn count(&self) -> (result: u32)
        ensures result == source_count(*self),
    { self.parts.count }

    /// Checks bulk extent arithmetic and detaches the measured plan.
    #[inline(always)]
    #[allow(non_shorthand_field_patterns)]
    pub fn plan(self, index: u32, added: u32, input_len: u32)
        -> (result: Result<HeaderBulkPlan>)
        ensures source_prepared(result, self, index, added, input_len),
    {
        hide(words::le_word);
        reveal(geometry);
        proof { use_type_invariant(&self); }
        let HeaderBulkSource { bytes, layout: _layout, parts } = self;
        if index > parts.count { return Err(Error::new(ErrorKind::IndexOutOfBounds, u32::MAX)); }
        if added == 0 && input_len != 0 { return Err(Error::new(ErrorKind::InvalidBoundary, u32::MAX)); }
        let count = checked_u32(checked_add(parts.count as usize, added as usize)?)?;
        let width = parts.element_width.unwrap_or(0);
        let indexed = parts.element_width.is_none();
        if !indexed {
            let expected = checked_u32(checked_mul(added as usize, width as usize)?)?;
            if expected != input_len { return Err(Error::new(ErrorKind::InvalidBoundary, u32::MAX)); }
        }
        let payload = if indexed {
            let ends = if count == 0 { 0 } else { count - 1 };
            checked_u32(checked_add(4, checked_mul(ends as usize, 4)?)?)?
        } else { 4 };
        proof {
            if !indexed {
                assert(0 <= index * width <= parts.count * width <= u32::MAX) by(nonlinear_arith)
                    requires index <= parts.count, 0 <= width, bytes.len() == 4 + parts.count * width, bytes.len() <= u32::MAX;
            }
        }
        let old_len = parts.payload.end;
        let offset = if indexed {
            if index == 0 { 0 } else if index == parts.count { old_len - parts.payload.start }
            else { crate::read_u32(bytes, 4 * index)? }
        } else { index * width };
        if offset > old_len - parts.payload.start {
            return Err(Error::new(ErrorKind::InvalidBoundary, 4 * index));
        }
        let len = checked_u32(checked_add(payload as usize,
            checked_add((old_len - parts.payload.start) as usize, input_len as usize)?)?)?;
        let result = HeaderBulkPlan { count: parts.count, index, added, old_payload: parts.payload.start,
            payload, old_len, offset, input_len, indexed, width, len };
        proof {
            assert forall|extended: Seq<u8>| extended.len() == len
                && extended.subrange(0, old_len as int) == bytes@ implies geometry(extended, result) by {
                indexed_insert::word_in_slice(extended, 0, old_len as int, 0);
                assert(words::le_word(extended, 0) == words::le_word(bytes@, 0));
                if indexed {
                    if 0 < index < parts.count {
                        indexed_insert::word_in_slice(extended, 0, old_len as int, 4 * index);
                    }

                }
            }
        }
        Ok(result)
    }

}

impl HeaderBulkPlan {
    /// Exact grown list extent, including count, directory and payload.
    #[inline(always)]
    pub fn encoded_len(&self) -> (result: u32)
        ensures result == plan_len(*self),
    { self.len }
}

#[verifier::spinoff_prover]
proof fn append_end(c: HeaderBulkPlan, ends: Seq<int>, end: int)
    requires ends_valid(c, ends), ends.len() <= c.added,
        ends[ends.len() - 1] <= end <= c.offset + c.input_len,
        !c.indexed ==> end == ends[ends.len() - 1] + c.width,
    ensures ends_valid(c, ends.push(end)),
{
    let next = ends.push(end);
    assert forall|i: int| 0 <= i < next.len() implies c.offset <= #[trigger] next[i] <= c.offset + c.input_len by { }
    assert forall|i: int| 0 <= i < next.len() - 1 implies #[trigger] next[i] <= next[i + 1] by { }
    assert forall|i: int| 0 <= i < next.len() && !c.indexed implies #[trigger] next[i] == c.offset + i * c.width by {
        if i == ends.len() {
            assert(c.offset + (ends.len() - 1) * c.width + c.width == c.offset + ends.len() * c.width) by(nonlinear_arith);
        }
    }
}

/// Exact cumulative end, including implicit endpoints, for either list format.
pub(super) open spec fn end_at(bytes: Seq<u8>, c: HeaderBulkPlan, grown: bool, k: int) -> int {
    if c.indexed { indexed_insert::boundary(bytes, 0,
        if grown { c.payload as int } else { c.old_payload as int },
        if grown { c.len as int } else { c.old_len as int },
        if grown { c.count + c.added } else { c.count as int }, k) }
    else { k * c.width }
}

pub(super) open spec fn ends_valid(c: HeaderBulkPlan, ends: Seq<int>) -> bool {
    &&& 1 <= ends.len() <= c.added + 1
    &&& ends[0] == c.offset
    &&& forall|i: int| 0 <= i < ends.len() ==> c.offset <= #[trigger] ends[i] <= c.offset + c.input_len
    &&& forall|i: int| 0 <= i < ends.len() - 1 ==> #[trigger] ends[i] <= ends[i + 1]
    &&& !c.indexed ==> forall|i: int| 0 <= i < ends.len() ==> #[trigger] ends[i] == c.offset + i * c.width
}

pub(super) open spec fn reserved(bytes: Seq<u8>, c: HeaderBulkPlan, ends: Seq<int>) -> bool {
    c.indexed ==> forall|i: int| 1 <= i < ends.len() && c.index + i < c.count + c.added
        ==> #[trigger] words::le_word(bytes, 4 * (c.index + i)) == ends[i]
}

pub(super) open spec fn retained(before: Seq<u8>, after: Seq<u8>, c: HeaderBulkPlan) -> bool {
    let growth = c.payload - c.old_payload + c.input_len;
    &&& before.len() == after.len() == c.len
    &&& forall|p: int| c.payload <= p < c.payload + c.offset ==> #[trigger] after[p] == before[p - (c.payload - c.old_payload)]
    &&& forall|p: int| c.payload + c.offset + c.input_len <= p < after.len() ==> #[trigger] after[p] == before[p - growth]
}

/// The executable layout result, before or after encoding only its new slots.
pub(super) open spec fn layout_result(before: Seq<u8>, after: Seq<u8>, c: HeaderBulkPlan, ends: Seq<int>) -> bool {
    &&& geometry(before, c)
    &&& retained(before, after, c)
    &&& ends_valid(c, ends) && ends.len() == c.added + 1 && ends[c.added as int] == c.offset + c.input_len
    &&& words::le_word(after, 0) == c.count + c.added
    &&& forall|k: int| 0 <= k <= c.count + c.added ==> #[trigger] end_at(after, c, true, k) ==
        if k <= c.index { end_at(before, c, false, k) }
        else if k <= c.index + c.added { ends[k - c.index] }
        else { end_at(before, c, false, k - c.added) + c.input_len }
}

/// A streaming layout loan. All measured lengths are reserved before encoding.
#[must_use = "reserve every measured element and finish the layout"]
pub struct HeaderBulkLayout<'a> {
    bytes: &'a mut [u8],
    plan: HeaderBulkPlan,
    done: u32,
    end: u32,
    original: Ghost<Seq<u8>>,
    ends: Ghost<Seq<int>>,
}

impl<'a> HeaderBulkLayout<'a> {
    closed spec fn valid(self) -> bool {
        &&& geometry(self.original@, self.plan)
        &&& moved(self.original@, self.bytes@, self.plan)
        &&& ends_valid(self.plan, self.ends@)
        &&& self.ends@.len() == self.done + 1
        &&& self.end == self.ends@[self.done as int]
        &&& reserved(self.bytes@, self.plan, self.ends@)
    }

    /// Reserves one exact element extent and writes its cumulative directory end.
    #[verifier::spinoff_prover]
    #[inline(always)]
    pub fn reserve(&mut self, len: u32) -> (result: Result<Span>)
        requires layout_checked(*old(self)),
        ensures reserved_slot(*old(self), *final(self), result, len), layout_checked(*final(self)),
        no_unwind
    {
        hide(words::le_word);

        proof { reveal(geometry); }
        let c = self.plan;
        if self.done == c.added || len > c.offset + c.input_len - self.end
            || (!c.indexed && len != c.width) {
            return Err(Error::new(ErrorKind::InvalidBoundary, u32::MAX));
        }
        let start = c.payload + self.end;
        self.done += 1;
        self.end += len;
        let boundary = c.index + self.done;
        let ghost previous = self.bytes@;
        if c.indexed && boundary < c.count + c.added {
            crate::words::write_contained_word(self.bytes, (4 * boundary) as usize, self.end);
            proof {
                same_word(previous, self.bytes@, 0);
                assert forall|k: int| 1 <= k <= c.index && k < c.count implies
                    #[trigger] words::le_word(self.bytes@, 4 * k) == words::le_word(self.original@, 4 * k) by {
                    same_word(previous, self.bytes@, 4 * k);
                }
                assert forall|k: int| c.index < k < c.count implies
                    #[trigger] words::le_word(self.bytes@, 4 * (k + c.added)) == words::le_word(self.original@, 4 * k) + c.input_len by {
                    same_word(previous, self.bytes@, 4 * (k + c.added));
                }
                if c.added > 0 && c.index == c.count && c.count > 0 {
                    same_word(previous, self.bytes@, 4 * c.count);
                }
                assert forall|i: int| 1 <= i < self.ends@.len() && c.index + i < c.count + c.added implies
                    #[trigger] words::le_word(self.bytes@, 4 * (c.index + i)) == self.ends@[i] by {
                    same_word(previous, self.bytes@, 4 * (c.index + i));
                }
            }
        }
        let ghost old_ends = self.ends@;
        proof { append_end(c, old_ends, self.end as int); }
        self.ends = Ghost(old_ends.push(self.end as int));
        proof {
            assert(reserved(self.bytes@, c, self.ends@));
            assert(moved(self.original@, self.bytes@, c));
        }
        Ok(Span { start, end: start + len })
    }

    /// Publishes the grown count only after every measured length is accounted for.
    #[allow(non_shorthand_field_patterns)]
    #[inline(always)]
    pub fn finish(self) -> (result: Result<HeaderBulkSlots<'a>>)
        requires layout_checked(self),
        ensures layout_finished(self, result),
        no_unwind
    {
        hide(words::le_word);

        if self.done != self.plan.added || self.end != self.plan.offset + self.plan.input_len {
            return Err(Error::new(ErrorKind::InvalidBoundary, u32::MAX));
        }
        proof { reveal(geometry); }
        let HeaderBulkLayout { bytes, plan: c, original, ends, .. } = self;
        let ghost previous = bytes@;
        crate::words::write_contained_word(bytes, 0, c.count + c.added);
        proof {
            assert forall|k: int| 0 <= k <= c.count + c.added implies #[trigger] end_at(bytes@, c, true, k) ==
                if k <= c.index { end_at(original@, c, false, k) }
                else if k <= c.index + c.added { ends@[k - c.index] }
                else { end_at(original@, c, false, k - c.added) + c.input_len } by {
                if c.indexed {
                    if 0 < k < c.count + c.added {
                        same_word(previous, bytes@, 4 * k);
                        if k <= c.index && k < c.count {
                            assert(words::le_word(previous, 4 * k) == words::le_word(original@, 4 * k));
                        } else if c.index < k <= c.index + c.added {
                            assert(reserved(previous, c, ends@));
                            assert(1 <= k - c.index < ends@.len());
                            assert(words::le_word(previous, 4 * (c.index + (k - c.index))) == ends@[k - c.index]);
                        } else if k > c.index + c.added {
                            assert(c.index < k - c.added < c.count);
                            assert(words::le_word(previous, 4 * ((k - c.added) + c.added)) == words::le_word(original@, 4 * (k - c.added)) + c.input_len);
                        }
                    }
                } else {
                    assert(k * c.width == if k <= c.index { k * c.width }
                        else if k <= c.index + c.added { c.offset + (k - c.index) * c.width }
                        else { (k - c.added) * c.width + c.input_len }) by(nonlinear_arith)
                        requires c.offset == c.index * c.width, c.input_len == c.added * c.width;
                }
            }
        }
        Ok(HeaderBulkSlots { bytes, plan: c, original, ends })
    }
}

/// Completed bulk layout lending only the exact slots of newly added elements.
pub struct HeaderBulkSlots<'a> {
    bytes: &'a mut [u8],
    plan: HeaderBulkPlan,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    original: Ghost<Seq<u8>>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    ends: Ghost<Seq<int>>,
}

pub closed spec fn slots_for(slots: HeaderBulkSlots, before: Seq<u8>, c: HeaderBulkPlan, ends: Seq<int>) -> bool {
    slots.original@ == before && slots.plan == c && slots.ends@ == ends && slots.valid()
}

impl<'a> HeaderBulkSlots<'a> {
    closed spec fn valid(self) -> bool { layout_result(self.original@, self.bytes@, self.plan, self.ends@) }

    /// Lends one new element's exact region; retained elements and all headers are framed.
    ///
    /// # Panics
    ///
    /// Panics if `index` is outside the added run.
    #[inline(always)]
    #[verifier::spinoff_prover]
    #[verifier::rlimit(240)]
    pub fn slot(&mut self, index: u32) -> (result: &mut [u8])
        requires slot_available(*old(self), index),
        ensures slot_loan(*old(self), *final(self), result@, final(result)@, index), slots_checked(*final(self)),
        no_unwind
    {
        hide(words::le_word);

        proof { reveal(geometry); }
        let c = self.plan;
        if index >= c.added { panic!("bulk element index out of bounds"); }
        proof {
            if !c.indexed {
                assert(0 <= (c.index + index + 1) * c.width <= (c.count + c.added) * c.width <= u32::MAX) by(nonlinear_arith)
                    requires c.index + index + 1 <= c.count + c.added, 0 <= c.width,
                        c.old_len == 4 + c.count * c.width, c.input_len == c.added * c.width,
                        c.len == c.old_len + c.input_len, c.len <= u32::MAX;
            }
        }
        let k = c.index + index;
        proof { if !c.indexed {
            assert(0 <= k * c.width <= (k + 1) * c.width <= u32::MAX) by(nonlinear_arith)
                requires 0 <= k, 0 <= c.width, (k + 1) * c.width <= u32::MAX;
        } }
        let start = if c.indexed {
            if k == 0 { 0 } else { crate::words::read_contained_word(self.bytes, (4 * k) as usize) }
        } else { k * c.width };
        let end = if c.indexed {
            if k + 1 == c.count + c.added { c.len - c.payload }
            else { crate::words::read_contained_word(self.bytes, (4 * (k + 1)) as usize) }
        } else { (k + 1) * c.width };
        proof {
            assert(end_at(self.bytes@, c, true, k as int) == self.ends@[index as int]);
            assert(end_at(self.bytes@, c, true, k + 1) == self.ends@[index + 1]);
        }
        assert(start == self.ends@[index as int]);
        assert(end == self.ends@[index + 1]);
        let ghost before = self.bytes@;
        proof { frame_layout(self.original@, before, c, self.ends@, index as int); }
        borrow_destination(self.bytes, c.payload + start, end - start)
    }
}

/// An exact element loan cannot disturb the completed layout or retained payload.
#[verifier::spinoff_prover]
proof fn frame_layout(original: Seq<u8>, before: Seq<u8>, c: HeaderBulkPlan, ends: Seq<int>, i: int)
    requires layout_result(original, before, c, ends), 0 <= i < c.added,
    ensures forall|after: Seq<u8>| after.len() == before.len()
        && (forall|p: int| 0 <= p < before.len() && !(c.payload + ends[i] <= p < c.payload + ends[i + 1])
            ==> after[p] == before[p]) ==> #[trigger] layout_result(original, after, c, ends),
{
    hide(words::le_word);
    reveal(geometry);
    assert forall|after: Seq<u8>| after.len() == before.len()
        && (forall|p: int| 0 <= p < before.len() && !(c.payload + ends[i] <= p < c.payload + ends[i + 1])
            ==> after[p] == before[p]) implies #[trigger] layout_result(original, after, c, ends) by {
        same_word(before, after, 0);
        assert forall|k: int| 0 <= k <= c.count + c.added implies
            #[trigger] end_at(after, c, true, k) == end_at(before, c, true, k) by {
            if c.indexed && 0 < k < c.count + c.added { same_word(before, after, 4 * k); }
        }
    }
}

impl HeaderBulkPlan {
    /// Completes fixed-stride layout directly from the checked measured total.
    ///
    /// Every measured fixed element has the same width, so no directory or
    /// second pass over individual lengths is needed. Indexed plans are rejected.
    ///
    /// The caller retains the header-admitted source prefix and exact grown extent.
    /// Invalid retained ends return an error with poison armed.
    #[inline(always)]
    pub fn fixed_layout<'a>(self, bytes: &'a mut [u8], poisoned: &mut bool) -> (result: Result<HeaderBulkSlots<'a>>)
        requires geometry(old(bytes)@, self),
        ensures fixed_begun(result, old(bytes)@, self),
        no_unwind
    {
        hide(words::le_word);
        proof { reveal(geometry); }
        if self.indexed { return Err(Error::new(ErrorKind::InvalidBoundary, u32::MAX)); }
        let ghost original = bytes@;
        match bulk_move(bytes, self, poisoned) { Ok(()) => (), Err(error) => return Err(error) };
        let ghost ends = Seq::new((self.added + 1) as nat, |i: int| self.offset + i * self.width);
        proof {
            assert forall|i: int| 0 <= i < ends.len() implies self.offset <= #[trigger] ends[i] <= self.offset + self.input_len by {
                assert(0 <= i * self.width <= self.added * self.width) by(nonlinear_arith)
                    requires 0 <= i <= self.added, 0 <= self.width;
            }
            assert forall|i: int| 0 <= i < ends.len() - 1 implies #[trigger] ends[i] <= ends[i + 1] by {
                assert(i * self.width <= (i + 1) * self.width) by(nonlinear_arith) requires 0 <= self.width;
            }
        }
        let layout = HeaderBulkLayout { bytes, plan: self, done: self.added, end: self.offset + self.input_len,
            original: Ghost(original), ends: Ghost(ends) };
        layout.finish()
    }

    /// Moves retained bytes and old directory ends before reserving new elements.
    ///
    /// The caller retains the header-admitted source prefix and exact grown extent.
    /// Invalid retained ends return an error with poison armed.
    #[inline(always)]
    pub fn begin<'a>(self, bytes: &'a mut [u8], poisoned: &mut bool) -> (result: Result<HeaderBulkLayout<'a>>)
        requires geometry(old(bytes)@, self),
        ensures result.is_ok() ==> begun(result, old(bytes)@, self),
            source_partition(old(bytes)@, self) ==> result.is_ok(),
        no_unwind
    {
        hide(words::le_word);
        proof { reveal(geometry); }
        let ghost original = bytes@;
        match bulk_move(bytes, self, poisoned) { Ok(()) => (), Err(error) => return Err(error) };
        let ghost ends = seq![self.offset as int];
        proof {
            assert(ends.len() == 1 && ends[0] == self.offset);
            assert forall|i: int| 0 <= i < ends.len() implies self.offset <= #[trigger] ends[i] <= self.offset + self.input_len by { assert(i == 0); }
            assert forall|i: int| 0 <= i < ends.len() - 1 implies #[trigger] ends[i] <= ends[i + 1] by { }
            assert forall|i: int| 0 <= i < ends.len() && !self.indexed implies #[trigger] ends[i] == self.offset + i * self.width by { assert(i == 0); }
            assert(ends_valid(self, ends)); assert(reserved(bytes@, self, ends));
        }
        Ok(HeaderBulkLayout { bytes, plan: self, done: 0, end: self.offset,
            original: Ghost(original), ends: Ghost(ends) })
    }
}

/// Exact result of checked bulk preparation.
pub closed spec fn prepare_result(result: Result<HeaderBulkPlan>, bytes: Seq<u8>, layout: Layout, index: u32, added: u32, input_len: u32) -> bool {
    result.is_ok() ==> prepared(bytes, result.unwrap())
        && result.unwrap().index == index && result.unwrap().added == added
        && result.unwrap().input_len == input_len
        && result.unwrap().indexed == list_layout::element_width(layout).is_none()
        && result.unwrap().width == list_layout::element_width(layout).unwrap_or(0)
}
/// The source gateway retains the exact checked input and metadata.
pub closed spec fn source_for(source: HeaderBulkSource, bytes: Seq<u8>, layout: Layout) -> bool {
    source.bytes@ == bytes && source.layout == layout && source.valid()
}
/// Original count certified by the header.
pub closed spec fn source_count(source: HeaderBulkSource) -> u32 { source.parts.count }
/// A detached bulk plan derived from the checked source and measured lengths.
pub closed spec fn source_prepared(result: Result<HeaderBulkPlan>, source: HeaderBulkSource,
    index: u32, added: u32, input_len: u32) -> bool {
    prepare_result(result, source.bytes@, source.layout, index, added, input_len)
}
/// Exact grown list length.
pub closed spec fn plan_len(c: HeaderBulkPlan) -> u32 { c.len }
/// Successful reservation appends exactly one measured cumulative end.
pub closed spec fn reserved_slot(before: HeaderBulkLayout, after: HeaderBulkLayout, result: Result<Span>, len: u32) -> bool {
    &&& result.is_ok() ==> after.ends@ == before.ends@.push(before.end + len)
        && after.plan == before.plan && after.original == before.original
        && result.unwrap() == (Span { start: (before.plan.payload + before.end) as u32,
            end: (before.plan.payload + before.end + len) as u32 })
    &&& result.is_err() ==> after == before
}
/// Completed layout carries the exact measured slots and original prefix.
pub closed spec fn layout_finished(before: HeaderBulkLayout, result: Result<HeaderBulkSlots>) -> bool {
    result.is_ok() ==> slots_for(result.unwrap(), before.original@, before.plan, before.ends@)
}
/// Fixed layout uses the same exact slots as streaming uniform measured lengths.
pub closed spec fn fixed_begun(result: Result<HeaderBulkSlots>, original: Seq<u8>, c: HeaderBulkPlan) -> bool {
    result.is_ok() ==> !c.indexed && slots_for(result.unwrap(), original, c,
        Seq::new((c.added + 1) as nat, |i: int| c.offset + i * c.width))
}
/// Exact original prefix and initial empty reservation state.
pub closed spec fn begun(result: Result<HeaderBulkLayout>, before: Seq<u8>, c: HeaderBulkPlan) -> bool {
    result.is_ok() && result.unwrap().valid() && result.unwrap().original@ == before && result.unwrap().plan == c
        && result.unwrap().ends@ == seq![c.offset as int]
}
/// Exact length, metadata frame and completed layout across an element loan.
pub closed spec fn slot_loan(before: HeaderBulkSlots, after: HeaderBulkSlots, initial: Seq<u8>, encoded: Seq<u8>, index: u32) -> bool {
    let a = before.plan.payload + before.ends@[index as int];
    let z = before.plan.payload + before.ends@[index + 1];
    &&& initial == before.bytes@.subrange(a, z)
    &&& after.bytes@.subrange(a, z) == encoded
    &&& after.plan == before.plan && after.original == before.original && after.ends == before.ends
    &&& after.bytes.len() == before.bytes.len()
    &&& forall|p: int| 0 <= p < before.bytes.len() && !(a <= p < z) ==> after.bytes@[p] == before.bytes@[p]
}
/// Availability of one exact new-element loan.
pub closed spec fn slot_available(slots: HeaderBulkSlots, index: u32) -> bool { slots.valid() && index < slots.plan.added }


/// The payload bytes occupying one old or grown list element.
pub(super) open spec fn item(bytes: Seq<u8>, c: HeaderBulkPlan, grown: bool, i: int) -> Seq<u8> {
    let payload = if grown { c.payload as int } else { c.old_payload as int };
    bytes.subrange(payload + end_at(bytes, c, grown, i), payload + end_at(bytes, c, grown, i + 1))
}

/// Only the newly encoded elements supply semantic premises.
pub(super) open spec fn inputs_valid(bytes: Seq<u8>, c: HeaderBulkPlan, ends: Seq<int>,
    layout: Layout, budget: nat, semantic: bool) -> bool {
    forall|i: int| 0 <= i < c.added ==> #[trigger] validation::schema_valid(
        bytes.subrange(c.payload + ends[i], c.payload + ends[i + 1]),
        validation::child_layout(layout, 0), (budget - 1) as nat, semantic)
}

/// List child spans coincide with the cumulative boundaries for both formats.
#[verifier::spinoff_prover]
proof fn projection(bytes: Seq<u8>, c: HeaderBulkPlan, grown: bool, layout: Layout, i: int)
    requires 4 <= c.old_payload <= c.old_len <= c.len == bytes.len() <= u32::MAX,
        4 <= c.payload <= c.len,
        matches!(layout.head(), LayoutHead::List { .. }),
        c.indexed == list_layout::element_width(layout).is_none(),
        !c.indexed ==> list_layout::element_width(layout) == Some(c.width),
        0 <= i < if grown { c.count + c.added } else { c.count as int },
        list_layout::list_shape(bytes.subrange(0, if grown { c.len as int } else { c.old_len as int }), layout),
        words::le_word(bytes, 0) == if grown { c.count + c.added } else { c.count as int },
        if c.indexed {
            (if grown { c.payload as int } else { c.old_payload as int }) == 4 + 4 * indexed_remove::words_for(
                if grown { c.count + c.added } else { c.count as int })
        } else { c.old_payload == 4 && c.payload == 4 },
    ensures item(bytes, c, grown, i) == bytes.subrange(0, if grown { c.len as int } else { c.old_len as int }).subrange(
        validation::child_span(bytes.subrange(0, if grown { c.len as int } else { c.old_len as int }), layout, i).start as int,
        validation::child_span(bytes.subrange(0, if grown { c.len as int } else { c.old_len as int }), layout, i).end as int),
{
    hide(words::le_word);
    let end = if grown { c.len as int } else { c.old_len as int };
    let payload = if grown { c.payload as int } else { c.old_payload as int };
    let count = if grown { c.count + c.added } else { c.count as int };
    let local = bytes.subrange(0, end);
    indexed_insert::word_in_slice(bytes, 0, end, 0);
    if c.indexed {
        indexed_insert::item_projection(bytes, 0, end, payload, count, layout, i);
    } else {
        assert(0 <= payload + i * c.width <= payload + (i + 1) * c.width <= end) by(nonlinear_arith)
            requires 0 <= i < count, 0 <= c.width, end == 4 + count * c.width, payload == 4;
        assert(item(bytes, c, grown, i) =~= bytes.subrange(0, end).subrange(
            validation::child_span(bytes.subrange(0, end), layout, i).start as int,
            validation::child_span(bytes.subrange(0, end), layout, i).end as int));
    }
}


/// Exact bounded monotone original boundaries, for either list format.
pub(super) open spec fn old_ends(bytes: Seq<u8>, c: HeaderBulkPlan) -> bool {
    &&& forall|k: int| 0 <= k <= c.count ==> 0 <= #[trigger] end_at(bytes, c, false, k) <= c.old_len - c.old_payload
    &&& forall|k: int| 0 <= k < c.count ==> #[trigger] end_at(bytes, c, false, k) <= end_at(bytes, c, false, k + 1)
}

#[verifier::spinoff_prover]
proof fn old_boundaries(before: Seq<u8>, c: HeaderBulkPlan, layout: Layout)
    requires geometry(before, c), matches!(layout.head(), LayoutHead::List { .. }),
        c.indexed == list_layout::element_width(layout).is_none(),
        !c.indexed ==> list_layout::element_width(layout) == Some(c.width),
        list_layout::list_shape(before.subrange(0, c.old_len as int), layout),
    ensures old_ends(before, c),
{
    hide(words::le_word); reveal(geometry);
    let old = before.subrange(0, c.old_len as int);
    indexed_insert::word_in_slice(before, 0, c.old_len as int, 0);
    assert(words::le_word(old, 0) == c.count);
    assert(list_layout::payload_start(old, layout) == c.old_payload);
    assert forall|k: int| 0 <= k <= c.count implies
        0 <= #[trigger] end_at(before, c, false, k) <= c.old_len - c.old_payload by {
        if c.indexed && 0 < k < c.count {
            assert(words::le_word(old, 4 + 4 * (k - 1)) <= c.old_len - c.old_payload);
            indexed_insert::word_in_slice(before, 0, c.old_len as int, 4 * k);
        } else if !c.indexed {
            assert(0 <= k * c.width <= c.count * c.width) by(nonlinear_arith)
                requires 0 <= k <= c.count, 0 <= c.width;
        }
    }
    assert forall|k: int| 0 <= k < c.count implies
        #[trigger] end_at(before, c, false, k) <= end_at(before, c, false, k + 1) by {
        assert(0 <= end_at(before, c, false, k) <= c.old_len - c.old_payload);
        assert(0 <= end_at(before, c, false, k + 1) <= c.old_len - c.old_payload);
        if !c.indexed {
            assert(k * c.width <= (k + 1) * c.width) by(nonlinear_arith) requires 0 <= c.width;
        } else if 0 < k && k + 1 < c.count {
            assert(words::le_word(old, 4 + 4 * k) >= words::le_word(old, 4 + 4 * (k - 1)));
            indexed_insert::word_in_slice(before, 0, c.old_len as int, 4 * k);
            indexed_insert::word_in_slice(before, 0, c.old_len as int, 4 * (k + 1));
        }
    }

}

proof fn end_order(bytes: Seq<u8>, c: HeaderBulkPlan, i: int, j: int)
    requires old_ends(bytes, c), 0 <= i <= j <= c.count,
    ensures end_at(bytes, c, false, i) <= end_at(bytes, c, false, j),
    decreases j - i,
{
    if i < j { end_order(bytes, c, i, j - 1); }
}

/// A completed directory has exact monotone partitions, including empty runs.
#[verifier::spinoff_prover]
proof fn resulting_shape(before: Seq<u8>, after: Seq<u8>, c: HeaderBulkPlan, ends: Seq<int>, layout: Layout)
    requires layout_result(before, after, c, ends),
        matches!(layout.head(), LayoutHead::List { .. }),
        c.indexed == list_layout::element_width(layout).is_none(),
        !c.indexed ==> list_layout::element_width(layout) == Some(c.width),
        list_layout::list_shape(before.subrange(0, c.old_len as int), layout),
    ensures list_layout::list_shape(after, layout),
{
    hide(words::le_word); reveal(geometry);
    let old = before.subrange(0, c.old_len as int);
    old_boundaries(before, c, layout);
    assert forall|k: int| 0 <= k <= c.count + c.added implies
        0 <= #[trigger] end_at(after, c, true, k) <= c.len - c.payload by {
        let source = if k <= c.index { k } else { k - c.added };
        if k <= c.index || k > c.index + c.added {
            assert(0 <= end_at(before, c, false, source) <= c.old_len - c.old_payload);
        }
    }
    assert forall|k: int| 0 <= k < c.count + c.added implies
        #[trigger] end_at(after, c, true, k) <= end_at(after, c, true, k + 1) by {
        if k < c.index {
            assert(end_at(before, c, false, k) <= end_at(before, c, false, k + 1));
        } else if k >= c.index + c.added {
            assert(end_at(before, c, false, k - c.added) <= end_at(before, c, false, k - c.added + 1));
        }
    }
    if c.indexed {
        assert forall|k: int| 0 <= k <= c.count + c.added implies 0 <= #[trigger] indexed_insert::boundary(after, 0, c.payload as int, c.len as int, c.count + c.added, k) <= c.len - c.payload by {
            assert(end_at(after, c, true, k) == indexed_insert::boundary(after, 0, c.payload as int, c.len as int, c.count + c.added, k));
        }
        assert forall|k: int| 0 <= k < c.count + c.added implies #[trigger] indexed_insert::boundary_step(after, 0, c.payload as int, c.len as int, c.count + c.added, k) by {
            assert(end_at(after, c, true, k) <= end_at(after, c, true, k + 1));
        }
        indexed_insert::shape_from_boundaries(after, 0, c.len as int, c.payload as int,
            c.count + c.added, layout);
        assert(after.subrange(0, c.len as int) =~= after);
    } else {
        assert(c.len == 4 + (c.count + c.added) * c.width) by(nonlinear_arith)
            requires c.old_len == 4 + c.count * c.width, c.input_len == c.added * c.width,
                c.len == c.old_len + c.input_len;
        assert(list_layout::list_shape(after, layout));
    }

}

/// Retained elements preserve every payload byte across directory growth.
#[verifier::spinoff_prover]
proof fn retained_item(before: Seq<u8>, after: Seq<u8>, c: HeaderBulkPlan, ends: Seq<int>, i: int)
    requires layout_result(before, after, c, ends), old_ends(before, c),
        0 <= i < c.count + c.added, i < c.index || c.index + c.added <= i,
    ensures item(after, c, true, i) == item(before, c, false, if i < c.index { i } else { i - c.added }),
{
    hide(words::le_word); reveal(geometry);
    let source = if i < c.index { i } else { i - c.added };
            end_order(before, c, source, source + 1);
            if i < c.index { end_order(before, c, source + 1, c.index as int); }
            else { end_order(before, c, c.index as int, source); }
            let a = (if i < c.index { c.payload - c.old_payload } else { c.payload - c.old_payload + c.input_len }) as int;
            let start = c.payload + end_at(after, c, true, i);
            let end = c.payload + end_at(after, c, true, i + 1);
            assert forall|k: int| 0 <= k < end - start implies #[trigger] item(after, c, true, i)[k] == item(before, c, false, source)[k] by {
                assert(after[start + k] == before[start + k - a]);
            }
            assert(item(after, c, true, i) =~= item(before, c, false, source));

}

/// Unfolds the validator's child relation without exposing recursive grandchildren.
proof fn child_domain(bytes: Seq<u8>, layout: Layout, i: int, budget: nat, semantic: bool)
    ensures validation::child_valid(bytes, layout, i, budget, semantic) == validation::schema_valid(
        bytes.subrange(validation::child_span(bytes, layout, i).start as int,
            validation::child_span(bytes, layout, i).end as int), validation::child_layout(layout, i), budget, semantic),
{ }

/// One resulting element is either a framed old child or an exact new slot.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn valid_item(before: Seq<u8>, after: Seq<u8>, c: HeaderBulkPlan, ends: Seq<int>,
    layout: Layout, budget: nat, semantic: bool, i: int)
    requires layout_result(before, after, c, ends), old_ends(before, c),
        matches!(layout.head(), LayoutHead::List { .. }),
        c.indexed == list_layout::element_width(layout).is_none(),
        !c.indexed ==> list_layout::element_width(layout) == Some(c.width),
        list_layout::list_shape(before.subrange(0, c.old_len as int), layout),
        list_layout::list_shape(after, layout),
        forall|j: int| 0 <= j < c.count ==> #[trigger] validation::child_valid(
            before.subrange(0, c.old_len as int), layout, j, (budget - 1) as nat, semantic),
        inputs_valid(after, c, ends, layout, budget, semantic), 0 <= i < c.count + c.added,
    ensures validation::child_valid(after, layout, i, (budget - 1) as nat, semantic),
{
    hide(layout_result);
    hide(validation::schema_valid); hide(validation::local_valid); hide(validation::child_valid);
    hide(words::le_word); reveal(geometry);
    assert(geometry(before, c)) by { reveal(layout_result); }
    assert(retained(before, after, c)) by { reveal(layout_result); }
    assert(words::le_word(after, 0) == c.count + c.added) by { reveal(layout_result); }
    assert(ends_valid(c, ends)) by { reveal(layout_result); }
    assert forall|k: int| 0 <= k <= c.count + c.added implies #[trigger] end_at(after, c, true, k) ==
        if k <= c.index { end_at(before, c, false, k) }
        else if k <= c.index + c.added { ends[k - c.index] }
        else { end_at(before, c, false, k - c.added) + c.input_len } by { reveal(layout_result); }
    let old = before.subrange(0, c.old_len as int);
    assert(after.subrange(0, c.len as int) =~= after);
        projection(after, c, true, layout, i);
        if c.index <= i < c.index + c.added {
            assert(end_at(after, c, true, i) == ends[i - c.index]);
            assert(end_at(after, c, true, i + 1) == ends[i - c.index + 1]);
            assert(validation::schema_valid(item(after, c, true, i), validation::child_layout(layout, 0),
                (budget - 1) as nat, semantic));
        } else {
            let source = if i < c.index { i } else { i - c.added };
            projection(before, c, false, layout, source);
            retained_item(before, after, c, ends, i);
            assert(validation::child_valid(old, layout, source, (budget - 1) as nat, semantic));
            assert(validation::child_layout(layout, source) == validation::child_layout(layout, 0));
            assert(validation::schema_valid(item(before, c, false, source), validation::child_layout(layout, 0),
                (budget - 1) as nat, semantic)) by { child_domain(old, layout, source, (budget - 1) as nat, semantic); }
        }
        assert(validation::schema_valid(item(after, c, true, i), validation::child_layout(layout, 0), (budget - 1) as nat, semantic));
        assert(validation::child_layout(layout, i) == validation::child_layout(layout, 0));
        assert(validation::schema_valid(after.subrange(validation::child_span(after, layout, i).start as int,
            validation::child_span(after, layout, i).end as int), validation::child_layout(layout, i), (budget - 1) as nat, semantic));
        child_domain(after, layout, i, (budget - 1) as nat, semantic);
}
/// Valid old children and valid new slots establish the complete resulting list.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
pub(super) proof fn selected_preserves(before: Seq<u8>, after: Seq<u8>, c: HeaderBulkPlan, ends: Seq<int>,
    layout: Layout, budget: nat, semantic: bool)
    requires layout_result(before, after, c, ends),
        matches!(layout.head(), LayoutHead::List { .. }),
        c.indexed == list_layout::element_width(layout).is_none(),
        !c.indexed ==> list_layout::element_width(layout) == Some(c.width),
        validation::schema_valid(before.subrange(0, c.old_len as int), layout, budget, semantic),
        inputs_valid(after, c, ends, layout, budget, semantic),
    ensures validation::schema_valid(after, layout, budget, semantic),
{
    hide(validation::schema_valid); hide(validation::local_valid); hide(validation::child_valid);
    hide(words::le_word);
    reveal(geometry);
    let old = before.subrange(0, c.old_len as int);
    reveal_with_fuel(validation::schema_valid, 1);
    indexed_insert::word_in_slice(before, 0, c.old_len as int, 0);
    assert(words::le_word(old, 0) == c.count);
    assert(list_layout::list_shape(old, layout)) by { reveal(validation::local_valid); }
    crate::preservation::children_all(old, layout, 0, (budget - 1) as nat, semantic);
    resulting_shape(before, after, c, ends, layout);
    old_boundaries(before, c, layout);
    assert(after.subrange(0, c.len as int) =~= after);
    assert(validation::local_valid(after, layout, semantic)) by { reveal(validation::local_valid); }
    assert forall|i: int| 0 <= i < c.count + c.added implies
        #[trigger] validation::child_valid(after, layout, i, (budget - 1) as nat, semantic) by {
        valid_item(before, after, c, ends, layout, budget, semantic, i);
    }
    crate::preservation::children_from_all(after, layout, 0, (budget - 1) as nat, semantic);
    reveal_with_fuel(validation::schema_valid, 1);
}

/// Legal reservation state.
pub closed spec fn layout_checked(layout: HeaderBulkLayout) -> bool { layout.valid() }
/// Legal completed layout state.
pub closed spec fn slots_checked(slots: HeaderBulkSlots) -> bool { slots.valid() }

/// Bulk insertion proves validity of each new slot in a nonempty batch.
#[verifier::spinoff_prover]
proof fn legacy_layout(before: Seq<u8>, after: Seq<u8>, c: HeaderBulkPlan, ends: Seq<int>)
    requires layout_result(before, after, c, ends), source_partition(before, c),
    ensures bulk_list::layout_result(before, after, legacy(c), ends),
{
    reveal(geometry);
    if c.indexed {
        assert forall|i: int| 1 <= i < c.count implies
            #[trigger] words::le_word(before, 4 * i) <= c.old_len - c.old_payload by {
            ordered_boundary(before, c, i, c.count as int);
        }
    }
    bulk_list::geometry_from_parts(before, legacy(c));
    assert(bulk_list::retained(before, after, legacy(c)));
    assert(bulk_list::ends_valid(legacy(c), ends));
    assert forall|k: int| 0 <= k <= c.count + c.added implies
        #[trigger] bulk_list::end_at(after, legacy(c), true, k) ==
            if k <= c.index { bulk_list::end_at(before, legacy(c), false, k) }
            else if k <= c.index + c.added { ends[k - c.index] }
            else { bulk_list::end_at(before, legacy(c), false, k - c.added) + c.input_len } by {
        assert(end_at(after, c, true, k) == if k <= c.index { end_at(before, c, false, k) }
            else if k <= c.index + c.added { ends[k - c.index] }
            else { end_at(before, c, false, k - c.added) + c.input_len });
    }
}

/// Header-only geometry bounds every reverse source/destination on arbitrary ends.
proof fn bulk_containment(bytes: Seq<u8>, c: HeaderBulkPlan)
    requires geometry(bytes, c),
    ensures 4 <= c.old_payload <= c.payload <= c.len == bytes.len() <= u32::MAX,
        forall|k: int| c.index < k < c.count && c.indexed ==> #[trigger] crate::patch_pass::directory_slot(0, c.old_payload as int, k)
            && crate::patch_pass::directory_slot(0, c.payload as int, k + c.added),
{ reveal(geometry); }

/// A concrete completed batch also satisfies the retained strict result.
#[verifier::spinoff_prover]
pub(super) proof fn bulk_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal_with_fuel(validation::schema_valid, 4);
    reveal_with_fuel(validation::children_valid, 5);
    reveal_with_fuel(crate::utf8::utf8_valid, 3);
    let before = witnesses::account(seq![1u8,0,0,0,7]);
    let after = witnesses::account(seq![3u8,0,0,0,7,8,9]);
    let c = HeaderBulkPlan { count: 1, index: 1, added: 2, old_payload: 4, payload: 4,
        old_len: 5, offset: 1, input_len: 2, indexed: false, width: 1, len: 7 };
    let plan = witnesses::replacement(before, witnesses::FIXED_LIST, 7);
    let initial = seq![1u8,0,0,0,7,0,0];
    assert(initial.subrange(0,5) =~= before.subrange(12,17));
    assert(geometry(initial, c));
    assert(layout_result(initial, after.subrange(12,19), c, seq![1int,2,3]));
    bulk_containment(initial, c);
    legacy_layout(initial, after.subrange(12,19), c, seq![1int,2,3]);
    bulk_list_insert_preserves(before, initial, after, after, plan, legacy(c), seq![1int,2,3]);
    bulk_list_extend_preserves(before, initial, after, after, plan, legacy(c), seq![1int,2,3]);
    let initial = seq![3u8,0,0,0,1,0,0,0,2,0,0,0,97,98,99,0,0,0,0,0];
    let after = seq![4u8,0,0,0,1,0,0,0,2,0,0,0,3,0,0,0,100,97,98,99];
    let c = HeaderBulkPlan { count: 3, index: 0, added: 1, old_payload: 12, payload: 16,
        old_len: 15, offset: 0, input_len: 1, indexed: true, width: 0, len: 20 };
    bulk_containment(initial, c);
    legacy_layout(initial, after, c, seq![0int,1]);

}

}
