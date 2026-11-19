//! Checked fixed-collection removals with no ancestor directory patches.
#![cfg_attr(verus_only, allow(missing_docs))]
use super::*;

verus! {

/// An unchanged collection loan, consumed only after old-value decoding.
#[doc(hidden)]
#[must_use = "decode the removed value before finishing under poison"]
pub struct TailRemove<'a> {
    bytes: &'a mut [u8],
    count_at: u32,
    count: u32,
    used: u32,
    start: u32,
    end: u32,
    value_at: u32,
    found: bool,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    original: Ghost<Seq<u8>>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    selection: Ghost<Selection>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    ordinal: Ghost<u32>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    stride: Ghost<u32>,
}

/// The complete physical result of ordered removal, including zeroed slack.
pub open spec fn removal_result(before: Seq<u8>, after: Seq<u8>, count_at: u32,
    count: u32, used: u32, start: u32, end: u32, found: bool) -> bool {
    let new_used = used - (end - start);
    &&& after.len() == before.len()
    &&& if found {
        &&& words::le_word(after, count_at as int) == count - 1
        &&& words::le_word(after, 8) == new_used
        &&& forall|p: int| 0 <= p < before.len() ==> {
            &&& start <= p < new_used ==> #[trigger] after[p] == before[p + end - start]
            &&& new_used <= p < used ==> after[p] == 0
            &&& !(8 <= p < 12) && !(count_at <= p < count_at + 4)
                && !(start <= p < used) ==> after[p] == before[p]
        }
    } else { after == before }
}

/// Original storage retained until the successful decode completes.
pub closed spec fn removal_original(loan: TailRemove) -> Seq<u8> { loan.original@ }
/// Exclusive physical storage owned by the removal.
pub closed spec fn removal_storage<'a>(loan: TailRemove<'a>) -> &'a mut [u8] { loan.bytes }
/// Exact physical result associated with a loan.
pub closed spec fn removal_finished(loan: TailRemove, after: Seq<u8>) -> bool {
    removal_result(loan.original@, after, loan.count_at, loan.count, loan.used, loan.start, loan.end, loan.found)
}
/// Required old extent, including bytes that will be cleared.
pub closed spec fn removal_required(loan: TailRemove) -> u32 { loan.used }
/// Exact old-value interval, excluding fixed-map keys.
pub closed spec fn removal_span(loan: TailRemove) -> Option<Span> {
    if loan.found { Some(Span { start: loan.value_at, end: loan.end }) } else { None }
}

/// Exact selection and fixed-stride geometry supplied by successful admission.
pub closed spec fn removal_admitted(loan: TailRemove, before: Seq<u8>, root: Layout,
    path: InlinePath, key: Option<&[u8]>) -> bool {
    let s = loan.selection@;
    let local = before.subrange(loan.count_at as int, loan.used as int);
    &&& loan.valid() && loan.original@ == before
    &&& !path.overflow && path.len == 1
    &&& selection_exact(s, before.subrange(12, loan.used as int), root, path.steps[0])
    &&& s.run.count == 0 && 12 + s.span.end == loan.used && 12 + s.span.start == loan.count_at
    &&& loan.ordinal@ <= loan.count && (loan.found ==> loan.ordinal@ < loan.count)
    &&& loan.start == loan.count_at + 4 + loan.ordinal@ as int * loan.stride@
    &&& loan.end == loan.start + if loan.found { loan.stride@ as int } else { 0int }
    &&& crate::edit_layout::header_valid(local, s.layout)
    &&& match key {
        None => loan.found && loan.value_at == loan.start && match s.layout.head() {
            LayoutHead::List { element } => width_spec(element) == Some(loan.stride@), _ => false },
        Some(query) => query.len() == crate::map_layout::key_width(s.layout)
            && loan.value_at == loan.start + if loan.found { query.len() as int } else { 0int }
            && (!crate::search::bool_keys(s.layout) || query@[0] <= 1)
            && crate::search::search_found(crate::Search { index: loan.ordinal@, found: loan.found }, local, s.layout, query@)
            && crate::search::search_partition(crate::Search { index: loan.ordinal@, found: loan.found }, local, s.layout, query@)
            && match s.layout.head() { LayoutHead::Map { key, value } => width_spec(value).is_some()
                && (loan.count > 0 ==> loan.stride@ == key.width + width_spec(value).unwrap()), _ => false },
    }
}

/// Clears exactly the vacated entry through one bounded slice loan.
#[inline(always)]
fn clear_fixed_tail(bytes: &mut [u8], start: u32, end: u32)
    requires start <= end <= old(bytes).len(),
    ensures final(bytes).len() == old(bytes).len(),
        forall|p: int| 0 <= p < old(bytes).len() ==> final(bytes)@[p]
            == if start <= p < end { 0u8 } else { old(bytes)@[p] },
    no_unwind
{
    let (_, rest) = bytes.split_at_mut(start as usize);
    let (tail, _) = rest.split_at_mut((end - start) as usize);
    let ghost original = tail@;
    let mut at = 0usize;
    while at < tail.len()
        invariant at <= tail.len(), tail.len() == original.len(),
            forall|p: int| 0 <= p < tail.len() ==> tail@[p]
                == if p < at { 0u8 } else { original[p] },
        decreases tail.len() - at,
    {
        tail[at] = 0;
        at += 1;
    }
}

impl<'a> TailRemove<'a> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        &&& self.bytes@ == self.original@
        &&& 12 <= self.count_at && self.count_at + 4 <= self.start <= self.end <= self.used <= self.bytes.len() <= u32::MAX
        &&& self.start <= self.value_at <= self.end
        &&& words::le_word(self.bytes@, 8) == self.used
        &&& words::le_word(self.bytes@, self.count_at as int) == self.count
        &&& self.found ==> self.count > 0
        &&& !self.found ==> self.start == self.end
    }

    /// The old live extent must fit both backend limits, including on absence.
    #[inline(always)]
    pub fn required_len(&self) -> (result: u32)
        ensures result == removal_required(*self),
        no_unwind
    { self.used }

    /// Borrows the complete unchanged physical buffer for Removed decoding.
    #[inline(always)]
    pub fn inspect(&self) -> (result: &[u8])
        ensures result@ == removal_original(*self),
        no_unwind
    {
        proof { use_type_invariant(self); }
        self.bytes
    }

    /// Returns only old value bytes; a missing map key has no Removed value.
    #[inline(always)]
    pub fn removed_span(&self) -> (result: Option<Span>)
        ensures result == removal_span(*self),
        no_unwind
    {
        if self.found { Some(Span { start: self.value_at, end: self.end }) } else { None }
    }

    /// Compacts in order, zeros the vacated tail and publishes used length last.
    #[inline(always)]
    pub fn finish(self)
        ensures removal_finished(self, final(removal_storage(self))@),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        if !self.found { return; }
        let bytes = self.bytes;
        let start = self.start;
        let end = self.end;
        let used = self.used;
        let count_at = self.count_at;
        let count = self.count;
        let new_used = used - (end - start);
        if end < used {
            match crate::move_bytes(bytes, Span { start: end, end: used }, start) {
                Ok(()) => (), Err(_) => panic!("checked removal move"),
            }
        }
        let ghost moved = bytes@;
        clear_tail(bytes, new_used, used);
        let ghost cleared = bytes@;
        crate::words::write_contained_word(bytes, count_at as usize, count - 1);
        let ghost counted = bytes@;
        proof {
            assert(words::le_word(bytes@, 8) == used);
            assert forall|p: int| new_used <= p < used implies bytes@[p] == 0 by {};
        }
        crate::words::write_contained_word(bytes, 8, new_used);
        proof {
            assert forall|p: int| 0 <= p < bytes.len() implies {
                &&& start <= p < new_used ==> bytes@[p] == self.original@[p + end - start]
                &&& new_used <= p < used ==> bytes@[p] == 0
                &&& !(8 <= p < 12) && !(count_at <= p < count_at + 4)
                    && !(start <= p < used) ==> bytes@[p] == self.original@[p]
            } by {
                if start <= p < new_used { assert(moved[p] == self.original@[p + end - start]); }
            }
        }
    }
    /// Removes a fixed-map entry with a reusable 3,072-byte movement buffer.
    ///
    /// Produces exactly the same bytes as [`Self::finish`], including the cleared
    /// tail and final count/used words. Decode the removed value before calling
    /// this method under the caller's poison guard.
    #[inline(always)]
    #[doc(hidden)]
    pub fn finish_fixed_map(self)
        ensures removal_finished(self, final(removal_storage(self))@),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        if !self.found { return; }
        let bytes = self.bytes;
        let start = self.start;
        let end = self.end;
        let used = self.used;
        let count_at = self.count_at;
        let count = self.count;
        let new_used = used - (end - start);
        if end < used {
            match crate::moves::move_fixed_removal(bytes, Span { start: end, end: used }, start) {
                Ok(()) => (), Err(_) => panic!("checked removal move"),
            }
        }
        let ghost moved = bytes@;
        clear_fixed_tail(bytes, new_used, used);
        let ghost cleared = bytes@;
        crate::words::write_contained_word(bytes, count_at as usize, count - 1);
        let ghost counted = bytes@;
        proof {
            assert(words::le_word(bytes@, 8) == used);
            assert forall|p: int| new_used <= p < used implies bytes@[p] == 0 by {};
        }
        crate::words::write_contained_word(bytes, 8, new_used);
        proof {
            assert forall|p: int| 0 <= p < bytes.len() implies {
                &&& start <= p < new_used ==> bytes@[p] == self.original@[p + end - start]
                &&& new_used <= p < used ==> bytes@[p] == 0
                &&& !(8 <= p < 12) && !(count_at <= p < count_at + 4)
                    && !(start <= p < used) ==> bytes@[p] == self.original@[p]
            } by {
                if start <= p < new_used { assert(moved[p] == self.original@[p + end - start]); }
            }
        }
    }
}

/// Both removal implementations produce identical bytes on arbitrary admitted storage.
proof fn fixed_removal_refinement(loan: TailRemove, generic: Seq<u8>, specialized: Seq<u8>)
    requires 12 <= loan.count_at, loan.count_at + 4 <= loan.start <= loan.end <= loan.used <= loan.original@.len(),
        removal_finished(loan, generic), removal_finished(loan, specialized),
    ensures specialized == generic,
{
    if loan.found {
        crate::words::word_roundtrip(generic, 8);
        crate::words::word_roundtrip(specialized, 8);
        crate::words::word_roundtrip(generic, loan.count_at as int);
        crate::words::word_roundtrip(specialized, loan.count_at as int);
        assert forall|p: int| 0 <= p < generic.len() implies specialized[p] == generic[p] by {
            if 8 <= p < 12 {
                if p == 8 {} else if p == 9 {} else if p == 10 {} else {}
            } else if loan.count_at <= p < loan.count_at + 4 {
                if p == loan.count_at {} else if p == loan.count_at + 1 {}
                else if p == loan.count_at + 2 {} else {}
            }
        }
        assert(specialized =~= generic);
    }
}

/// Executes both removal bodies before comparing their complete physical results.
#[cfg(verus_only)]
fn fixed_removal_witness()
    ensures true,
    no_unwind
{
    let mut generic = [1u8,0,0,0,0,0,0,0,20,0,0,0,2,0,0,0,1,7,3,9,165];
    let mut specialized = [1u8,0,0,0,0,0,0,0,20,0,0,0,2,0,0,0,1,7,3,9,165];
    let ghost before = specialized@;
    let ghost selection = Selection { payload_len:8,span:Span {start:0,end:8},
        layout:witnesses::FIXED_MAP,ordinal:0,run:empty_run() };
    let left = TailRemove { bytes:&mut generic,count_at:12,count:2,used:20,start:16,end:18,
        value_at:17,found:true,original:Ghost(before),selection:Ghost(selection),
        ordinal:Ghost(0),stride:Ghost(2) };
    let right = TailRemove { bytes:&mut specialized,count_at:12,count:2,used:20,start:16,end:18,
        value_at:17,found:true,original:Ghost(before),selection:Ghost(selection),
        ordinal:Ghost(0),stride:Ghost(2) };
    let ghost initial = right;
    left.finish();
    right.finish_fixed_map();
    proof {
        fixed_removal_refinement(initial, generic@, specialized@);
        assert(specialized@ == seq![1u8,0,0,0,0,0,0,0,18,0,0,0,1,0,0,0,3,9,0,0,165]);
    }
}

/// Admits fixed list/map removals only for direct fields ending at the live tail.
/// Map queries retain the generic local gateway and its first-fault offsets. Unsupported
/// ancestry or variable elements leave storage unchanged for the generic planner.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn prepare_tail_remove<'a>(bytes: &'a mut [u8], discriminator: [u8; 8], minimum: u32,
    layout: &Layout, path: &InlinePath, index: usize, key: Option<&[u8]>)
    -> (result: Result<Option<TailRemove<'a>>>)
    ensures result.is_err() || (result.is_ok() && result.unwrap().is_none()) ==> final(bytes)@ == old(bytes)@,
        result.is_ok() && result.unwrap().is_some() ==> removal_admitted(result.unwrap().unwrap(), old(bytes)@, *layout, *path, key),
{
    let (s, used) = match tail_edit::direct_field(bytes, discriminator, minimum, layout, path)? {
        Some(pair) => pair, None => return Ok(None),
    };
    if s.run.count != 0 || 12 + s.span.end != used { return Ok(None); }
    let count_at = 12 + s.span.start;
    let local = &bytes[count_at as usize..used as usize];
    let (index, found, stride, key_width) = match key {
        None => {
            let element = if let Layout::Flat { checked } = s.layout {
                proof { reveal(crate::layout_head::head_spec); }
                if checked.header(0) != 7 { return Ok(None); }
                Layout::Flat { checked: checked.only_child() }
            } else { match s.layout.head() { LayoutHead::List { element } => element, _ => return Ok(None) } };
            let width = match fixed_width(&element) { Some(width) => width, None => return Ok(None) };
            crate::list_layout::partition_list_header(local, &s.layout).map_err(|e: Error| crate::edit_layout::count_error(e).rebase(count_at))?;
            let count = read_u32(local, 0)?;
            if index >= count as usize { return Err(ErrorKind::IndexOutOfBounds.into()); }
            assert(local.len() == 4 + count as int * width);
            (index as u32, true, width, 0u32)
        },
        Some(key) => {
            let value = if let Layout::Flat { checked } = s.layout {
                proof { reveal(crate::layout_head::head_spec); }
                if checked.header(0) != 8 { return Ok(None); }
                Layout::Flat { checked: checked.only_child() }
            } else { match s.layout.head() { LayoutHead::Map { value, .. } => value, _ => return Ok(None) } };
            let width = match fixed_width(&value) { Some(width) => width, None => return Ok(None) };
            let lookup = crate::search_map_local(local, &s.layout, key)
                .map_err(|e: Error| e.rebase(count_at))?;
            let key_width = key.len() as u32;
            let stride = if read_u32(local, 0)? == 0 { 0u32 }
                else { checked_u32(checked_add(key_width as usize, width as usize)?)? };
            assert(local.len() == 4 + words::le_word(local@, 0) * stride) by(nonlinear_arith)
                requires crate::map_layout::map_shape(local@, s.layout),
                    crate::map_layout::value_width(s.layout) == Some(width),
                    key_width == crate::map_layout::key_width(s.layout),
                    words::le_word(local@, 0) == 0 ==> stride == 0,
                    words::le_word(local@, 0) != 0 ==> stride == key_width + width;
            assert(lookup.found ==> lookup.index < words::le_word(local@, 0));
            (lookup.index, lookup.found, stride, key_width)
        },
    };
    let count = read_u32(local, 0)?;
    proof {
        indexed_insert::word_in_slice(bytes@, count_at as int, used as int, 0);
        assert(count_at + 4 + count as int * stride == used);
        assert(index <= count);
        assert(index as int * stride <= count as int * stride) by(nonlinear_arith)
            requires index <= count;
        assert(found ==> (index + 1) as int * stride <= count as int * stride) by(nonlinear_arith)
            requires found ==> index < count;
    }
    let start = (count_at as u64 + 4 + index as u64 * stride as u64) as u32;
    proof {
        assert(start == count_at + 4 + index as int * stride);
        assert(found ==> start as int + stride <= used) by(nonlinear_arith)
            requires found ==> index < count,
                start == count_at + 4 + index as int * stride,
                used == count_at + 4 + count as int * stride;
        assert(found ==> key_width <= stride);
    }
    let end = if found { start + stride } else { start };
    let value_at = if found { start + key_width } else { start };
    let original = Ghost(bytes@);
    Ok(Some(TailRemove { bytes, count_at, count, used, start, end, value_at, found, original,
        selection: Ghost(s), ordinal: Ghost(index), stride: Ghost(stride) }))
}

/// The specialized bytes implement the existing checked sequence/map plan.
#[verifier::spinoff_prover]
#[verifier::rlimit(240)]
proof fn removal_matches_generic<'key>(loan: TailRemove, after: Seq<u8>, root: Layout,
    path: InlinePath, key: Option<&'key [u8]>) -> (plan: Plan<'key>)
    requires removal_admitted(loan, loan.original@, root, path, key), loan.found,
        removal_finished(loan, after),
        validation::schema_valid(loan.original@.subrange(12, loan.used as int), root, MAX_DEPTH as nat, true),
    ensures replacement_checked(loan.original@, plan),
        plan.list.is_some(),
        replacement_result(loan.original@, loan.original@, after, plan),
        list_model_refines(loan.original@, loan.original@, after, plan),
        plan.old_used == loan.used, plan.new_used == loan.used - (loan.end - loan.start),
        plan.trail.root == root,
        plan.trail.selected == loan.selection@.layout,
        key.is_none() ==> plan.map.is_none() && matches!(plan.list.unwrap().operation, ListEdit::Remove(_)),
        key.is_some() ==> plan.map.is_some() && matches!(plan.map.unwrap().operation, MapEdit::Remove)
            && fixed_map::key_written(loan.original@, plan)
            && fixed_map::map_result(loan.original@, loan.original@, after, plan),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    let before = loan.original@;
    let s = loan.selection@;
    let index = loan.ordinal@;
    let width = loan.stride@;
    admission::direct_checked(before, root, path, loan.used, s);
    let trail = admission::direct_trail(before, root, path, loan.used, s);
    assert(admission::entries_ready(before, trail, ListEdit::Remove(index), 0, width, before.len() as u32)) by {
        reveal(removal_admitted);
        reveal(TailRemove::valid);
        reveal(admission::entries_ready);
        assert(trail.depth == 1 && trail.nodes[1].region.start == loan.count_at);
        assert(index < loan.count);
        let indices = list_indices(ListEdit::Remove(index), loan.count);
        assert(indices.0 == index && indices.1 == index as int + 1);
        assert((indices.1 - indices.0) * width == width) by(nonlinear_arith)
            requires indices.1 - indices.0 == 1;
        assert(words::le_word(before, 8) == loan.used);
        assert(before.len() <= u32::MAX);
        assert(list_input(ListEdit::Remove(index), width) == 0);
        assert(12 <= loan.used - width <= before.len());
        assert forall|i: int| 0 <= i < trail.depth implies run_safe(before, trail.nodes[i].run, (-(width as int)) as i64)
            && run_payload(before, trail.nodes[i].run, trail.nodes[i].payload_len) by {
            assert(i == 0);
            assert(trail.nodes[i].run.count == 0);
        }
        assert(words::le_word(before, loan.count_at as int) == loan.count);
        assert(loan.count_at == 12 + s.span.start);
        assert(loan.end - loan.start == width);
    }
    admission::entries_checked(before, path, trail, ListEdit::Remove(index), 0, width, before.len() as u32);
    let base = admission::entries_plan_for(before, path, trail, ListEdit::Remove(index), 0, width, before.len() as u32);
    let plan = match key {
        None => base,
        Some(key) => {
            let value_width = width_spec(validation::child_layout(s.layout, 0)).unwrap();
            let change = MapChange { operation: MapEdit::Remove, key_width: key.len() as u32,
                value_width, index, found: true };
            Plan { key, map: Some(change), ..base }
        },
    };
    assert(replacement_checked(before, plan));
    assert(plan.region.start == loan.start);
    assert(plan.region.end == loan.end) by(nonlinear_arith)
        requires loan.end == loan.start + width,
            loan.start == loan.count_at + 4 + index as int * width,
            plan.region.end == loan.count_at + 4 + (index + 1) as int * width;
    assert forall|i: int| 0 <= i < trail.depth implies
        #[trigger] patch_delta(before, after, trail.nodes[i].run, plan.delta, trail.nodes[i].run.count as int) by {};
    assert(replacement_result(before, before, after, plan));
    list_refinement(before, before, after, plan);
    if key.is_some() {
        reveal(fixed_map::key_written);
        fixed_map::refinement(before, before, after, plan);
    }
    plan
}

/// A completed specialized removal preserves the complete checked root.
#[verifier::spinoff_prover]
proof fn removal_root_preserves(loan: TailRemove, after: Seq<u8>, root: Layout,
    path: InlinePath, key: Option<&[u8]>)
    requires removal_admitted(loan, loan.original@, root, path, key), removal_finished(loan, after),
        validation::schema_valid(loan.original@.subrange(12, loan.used as int), root, MAX_DEPTH as nat, true),
    ensures validation::schema_valid(after.subrange(12, words::le_word(after, 8)), root, MAX_DEPTH as nat, true),
{
    if !loan.found { return; }
    let plan = removal_matches_generic(loan, after, root, path, key);
    if key.is_none() { fixed_list_remove_preserves(loan.original@, loan.original@, after, plan); }
    else { fixed_map_remove_preserves(loan.original@, loan.original@, after, plan); }
}

/// Physical spare capacity and the discriminator retain their original bytes.
proof fn removal_spare_frame(before: Seq<u8>, after: Seq<u8>, count_at: u32,
    count: u32, used: u32, start: u32, end: u32, found: bool)
    requires 12 <= count_at, count_at + 4 <= start <= end <= used <= before.len(),
        removal_result(before, after, count_at, count, used, start, end, found),
    ensures after.len() == before.len(),
        after.subrange(0, 8) == before.subrange(0, 8),
        after.subrange(used as int, after.len() as int) == before.subrange(used as int, before.len() as int),
{
    assert(after.subrange(0, 8) =~= before.subrange(0, 8));
    assert(after.subrange(used as int, after.len() as int) =~= before.subrange(used as int, before.len() as int));
}

#[cfg(verus_only)]
fn removal_witness()
    ensures true,
{
    broadcast use vstd::array::group_array_axioms;
    let mut bytes = [1u8,0,0,0,0,0,0,0,18,0,0,0,2,0,0,0,7,9,165];
    let root = Layout::Struct { fixed_bytes: 0, fixed: &[], variable: &[
        crate::VariableField { declaration: 0, layout: &witnesses::FIXED_LIST }] };
    let ghost before = bytes@;
    let ghost path = witnesses::field_path();
    let ghost selection = Selection { payload_len: 6, span: Span { start:0,end:6 },
        layout: witnesses::FIXED_LIST, ordinal:0, run:empty_run() };
    let loan = TailRemove { bytes: &mut bytes, count_at:12,count:2,used:18,start:16,end:17,
        value_at:16,found:true,original:Ghost(before),selection:Ghost(selection),
        ordinal:Ghost(0),stride:Ghost(1) };
    let ghost initial = loan;
    proof {
        reveal(crate::Layout::head_spec);
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
        reveal_with_fuel(validation::schema_valid, 8);
        reveal_with_fuel(validation::child_valid, 8);
        reveal_with_fuel(validation::children_valid, 8);
        assert(removal_admitted(initial,before,root,path,None));
        assert(validation::schema_valid(before.subrange(12,18), root, MAX_DEPTH as nat, true));
    }
    loan.finish();
    proof {
        assert(validation::schema_valid(initial.original@.subrange(12,18), root, MAX_DEPTH as nat, true));
        let _ = removal_matches_generic(initial, bytes@, root, path, None);
        removal_root_preserves(initial, bytes@, root, path, None);
        removal_spare_frame(initial.original@, bytes@,12,2,18,16,17,true);
        assert(bytes@ == seq![1u8,0,0,0,0,0,0,0,17,0,0,0,1,0,0,0,9,0,165]);
    }
}

#[cfg(verus_only)]
fn removal_map_witness()
    ensures true,
{
    broadcast use vstd::array::group_array_axioms;
    let mut bytes = [1u8,0,0,0,0,0,0,0,20,0,0,0,2,0,0,0,1,7,3,9,165];
    let root = Layout::Struct { fixed_bytes:0,fixed:&[],variable:&[
        crate::VariableField { declaration:0,layout:&witnesses::FIXED_MAP }] };
    let key = [1u8];
    let ghost before = bytes@;
    let ghost path = witnesses::field_path();
    let ghost selection = Selection { payload_len:8,span:Span {start:0,end:8},
        layout:witnesses::FIXED_MAP,ordinal:0,run:empty_run() };
    let loan = TailRemove { bytes:&mut bytes,count_at:12,count:2,used:20,start:16,end:18,
        value_at:17,found:true,original:Ghost(before),selection:Ghost(selection),
        ordinal:Ghost(0),stride:Ghost(2) };
    let ghost initial = loan;
    proof {
        reveal(crate::Layout::head_spec); reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid,4);
        reveal_with_fuel(crate::layout::descriptor_fields,4);
        reveal_with_fuel(validation::schema_valid,8);
        reveal_with_fuel(validation::child_valid,8);
        reveal_with_fuel(validation::children_valid,8);
        let local = before.subrange(12,20);
        assert(key@ =~= seq![1u8]);
        assert(crate::search::encoded_key(local,witnesses::FIXED_MAP,0) =~= seq![1u8]);
        assert(crate::search::encoded_key(local,witnesses::FIXED_MAP,1) =~= seq![3u8]);
        assert(crate::search::same_prefix(seq![1u8],seq![3u8],0));
        assert(crate::search::lex_less(seq![1u8],seq![3u8]));
        assert(crate::search::keys_ordered(local,witnesses::FIXED_MAP,2));
        assert(crate::search::keys_canonical(local,witnesses::FIXED_MAP,2));
        assert(initial.valid());
        assert(selection_exact(selection,before.subrange(12,20),root,path.steps[0]));
        assert(crate::edit_layout::header_valid(local,witnesses::FIXED_MAP));
        assert(crate::search::search_found(crate::Search{index:0,found:true},local,witnesses::FIXED_MAP,key@));
        assert(crate::search::search_partition(crate::Search{index:0,found:true},local,witnesses::FIXED_MAP,key@));
        assert(validation::local_valid(local,witnesses::FIXED_MAP,true));
        assert(validation::schema_valid(local,witnesses::FIXED_MAP,(MAX_DEPTH-1) as nat,true));
        assert(removal_admitted(initial,before,root,path,Some(&key)));
        assert(validation::child_layout(root,0) == witnesses::FIXED_MAP);
        assert(validation::child_span(local,root,0) == (Span{start:0,end:8}));
        assert(validation::local_valid(local,root,true));
        assert(local.subrange(0,8) =~= local);
        assert(validation::child_valid(local,root,0,(MAX_DEPTH-1) as nat,true));
        assert(validation::children_valid(local,root,1,(MAX_DEPTH-1) as nat,true));
        assert(validation::children_valid(local,root,0,(MAX_DEPTH-1) as nat,true));
        assert(validation::schema_valid(before.subrange(12,20),root,MAX_DEPTH as nat,true));
    }
    loan.finish();
    proof {
        let _ = removal_matches_generic(initial,bytes@,root,path,Some(&key));
        removal_root_preserves(initial,bytes@,root,path,Some(&key));
        removal_spare_frame(before,bytes@,12,2,20,16,18,true);
        assert(bytes@ == seq![1u8,0,0,0,0,0,0,0,18,0,0,0,1,0,0,0,3,9,0,0,165]);
    }
}

}
