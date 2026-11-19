//! Checked reverse transport of indexed insertion ends.
use super::*;

verus! {

proof fn same_word(before: Seq<u8>, after: Seq<u8>, at: int)
    requires before.len() == after.len(), 0 <= at, at + 4 <= before.len(),
        forall|p: int| at <= p < at + 4 ==> after[p] == before[p],
    ensures words::le_word(after, at) == words::le_word(before, at),
{ }

pub(in crate::replace_plan) open spec fn header_parameters(bytes: Seq<u8>, region: Span, c: IndexedInsert) -> bool {
    &&& region.start + 4 <= c.payload <= region.end <= bytes.len() <= u32::MAX
    &&& c.count == words::le_word(bytes, region.start as int)
    &&& c.index <= c.count < u32::MAX
    &&& c.payload == region.start + 4 + if c.count == 0 { 0int } else { 4 * (c.count - 1) }
    &&& c.offset == boundary(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, c.index as int)
    &&& 0 <= c.offset <= region.end - c.payload
    &&& c.table_growth == if c.count == 0 { 0u32 } else { 4u32 }
    &&& (c.count == 0 ==> region.end == c.payload)
}

#[verifier::spinoff_prover]
pub(in crate::replace_plan) fn prepare(bytes: &[u8], region: Span, layout: &Layout,
    operation: ListEdit, input_len: u32) -> (result: Result<IndexedInsert>)
    requires region.start <= region.end <= bytes.len() <= u32::MAX,
    ensures result.is_ok() ==> header_parameters(bytes@, region, result.unwrap())
        && result.unwrap().input_len == input_len && operation == ListEdit::Insert(result.unwrap().index),
{
    hide(words::le_word);
    match layout.head() {
        LayoutHead::List { element } => if fixed_width(&element).is_some() { return Err(ErrorKind::InvalidSchema.into()); },
        _ => return Err(ErrorKind::InvalidSchema.into()),
    }
    let local = &bytes[region.start as usize..region.end as usize];
    let parts = crate::list_layout::partition_list_header(local, layout).map_err(|e: Error| e.rebase(region.start))?;
    let index = match operation { ListEdit::Insert(index) => index, _ => return Err(ErrorKind::InvalidSchema.into()) };
    if index > parts.count { return Err(ErrorKind::IndexOutOfBounds.into()); }
    let _count = checked_delta(parts.count, 1)?;
    let table_growth = if parts.count == 0 { 0u32 } else { 4u32 };
    let payload = region.start + parts.payload.start;
    proof {
        word_in_slice(bytes@, region.start as int, region.end as int, 0);
        if 0 < index < parts.count { word_in_slice(bytes@, region.start as int, region.end as int, 4 * index); }
    }
    let offset = if index == 0 { 0u32 } else if index == parts.count { region.end - payload }
        else { read_u32(local, 4 * index)? };
    if offset > region.end - payload { return Err(Error::new(ErrorKind::InvalidBoundary, region.start)); }
    Ok(IndexedInsert { index, count: parts.count, payload, offset, table_growth, input_len })
}

pub(in crate::replace_plan) open spec fn admitted(bytes: Seq<u8>, plan: Plan, c: IndexedInsert) -> bool {
    &&& header_parameters(bytes, plan.region, c)
    &&& 12 <= plan.region.start <= plan.region.end <= plan.old_used <= bytes.len() <= u32::MAX
    &&& plan.new_used == plan.old_used + plan.delta
    &&& plan.required == plan.new_used
    &&& plan.delta == c.table_growth + c.input_len
    &&& plan.replacement_len == plan.region.end - plan.region.start + c.table_growth + c.input_len
}

pub(in crate::replace_plan) open spec fn complete(original: Seq<u8>, after: Seq<u8>, plan: Plan, c: IndexedInsert) -> bool {
    let tagged = with_insert(plan, c);
    &&& payload_frame(original, after, plan)
    &&& payload_preserved(original, after, tagged)
    &&& words::le_word(after, plan.region.start as int) == c.count
    &&& directory_done(original, after, tagged, c.count as int)
}

/// Payload moves precede reverse end transport; no old end is copied unchecked.
#[verifier::spinoff_prover]
#[allow(clippy::question_mark)]
pub(in crate::replace_plan) fn move_checked(bytes: &mut [u8], plan: &Plan, c: IndexedInsert,
    poisoned: &mut bool) -> (result: Result<()>)
    requires admitted(old(bytes)@, *plan, c), plan.required <= old(bytes).len(),
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        forall|p: int| 0 <= p < plan.region.start ==> final(bytes)@[p] == old(bytes)@[p],
        result.is_ok() ==> complete(old(bytes)@, final(bytes)@, *plan, c),
        parameters(old(bytes)@, plan.region, c) ==> result.is_ok(),
    no_unwind
{
    hide(words::le_word);
    hide(parameters);
    reveal(with_insert); reveal(target); reveal(payload_frame);
    let ghost original = bytes@;
    let ghost tagged = with_insert(*plan, c);
    *poisoned = true;
    let split = c.payload + c.offset;
    if plan.delta != 0 && split < plan.old_used {
        match crate::move_bytes(bytes, Span { start: split, end: plan.old_used }, (split as i64 + plan.delta) as u32) {
            Ok(()) => (), Err(error) => return Err(error),
        }
    }
    let ghost suffix_moved = bytes@;
    if c.table_growth != 0 && c.offset != 0 {
        match crate::move_bytes(bytes, Span { start: c.payload, end: split }, c.payload + c.table_growth) {
            Ok(()) => (), Err(error) => return Err(error),
        }
    }
    let ghost payload_moved = bytes@;
    proof {
        assert(payload_frame(original, bytes@, *plan));
        assert(payload_preserved(original, bytes@, tagged));
        same_word(original, bytes@, plan.region.start as int);
        assert forall|k: int| 1 <= k < c.count implies
            #[trigger] words::le_word(bytes@, plan.region.start + 4 * k) == words::le_word(original, plan.region.start + 4 * k) by {
            same_word(original, bytes@, plan.region.start + 4 * k);
        }
    }
    let mut k = c.count;
    let mut upper = plan.region.end - c.payload;
    while k > c.index + 1
        invariant original == old(bytes)@, admitted(original, *plan, c), plan.required <= bytes.len(), tagged == with_insert(*plan, c),
            bytes.len() == original.len(), *poisoned, c.index <= k <= c.count, c.index < k || c.index == c.count,
            payload_frame(original, bytes@, *plan), payload_preserved(original, bytes@, tagged),
            words::le_word(bytes@, plan.region.start as int) == c.count,
            forall|j: int| 1 <= j < k ==> #[trigger] words::le_word(bytes@, plan.region.start + 4 * j) == words::le_word(original, plan.region.start + 4 * j),
            forall|j: int| k <= j < c.count ==> #[trigger] words::le_word(bytes@, plan.region.start + 4 * (j + 1)) == words::le_word(original, plan.region.start + 4 * j) + c.input_len,
            upper == boundary(original, plan.region.start as int, c.payload as int, plan.region.end as int, c.count as int, k as int),
            0 <= upper <= plan.region.end - c.payload,
        decreases k,
    {
        k -= 1;
        let source = plan.region.start as usize + 4 * k as usize;
        let destination = source + 4;
        let ghost before = bytes@;
        proof {
            if parameters(original, plan.region, c) {
                reveal(parameters);
                boundary_order(original, plan.region, c, c.index as int, k as int);
                boundary_order(original, plan.region, c, k as int, k as int + 1);
                assert(c.offset <= words::le_word(before, source as int) <= upper);
                assert(words::le_word(before, source as int) + c.input_len <= u32::MAX);
            }
        }
        let transported = crate::patch_pass::transport_contained(bytes, source, destination, c.offset, upper, c.input_len as i64, poisoned);
        upper = match transported { Ok(value) => value, Err(error) => {
            assert(!parameters(original, plan.region, c)); return Err(error);
        } };
        proof {
            same_word(before, bytes@, plan.region.start as int);
            assert forall|j: int| 1 <= j < k implies
                #[trigger] words::le_word(bytes@, plan.region.start + 4 * j) == words::le_word(original, plan.region.start + 4 * j) by {
                same_word(before, bytes@, plan.region.start + 4 * j);
            }
            assert forall|j: int| k < j < c.count implies
                #[trigger] words::le_word(bytes@, plan.region.start + 4 * (j + 1)) == words::le_word(original, plan.region.start + 4 * j) + c.input_len by {
                same_word(before, bytes@, plan.region.start + 4 * (j + 1));
            }
        }
    }
    let ghost shifted = bytes@;
    proof {
        assert forall|j: int| 0 <= j < c.index && j + 1 < c.count implies
            #[trigger] words::le_word(shifted, plan.region.start + 4 + 4 * j) == words::le_word(original, plan.region.start + 4 + 4 * j) by {
            assert(words::le_word(shifted, plan.region.start + 4 * (j + 1)) == words::le_word(original, plan.region.start + 4 * (j + 1)));
        }
        assert forall|j: int| c.index < j < c.count implies
            #[trigger] words::le_word(shifted, plan.region.start + 4 + 4 * j) == words::le_word(original, plan.region.start + 4 * j) + c.input_len by {
            assert(words::le_word(shifted, plan.region.start + 4 * (j + 1)) == words::le_word(original, plan.region.start + 4 * j) + c.input_len);
        }
    }
    if c.index < c.count {
        let end = match crate::patch_pass::checked_end(c.offset, 0, plan.region.end - c.payload,
            c.input_len as i64, plan.region.start + 4 * (c.index + 1)) {
            Ok(value) => value, Err(error) => return Err(error),
        };
        match crate::write_u32(bytes, plan.region.start + 4 * (c.index + 1), end) {
            Ok(()) => (), Err(error) => return Err(error),
        }
    } else if c.count != 0 {
        match crate::write_u32(bytes, plan.region.start + 4 * c.count, plan.region.end - c.payload) {
            Ok(()) => (), Err(error) => return Err(error),
        }
    }
    proof {
        same_word(shifted, bytes@, plan.region.start as int);
        assert(payload_frame(original, bytes@, *plan));
        assert(payload_preserved(original, bytes@, tagged));
        assert forall|j: int| 0 <= j < c.count implies
            #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * j) == new_end(original, tagged, j) by {
            let at = plan.region.start + 4 + 4 * j;
            if j < c.index && j + 1 < c.count {
                same_word(shifted, bytes@, at);
                assert(words::le_word(bytes@, at) == words::le_word(original, at));
            } else if j > c.index {
                same_word(shifted, bytes@, at);
                assert(words::le_word(bytes@, at) == words::le_word(original, plan.region.start + 4 * j) + c.input_len);
            }
        }
    }
    Ok(())
}

}
