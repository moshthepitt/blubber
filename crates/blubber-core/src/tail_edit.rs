//! Compact loans for a direct root field ending at the live tail.

use super::*;

verus! {

/// Checks the account envelope, discriminator and minimum logical length.
/// Returns the current logical end, excluding physical spare capacity.
#[doc(hidden)]
#[inline(always)]
pub fn account_used(bytes: &[u8], discriminator: [u8; 8], minimum: u32) -> (result: Result<u32>)
    ensures result.is_ok() == prepared::account_admitted(bytes@, discriminator, minimum),
        result.is_ok() ==> 12 <= result.unwrap() <= bytes.len() <= u32::MAX
        && words::le_word(bytes@, 8) == result.unwrap()
        && prepared::account_identity(bytes@, discriminator, minimum),
{
    if discriminator == [0u8; 8] { return Err(ErrorKind::InvalidSchema.into()); }
    checked_u32(bytes.len())?;
    if bytes.len() < 12 { return Err(Error::new(ErrorKind::Truncated, 0)); }
    let actual_bytes = &bytes[..8];
    let expected_bytes = &discriminator[..];
    let actual = crate::words::header_word64(actual_bytes);
    let expected = crate::words::header_word64(expected_bytes);
    proof {
        if prepared::account_admitted(bytes@, discriminator, minimum) {
            assert(actual_bytes@ =~= expected_bytes@);
        }
    }
    if actual != expected {
        return Err(Error::new(ErrorKind::Discriminator, 0));
    }
    proof {
        vstd::bytes::lemma_auto_spec_u64_to_from_le_bytes();
        assert(vstd::bytes::spec_u64_to_le_bytes(actual) == actual_bytes@);
        assert(vstd::bytes::spec_u64_to_le_bytes(expected) == expected_bytes@);
        assert(actual_bytes@ == discriminator@);
        assert(bytes@.subrange(0, 8) == discriminator@);
    }
    let minimum_used = checked_delta(12, minimum as i64)?;
    let used = read_u32(bytes, 8)?;
    if used < minimum_used || used as usize > bytes.len() { return Err(Error::new(ErrorKind::UsedLength, 8)); }
    proof {
        reveal(prepared::account_identity);
        assert forall|i: int| 0 <= i < 8 implies #[trigger] bytes@[i] == discriminator[i] by {}
    }
    Ok(used)
}

#[inline(always)]
pub(super) fn direct_field(bytes: &[u8], discriminator: [u8; 8], minimum: u32, layout: &Layout, path: &InlinePath)
    -> (result: Result<Option<(Selection, u32)>>)
    ensures result.is_ok() && result.unwrap().is_some() ==> {
        let (s, used) = result.unwrap().unwrap();
        &&& !path.overflow && path.len == 1
        &&& 12 <= used <= bytes.len() <= u32::MAX && words::le_word(bytes@, 8) == used
        &&& prepared::account_identity(bytes@, discriminator, minimum)
        &&& selection_exact(s, bytes@.subrange(12, used as int), *layout, path.steps[0])
    },
{
    if path.overflow || path.len != 1 || !matches!(path.steps[0], Step::Field(_)) { return Ok(None); }
    let used = account_used(bytes, discriminator, minimum)?;
    let selected = match layout {
        Layout::Flat { checked } => {
            checked.witness();
            select_checked_child(&bytes[12..used as usize], *checked, path.steps[0])
        },
        _ => select_child_local(&bytes[12..used as usize], layout, path.steps[0]),
    };
    let s = selected.map_err(|e: Error| e.rebase(12))?;
    Ok(Some((s, used)))
}

/// Exact append bytes and complete physical frame, including spare capacity.
pub open spec fn tail_result(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>, count_at: u32,
    count: u32, used: u32, start: u32, end: u32, key: Seq<u8>) -> bool {
    &&& before.len() == encoded.len() == after.len()
    &&& words::encode_exact(after, 8, end)
    &&& words::encode_exact(after, count_at as int, (count + 1) as u32)
    &&& after.subrange(used as int, start as int) == key
    &&& after.subrange(start as int, end as int) == encoded.subrange(start as int, end as int)
    &&& forall|i: int| 0 <= i < before.len() && !(8 <= i < 12)
        && !(count_at <= i < count_at + 4) && !(used <= i < end) ==> #[trigger] after[i] == before[i]
}

#[inline(always)]
fn copy_four(destination: &mut [u8], source: &[u8])
    requires old(destination).len() == 4, source.len() == 4,
    ensures final(destination)@ == source@,
    no_unwind
{
    destination[0] = source[0];
    destination[1] = source[1];
    destination[2] = source[2];
    destination[3] = source[3];
    assert(destination@ =~= source@);
}

#[inline(always)]
pub(super) fn copy_key(destination: &mut [u8], key: &[u8])
    requires old(destination).len() == key.len(), key.len() <= u32::MAX,
    ensures final(destination)@ == key@,
    no_unwind
{
    let mut at = 0usize;
    while key.len() - at >= 4
        invariant at <= key.len() <= u32::MAX, destination.len() == key.len(),
            forall|i: int| 0 <= i < at ==> destination@[i] == key@[i],
        decreases key.len() - at,
    {
        let ghost before = destination@;
        let (_, suffix) = key.split_at(at);
        let (source, _) = suffix.split_at(4);
        copy_four(borrow_destination(destination, at as u32, 4), source);
        proof {
            assert forall|i: int| 0 <= i < at + 4 implies destination@[i] == key@[i] by {
                if i < at { assert(destination@[i] == before[i]); }
                else { assert(destination@[i] == source@[i - at]); }
            }
        }
        at += 4;
    }
    while at < key.len()
        invariant at <= key.len(), destination.len() == key.len(),
            forall|i: int| 0 <= i < at ==> destination@[i] == key@[i],
        decreases key.len() - at,
    {
        destination[at] = key[at];
        at += 1;
    }
    assert(destination@ =~= key@);
}

/// A checked fixed-stride tail slot; metadata is published after encoding.
#[doc(hidden)]
#[must_use = "encode the destination and finish before clearing poison"]
pub struct TailAppend<'a, 'key> {
    pub(super) bytes: &'a mut [u8],
    pub(super) count_at: u32,
    pub(super) count: u32,
    pub(super) used: u32,
    pub(super) start: u32,
    pub(super) end: u32,
    pub(super) key: &'key [u8],
    pub(super) issued: bool,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    pub(super) original: Ghost<Seq<u8>>,
}

/// Exact selection, shape, endpoint split and original bytes of a tail loan.
pub closed spec fn tail_admitted(tail: TailAppend, before: Seq<u8>, layout: Layout,
    path: InlinePath, width: u32, key: Option<&[u8]>) -> bool {
    &&& tail.original@ == before && tail.bytes@ == before && !tail.issued
    &&& !path.overflow && path.len == 1
    &&& words::le_word(before, 8) == tail.used
    &&& words::le_word(before, tail.count_at as int) == tail.count
    &&& tail.end - tail.start == width
    &&& 12 <= tail.count_at && tail.count_at + 4 <= tail.used <= tail.start <= tail.end <= before.len() <= u32::MAX
    &&& tail.count < u32::MAX && tail.start == tail.used + tail.key.len()
    &&& exists|s: Selection| #[trigger] tail_selection_matches(tail, before, layout, path, width, key, s)
}

pub(super) open spec fn tail_selection_matches(tail: TailAppend, before: Seq<u8>, layout: Layout,
    path: InlinePath, width: u32, key: Option<&[u8]>, s: Selection) -> bool {
    let local = before.subrange(tail.count_at as int, tail.used as int);
    &&& #[trigger] selection_exact(s, before.subrange(12, tail.used as int), layout, path.steps[0])
    &&& s.run.count == 0 && 12 + s.span.end == tail.used && 12 + s.span.start == tail.count_at
    &&& crate::edit_layout::header_valid(local, s.layout)
    &&& match key {
        None => tail.key.len() == 0 && match s.layout.head() {
            LayoutHead::List { element } => width_spec(element) == Some(width), _ => false,
        },
        Some(query) => tail.key@ == query@ && query.len() == crate::map_layout::key_width(s.layout)
            && (!crate::search::bool_keys(s.layout) || query@[0] <= 1)
            && match s.layout.head() {
            LayoutHead::Map { value, .. } => width_spec(value) == Some(width)
                && crate::search::search_found(crate::Search { index: tail.count, found: false }, local, s.layout, query@)
                && crate::search::search_partition(crate::Search { index: tail.count, found: false }, local, s.layout, query@),
            _ => false,
        },
    }
}

pub(super) open spec fn tail_selection(tail: TailAppend, before: Seq<u8>, layout: Layout,
    path: InlinePath, width: u32, key: Option<&[u8]>) -> Selection {
    choose|s: Selection| tail_selection_matches(tail, before, layout, path, width, key, s)
}

/// Count, old extent and value-slot coordinates of a successful admission.
pub(super) open spec fn tail_coordinates(tail: TailAppend) -> (u32, u32, u32, u32, u32) {
    (tail.count_at, tail.count, tail.used, tail.start, tail.end)
}

/// Exact original physical buffer retained by an append loan.
pub closed spec fn tail_original(tail: TailAppend) -> Seq<u8> { tail.original@ }
/// Storage retained by an append loan.
pub closed spec fn tail_storage<'a>(tail: TailAppend<'a, '_>) -> &'a mut [u8] { tail.bytes }


/// Checked publication phase of an append loan.
pub closed spec fn tail_phase(tail: TailAppend, issued: bool) -> bool { tail.issued == issued }
/// Required physical extent for an append loan.
pub closed spec fn tail_required(tail: TailAppend) -> u32 { tail.end }
/// Exact encoder borrow and unchanged append geometry.
pub closed spec fn tail_loan(before: TailAppend, after: TailAppend, initial: Seq<u8>, encoded: Seq<u8>) -> bool {
    after.issued && after.count_at == before.count_at && after.count == before.count
        && after.used == before.used && after.start == before.start && after.end == before.end
        && after.key == before.key && after.original == before.original
        && initial.len() == before.end - before.start
        && after.bytes@.subrange(before.start as int, before.end as int) == encoded
}
/// Append publication's complete byte result.
pub closed spec fn tail_finished(tail: TailAppend, after: Seq<u8>) -> bool {
    tail_result(tail.original@, tail.bytes@, after, tail.count_at, tail.count, tail.used, tail.start, tail.end, tail.key@)
}

/// Exposes the successful compact admission at the generic planning boundary.
pub(super) proof fn admitted_facts(tail: TailAppend, before: Seq<u8>, layout: Layout,
    path: InlinePath, width: u32, key: Option<&[u8]>)
    requires tail_admitted(tail, before, layout, path, width, key),
    ensures !path.overflow, path.len == 1,
        12 <= tail.count_at, tail.count_at + 4 <= tail.used <= tail.start <= tail.end <= before.len() <= u32::MAX,
        tail.count < u32::MAX, tail.start == tail.used + tail.key.len(), tail.end - tail.start == width,
        words::le_word(before, 8) == tail.used, words::le_word(before, tail.count_at as int) == tail.count,
        tail_selection_matches(tail, before, layout, path, width, key,
            tail_selection(tail, before, layout, path, width, key)),
{ }

impl<'a, 'key> TailAppend<'a, 'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        &&& self.bytes.len() == self.original@.len() <= u32::MAX
        &&& 12 <= self.count_at && self.count_at + 4 <= self.used <= self.start <= self.end <= self.bytes.len()
        &&& self.count < u32::MAX && self.start == self.used + self.key.len()
        &&& forall|i: int| 0 <= i < self.bytes.len() && !(self.start <= i < self.end)
            ==> #[trigger] self.bytes@[i] == self.original@[i]
    }

    /// Required physical extent, checked before lending the value slot.
    #[inline(always)]
    pub fn required_len(&self) -> (result: u32)
        ensures result == tail_required(*self),
    { self.end }

    /// Lends exactly the value slot once, before any metadata write.
    #[inline(always)]
    pub fn destination(&mut self) -> (result: &mut [u8])
        requires tail_phase(*old(self), false),
        ensures tail_loan(*old(self), *final(self), result@, final(result)@),
        no_unwind
    {
        proof { use_type_invariant(&*self); }
        if self.issued { vstd::pervasive::__call_panic(&["tail destination issued twice"]); }
        self.issued = true;
        borrow_destination(self.bytes, self.start, self.end - self.start)
    }

    /// Writes the key and count, then publishes used length last.
    #[inline(always)]
    pub fn finish(self)
        requires tail_phase(self, true),
        ensures tail_finished(self, final(tail_storage(self))@),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        if !self.issued { vstd::pervasive::__call_panic(&["tail finish requires destination"]); }
        let ghost original = self.original@;
        let bytes = self.bytes;
        let count_at = self.count_at;
        let count = self.count;
        let used = self.used;
        let start = self.start;
        let end = self.end;
        let key = self.key;
        let ghost encoded = bytes@;
        copy_key(borrow_destination(bytes, used, start - used), key);
        assert(bytes@.subrange(used as int, start as int) =~= key@);
        let ghost keyed = bytes@;
        assert forall|i: int| 0 <= i < bytes.len() && !(used <= i < start) implies #[trigger] bytes@[i] == encoded[i] by {};
        match crate::write_u32(bytes, count_at, count + 1) { Ok(()) => (), Err(_) => panic!("checked tail count") };
        let ghost counted = bytes@;
        match crate::write_u32(bytes, 8, end) { Ok(()) => (), Err(_) => panic!("checked tail used length") };
        proof {
            assert(words::encode_exact(bytes@, count_at as int, (count + 1) as u32));
            assert(bytes@.subrange(used as int, start as int) =~= key@);
            assert(bytes@.subrange(start as int, end as int) =~= encoded.subrange(start as int, end as int));
            assert forall|i: int| 0 <= i < bytes.len() && !(8 <= i < 12)
                && !(count_at <= i < count_at + 4) && !(used <= i < end)
                implies #[trigger] bytes@[i] == original[i] by {
                assert(bytes@[i] == encoded[i]);
            }
            assert(tail_result(original, encoded, bytes@, count_at, count, used, start, end, key@));
        }
    }
}

/// Admits a root field at the live tail when no ancestor end needs patching.
/// Capacity misses, interior selections and variable elements use the generic path.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn prepare_tail_append<'a, 'key>(bytes: &'a mut [u8], discriminator: [u8; 8], minimum: u32,
    layout: &Layout, path: &InlinePath, width: u32, key: Option<&'key [u8]>)
    -> (result: Result<Option<TailAppend<'a, 'key>>>)
    ensures result.is_err() || (result.is_ok() && result.unwrap().is_none()) ==> final(bytes)@ == old(bytes)@,
        result.is_ok() && result.unwrap().is_some() ==> tail_admitted(result.unwrap().unwrap(), old(bytes)@, *layout, *path, width, key),
{
    let (s, used) = match direct_field(bytes, discriminator, minimum, layout, path)? {
        Some(pair) => pair, None => return Ok(None),
    };
    if s.run.count != 0 || 12 + s.span.end != used { return Ok(None); }
    let count_at = 12 + s.span.start;
    let local = &bytes[count_at as usize..used as usize];
    let query = match key {
        None => {
            let element = if let Layout::Flat { checked } = s.layout {
                proof { reveal(crate::layout_head::head_spec); }
                if checked.header(0) != 7 { return Ok(None); }
                Layout::Flat { checked: checked.only_child() }
            } else { match s.layout.head() { LayoutHead::List { element } => element, _ => return Ok(None) } };
            if fixed_width(&element) != Some(width) { return Ok(None); }
            crate::list_layout::partition_list_header(local, &s.layout).map_err(|e: Error| crate::edit_layout::count_error(e).rebase(count_at))?;
            &[][..]
        },
        Some(key) => {
            let value = if let Layout::Flat { checked } = s.layout {
                proof { reveal(crate::layout_head::head_spec); }
                if checked.header(0) != 8 { return Ok(None); }
                Layout::Flat { checked: checked.only_child() }
            } else { match s.layout.head() { LayoutHead::Map { value, .. } => value, _ => return Ok(None) } };
            if fixed_width(&value) != Some(width) { return Ok(None); }
            if !crate::search::search_append(local, &s.layout, key).map_err(|e: Error| e.rebase(count_at))? {
                return Ok(None);
            }
            key
        },
    };
    let count = read_u32(bytes, count_at)?;
    let next_count = checked_delta(count, 1)?;
    let start = checked_u32(checked_add(used as usize, query.len())?)?;
    let end = checked_delta(start, width as i64)?;
    if end as usize > bytes.len() { return Ok(None); }
    let original = Ghost(bytes@);
    let _ = next_count;
    let tail = TailAppend { bytes, count_at, count, used, start, end, key: query, issued: false, original };
    proof { assert(tail_selection_matches(tail, original@, *layout, *path, width, key, s)); }
    Ok(Some(tail))
}



/// A nonempty list tail witnesses the complete append admission and result.
#[cfg(verus_only)]
#[verifier::spinoff_prover]
fn tail_list_witness()
    ensures true,
    no_unwind
{
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
        reveal_with_fuel(validation::schema_valid, 4);
        reveal_with_fuel(validation::children_valid, 5);
        reveal_with_fuel(crate::utf8::utf8_valid, 3);
    }

    let mut raw = [0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    raw[0] = 1; raw[8] = 17; raw[12] = 1; raw[16] = 7;
    let original = Ghost(raw@);
    let empty: [u8; 0] = [];
    let query = empty.as_slice();
    let tail = TailAppend { bytes: &mut raw, count_at: 12, count: 1, used: 17,
        start: 17, end: 18, key: query, issued: false, original };
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
        reveal_with_fuel(validation::schema_valid, 4);
        reveal_with_fuel(validation::children_valid, 5);
        reveal_with_fuel(crate::utf8::utf8_valid, 3);
        assert(original@ =~= witnesses::account(seq![1u8,0,0,0,7]));
        let selection = Selection { payload_len: 5, span: Span { start: 0, end: 5 }, layout: witnesses::FIXED_LIST, ordinal: 0, run: empty_run() };
        assert(tail_selection_matches(tail, original@, witnesses::list_root(), witnesses::field_path(), 1, None, selection));
        let after = witnesses::account(seq![2u8,0,0,0,7,8]);
        words::word_roundtrip(after, 8);
        words::word_roundtrip(after, 12);
        assert(words::encode_exact(after, 8, 18));
        assert(words::encode_exact(after, 12, 2));
        assert(after.subrange(17, 18) =~= original@.update(17,8).subrange(17,18));
        let live = original@.subrange(12,17);
        let root = witnesses::list_root();
        assert(validation::local_valid(live, root, true));
        assert(validation::child_count(live, root) == 1);
        assert(validation::child_layout(root, 0) == witnesses::FIXED_LIST);
        assert(validation::child_span(live, root, 0) == (Span { start: 0, end: 5 }));
        assert(live.subrange(0,5) =~= live);
        assert(validation::schema_valid(live, witnesses::FIXED_LIST, (MAX_DEPTH - 1) as nat, true));
        assert(validation::child_valid(live, root, 0, (MAX_DEPTH - 1) as nat, true));
        crate::preservation::children_from_all(live, root, 0, (MAX_DEPTH - 1) as nat, true);
        assert(after.subrange(17,17) =~= Seq::<u8>::empty());
        assert(tail_result(original@, original@.update(17,8), after, 12, 1, 17, 17, 18, Seq::empty()));
        tail_list_admission_matches_generic(tail, original@, original@.update(17, 8),
        witnesses::account(seq![2u8,0,0,0,7,8]), witnesses::list_root(), witnesses::field_path(), 1); }
}


/// A nonempty sorted map tail witnesses strict search and its exact new key.
#[cfg(verus_only)]
#[verifier::spinoff_prover]
fn tail_map_witness()
    ensures true,
    no_unwind
{
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
        reveal_with_fuel(validation::schema_valid, 4);
        reveal_with_fuel(validation::children_valid, 5);
        reveal_with_fuel(crate::utf8::utf8_valid, 3);
    }

    let mut raw = [0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    raw[0] = 1; raw[8] = 18; raw[12] = 1; raw[16] = 1; raw[17] = 7;
    let original = Ghost(raw@);
    let key = [2u8];
    let query = key.as_slice();
    let tail = TailAppend { bytes: &mut raw, count_at: 12, count: 1, used: 18,
        start: 19, end: 20, key: query, issued: false, original };
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
        reveal_with_fuel(validation::schema_valid, 4);
        reveal_with_fuel(validation::children_valid, 5);
        reveal_with_fuel(crate::utf8::utf8_valid, 3);
        assert(original@ =~= witnesses::account(seq![1u8,0,0,0,1,7]));
        let selection = Selection { payload_len: 6, span: Span { start: 0, end: 6 }, layout: witnesses::FIXED_MAP, ordinal: 0, run: empty_run() };
        assert(crate::search::same_prefix(seq![1u8], seq![2u8], 0));
        assert(crate::search::lex_less(seq![1u8], seq![2u8]));
        assert(crate::search::encoded_key(original@.subrange(12,18), witnesses::FIXED_MAP, 0) =~= seq![1u8]);
        assert(query@ =~= seq![2u8]);
        assert(selection_exact(selection, original@.subrange(12,18), witnesses::map_root(), Step::Field(0)));
        assert(crate::search::search_partition(crate::Search { index: 1, found: false }, original@.subrange(12,18), witnesses::FIXED_MAP, query@));
        assert(tail_selection_matches(tail, original@, witnesses::map_root(), witnesses::field_path(), 1, Some(query), selection));
        let after = witnesses::account(seq![2u8,0,0,0,1,7,2,8]);
        words::word_roundtrip(after, 8);
        words::word_roundtrip(after, 12);
        assert(words::encode_exact(after, 8, 20));
        assert(words::encode_exact(after, 12, 2));
        assert(after.subrange(19, 20) =~= original@.update(19,8).subrange(19,20));
        assert(after.subrange(18,19) =~= query@);
        let live = original@.subrange(12,18);
        let root = witnesses::map_root();
        assert(validation::local_valid(live, root, true));
        assert(validation::child_count(live, root) == 1);
        assert(validation::child_layout(root, 0) == witnesses::FIXED_MAP);
        assert(validation::child_span(live, root, 0) == (Span { start: 0, end: 6 }));
        assert(live.subrange(0,6) =~= live);
        assert(validation::schema_valid(live, witnesses::FIXED_MAP, (MAX_DEPTH - 1) as nat, true));
        assert(validation::child_valid(live, root, 0, (MAX_DEPTH - 1) as nat, true));
        crate::preservation::children_from_all(live, root, 0, (MAX_DEPTH - 1) as nat, true);
        tail_map_admission_matches_generic(tail, original@, original@.update(19, 8),
        witnesses::account(seq![2u8,0,0,0,1,7,2,8]), witnesses::map_root(), witnesses::field_path(), 1, query); }
}

}
