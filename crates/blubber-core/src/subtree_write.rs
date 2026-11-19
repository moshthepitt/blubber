//! Compact immediate subtree replacement with the generic plan kept in proof state.
#![cfg_attr(verus_only, allow(missing_docs))]
use super::*;

verus! {

/// An exclusive replacement loan with only active patch coordinates at runtime.
#[doc(hidden)]
#[must_use = "encode the destination and finish before clearing poison"]
pub struct SubtreeWrite<'a> {
    bytes: &'a mut [u8],
    runs: [PatchRun; MAX_DEPTH],
    depth: usize,
    region: Span,
    input: u32,
    delta: i64,
    old_used: u32,
    new_used: u32,
    required: u32,
    phase: u8,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    plan: Ghost<Plan<'static>>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    original: Ghost<Seq<u8>>,
}

/// The compact loan retains the checked generic replacement geometry.
pub(super) closed spec fn write_checked(write: SubtreeWrite) -> bool { write.valid() }

/// Exact account bytes retained by an immediate replacement.
pub closed spec fn write_original(write: SubtreeWrite) -> Seq<u8> { write.original@ }
/// The generic plan refined by this immediate execution.
pub closed spec fn write_plan(write: SubtreeWrite) -> Plan<'static> { write.plan@ }
/// Storage retained by an immediate replacement.
pub closed spec fn write_storage<'a>(write: SubtreeWrite<'a>) -> &'a mut [u8] { write.bytes }
/// Legal replacement phase: zero before movement, one after issuing the slot.
pub closed spec fn write_phase(write: SubtreeWrite, phase: u8) -> bool { write.phase == phase }
/// Required initialized physical extent, including a shrinking source tail.
pub closed spec fn write_required(write: SubtreeWrite) -> u32 { write.required }
/// Exact original bytes and requested generic replacement policy.
pub closed spec fn write_for(write: SubtreeWrite, original: Seq<u8>, used: u32,
    root: Layout, path: InlinePath, input: u32) -> bool {
    write.original@ == original && write.bytes@ == original && write.phase == 0
        && replacement_checked(original, write.plan@) && subtree_plan(write.plan@)
        && planned_for(write.plan@, used, root, path, input, u32::MAX)
}
/// The encoder changes only the checked replacement region.
pub closed spec fn write_loan(before: SubtreeWrite, after: SubtreeWrite, initial: Seq<u8>, encoded: Seq<u8>) -> bool {
    &&& after.phase == 1 && after.plan == before.plan && after.original == before.original
    &&& after.runs == before.runs && after.depth == before.depth && after.region == before.region
    &&& after.input == before.input && after.delta == before.delta
    &&& after.old_used == before.old_used && after.new_used == before.new_used && after.required == before.required
    &&& initial.len() == before.input
    &&& initial == before.original@.subrange(before.region.start as int, before.region.start + before.input)
    &&& after.bytes@.subrange(before.region.start as int, before.region.start + before.input) == encoded
}
/// Identical byte result and full physical frame to generic subtree Apply.
pub closed spec fn write_finished(write: SubtreeWrite, after: Seq<u8>) -> bool {
    replacement_result(write.original@, write.bytes@, after, write.plan@)
}

/// The private compact loan certifies its original generic plan.
pub(super) proof fn checked_plan(write: SubtreeWrite, after: Seq<u8>)
    requires write_checked(write), write_finished(write, after),
    ensures replacement_checked(write_original(write), write_plan(write)), subtree_plan(write_plan(write)),
        replacement_result(write_original(write), write_storage(write)@, after, write_plan(write)),
{ }

impl<'a> SubtreeWrite<'a> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        &&& replacement_checked(self.original@, self.plan@) && subtree_plan(self.plan@)
        &&& self.bytes.len() == self.original@.len() && self.required <= self.bytes.len()
        &&& self.region == self.plan@.region && self.input == self.plan@.replacement_len
        &&& self.delta == self.plan@.delta && self.depth == self.plan@.trail.depth
        &&& self.old_used == self.plan@.old_used && self.new_used == self.plan@.new_used && self.required == self.plan@.required
        &&& forall|i: int| 0 <= i < self.depth ==> #[trigger] self.runs[i] == self.plan@.trail.nodes[i].run
        &&& self.phase <= 3
        &&& self.phase == 0 ==> self.bytes@ == self.original@
        &&& self.phase == 1 ==> payload_frame(self.original@, self.bytes@, self.plan@)
    }

    /// Physical extent checked before poisoning or movement.
    #[inline(always)]
    pub fn required_len(&self) -> (result: u32)
        ensures result == write_required(*self),
    { self.required }

    /// Moves the suffix once and lends the replacement slot once.
    #[inline(always)]
    pub fn destination(&mut self) -> (result: &mut [u8])
        requires write_phase(*old(self), 0),
        ensures write_loan(*old(self), *final(self), result@, final(result)@), write_phase(*final(self), 1),
        no_unwind
    {
        proof { use_type_invariant(&*self); reveal(subtree_plan); reveal(target); }
        if self.phase != 0 { vstd::pervasive::__call_panic(&["subtree destination issued twice"]); }
        self.phase = 3;
        if self.delta != 0 && self.region.end < self.old_used {
            let dest = (self.region.end as i64 + self.delta) as u32;
            match crate::move_bytes(self.bytes, Span { start: self.region.end, end: self.old_used }, dest) {
                Ok(()) => (), Err(_) => panic!("checked subtree movement"),
            };
        }
        proof {
            assert(payload_frame(self.original@, self.bytes@, self.plan@));
            assert(self.bytes@.subrange(self.region.start as int,self.region.start+self.input) =~=
                self.original@.subrange(self.region.start as int,self.region.start+self.input));
        }
        self.phase = 1;
        borrow_destination(self.bytes, self.region.start, self.input)
    }

    /// Patches ancestors, publishes used length and clears any vacated tail.
    #[inline(always)]
    #[allow(non_shorthand_field_patterns)]
    pub fn finish(self)
        requires write_phase(self, 1),
        ensures write_finished(self, final(write_storage(self))@),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        let SubtreeWrite { bytes, runs, depth, delta, old_used, new_used, plan, original, .. } = self;
        finish_subtree(bytes, &runs, depth, delta, old_used, new_used, plan, original);
    }
}

#[verifier::spinoff_prover]
#[inline(always)]
pub(super) fn in_resolved<'a>(bytes: &'a mut [u8], used: u32, path: &InlinePath, trail: Trail, input: u32)
    -> (result: Result<SubtreeWrite<'a>>)
    requires trail_valid(old(bytes)@, trail.root, *path, trail, used),
        12 <= used <= old(bytes).len() <= u32::MAX, words::le_word(old(bytes)@, 8) == used,
        12 <= used as int + input - (trail.nodes[trail.depth as int].region.end - trail.nodes[trail.depth as int].region.start)
            <= old(bytes).len(),
    ensures result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> write_for(result.unwrap(), old(bytes)@, used, trail.root, *path, input),
{
    let plan = plan_in_trail(bytes, used, path, &trail, input, u32::MAX)?;
    let mut runs = [PatchRun { offset: 0, stride: 4, count: 0 }; MAX_DEPTH];
    let mut i = 0usize;
    while i < trail.depth
        invariant i <= trail.depth < MAX_DEPTH,
            plan.trail == trail,
            forall|j: int| 0 <= j < i ==> #[trigger] runs[j] == plan.trail.nodes[j].run,
        decreases trail.depth - i,
    {
        runs[i] = trail.nodes[i].run;
        i += 1;
    }
    proof { replacement_extent(bytes@, plan); reveal(target); reveal(subtree_plan); }
    let region = plan.region;
    let delta = plan.delta;
    let new_used = plan.new_used;
    let required = plan.required;
    let original = Ghost(bytes@);
    Ok(SubtreeWrite { bytes, runs, depth: trail.depth, region, input, delta,
        old_used: used, new_used, required, phase: 0, plan: Ghost(plan), original })
}

#[verifier::spinoff_prover]
fn patch_subtree_runs(bytes: &mut [u8], runs: &[PatchRun; MAX_DEPTH], depth: usize, delta: i64,
    Ghost(plan): Ghost<Plan<'static>>, Ghost(original): Ghost<Seq<u8>>)
    requires patch_geometry(original, plan), old(bytes).len() == original.len(), depth == plan.trail.depth,
        delta == plan.delta, forall|i: int| 0 <= i < depth ==> runs[i] == plan.trail.nodes[i].run,
        forall|p: int| 0 <= p < target(plan).start ==> old(bytes)@[p] == original[p],
    ensures ancestors_delta(original, final(bytes)@, plan, plan.trail.depth as int),
        ancestors_frame(old(bytes)@, final(bytes)@, plan, plan.trail.depth as int),
    no_unwind
{
    let ghost before = bytes@;
    let mut i = 0usize;
    while i < depth
        invariant i <= plan.trail.depth < MAX_DEPTH, depth == plan.trail.depth, delta == plan.delta,
            forall|j: int| 0 <= j < depth ==> runs[j] == plan.trail.nodes[j].run, patch_geometry(original, plan),
            bytes.len() == original.len(), before.len() == original.len(),
            forall|p: int| 0 <= p < target(plan).start ==> before[p] == original[p],
            ancestors_delta(original, bytes@, plan, i as int),
            ancestors_frame(before, bytes@, plan, i as int),
        decreases plan.trail.depth - i,
    {
        let run = runs[i];
        proof {
            assert forall|p: int| run_changed(run, run.count as int, p)
                implies bytes@[p] == original[p] by {
                let k = choose|k: int| 0 <= k < run.count && #[trigger] run_at(run, k) <= p < run_at(run, k) + 4;
                run_index_bounds(run, plan.trail.nodes[i as int + 1].region.start as int, k);
                if ancestor_changed(plan, i as int, p) {
                    let j = choose|j: int| 0 <= j < i && #[trigger] run_changed(plan.trail.nodes[j].run, plan.trail.nodes[j].run.count as int, p);
                    ancestor_separation(original, plan, j, i as int, p, p);
                }
            }
            assert forall|k: int| 0 <= k < run.count implies #[trigger] patch_safe_at(bytes@, run, plan.delta, k) by {
                run_index_bounds(run, plan.trail.nodes[i as int + 1].region.start as int, k);
                assert forall|p: int| run_at(run, k) <= p < run_at(run, k) + 4
                    implies bytes@[p] == original[p] by {
                    assert(run_changed(run, run.count as int, p));
                    if ancestor_changed(plan, i as int, p) {
                        let j = choose|j: int| 0 <= j < i && #[trigger] run_changed(plan.trail.nodes[j].run, plan.trail.nodes[j].run.count as int, p);
                        ancestor_separation(original, plan, j, i as int, p, p);
                    }
                }
                assert(words::le_word(bytes@, run_at(run, k)) == words::le_word(original, run_at(run, k)));
                assert(patch_safe_at(original, run, plan.delta, k));
            }
        }
        let ghost previous = bytes@;
        patch_run(bytes, run, delta);
        proof {
            assert forall|j: int| 0 <= j < i + 1 implies #[trigger] patch_delta(original, bytes@,
                plan.trail.nodes[j].run, plan.delta, plan.trail.nodes[j].run.count as int) by {
                let other = plan.trail.nodes[j].run;
                assert forall|k: int| 0 <= k < other.count implies #[trigger] words::encode_exact(bytes@,
                    run_at(other, k), (words::le_word(original, run_at(other, k)) + plan.delta) as u32) by {
                    assert(patch_safe_at(original, other, plan.delta, k));
                    if j < i {
                        assert(patch_delta(original, previous, other, plan.delta, other.count as int));
                        assert(words::encode_exact(previous, run_at(other, k),
                            (words::le_word(original, run_at(other, k)) + plan.delta) as u32));
                        assert forall|p: int| run_at(other, k) <= p < run_at(other, k) + 4
                            implies !run_changed(run, run.count as int, p) by {
                            assert(run_changed(other, other.count as int, p));
                            if run_changed(run, run.count as int, p) { ancestor_separation(original, plan, j, i as int, p, p); }
                        }
                        assert(bytes@[run_at(other, k)] == previous[run_at(other, k)]);
                        assert(bytes@[run_at(other, k) + 1] == previous[run_at(other, k) + 1]);
                        assert(bytes@[run_at(other, k) + 2] == previous[run_at(other, k) + 2]);
                        assert(bytes@[run_at(other, k) + 3] == previous[run_at(other, k) + 3]);
                    } else {
                        assert(j == i);
                        assert(patch_delta(previous, bytes@, run, plan.delta, run.count as int));
                        assert(words::le_word(previous, run_at(other, k)) == words::le_word(original, run_at(other, k)));
                        assert(words::encode_exact(bytes@, run_at(run, k),
                            (words::le_word(previous, run_at(run, k)) + plan.delta) as u32));
                    }
                }
            }
            assert forall|p: int| 0 <= p < before.len() && !ancestor_changed(plan, i as int + 1, p)
                implies bytes@[p] == before[p] by {
                if run_changed(run, run.count as int, p) { assert(ancestor_changed(plan, i as int + 1, p)); }
                if ancestor_changed(plan, i as int, p) {
                    let j = choose|j: int| 0 <= j < i && #[trigger] run_changed(plan.trail.nodes[j].run, plan.trail.nodes[j].run.count as int, p);
                    assert(ancestor_changed(plan, i as int + 1, p));
                }
            }
        }
        i += 1;
    }
}

#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
#[allow(clippy::too_many_arguments)] // The final two arguments exist only in proof state.
fn finish_subtree(bytes: &mut [u8], runs: &[PatchRun; MAX_DEPTH], depth: usize, delta: i64,
    old_used: u32, new_used: u32, Ghost(plan): Ghost<Plan<'static>>, Ghost(original): Ghost<Seq<u8>>)
    requires replacement_checked(original, plan), subtree_plan(plan), payload_frame(original, old(bytes)@, plan),
        plan.required <= old(bytes).len(), depth == plan.trail.depth, delta == plan.delta,
        old_used == plan.old_used, new_used == plan.new_used,
        forall|i: int| 0 <= i < depth ==> runs[i] == plan.trail.nodes[i].run,
    ensures replacement_result(original, old(bytes)@, final(bytes)@, plan),
    no_unwind
{
    let ghost encoded = bytes@;
    proof { replacement_geometry(original, plan); reveal(subtree_plan); }
    if delta != 0 {
        patch_subtree_runs(bytes, runs, depth, delta, Ghost(plan), Ghost(original));
    } else {
        proof { ancestors_unchanged(original, bytes@, plan); }
    }
    let ghost patched = bytes@;
    let ghost counted = bytes@;
    if delta != 0 {
        match crate::write_u32(bytes, 8, new_used) { Ok(()) => (), Err(_) => panic!("checked used length") };
    }
    proof {
        assert forall|p: int| ancestor_changed(plan, plan.trail.depth as int, p)
            implies bytes@[p] == patched[p] by {
            ancestor_range(original, plan, p);
            let j = choose|j: int| 0 <= j < plan.trail.depth && #[trigger] run_changed(plan.trail.nodes[j].run,
                plan.trail.nodes[j].run.count as int, p);
            let k = choose|k: int| 0 <= k < plan.trail.nodes[j].run.count
                && #[trigger] run_at(plan.trail.nodes[j].run, k) <= p < run_at(plan.trail.nodes[j].run, k) + 4;
            run_index_bounds(plan.trail.nodes[j].run, plan.trail.nodes[j + 1].region.start as int, k);
            trail_order(original, plan, j + 1, plan.trail.depth as int);
        }
        ancestors_reframe(original, patched, bytes@, plan);
    }
    let ghost published = bytes@;
    assert(words::le_word(published, 8) == plan.new_used);
    clear_tail(bytes, new_used, old_used);
    proof {
        assert forall|p: int| ancestor_changed(plan, plan.trail.depth as int, p)
            implies bytes@[p] == published[p] by { ancestor_range(original, plan, p); }
        ancestors_reframe(original, published, bytes@, plan);
        assert(count_published(published, plan));
        assert(count_published(bytes@, plan));
        assert(words::le_word(bytes@, 8) == plan.new_used);
        assert forall|p: int| #![trigger bytes@[p]] 0 <= p < bytes.len() implies {
            &&& (p < 8 || plan.required <= p) ==> bytes@[p] == original[p]
            &&& (12 <= p < target(plan).start && (plan.delta == 0 || !ancestor_changed(plan, plan.trail.depth as int, p))
                    && !count_changed(plan, p))
                ==> bytes@[p] == original[p]
            &&& target(plan).start <= p < target(plan).start + plan.replacement_len ==> bytes@[p] == encoded[p]
            &&& target(plan).start + plan.replacement_len <= p < plan.new_used ==> bytes@[p] == original[p - plan.delta]
            &&& plan.new_used <= p < plan.old_used ==> bytes@[p] == 0
        } by {
            if ancestor_changed(plan, plan.trail.depth as int, p) { ancestor_range(original, plan, p); }
        }
        assert(replacement_result(original, encoded, bytes@, plan));
    }
}

/// Prepares a string replacement with a proof-only traversal trail.
/// Checks ingress, selected boundaries, patch arithmetic and initialized capacity.
#[doc(hidden)]
#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline)]
pub fn prepare_byte_replacement<'a>(bytes: &'a mut [u8], discriminator: [u8; 8], minimum: u32,
    checked: crate::CheckedLayout<'static>, path: &InlinePath, input_len: usize)
    -> (result: Result<SubtreeWrite<'a>>)
    ensures result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> input_len <= u32::MAX && write_for(result.unwrap(), old(bytes)@,
            words::le_word(old(bytes)@, 8) as u32, Layout::Flat { checked }, *path, input_len as u32),
{ hide(run_safe); hide(run_payload);
    let used = super::tail_edit::account_used(bytes, discriminator, minimum)?;
    checked.witness();
    if path.overflow || path.len >= MAX_DEPTH { return Err(ErrorKind::DepthExceeded.into()); }
    let ghost node = Node { payload_len: 0, region: Span { start: 12, end: used }, #[cfg(verus_only)] layout: (Layout::Flat { checked }), ordinal: 0,
        run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let ghost empty = Node { payload_len: 0, region: Span { start: 0, end: 0 }, #[cfg(verus_only)] layout: Layout::Str,
        ordinal: 0, run: PatchRun { offset: 0, stride: 4, count: 0 } };
    let ghost mut nodes = [empty; MAX_DEPTH];
    let mut runs = [PatchRun { offset: 0, stride: 4, count: 0 }; MAX_DEPTH];
    let mut payloads = [0u32; MAX_DEPTH];
    proof { nodes = vstd::array::spec_array_update(nodes, 0, node); }
    let mut region = Span { start: 12, end: used };
    let mut active = checked;
    let mut depth = 0usize;
    while depth < path.len
        invariant
            !path.overflow, depth <= path.len < MAX_DEPTH,
            nodes[0].region == (Span { start: 12, end: used }), nodes[0].layout == (Layout::Flat { checked }),
            (Layout::Flat { checked: active }) == nodes[depth as int].layout,
            region == nodes[depth as int].region,
            forall|j: int| 0 <= j < depth ==> runs[j] == nodes[j].run && payloads[j] == nodes[j].payload_len,
            12 <= used <= bytes.len() <= u32::MAX,
            forall|i: int| #![trigger nodes[i]] 0 <= i <= depth ==> {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= nodes[depth as int].region.start
                &&& nodes[depth as int].region.end <= n.region.end
            },
            forall|i: int| 0 <= i < depth
                ==> edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]),
        decreases path.len - depth,
    {
        let ghost parent = nodes[depth as int];
        let local = &bytes[region.start as usize..region.end as usize];
        active.witness();
        let selected = select_checked_child(local, active, path.steps[depth])
            .map_err(|e: Error| e.rebase(region.start))?;
        let run = if selected.run.count == 0 { PatchRun { offset: 0, stride: 4, count: 0 } }
            else { PatchRun { offset: ((region.start as u64) + (selected.run.offset as u64)) as u32, ..selected.run } };
        runs[depth] = run;
        payloads[depth] = selected.payload_len;
        let next_region = Span { start: ((region.start as u64) + (selected.span.start as u64)) as u32,
            end: ((region.start as u64) + (selected.span.end as u64)) as u32 };
        let ghost next = Node { payload_len: 0, region: next_region, #[cfg(verus_only)] layout: selected.layout, ordinal: 0,
            run: PatchRun { offset: 0, stride: 4, count: 0 } };
        let ghost previous = nodes@;
        proof { nodes = vstd::array::spec_array_update(nodes, depth as int, Node { payload_len: selected.payload_len, ordinal: selected.ordinal, run, ..parent });
        nodes = vstd::array::spec_array_update(nodes, depth as int + 1, next);
            assert(edge_valid(bytes@, nodes[depth as int], path.steps[depth as int], nodes[depth as int + 1]));
            assert forall|i: int| 0 <= i < depth implies
                edge_valid(bytes@, nodes[i], path.steps[i], nodes[i + 1]) by {
                assert(edge_valid(bytes@, previous[i], path.steps[i], previous[i + 1]));
                if i + 1 == depth {
                    assert(nodes[i + 1].region == previous[i + 1].region);
                    assert(nodes[i + 1].layout == previous[i + 1].layout);
                }
            }
            assert forall|i: int| #![trigger nodes[i]] 0 <= i <= depth + 1 implies {
                let n = nodes[i];
                &&& 12 <= n.region.start <= n.region.end <= used
                &&& n.region.start <= next.region.start
                &&& next.region.end <= n.region.end
            } by {
                if i <= depth {
                    assert(previous[i].region == nodes[i].region);
                    assert(previous[i].region.start <= parent.region.start);
                    assert(parent.region.end <= previous[i].region.end);
                }
            }
        }
        region = next_region;
        active = match selected.layout { Layout::Flat { checked } => { checked.witness(); checked }, _ => { assert(false); return Err(ErrorKind::InvalidSchema.into()); } };
        depth += 1;
    }
    let ghost target = nodes[depth as int];
    if active.header(0) != 5 { return Err(ErrorKind::InvalidSchema.into()); }
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        assert(crate::edit_layout::header_valid(bytes@.subrange(region.start as int, region.end as int),
            Layout::Flat { checked: active }));
    }

    let ghost trail = Trail { root: (Layout::Flat { checked }), selected: Layout::Flat { checked: active }, local: true, nodes, depth };
    proof {
        assert(trail_valid(bytes@, (Layout::Flat { checked }), *path, trail, used));
        assert(resolve_path(Ok(region), bytes@, (Layout::Flat { checked }), *path));
    }
    let input = checked_u32(input_len)?;
    let delta = input as i64 - (region.end - region.start) as i64;
    let new_used = checked_delta(used, delta)?;
    let required = if new_used > used { new_used } else { used };
    if required as usize > bytes.len() { return Err(ErrorKind::Capacity.into()); }
    let mut i = 0usize;
    while i < depth
        invariant i <= depth < MAX_DEPTH, trail.depth == depth, trail.nodes == nodes,
            trail_valid(bytes@, Layout::Flat { checked }, *path, trail, used),
            bytes.len() <= u32::MAX,
            forall|j: int| 0 <= j < depth ==> runs[j] == nodes[j].run && payloads[j] == nodes[j].payload_len,
            forall|j: int| 0 <= j < i ==> run_safe(bytes@, nodes[j].run, delta)
                && (delta != 0 ==> run_payload(bytes@, nodes[j].run, nodes[j].payload_len)),
        decreases depth - i,
    {
        proof {
            assert(edge_valid(bytes@, nodes[i as int], path.steps[i as int], nodes[i as int+1]));
            assert(run_within(runs[i as int], 0, bytes.len() as int));
        }
        check_run(bytes, runs[i], delta, payloads[i])?;
        i += 1;
    }
    let ghost plan = Plan { key: &[], map: None, variable_map: None, region, indexed: None,
        splice: None, list: None, trail, path: *path,
        discriminator: [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]],
        old_used: used, replacement_len: input, delta, new_used, required, storage_limit: u32::MAX };
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        assert(width_spec(Layout::Flat { checked: active }).is_none());
        assert(replacement_checked(bytes@, plan));
        assert(subtree_plan(plan));
        assert(planned_for(plan, used, Layout::Flat { checked }, *path, input, u32::MAX));
    }
    let original = Ghost(bytes@);
    Ok(SubtreeWrite { bytes, runs, depth, region, input, delta, old_used: used, new_used, required,
        phase: 0, plan: Ghost(plan), original })
}


/// Concrete storage witnesses the compact subtree loan's full preservation premise.
#[cfg(verus_only)]
#[verifier::spinoff_prover]
fn subtree_loan_witness()
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
    raw[0] = 1; raw[8] = 13; raw[12] = 7;
    let original = Ghost(raw@);
    proof { assert(raw@ =~= witnesses::account(seq![7u8])); }
    let plan = Ghost(witnesses::replacement(original@, witnesses::BYTE, 1));
    #[cfg(feature = "max-depth-16")]
    let runs = [PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }];
    #[cfg(not(feature = "max-depth-16"))]
    let runs = [PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }, PatchRun { offset: 0, stride: 4, count: 0 }];
    let write = SubtreeWrite { bytes: &mut raw, runs, depth: 0,
        region: Span { start: 12, end: 13 }, input: 1, delta: 0, old_used: 13, new_used: 13,
        required: 13, phase: 1, plan, original };
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
        reveal_with_fuel(validation::schema_valid, 4);
        reveal_with_fuel(validation::children_valid, 5);
        reveal_with_fuel(crate::utf8::utf8_valid, 3); subtree_write_preserves(write, original@); }
}

}
