//! Fallible indexed splice movement, with directory validity used only in proofs.
use super::*;

verus! {

proof fn same_word(before: Seq<u8>, after: Seq<u8>, at: int)
    requires before.len() == after.len(), 0 <= at, at + 4 <= before.len(),
        forall|p: int| at <= p < at + 4 ==> after[p] == before[p],
    ensures words::le_word(after, at) == words::le_word(before, at),
{ }

/// Numeric movement admission; no unvisited directory word is constrained.
pub(in crate::replace_plan) open spec fn header_parameters(bytes: Seq<u8>, region: Span, c: IndexedSplice) -> bool {
    let indices = list_indices(c.operation, c.count);
    &&& !matches!(c.operation, ListEdit::Insert(_))
    &&& region.start + 4 <= c.payload <= region.end <= bytes.len() <= u32::MAX
    &&& c.count == words::le_word(bytes, region.start as int)
    &&& c.payload == region.start + 4 + 4 * words_for(c.count as int)
    &&& 0 <= indices.0 <= indices.1 <= c.count
    &&& c.first == indices.0 && c.last == indices.1 && c.new_count == indices.2
    &&& c.new_count == c.count - (c.last - c.first) + inserted(c)
    &&& c.new_count <= c.count
    &&& (inserted(c) == 0 ==> c.input_len == 0)
    &&& c.offset == boundary(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, c.first as int)
    &&& c.end_offset == boundary(bytes, region.start as int, c.payload as int, region.end as int, c.count as int, c.last as int)
    &&& 0 <= c.offset <= c.end_offset <= region.end - c.payload
    &&& c.table_shrink == 4 * (words_for(c.count as int) - words_for(c.new_count as int))
    &&& (c.count == 0 ==> region.end == c.payload)
}

/// Header and selected-span checks precede all writes and all removed decoding.
#[verifier::spinoff_prover]
pub(in crate::replace_plan) fn prepare(bytes: &[u8], region: Span, layout: &Layout,
    operation: ListEdit, input_len: u32) -> (result: Result<IndexedSplice>)
    requires region.start <= region.end <= bytes.len() <= u32::MAX,
    ensures result.is_ok() ==> header_parameters(bytes@, region, result.unwrap())
        && result.unwrap().input_len == input_len && result.unwrap().operation == operation,
{
    hide(words::le_word);
    match layout.head() {
        LayoutHead::List { element } => if fixed_width(&element).is_some() { return Err(ErrorKind::InvalidSchema.into()); },
        _ => return Err(ErrorKind::InvalidSchema.into()),
    }
    let local = &bytes[region.start as usize..region.end as usize];
    let parts = crate::list_layout::partition_list_header(local, layout).map_err(|e: Error| e.rebase(region.start))?;
    let count = parts.count;
    let (first, last, new_count) = match operation {
        ListEdit::Insert(_) => return Err(ErrorKind::InvalidSchema.into()),
        ListEdit::Remove(i) | ListEdit::Replace(i) => {
            if i >= count { return Err(ErrorKind::IndexOutOfBounds.into()); }
            (i, i + 1, if matches!(operation, ListEdit::Remove(_)) { count - 1 } else { count })
        },
        ListEdit::Pop => (if count == 0 { 0u32 } else { count - 1 }, count, if count == 0 { 0u32 } else { count - 1 }),
        ListEdit::Truncate(n) => { let kept = if n < count { n } else { count }; (kept, count, kept) },
        ListEdit::Clear => (0u32, count, 0u32),
    };
    if !matches!(operation, ListEdit::Replace(_)) && input_len != 0 { return Err(ErrorKind::InvalidBoundary.into()); }
    let payload = region.start + parts.payload.start;
    proof {
        word_in_slice(bytes@, region.start as int, region.end as int, 0);
        if 0 < first < count { word_in_slice(bytes@, region.start as int, region.end as int, 4 * first); }
        if 0 < last < count { word_in_slice(bytes@, region.start as int, region.end as int, 4 * last); }
    }
    let offset = if first == 0 { 0u32 } else if first == count { region.end - payload } else { read_u32(local, 4 * first)? };
    let end_offset = if last == 0 { 0u32 } else if last == count { region.end - payload } else { read_u32(local, 4 * last)? };
    if offset > end_offset || end_offset > region.end - payload {
        return Err(Error::new(ErrorKind::InvalidBoundary, region.start));
    }
    let old_words = if count == 0 { 0u32 } else { count - 1 };
    let new_words = if new_count == 0 { 0u32 } else { new_count - 1 };
    Ok(IndexedSplice { operation, count, new_count, first, last, payload, offset, end_offset,
        table_shrink: 4 * (old_words - new_words), input_len })
}

/// The outer plan carries parent admission; local ends remain arbitrary bytes.
pub(in crate::replace_plan) open spec fn admitted(bytes: Seq<u8>, plan: Plan, c: IndexedSplice) -> bool {
    &&& header_parameters(bytes, plan.region, c)
    &&& 12 <= plan.region.start <= plan.region.end <= plan.old_used <= bytes.len() <= u32::MAX
    &&& plan.new_used == plan.old_used + plan.delta
    &&& plan.required == if plan.new_used > plan.old_used { plan.new_used } else { plan.old_used }
    &&& plan.delta == payload_delta(c) - c.table_shrink
    &&& plan.replacement_len == plan.region.end - plan.region.start + plan.delta
    &&& plan.region.start + 4 <= plan.region.end + plan.delta <= plan.new_used
}

/// A successful pass has the original mover's retained payload and directory.
pub(in crate::replace_plan) open spec fn complete(original: Seq<u8>, after: Seq<u8>, plan: Plan, c: IndexedSplice) -> bool {
    let tagged = with_splice(plan, c);
    &&& payload_frame(original, after, plan)
    &&& payload_preserved(original, after, tagged)
    &&& words::le_word(after, plan.region.start as int) == c.count
    &&& directory_done(original, after, tagged, words_for(c.new_count as int))
}

/// Consumes retained ends forward, checking source order at the word write.
#[verifier::spinoff_prover]
#[allow(clippy::question_mark)]
pub(in crate::replace_plan) fn move_checked(bytes: &mut [u8], plan: &Plan, c: IndexedSplice,
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
    reveal(with_splice);
    let ghost original = bytes@;
    let ghost tagged = with_splice(*plan, c);
    proof { reveal(target); reveal(payload_frame); }
    *poisoned = true;
    let stored = if c.new_count == 0 { 0u32 } else { c.new_count - 1 };
    let mut j = c.first;
    let mut previous_end = c.end_offset;
    proof {
        assert forall|k: int| 0 <= k < c.first && k < stored implies
            #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * k) == new_end(original, tagged, k) by { }
    }
    while j < stored
        invariant original == old(bytes)@, admitted(original, *plan, c), plan.required <= bytes.len(), tagged == with_splice(*plan, c), bytes.len() == original.len(), *poisoned,
            c.first <= j <= c.count, stored == words_for(c.new_count as int),
            directory_frame(original, bytes@, tagged),
            directory_done(original, bytes@, tagged, if j < stored { j as int } else { stored as int }),
            forall|p: int| plan.region.start + 4 + 4 * j <= p < original.len() ==> bytes@[p] == original[p],
            c.end_offset <= previous_end <= plan.region.end - c.payload,
            previous_end == if j == c.first { c.end_offset as int } else {
                boundary(original, plan.region.start as int, c.payload as int, plan.region.end as int,
                    c.count as int, source_boundary(c, j as int - 1)) },
        decreases if j < stored { stored - j } else { 0int },
    {
        let source = j + (c.last - c.first - if matches!(c.operation, ListEdit::Replace(_)) { 1u32 } else { 0u32 });
        let src = plan.region.start as usize + 4 + 4 * source as usize;
        let at = plan.region.start as usize + 4 + 4 * j as usize;
        let ghost before = bytes@;
        proof {
            assert forall|p: int| src <= p < src + 4 implies before[p] == original[p] by { }
            same_word(original, before, src as int);
            assert(words::le_word(before, src as int) == words::le_word(original, src as int));
            if parameters(original, plan.region, c) {
                reveal(parameters);
                boundary_order(original, plan.region, c, c.last as int, source as int + 1);
                if j > c.first { boundary_order(original, plan.region, c,
                    source_boundary(c, j as int - 1), source as int + 1); }
                assert(previous_end <= words::le_word(before, src as int) <= plan.region.end - c.payload);
                assert(0 <= words::le_word(before, src as int) + c.input_len - (c.end_offset - c.offset) <= u32::MAX);
            }
        }
        let transported = crate::patch_pass::transport_contained(bytes, src, at, previous_end,
            plan.region.end - c.payload, c.input_len as i64 - (c.end_offset - c.offset) as i64, poisoned);
        assert(bytes.len() == original.len());
        let old_end = match transported {
            Ok(value) => value,
            Err(error) => { assert(!parameters(original, plan.region, c)); return Err(error); },
        };
        previous_end = old_end;
        proof {
            assert(words::le_word(bytes@, at as int) == new_end(original, tagged, j as int));
            assert forall|k: int| 0 <= k < j implies
                #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * k) == new_end(original, tagged, k) by {
                let other = plan.region.start + 4 + 4 * k;
                assert forall|p: int| other <= p < other + 4 implies bytes@[p] == before[p] by { }
                same_word(before, bytes@, other);
            }
        }
        j += 1;
    }
    let ghost directory = bytes@;
    if c.table_shrink != 0 && c.offset != 0 {
        match crate::move_bytes(bytes, Span { start: c.payload, end: c.payload + c.offset }, c.payload - c.table_shrink) {
            Ok(()) => (), Err(error) => return Err(error),
        }
    }
    let ghost prefix = bytes@;
    let split = c.payload + c.end_offset;
    if plan.delta != 0 && split < plan.old_used {
        match crate::move_bytes(bytes, Span { start: split, end: plan.old_used },
            c.payload - c.table_shrink + c.offset + c.input_len) {
            Ok(()) => (), Err(error) => return Err(error),
        }
    }
    proof {
        assert(payload_frame(original, bytes@, *plan));
        assert(payload_preserved(original, bytes@, tagged));
        assert forall|p: int| plan.region.start <= p < plan.region.start + 4 implies bytes@[p] == original[p] by { }
        same_word(original, bytes@, plan.region.start as int);
        assert(words::le_word(bytes@, plan.region.start as int) == c.count);
        assert forall|k: int| 0 <= k < words_for(c.new_count as int) implies
            #[trigger] words::le_word(bytes@, plan.region.start + 4 + 4 * k) == new_end(original, tagged, k) by {
            let at = plan.region.start + 4 + 4 * k;
            assert forall|p: int| at <= p < at + 4 implies bytes@[p] == directory[p] by { }
            same_word(directory, bytes@, at);
        }
    }
    Ok(())
}

}
