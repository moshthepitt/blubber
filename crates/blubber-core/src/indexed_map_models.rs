//! Fixed map geometries exercise the production checked movement on wide ends.
#![cfg_attr(verus_only, allow(missing_docs))]
use super::*;

verus! {
/// Calls the actual checked map mover without granting a source partition.
#[doc(hidden)]
pub fn __map_patch_model(bytes: &mut [u8], operation: MapEdit, poisoned: &mut bool) -> (result: Result<()>)
    requires old(bytes).len() == 40, words::le_word(old(bytes)@, 12) == 3,
        old(bytes)@[16] == 2, old(bytes)@[21] == 4, old(bytes)@[26] == 6,
        words::le_word(old(bytes)@, 27) == 3,
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        forall|i: int| 0 <= i < 16 ==> final(bytes)@[i] == old(bytes)@[i],
{
    *poisoned = true;
    let end = if matches!(operation, MapEdit::Insert) { 0u32 }
        else if matches!(operation, MapEdit::Clear) { 3u32 }
        else {
            let end = match read_u32(bytes, 17) { Ok(end) => end, Err(error) => return Err(error) };
            if end > 3 { return Err(Error::new(ErrorKind::InvalidBoundary, 17)); }
            end
        };
    let layout = Layout::Map { key: crate::KeyLayout { width: 1, domain: crate::KeyDomain::Raw }, value: &Layout::Str };
    let first = [1u8];
    let existing = [2u8];
    let empty: [u8; 0] = [];
    let key = if matches!(operation, MapEdit::Insert) { &first[..] }
        else if matches!(operation, MapEdit::Clear) { &empty[..] } else { &existing[..] };
    let input = if matches!(operation, MapEdit::Insert | MapEdit::Replace) { 1u32 } else { 0u32 };
    let count = match operation { MapEdit::Insert => 4u32, MapEdit::Remove => 2u32, MapEdit::Clear => 0u32, MapEdit::Replace => 3u32 };
    let table_delta = match operation { MapEdit::Insert => 5i64, MapEdit::Remove => -5i64, MapEdit::Clear => -15i64, MapEdit::Replace => 0i64 };
    let delta = table_delta + input as i64 - end as i64;
    let new_used = match checked_delta(34, delta) { Ok(n) => n, Err(error) => return Err(error) };
    let region = Span { start: 12, end: 34 };
    let node = Node { region, payload_len: 0, ordinal: 0,
        #[cfg(verus_only)] layout,
        run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let c = IndexedMap { change: MapChange { operation, index: 0,
        found: matches!(operation, MapEdit::Remove | MapEdit::Replace), key_width: 1, value_width: input },
        count: 3, new_count: count, stride: 5, payload: 31, offset: 0, end_offset: end,
        input_len: input, table_delta };
    let plan = Plan { key, map: None, variable_map: Some(c), indexed: None, splice: None, list: None,
        region, trail: Trail { root: layout, selected: layout, local: true, nodes: [node; MAX_DEPTH], depth: 0 },
        path: InlinePath { steps: [Step::Some; MAX_DEPTH], len: 0, overflow: false }, discriminator: [0;8],
        old_used: 34, delta, new_used, replacement_len: new_used - 12,
        required: if new_used > 34 { new_used } else { 34 }, storage_limit: 40 };
    proof {
        reveal(crate::layout_head::head_spec);
        broadcast use vstd::array::group_array_axioms;
        words::word_subrange(bytes@,12,34,0);
        words::word_subrange(bytes@,12,34,15);
        words::word_subrange(bytes@,12,34,5);
        let local = bytes@.subrange(12,34);
        assert(crate::map_layout::header_shape(local,layout));
        assert(crate::search::encoded_key(local,layout,0) =~= seq![2u8]);
        assert(crate::search::encoded_key(local,layout,1) =~= seq![4u8]);
        assert(crate::search::encoded_key(local,layout,2) =~= seq![6u8]);
        assert(crate::search::same_prefix(seq![1u8],seq![2u8],0));
        assert(crate::search::same_prefix(seq![2u8],seq![4u8],0));
        if matches!(operation,MapEdit::Insert) { assert(key@ =~= seq![1u8]); }
        else if matches!(operation,MapEdit::Clear) { assert(key@ =~= Seq::<u8>::empty()); }
        else { assert(key@ =~= seq![2u8]); }
        if !matches!(operation,MapEdit::Clear) {
            assert(crate::search::search_found(crate::Search { index:0,found:c.change.found },local,layout,key@));
            assert forall|i: int| (if c.change.found {1int} else {0int}) <= i < 3 implies crate::search::lex_less(
                key@, #[trigger] crate::search::encoded_key(local,layout,i)) by {
                if i == 0 { assert(!c.change.found); }
                else if i == 1 { }
                else { assert(i == 2); }
                assert(crate::search::same_prefix(key@,crate::search::encoded_key(local,layout,i),0));
            }
            if !c.change.found {
                assert forall|i: int| 0 <= i < 3 implies #[trigger] crate::search::encoded_key(local,layout,i) != key@ by {
                    if i == 0 { } else if i == 1 { } else { assert(i == 2); }
                }
            }
            assert(crate::search::search_partition(crate::Search { index:0,found:c.change.found },local,layout,key@));
        }
        assert(indexed_map::header_parameters(bytes@, region, layout, c, key@));
    }
    indexed_map::checked::move_checked(bytes, &plan, poisoned)
}
}
