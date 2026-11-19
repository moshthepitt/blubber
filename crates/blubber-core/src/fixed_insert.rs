//! A bounded loan for a vacant fixed-map slot in the final root field.
#![cfg_attr(verus_only, allow(missing_docs))]
use super::tail_edit::{copy_key, direct_field};
use super::*;
#[cfg(verus_only)]
use crate::search;

verus! {

/// One checked insertion, retaining exact bytes until movement begins.
#[doc(hidden)]
#[must_use = "encode the destination and finish before clearing poison"]
pub struct FixedInsert<'a, 'key> {
    bytes: &'a mut [u8],
    key: &'key [u8],
    count_at: u32,
    count: u32,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    index: u32,
    used: u32,
    insert_at: u32,
    value_at: u32,
    end: u32,
    new_used: u32,
    phase: u8,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    layout: Ghost<Layout>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    original: Ghost<Seq<u8>>,
}


/// Original account and encoded value retained by the specialized insertion.
pub(super) closed spec fn insertion_original(loan: FixedInsert) -> Seq<u8> { loan.original@ }
/// Valid replacement value at the selected map's child depth.
pub(super) closed spec fn insertion_input_valid(loan: FixedInsert, plan: Plan) -> bool {
    validation::schema_valid(loan.bytes@.subrange(loan.value_at as int, loan.end as int),
        validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true)
}
/// Geometry identifying a specialized vacant slot with a checked generic plan.
pub(super) closed spec fn insertion_plan(loan: FixedInsert, plan: Plan) -> bool {
    &&& loan.valid()
    &&& plan.key@ == loan.key@
    &&& plan.old_used == loan.used && plan.new_used == loan.new_used
    &&& plan.trail.selected == loan.layout@
    &&& plan.trail.nodes[plan.trail.depth as int].region == (Span { start: loan.count_at, end: loan.used })
    &&& plan.region == (Span { start: loan.insert_at, end: loan.insert_at })
    &&& plan.replacement_len == loan.end - loan.insert_at
    &&& plan.map == Some(MapChange { operation: MapEdit::Insert, key_width: loan.key.len() as u32,
        value_width: (loan.end - loan.value_at) as u32, index: loan.index, found: false })
    &&& plan.list.is_some()
    &&& plan.list.unwrap().old_count == loan.count && plan.list.unwrap().new_count == loan.count + 1
    &&& plan.variable_map.is_none() && plan.indexed.is_none() && plan.splice.is_none()
    &&& forall|i: int| 0 <= i < plan.trail.depth ==> plan.trail.nodes[i].run.count == 0
}

/// Exact specialized writes refine generic insertion, including its physical frame.
#[verifier::spinoff_prover]
pub(super) proof fn insertion_refines_plan(loan: FixedInsert, after: Seq<u8>, plan: Plan)
    requires insertion_plan(loan, plan), replacement_checked(insertion_original(loan), plan),
        fixed_insert_finished(loan, after), insertion_input_valid(loan, plan),
    ensures plan.map.is_some(), matches!(plan.map.unwrap().operation, MapEdit::Insert),
        replacement_result(insertion_original(loan), after, after, plan), fixed_map::key_written(after, plan),
        validation::schema_valid(after.subrange(fixed_map::value_start(plan) as int,
            fixed_map::value_start(plan) + fixed_map::input_len(plan)),
            validation::child_layout(plan.trail.selected, 0), (MAX_DEPTH - plan.trail.depth - 1) as nat, true),
{
    hide(validation::schema_valid); hide(validation::local_valid);
    replacement_extent(loan.original@, plan);
    reveal(fixed_map::key_written); reveal(target); reveal(count_published); reveal(count_changed);
    assert(fixed_map::value_start(plan) == loan.value_at);
    assert(fixed_map::input_len(plan) == loan.end - loan.value_at);
    assert forall|i: int| 0 <= i < plan.trail.depth implies
        #[trigger] patch_delta(loan.original@, after, plan.trail.nodes[i].run, plan.delta,
            plan.trail.nodes[i].run.count as int) by { }
}

/// Exact storage retained by an insertion loan.
pub closed spec fn insert_storage<'a>(loan: FixedInsert<'a, '_>) -> &'a mut [u8] { loan.bytes }
/// Legal insertion phase: zero before movement, one after lending.
pub closed spec fn insert_phase(loan: FixedInsert, phase: u8) -> bool { loan.phase == phase }
/// Required physical extent of an insertion loan.
pub closed spec fn insert_required(loan: FixedInsert) -> u32 { loan.new_used }

spec fn insertion_frame(before: Seq<u8>, after: Seq<u8>, loan: FixedInsert) -> bool {
    &&& after.len() == before.len()
    &&& forall|p: int| 0 <= p < before.len() ==> {
        &&& loan.end <= p < loan.new_used ==> #[trigger] after[p]
            == before[p - (loan.end - loan.insert_at)]
        &&& !(loan.value_at <= p < loan.end) && !(loan.end <= p < loan.new_used)
            ==> after[p] == before[p]
    }
}

/// Exact encoder loan and insertion geometry across its mutable borrow.
pub closed spec fn fixed_insert_loan(before: FixedInsert, after: FixedInsert, initial: Seq<u8>, encoded: Seq<u8>) -> bool {
    &&& after.valid()
    &&& after.phase == 1 && after.original == before.original && after.layout == before.layout
    &&& after.count_at == before.count_at && after.count == before.count && after.index == before.index
    &&& after.used == before.used && after.new_used == before.new_used && after.insert_at == before.insert_at
    &&& after.value_at == before.value_at && after.end == before.end && after.key == before.key
    &&& initial.len() == before.end - before.value_at
    &&& after.bytes@.subrange(before.value_at as int, before.end as int) == encoded
}

/// Exact encoded slot, shifted suffix and physical frame after publication.
pub closed spec fn fixed_insert_finished(loan: FixedInsert, after: Seq<u8>) -> bool {
    &&& after.len() == loan.original@.len()
    &&& words::le_word(after, 8) == loan.new_used
    &&& words::le_word(after, loan.count_at as int) == loan.count + 1
    &&& after.subrange(loan.insert_at as int, loan.value_at as int) == loan.key@
    &&& after.subrange(loan.value_at as int, loan.end as int)
        == loan.bytes@.subrange(loan.value_at as int, loan.end as int)
    &&& forall|p: int| 0 <= p < after.len() ==> {
        &&& loan.end <= p < loan.new_used ==> #[trigger] after[p]
            == loan.original@[p - (loan.end - loan.insert_at)]
        &&& !(8 <= p < 12) && !(loan.count_at <= p < loan.count_at + 4)
            && !(loan.insert_at <= p < loan.new_used) ==> after[p] == loan.original@[p]
    }
}

/// Conditional sorted-map refinement of the inserted canonical record.
pub closed spec fn fixed_insert_refines(loan: FixedInsert, after: Seq<u8>) -> bool {
    let before = loan.original@.subrange(loan.count_at as int, loan.used as int);
    let live = after.subrange(loan.count_at as int, loan.new_used as int);
    search::rep_map(before, loan.layout@, search::map_model(before, loan.layout@)) ==>
        search::rep_map(live, loan.layout@, search::map_model(before, loan.layout@).insert(
            loan.key@, loan.bytes@.subrange(loan.value_at as int, loan.end as int)))
}

/// The loan selects a vacant slot in a fixed-value map at the root tail.
pub closed spec fn fixed_insert_admitted(loan: FixedInsert, before: Seq<u8>, root: Layout,
    path: InlinePath, width: u32, key: Seq<u8>) -> bool {
    &&& loan.valid()
    &&& loan.original@ == before && loan.bytes@ == before && loan.phase == 0
    &&& !path.overflow && path.len == 1 && words::le_word(before, 8) == loan.used
    &&& loan.end - loan.value_at == width && loan.key@ == key
    &&& exists|s: Selection| #[trigger] selection_exact(s, before.subrange(12, loan.used as int), root, path.steps[0])
        && s.run.count == 0 && 12 + s.span.end == loan.used
        && 12 + s.span.start == loan.count_at && s.layout == loan.layout@
}


/// Admission constructs the strict resolver trail and its exact generic map plan.
#[verifier::spinoff_prover]
pub(super) proof fn admitted_plan<'key>(loan: FixedInsert<'_, 'key>, before: Seq<u8>, root: Layout,
    path: InlinePath, width: u32, key: &'key [u8]) -> (plan: Plan<'key>)
    requires fixed_insert_admitted(loan, before, root, path, width, key@),
        !path.overflow, path.len == 1,
        validation::schema_valid(before.subrange(12, insertion_coordinates(loan).0 as int), root, MAX_DEPTH as nat, true),
        words::le_word(before, 8) == insertion_coordinates(loan).0,
    ensures insertion_plan(loan, plan), replacement_checked(before, plan),
        planned_map(plan, MapEdit::Insert, key@, width),
        admission::map_insert_ready(before, plan.trail, width, key, plan.map.unwrap().index, before.len() as u32),
        plan == admission::map_insert_plan(before, path, plan.trail, width, key, plan.map.unwrap().index, before.len() as u32),
        plan.trail.root == root, plan.new_used == insertion_coordinates(loan).1,
{
    hide(validation::schema_valid); hide(validation::local_valid);
    hide(search::lex_less); hide(search::search_found); hide(search::search_partition);
    let s = choose|s: Selection| #[trigger] selection_exact(s, before.subrange(12, loan.used as int), root, path.steps[0])
        && s.run.count == 0 && 12 + s.span.end == loan.used
        && 12 + s.span.start == loan.count_at && s.layout == loan.layout@;
    admission::direct_checked(before, root, path, loan.used, s);
    let trail = Trail { local: false, ..admission::direct_trail(before, root, path, loan.used, s) };
    let stride = (width + key.len()) as u32;
    assert(width + key.len() == loan.end - loan.insert_at);
    assert(admission::entries_ready(before, trail, ListEdit::Insert(loan.index), stride, stride, before.len() as u32)) by {
        reveal(fixed_insert_admitted);
        reveal(FixedInsert::valid);
        reveal(admission::entries_ready);
        assert(trail.depth == 1 && trail.nodes[1].region.start == loan.count_at);
        assert(stride == loan.end - loan.insert_at);
        assert(loan.used + stride == loan.new_used <= before.len());
        assert(loan.index <= loan.count < u32::MAX);
        assert forall|i: int| 0 <= i < trail.depth implies run_safe(before, trail.nodes[i].run, stride as i64)
            && run_payload(before, trail.nodes[i].run, trail.nodes[i].payload_len) by {
            assert(i == 0);
            assert(trail.nodes[i].run.count == 0);
        }
    }
    assert(admission::map_insert_ready(before, trail, width, key, loan.index, before.len() as u32));
    admission::map_insert_attachment(before, path, trail, width, key, loan.index, before.len() as u32);
    let plan = admission::map_insert_plan(before, path, trail, width, key, loan.index, before.len() as u32);
    assert(plan.region.start == loan.insert_at) by(nonlinear_arith)
        requires plan.region.start == loan.count_at + 4 + loan.index * stride,
            loan.insert_at == loan.count_at + 4 + loan.index * stride;
    plan
}

pub(super) closed spec fn insertion_layout(loan: FixedInsert) -> Layout { loan.layout@ }

/// Public coordinates needed to compose admission with the finished encoder loan.
pub(super) closed spec fn insertion_coordinates(loan: FixedInsert) -> (u32, u32, u32, u32) {
    (loan.used, loan.new_used, loan.value_at, loan.end)
}

/// Encoding changes phase and slot bytes while retaining the admitted plan.
pub(super) proof fn encoded_plan(initial: FixedInsert, encoded: FixedInsert, plan: Plan, slot: Seq<u8>)
    requires insertion_plan(initial, plan), fixed_insert_loan(initial, encoded, slot, insert_storage(encoded)@.subrange(insertion_coordinates(encoded).2 as int, insertion_coordinates(encoded).3 as int)),
    ensures insertion_plan(encoded, plan),
{
}

/// Composes admission and the exact encoder result without exposing loan fields.
#[verifier::spinoff_prover]
pub(super) proof fn admitted_preserves<'key>(initial: FixedInsert<'_, 'key>, encoded: FixedInsert,
    before: Seq<u8>, after: Seq<u8>, root: Layout, path: InlinePath, width: u32, key: &'key [u8], slot: Seq<u8>)
    requires fixed_insert_admitted(initial, before, root, path, width, key@),
        !path.overflow, path.len == 1,
        words::le_word(before, 8) == insertion_coordinates(initial).0,
        validation::schema_valid(before.subrange(12, insertion_coordinates(initial).0 as int), root, MAX_DEPTH as nat, true),
        fixed_insert_loan(initial, encoded, slot, insert_storage(encoded)@.subrange(
            insertion_coordinates(encoded).2 as int, insertion_coordinates(encoded).3 as int)),
        fixed_insert_finished(encoded, after),
        validation::schema_valid(insert_storage(encoded)@.subrange(
            insertion_coordinates(encoded).2 as int, insertion_coordinates(encoded).3 as int),
            validation::child_layout(insertion_layout(initial), 0), (MAX_DEPTH - 2) as nat, true),
    ensures validation::schema_valid(after.subrange(12, insertion_coordinates(initial).1 as int), root, MAX_DEPTH as nat, true),
{
    let plan = fixed_insert_admission_matches_generic(initial, before, root, path, width, key);
    encoded_plan(initial, encoded, plan, slot);
    fixed_insert_preserves(encoded, after, plan);
}

impl<'a, 'key> FixedInsert<'a, 'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        let before = self.original@.subrange(self.count_at as int, self.used as int);
        let stride = self.end - self.insert_at;
        &&& self.bytes.len() == self.original@.len() <= u32::MAX
        &&& 12 <= self.count_at && self.count_at + 4 <= self.insert_at
            <= self.value_at <= self.end <= self.new_used <= self.bytes.len()
        &&& self.count_at + 4 <= self.used <= self.new_used
        &&& self.count < u32::MAX && self.index <= self.count
        &&& self.value_at == self.insert_at + self.key.len()
        &&& self.new_used == self.used + stride
        &&& self.insert_at == self.count_at + 4 + self.index * stride
        &&& self.used == self.count_at + 4 + self.count * stride
        &&& self.phase <= 3
        &&& crate::map_layout::header_shape(before, self.layout@)
        &&& crate::map_layout::value_width(self.layout@) == Some((self.end - self.value_at) as u32)
        &&& self.key.len() == crate::map_layout::key_width(self.layout@)
        &&& stride == crate::map_layout::entry_stride(self.layout@)
        &&& words::le_word(before, 0) == self.count
        &&& (!search::bool_keys(self.layout@) || self.key@[0] <= 1)
        &&& search::search_found(crate::Search { index: self.index, found: false }, before, self.layout@, self.key@)
        &&& search::search_partition(crate::Search { index: self.index, found: false }, before, self.layout@, self.key@)
        &&& self.phase == 0 ==> self.bytes@ == self.original@
        &&& self.phase == 1 ==> insertion_frame(self.original@, self.bytes@, self)
    }

    /// Physical extent checked before the first write.
    #[inline(always)]
    pub fn required_len(&self) -> (result: u32)
        ensures result == insert_required(*self),
    { self.new_used }

    /// Moves the suffix once and lends only the new value slot.
    #[inline(always)]
    pub fn destination(&mut self) -> (result: &mut [u8])
        requires insert_phase(*old(self), 0),
        ensures fixed_insert_loan(*old(self), *final(self), result@, final(result)@),
            insert_phase(*final(self), 1),
        no_unwind
    {
        proof { use_type_invariant(&*self); }
        if self.phase != 0 { vstd::pervasive::__call_panic(&["fixed insertion destination issued twice"]); }
        let ghost before = self.original@;
        let source = Span { start: self.insert_at, end: self.used };
        let end = self.end;
        self.phase = 3;
        match crate::move_bytes(self.bytes, source, end) { Ok(()) => (), Err(_) => panic!("checked fixed insertion move") };
        proof { assert(insertion_frame(before, self.bytes@, *self)); }
        let value_at = self.value_at;
        self.phase = 1;
        borrow_destination(self.bytes, value_at, end - value_at)
    }

    /// Publishes the key and count, then the new used length.
    #[inline(always)]
    pub fn finish(self)
        requires insert_phase(self, 1),
        ensures fixed_insert_finished(self, final(insert_storage(self))@),
            fixed_insert_refines(self, final(insert_storage(self))@),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        if self.phase != 1 { vstd::pervasive::__call_panic(&["fixed insertion finish requires destination"]); }
        let ghost loan = self;
        let ghost encoded = self.bytes@;
        let bytes = self.bytes;
        copy_key(borrow_destination(bytes, self.insert_at, self.value_at - self.insert_at), self.key);
        let ghost keyed = bytes@;
        crate::words::write_contained_word(bytes, self.count_at as usize, self.count + 1);
        let ghost counted = bytes@;
        crate::words::write_contained_word(bytes, 8, self.new_used);
        proof {
            assert(bytes@.subrange(loan.insert_at as int, loan.value_at as int) =~= loan.key@);
            assert(bytes@.subrange(loan.value_at as int, loan.end as int) =~= encoded.subrange(loan.value_at as int, loan.end as int));
            assert forall|p: int| 0 <= p < bytes.len() implies {
                &&& loan.end <= p < loan.new_used ==> #[trigger] bytes@[p] == loan.original@[p - (loan.end - loan.insert_at)]
                &&& !(8 <= p < 12) && !(loan.count_at <= p < loan.count_at + 4)
                    && !(loan.insert_at <= p < loan.new_used) ==> bytes@[p] == loan.original@[p]
            } by {
                if loan.end <= p < loan.new_used { assert(bytes@[p] == encoded[p]); }
                if !(8 <= p < 12) && !(loan.count_at <= p < loan.count_at + 4) && !(loan.insert_at <= p < loan.new_used) {
                    assert(bytes@[p] == encoded[p]);
                }
            }
            assert(fixed_insert_finished(loan, bytes@));
            fixed_insert_model(loan, bytes@);
        }
    }
}

#[verifier::spinoff_prover]
proof fn fixed_insert_model(loan: FixedInsert, after: Seq<u8>)
    requires loan.valid(), fixed_insert_finished(loan, after),
    ensures fixed_insert_refines(loan, after),
{
    let before = loan.original@.subrange(loan.count_at as int, loan.used as int);
    let layout = loan.layout@;
    if !search::rep_map(before, layout, search::map_model(before, layout)) { return; }
    let live = after.subrange(loan.count_at as int, loan.new_used as int);
    let value = loan.bytes@.subrange(loan.value_at as int, loan.end as int);
    let change = MapChange { operation: MapEdit::Insert, key_width: loan.key.len() as u32,
        value_width: (loan.end - loan.value_at) as u32, index: loan.index, found: false };
    let stride = loan.end - loan.insert_at;
    indexed_insert::word_in_slice(after, loan.count_at as int, loan.new_used as int, 0);
    assert(words::le_word(live, 0) == loan.count + 1);
    assert(live.len() == 4 + (loan.count + 1) * stride) by(nonlinear_arith)
        requires live.len() == loan.new_used - loan.count_at,
            loan.used == loan.count_at + 4 + loan.count * stride,
            loan.new_used == loan.used + stride;
    assert(crate::map_layout::map_shape(live, layout));
    assert forall|i: int| 0 <= i < loan.count + 1 implies #[trigger]
        fixed_map::entry_matches(before, live, layout, change, loan.key@, value, i) by {
        let at = loan.count_at + 4 + i * stride;
        let kw = loan.key.len();
        let src = if i < loan.index { i } else { i - 1 };
        let source = loan.count_at + 4 + src * stride;
        assert(loan.count_at + 4 <= at && at + stride <= loan.new_used) by(nonlinear_arith)
            requires 0 <= i <= loan.count, stride >= 0,
                loan.new_used == loan.count_at + 4 + (loan.count + 1) * stride,
                at == loan.count_at + 4 + i * stride;
        assert(kw <= stride);
        assert(search::encoded_key(live, layout, i) =~= after.subrange(at, at + kw));
        assert(fixed_map::value_at(live, layout, i) =~= after.subrange(at + kw, at + stride));
        if i == loan.index {
            assert(at == loan.insert_at);
            assert(search::encoded_key(live, layout, i) == loan.key@);
            assert(fixed_map::value_at(live, layout, i) == value);
        } else {
            assert(0 <= src < loan.count);
            assert(loan.count_at + 4 <= source && source + stride <= loan.used) by(nonlinear_arith)
                requires 0 <= src < loan.count, stride >= 0,
                    loan.used == loan.count_at + 4 + loan.count * stride,
                    source == loan.count_at + 4 + src * stride;
            if i < loan.index {
                assert(at + stride <= loan.insert_at) by(nonlinear_arith)
                    requires i < loan.index, stride >= 0,
                        at == loan.count_at + 4 + i * stride,
                        loan.insert_at == loan.count_at + 4 + loan.index * stride;
            } else {
                assert(loan.end <= at) by(nonlinear_arith)
                    requires i > loan.index, stride >= 0,
                        at == loan.count_at + 4 + i * stride,
                        loan.insert_at == loan.count_at + 4 + loan.index * stride,
                        loan.end == loan.insert_at + stride;
                assert(source == at - stride) by(nonlinear_arith)
                    requires src == i - 1, source == loan.count_at + 4 + src * stride,
                        at == loan.count_at + 4 + i * stride;
            }
            assert forall|p: int| 0 <= p < stride implies #[trigger] after[at+p] == loan.original@[source+p] by {
                if i < loan.index { assert(!(loan.insert_at <= at+p < loan.new_used)); }
                else { assert(source+p == at+p-stride); }
            }
            assert(after.subrange(at, at + stride) =~= loan.original@.subrange(source, source + stride));
            assert(fixed_map::source_index(change, i) == src);
            assert(after.subrange(at, at + kw) =~= loan.original@.subrange(source, source + kw));
            assert(after.subrange(at + kw, at + stride) =~= loan.original@.subrange(source + kw, source + stride));
            assert(search::encoded_key(before, layout, src) =~= loan.original@.subrange(source, source + kw));
            assert(fixed_map::value_at(before, layout, src) =~= loan.original@.subrange(source + kw, source + stride));
        }
    }
    assert(fixed_map::entry_relation(before, live, layout, change, loan.key@, value));
    fixed_map::map_relation(before, live, layout, change, loan.key@, value);
}

/// Admits vacant insertion in a final root fixed-map field with enough capacity.
/// Other selections, occupied entries and capacity misses use the generic planner.
#[doc(hidden)]
#[inline(always)]
pub fn prepare_fixed_insert<'a, 'key>(bytes: &'a mut [u8], discriminator: [u8; 8], minimum: u32,
    layout: &Layout, path: &InlinePath, width: u32, key: &'key [u8])
    -> (result: Result<Option<FixedInsert<'a, 'key>>>)
    ensures result.is_err() || (result.is_ok() && result.unwrap().is_none()) ==> final(bytes)@ == old(bytes)@,
        result.is_ok() && result.unwrap().is_some() ==> fixed_insert_admitted(
            result.unwrap().unwrap(), old(bytes)@, *layout, *path, width, key@),
{
    let (s, used) = match direct_field(bytes, discriminator, minimum, layout, path)? {
        Some(pair) => pair, None => return Ok(None),
    };
    if s.run.count != 0 || 12 + s.span.end != used { return Ok(None); }
    let value = if let Layout::Flat { checked } = s.layout {
        proof { reveal(crate::layout_head::head_spec); }
        if checked.header(0) != 8 { return Ok(None); }
        Layout::Flat { checked: checked.only_child() }
    } else { match s.layout.head() { LayoutHead::Map { value, .. } => value, _ => return Ok(None) } };
    if fixed_width(&value) != Some(width) { return Ok(None); }
    let count_at = 12 + s.span.start;
    let local = &bytes[count_at as usize..used as usize];
    let lookup = crate::search::search_map_local(local, &s.layout, key).map_err(|e: Error| e.rebase(count_at))?;
    if lookup.found { return Ok(None); }
    let count = read_u32(bytes, count_at)?;
    let _next_count = checked_delta(count, 1)?;
    let stride = checked_u32(checked_add(key.len(), width as usize)?)?;
    let new_used = checked_delta(used, stride as i64)?;
    if new_used as usize > bytes.len() { return Ok(None); }
    let insert_at = checked_u32(checked_add(checked_add(count_at as usize, 4)?,
        checked_mul(lookup.index as usize, stride as usize)?)?)?;
    let value_at = checked_delta(insert_at, key.len() as i64)?;
    let end = checked_delta(value_at, width as i64)?;
    proof {
        indexed_insert::word_in_slice(bytes@, count_at as int, used as int, 0);
        assert(crate::map_layout::value_width(s.layout) == Some(width));
        assert(words::le_word(local@, 0) == count);
        assert(crate::map_layout::entry_stride(s.layout) == stride);
        assert(local.len() == 4 + count as int * stride) by(nonlinear_arith)
            requires crate::map_layout::header_shape(local@, s.layout),
                crate::map_layout::value_width(s.layout) == Some(width),
                key.len() == crate::map_layout::key_width(s.layout),
                stride == key.len() + width, words::le_word(local@, 0) == count,
                crate::map_layout::entry_stride(s.layout) == stride;
    }
    proof {
        assert(insert_at <= used) by(nonlinear_arith)
            requires insert_at == count_at + 4 + lookup.index as int * stride,
                used == count_at + 4 + count as int * stride, lookup.index <= count;
        assert(end <= new_used);
    }
    let original = Ghost(bytes@);
    Ok(Some(FixedInsert { bytes, key, count_at, count, index: lookup.index, used, insert_at,
        value_at, end, new_used, phase: 0, layout: Ghost(s.layout), original }))
}
/// Concrete front insertion witnesses admission, the checked Plan and root preservation.
#[cfg(verus_only)]
#[verifier::spinoff_prover]
fn front_insertion_witness()
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
    raw[0] = 1; raw[8] = 20; raw[12] = 2;
    raw[16] = 1; raw[17] = 7; raw[18] = 3; raw[19] = 9;
    let original = Ghost(raw@);
    let mut moved = raw;
    moved[18] = 1; moved[19] = 7; moved[20] = 3; moved[21] = 9; moved[17] = 8;
    let key = [0u8];
    let query = key.as_slice();
    proof {
        let local = original@.subrange(12,20);
        assert(original@ =~= witnesses::account(seq![2u8,0,0,0,1,7,3,9]));
        assert(search::encoded_key(local, witnesses::FIXED_MAP, 0) =~= seq![1u8]);
        assert(search::encoded_key(local, witnesses::FIXED_MAP, 1) =~= seq![3u8]);
        assert(search::same_prefix(seq![0u8], seq![1u8], 0));
        assert(search::lex_less(seq![0u8], seq![1u8]));
        assert(search::same_prefix(seq![0u8], seq![3u8], 0));
        assert(search::lex_less(seq![0u8], seq![3u8]));
        assert(search::same_prefix(seq![1u8], seq![2u8], 0));
        assert(search::lex_less(seq![1u8], seq![2u8]));
        assert(search::same_prefix(seq![2u8], seq![3u8], 0));
        assert(search::lex_less(seq![2u8], seq![3u8]));
        assert(search::same_prefix(seq![1u8], seq![3u8], 0));
        assert(search::lex_less(seq![1u8], seq![3u8]));
        assert(query@ =~= seq![0u8]);
        assert(crate::map_layout::header_shape(local, witnesses::FIXED_MAP));
        assert(crate::map_layout::value_width(witnesses::FIXED_MAP) == Some(1u32));
        assert(search::search_found(crate::Search { index: 0, found: false }, local, witnesses::FIXED_MAP, query@));
        assert(search::search_partition(crate::Search { index: 0, found: false }, local, witnesses::FIXED_MAP, query@));
    }
    let initial = FixedInsert { bytes: &mut raw, key: query, count_at: 12, count: 2,
        index: 0, used: 20, insert_at: 16, value_at: 17, end: 18, new_used: 22,
        phase: 0, layout: Ghost(witnesses::FIXED_MAP), original };
    let encoded = FixedInsert { bytes: &mut moved, key: query, count_at: 12, count: 2,
        index: 0, used: 20, insert_at: 16, value_at: 17, end: 18, new_used: 22,
        phase: 1, layout: Ghost(witnesses::FIXED_MAP), original };
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
        reveal_with_fuel(validation::schema_valid, 4);
        reveal_with_fuel(validation::children_valid, 5);
        reveal_with_fuel(crate::utf8::utf8_valid, 3);
        use_type_invariant(&initial);
        use_type_invariant(&encoded);
        let before = original@;
        let after = witnesses::account(seq![3u8,0,0,0,0,8,1,7,3,9]);
        let selection = Selection { payload_len: 8, span: Span { start: 0, end: 8 },
            layout: witnesses::FIXED_MAP, ordinal: 0, run: empty_run() };
        assert(selection_exact(selection, before.subrange(12,20), witnesses::map_root(), Step::Field(0)));
        assert(initial.valid());
        assert(initial.bytes@ == before);
        assert(initial.original@ == before);
        assert(initial.end - initial.value_at == 1);
        assert(initial.key@ == query@);
        assert(selection.run.count == 0 && 12 + selection.span.end == initial.used
            && 12 + selection.span.start == initial.count_at && selection.layout == initial.layout@);
        reveal(fixed_insert_admitted);
        assert(initial.phase == 0);
        assert(!witnesses::field_path().overflow && witnesses::field_path().len == 1);
        assert(words::le_word(before, 8) == initial.used);
        assert(selection_exact(selection, before.subrange(12, initial.used as int), witnesses::map_root(), witnesses::field_path().steps[0]));
        assert(fixed_insert_admitted(initial, before, witnesses::map_root(), witnesses::field_path(), 1, query@));
        assert(search::keys_ordered(before.subrange(12,20), witnesses::FIXED_MAP, 2));
        assert(validation::schema_valid(before.subrange(12,20), witnesses::FIXED_MAP, (MAX_DEPTH - 1) as nat, true));
        let live = before.subrange(12,20);
        let root = witnesses::map_root();
        assert(validation::local_valid(live, root, true));
        assert(validation::child_count(live, root) == 1);
        assert(validation::child_layout(root, 0) == witnesses::FIXED_MAP);
        assert(validation::child_span(live, root, 0) == (Span { start: 0, end: 8 }));
        assert(live.subrange(0,8) =~= live);
        assert(validation::child_valid(live, root, 0, (MAX_DEPTH - 1) as nat, true));
        crate::preservation::children_from_all(live, root, 0, (MAX_DEPTH - 1) as nat, true);
        assert(validation::schema_valid(live, root, MAX_DEPTH as nat, true));
        assert(after.subrange(16, 17) =~= query@);
        assert(after.subrange(17, 18) =~= encoded.bytes@.subrange(17, 18));
        assert(fixed_insert_finished(encoded, after));
        let plan = fixed_insert_admission_matches_generic(initial, before, witnesses::map_root(), witnesses::field_path(), 1, query);
        encoded_plan(initial, encoded, plan, seq![9u8]);
        fixed_insert_preserves(encoded, after, plan);
        fixed_insert_admission_preserves(initial, encoded, before, after, witnesses::map_root(), witnesses::field_path(), 1, query, seq![9u8]);
    }
}

broadcast use witnesses::fixed_witness_header;

/// Concrete middle insertion witnesses admission, the checked Plan and root preservation.
#[cfg(verus_only)]
#[verifier::spinoff_prover]
#[verifier::rlimit(480)]
fn middle_insertion_witness()
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
    raw[0] = 1; raw[8] = 20; raw[12] = 2;
    raw[16] = 1; raw[17] = 7; raw[18] = 3; raw[19] = 9;
    let original = Ghost(raw@);
    let mut moved = raw;
    moved[20] = 3; moved[21] = 9; moved[19] = 8;
    let key = [2u8];
    let query = key.as_slice();
    proof {
        let local = original@.subrange(12,20);
        assert(original@ =~= witnesses::account(seq![2u8,0,0,0,1,7,3,9]));
        assert(search::encoded_key(local, witnesses::FIXED_MAP, 0) =~= seq![1u8]);
        assert(search::encoded_key(local, witnesses::FIXED_MAP, 1) =~= seq![3u8]);
        assert(search::same_prefix(seq![0u8], seq![1u8], 0));
        assert(search::lex_less(seq![0u8], seq![1u8]));
        assert(search::same_prefix(seq![0u8], seq![3u8], 0));
        assert(search::lex_less(seq![0u8], seq![3u8]));
        assert(search::same_prefix(seq![1u8], seq![2u8], 0));
        assert(search::lex_less(seq![1u8], seq![2u8]));
        assert(search::same_prefix(seq![2u8], seq![3u8], 0));
        assert(search::lex_less(seq![2u8], seq![3u8]));
        assert(search::same_prefix(seq![1u8], seq![3u8], 0));
        assert(search::lex_less(seq![1u8], seq![3u8]));
        assert(query@ =~= seq![2u8]);
        assert(crate::map_layout::header_shape(local, witnesses::FIXED_MAP));
        assert(crate::map_layout::value_width(witnesses::FIXED_MAP) == Some(1u32));
        assert(search::search_found(crate::Search { index: 1, found: false }, local, witnesses::FIXED_MAP, query@));
        assert(search::search_partition(crate::Search { index: 1, found: false }, local, witnesses::FIXED_MAP, query@));
    }
    let initial = FixedInsert { bytes: &mut raw, key: query, count_at: 12, count: 2,
        index: 1, used: 20, insert_at: 18, value_at: 19, end: 20, new_used: 22,
        phase: 0, layout: Ghost(witnesses::FIXED_MAP), original };
    let encoded = FixedInsert { bytes: &mut moved, key: query, count_at: 12, count: 2,
        index: 1, used: 20, insert_at: 18, value_at: 19, end: 20, new_used: 22,
        phase: 1, layout: Ghost(witnesses::FIXED_MAP), original };
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
        reveal_with_fuel(validation::schema_valid, 4);
        reveal_with_fuel(validation::children_valid, 5);
        reveal_with_fuel(crate::utf8::utf8_valid, 3);
        use_type_invariant(&initial);
        use_type_invariant(&encoded);
        let before = original@;
        let after = witnesses::account(seq![3u8,0,0,0,1,7,2,8,3,9]);
        let selection = Selection { payload_len: 8, span: Span { start: 0, end: 8 },
            layout: witnesses::FIXED_MAP, ordinal: 0, run: empty_run() };
        assert(selection_exact(selection, before.subrange(12,20), witnesses::map_root(), Step::Field(0)));
        assert(initial.valid());
        assert(initial.bytes@ == before);
        assert(initial.original@ == before);
        assert(initial.end - initial.value_at == 1);
        assert(initial.key@ == query@);
        assert(selection.run.count == 0 && 12 + selection.span.end == initial.used
            && 12 + selection.span.start == initial.count_at && selection.layout == initial.layout@);
        reveal(fixed_insert_admitted);
        assert(initial.phase == 0);
        assert(!witnesses::field_path().overflow && witnesses::field_path().len == 1);
        assert(words::le_word(before, 8) == initial.used);
        assert(selection_exact(selection, before.subrange(12, initial.used as int), witnesses::map_root(), witnesses::field_path().steps[0]));
        assert(fixed_insert_admitted(initial, before, witnesses::map_root(), witnesses::field_path(), 1, query@));
        assert(search::keys_ordered(before.subrange(12,20), witnesses::FIXED_MAP, 2));
        assert(validation::schema_valid(before.subrange(12,20), witnesses::FIXED_MAP, (MAX_DEPTH - 1) as nat, true));
        let live = before.subrange(12,20);
        let root = witnesses::map_root();
        assert(validation::local_valid(live, root, true));
        assert(validation::child_count(live, root) == 1);
        assert(validation::child_layout(root, 0) == witnesses::FIXED_MAP);
        assert(validation::child_span(live, root, 0) == (Span { start: 0, end: 8 }));
        assert(live.subrange(0,8) =~= live);
        assert(validation::child_valid(live, root, 0, (MAX_DEPTH - 1) as nat, true));
        crate::preservation::children_from_all(live, root, 0, (MAX_DEPTH - 1) as nat, true);
        assert(validation::schema_valid(live, root, MAX_DEPTH as nat, true));
        assert(after.subrange(18, 19) =~= query@);
        assert(after.subrange(19, 20) =~= encoded.bytes@.subrange(19, 20));
        assert(fixed_insert_finished(encoded, after));
        let plan = fixed_insert_admission_matches_generic(initial, before, witnesses::map_root(), witnesses::field_path(), 1, query);
        encoded_plan(initial, encoded, plan, seq![9u8]);
        fixed_insert_preserves(encoded, after, plan);
        fixed_insert_admission_preserves(initial, encoded, before, after, witnesses::map_root(), witnesses::field_path(), 1, query, seq![9u8]);
    }
}

}
