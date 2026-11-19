//! Compact immediate subtree replacement with the generic plan kept in proof state.
#![cfg_attr(verus_only, allow(missing_docs))]
use super::*;

verus! {

/// The admitted numeric interval makes every accepted end representable.
pub(super) open spec fn bounds_admitted(upper: u32, delta: i64) -> bool {
    let lower = if delta < 0 { -delta } else { 0int };
    0 <= lower <= upper && upper as int + delta <= u32::MAX
}
/// Omits an empty suffix of patch runs and every run for an equal-size edit.
#[inline(always)]
fn active_depth(runs:&[PatchRun;MAX_DEPTH],depth:usize,delta:i64)->(result:usize)
    requires depth<MAX_DEPTH,
    ensures result<=depth,delta==0 ==> result==0,
        forall|i:int| result<=i<depth ==> delta==0 || #[trigger] runs[i].count==0,
    no_unwind
{
    if delta==0{return 0;}
    let mut active=depth;
    while active>0
        invariant delta!=0,active<=depth<MAX_DEPTH,
            forall|i:int| active<=i<depth ==> #[trigger] runs[i].count==0,
        decreases active,
    {
        if runs[active-1].count>0{return active;}
        active-=1;
    }
    active
}

proof fn admitted_order(bytes: Seq<u8>, plan: Plan, i: int, j: int)
    requires replacement_admitted(bytes, plan), 0 <= i <= j <= plan.trail.depth,
    ensures plan.trail.nodes[i].region.start <= plan.trail.nodes[j].region.start,
    decreases j - i,
{
    if i < j {
        admitted_order(bytes, plan, i, j - 1);
        assert(edge_valid(bytes, plan.trail.nodes[j - 1], plan.path.steps[j - 1], plan.trail.nodes[j]));
    }
}

/// Separation and physical extent needed by ancestor publication.
pub(super) open spec fn admitted_geometry(original: Seq<u8>, plan: Plan) -> bool {
    &&& plan.trail.depth < MAX_DEPTH
    &&& target(plan).start <= original.len() <= u32::MAX
    &&& forall|j: int| 0 <= j < plan.trail.depth ==> {
        &&& run_within(plan.trail.nodes[j].run, plan.trail.nodes[j].region.start as int,
            plan.trail.nodes[j + 1].region.start as int)
        &&& 12 <= plan.trail.nodes[j].region.start
        &&& plan.trail.nodes[j + 1].region.start <= target(plan).start <= original.len() <= u32::MAX
    }
    &&& forall|i: int, j: int| 0 <= i <= j <= plan.trail.depth
        ==> plan.trail.nodes[i].region.start <= plan.trail.nodes[j].region.start
}

/// Derives ancestor separation from header and selected-child admission.
pub(super) proof fn admitted_geometry_proof(original: Seq<u8>, plan: Plan)
    requires replacement_admitted(original, plan),
    ensures admitted_geometry(original, plan),
{
    assert forall|i: int, j: int| 0 <= i <= j <= plan.trail.depth
        implies plan.trail.nodes[i].region.start <= plan.trail.nodes[j].region.start by {
        admitted_order(original, plan, i, j);
    }
    assert forall|j: int| 0 <= j < plan.trail.depth implies {
        &&& run_within(plan.trail.nodes[j].run, plan.trail.nodes[j].region.start as int,
            plan.trail.nodes[j + 1].region.start as int)
        &&& 12 <= plan.trail.nodes[j].region.start
        &&& plan.trail.nodes[j + 1].region.start <= target(plan).start <= original.len() <= u32::MAX
    } by {
        assert(edge_valid(original, plan.trail.nodes[j], plan.path.steps[j], plan.trail.nodes[j + 1]));
    }
}

proof fn admitted_separation(bytes: Seq<u8>, plan: Plan, i: int, j: int, p: int, q: int)
    requires admitted_geometry(bytes, plan), 0 <= i < j < plan.trail.depth,
        run_changed(plan.trail.nodes[i].run, plan.trail.nodes[i].run.count as int, p),
        run_changed(plan.trail.nodes[j].run, plan.trail.nodes[j].run.count as int, q),
    ensures p < q,
{
    let a = plan.trail.nodes[i].run;
    let b = plan.trail.nodes[j].run;
    let k = choose|k: int| 0 <= k < a.count && #[trigger] run_at(a, k) <= p < run_at(a, k) + 4;
    let l = choose|l: int| 0 <= l < b.count && #[trigger] run_at(b, l) <= q < run_at(b, l) + 4;
    run_index_bounds(a, plan.trail.nodes[i + 1].region.start as int, k);
    assert(b.offset >= plan.trail.nodes[j].region.start);
}


/// An exclusive replacement loan with only active patch coordinates at runtime.
#[doc(hidden)]
#[must_use = "encode the destination and finish before clearing poison"]
#[repr(C)]
pub struct CheckedSubtreeWrite<'a> {
    bytes: &'a mut [u8],
    runs: [PatchRun; MAX_DEPTH],
    payloads: [u32; MAX_DEPTH],
    depth: usize,
    region: Span,
    input: u32,
    delta: i64,
    old_used: u32,
    new_used: u32,
    required: u32,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    phase: Ghost<u8>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    plan: Ghost<Plan<'static>>,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    original: Ghost<Seq<u8>>,
}

/// A prepared subtree replacement; movement has not begun.
#[doc(hidden)]
#[repr(transparent)]
pub struct CheckedSubtreeReady<'a> { pub(super) inner: CheckedSubtreeWrite<'a> }
/// A moved subtree replacement; its destination has not been issued.
#[doc(hidden)]
#[repr(transparent)]
pub struct CheckedSubtreeMoved<'a> { inner: CheckedSubtreeWrite<'a> }
/// Proof view of the sealed prepared phase.
pub closed spec fn ready_value<'a>(value: CheckedSubtreeReady<'a>) -> CheckedSubtreeWrite<'a> { value.inner }
/// Proof view of the sealed moved phase.
pub closed spec fn moved_value<'a>(value: CheckedSubtreeMoved<'a>) -> CheckedSubtreeWrite<'a> { value.inner }
impl<'a> CheckedSubtreeReady<'a> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool { write_phase(self.inner, 0) }
    /// Checks the old source before an ordinary pre-write rejection.
    pub fn check_before_error(&self, poisoned: &mut bool) -> (r: Result<()>)
        ensures *old(poisoned) ==> *final(poisoned),
            r.is_ok() ==> *final(poisoned) == *old(poisoned),
    { proof { use_type_invariant(self); } self.inner.check_source_before_error(poisoned) }
    /// Initialized physical extent needed by this request.
    #[inline(always)]
    pub fn required_len(&self) -> (n: u32)
        ensures n == write_required(ready_value(*self)),
    { self.inner.required_extent() }
    /// Consumes preparation and moves the suffix under sticky poison.
    #[inline(always)]
    #[allow(non_shorthand_field_patterns)]
    pub fn move_regions(self, poisoned: &mut bool) -> (r: CheckedSubtreeMoved<'a>)
        ensures *final(poisoned), write_phase(moved_value(r), 1),
        no_unwind
    { proof { use_type_invariant(&self); } let CheckedSubtreeReady { mut inner } = self; inner.move_inner(poisoned); CheckedSubtreeMoved { inner } }
}
impl<'a> CheckedSubtreeMoved<'a> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool { write_phase(self.inner, 1) }
    /// Issues the destination once; only this state can encode and finish.
    #[inline(always)]
    #[allow(non_shorthand_field_patterns)]
    pub fn destination(self) -> (r: CheckedSubtreeWrite<'a>)
        ensures write_phase(r, 1),
        no_unwind
    { proof { use_type_invariant(&self); } self.inner }
}

#[inline(always)]
pub(super) fn seal_ready(inner: CheckedSubtreeWrite) -> (r: CheckedSubtreeReady)
    requires write_phase(inner, 0),
    ensures ready_value(r) == inner,
{ CheckedSubtreeReady { inner } }
/// Exact width of the issued subtree destination.
pub closed spec fn write_input(write: CheckedSubtreeWrite) -> u32 { write.input }

/// The compact loan retains the checked generic replacement geometry.
pub(super) closed spec fn write_checked(write: CheckedSubtreeWrite) -> bool { write.valid() }

/// Exact account bytes retained by an immediate replacement.
pub closed spec fn write_original(write: CheckedSubtreeWrite) -> Seq<u8> { write.original@ }
/// The generic plan refined by this immediate execution.
pub closed spec fn write_plan(write: CheckedSubtreeWrite) -> Plan<'static> { write.plan@ }
/// Storage retained by an immediate replacement.
pub closed spec fn write_storage<'a>(write: CheckedSubtreeWrite<'a>) -> &'a mut [u8] { write.bytes }
/// Legal replacement phase: zero before movement, one after issuing the slot.
pub closed spec fn write_phase(write: CheckedSubtreeWrite, phase: u8) -> bool { write.phase@ == phase }
/// Required initialized physical extent, including a shrinking source tail.
pub closed spec fn write_required(write: CheckedSubtreeWrite) -> u32 { write.required }
/// Exact original bytes and requested generic replacement policy.
pub closed spec fn write_for(write: CheckedSubtreeWrite, original: Seq<u8>, used: u32,
    root: Layout, path: InlinePath, input: u32) -> bool {
    write.original@ == original && write.bytes@ == original && write.phase@ == 0
        && replacement_admitted(original, write.plan@) && subtree_plan(write.plan@)
        && planned_for(write.plan@, used, root, path, input, u32::MAX)
}
/// The encoder changes only the checked replacement region.
pub closed spec fn write_loan(before: CheckedSubtreeWrite, after: CheckedSubtreeWrite, initial: Seq<u8>, encoded: Seq<u8>) -> bool {
    &&& after.phase@ == 1 && after.plan == before.plan && after.original == before.original
    &&& after.runs == before.runs && after.payloads == before.payloads && after.depth == before.depth && after.region == before.region
    &&& after.input == before.input && after.delta == before.delta
    &&& after.old_used == before.old_used && after.new_used == before.new_used && after.required == before.required
    &&& initial.len() == before.input
    &&& initial == before.bytes@.subrange(before.region.start as int, before.region.start + before.input)
    &&& after.bytes@.subrange(before.region.start as int, before.region.start + before.input) == encoded
}
/// Identical byte result and full physical frame to generic subtree Apply.
pub closed spec fn write_finished(write: CheckedSubtreeWrite, after: Seq<u8>) -> bool {
    replacement_result(write.original@, write.bytes@, after, write.plan@)
}

impl<'a> CheckedSubtreeWrite<'a> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        &&& replacement_admitted(self.original@, self.plan@) && subtree_plan(self.plan@)
        &&& self.bytes.len() == self.original@.len() && self.required <= self.bytes.len()
        &&& self.region == self.plan@.region && self.input == self.plan@.replacement_len
        &&& self.delta == self.plan@.delta && self.depth <= self.plan@.trail.depth
        &&& self.old_used == self.plan@.old_used && self.new_used == self.plan@.new_used && self.required == self.plan@.required
        &&& forall|i: int| 0 <= i < self.depth ==> #[trigger] self.runs[i] == self.plan@.trail.nodes[i].run
        &&& forall|i: int| 0 <= i < self.depth ==> #[trigger] self.payloads[i] == self.plan@.trail.nodes[i].payload_len
        &&& forall|i: int| 0 <= i < self.depth && self.runs[i].count > 0 ==>
            #[trigger] bounds_admitted(self.payloads[i], self.delta)
        &&& forall|i: int| 0 <= i < self.depth && self.runs[i].stride != 4 ==>
            #[trigger] run_safe(self.original@, self.runs[i], self.delta)
                && (self.delta != 0 ==> run_payload(self.original@, self.runs[i], self.payloads[i]))
        &&& forall|i:int| self.depth<=i<self.plan@.trail.depth ==>
            self.delta==0 || #[trigger] self.plan@.trail.nodes[i].run.count==0
        &&& self.phase@ <= 3
        &&& self.phase@ == 0 ==> self.bytes@ == self.original@
        &&& self.phase@ == 1 ==> payload_frame(self.original@, self.bytes@, self.plan@)
    }

    /// Preserves the old ancestor error order on ordinary pre-write rejection.
    fn check_source_before_error(&self,poisoned:&mut bool)->(result:Result<()>)
        requires write_phase(*self,0),
        ensures *old(poisoned) ==> *final(poisoned),
            result.is_ok() ==> *final(poisoned)==*old(poisoned),
    {
        proof{use_type_invariant(self);ancestor_edit::scan_geometry(self.original@,self.plan@);}
        let mut i=0usize;
        while i<self.depth
            invariant i<=self.depth<MAX_DEPTH,self.bytes@==self.original@,
                *poisoned==*old(poisoned),
                replacement_admitted(self.original@,self.plan@),self.depth<=self.plan@.trail.depth,
                self.delta==self.plan@.delta,
                forall|j:int| 0<=j<self.depth ==> #[trigger] self.runs[j]==self.plan@.trail.nodes[j].run
                    && self.payloads[j]==self.plan@.trail.nodes[j].payload_len,
                forall|j:int| 0<=j<self.depth ==> #[trigger] run_within(self.runs[j],0,self.bytes.len() as int),
            decreases self.depth-i,
        {
            match check_run(self.bytes,self.runs[i],self.delta,self.payloads[i]) {
                Ok(())=>(),Err(error)=>{list_patch::source_error(error,poisoned);return Err(error);},
            }
            i+=1;
        }
        Ok(())
    }

    /// Physical extent checked before poisoning or movement.
    #[inline(always)]
    fn required_extent(&self) -> (result: u32)
        ensures result == write_required(*self),
    { self.required }

    /// Moves the suffix once and lends the replacement slot once.
    #[inline(always)]
    #[verifier::rlimit(160)]
    fn move_inner(&mut self, poisoned: &mut bool)
        requires write_phase(*old(self), 0),
        ensures *final(poisoned), write_phase(*final(self), 1),
        no_unwind
    {
        proof { use_type_invariant(&*self); reveal(subtree_plan); reveal(target); reveal(replacement_admitted); }
        *poisoned = true;
        proof { self.phase = Ghost(3u8); }
        if self.delta != 0 && self.region.end < self.old_used {
            let dest = (self.region.end as i64 + self.delta) as u32;
            match crate::move_bytes(self.bytes, Span { start: self.region.end, end: self.old_used }, dest) {
                Ok(()) => (), Err(_) => panic!("checked subtree movement"),
            };
        }
        proof {
            use_type_invariant(&*self);
            assert(payload_frame(self.original@, self.bytes@, self.plan@));
            assert(self.bytes@.subrange(self.region.start as int,self.region.start+self.input) =~=
                self.original@.subrange(self.region.start as int,self.region.start+self.input));
        }
        proof { self.phase = Ghost(1u8); }

    }

    /// Borrows the issued destination; the parent loan cannot publish while borrowed.
    #[inline(always)]
    pub fn bytes(&mut self) -> (result: &mut [u8])
        requires write_phase(*old(self), 1),
        ensures write_phase(*final(self), 1), result.len() == write_input(*old(self)),
            write_loan(*old(self), *final(self), result@, final(result)@),
        no_unwind
    {
        proof { use_type_invariant(&*self); reveal(subtree_plan); reveal(target); }
        borrow_destination(self.bytes, self.region.start, self.input)
    }

    /// Patches ancestors, publishes used length and clears any vacated tail.
    #[inline(always)]
    #[allow(non_shorthand_field_patterns)]
    pub fn finish(self, poisoned: &mut bool) -> (result: Result<()>)
        requires write_phase(self, 1), *old(poisoned),
        ensures *final(poisoned) == result.is_err(),
            replacement_checked(write_original(self),write_plan(self)) ==> result.is_ok(),
            result.is_ok() ==> write_finished(self, final(write_storage(self))@)
            && replacement_checked(write_original(self), write_plan(self)),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        if self.delta==0 {
            let CheckedSubtreeWrite {bytes,runs,payloads,old_used,new_used,plan,original,..}=self;
            let outcome=finish_subtree(bytes,&runs,&payloads,0,0,old_used,new_used,plan,original);
            return crate::patch_pass::finish_publication(poisoned,outcome);
        }
        let CheckedSubtreeWrite { bytes, runs, payloads, depth, delta, old_used, new_used, plan, original, .. } = self;
        let outcome = finish_subtree(bytes, &runs, &payloads, depth, delta, old_used, new_used, plan, original);
        // control: ancestor-publication
        crate::patch_pass::finish_publication(poisoned, outcome)
    }
}

/// Lifts checked local end bounds into the original account coordinates.
#[verifier::spinoff_prover]
proof fn checked_source(before: Seq<u8>, local: Seq<u8>, run: PatchRun, lower: u32, upper: u32, delta: i64)
    requires run.stride == 4, run.count > 0, run_within(run,0,before.len() as int),
        local == before.subrange(run.offset as int, run.offset + 4*run.count),
        crate::ancestor_pass::ends_within(local,lower,upper),
        0 <= lower as int + delta, upper as int + delta <= u32::MAX,
    ensures run_safe(before,run,delta), run_payload(before,run,upper),
{
    reveal(run_safe); reveal(run_payload); reveal(patch_safe_at);
    assert forall|k: int| 0 <= k < run.count implies #[trigger] patch_safe_at(before,run,delta,k) by {
        run_index_bounds(run,before.len() as int,k);
        indexed_insert::word_in_slice(before,run.offset as int,run.offset + 4*run.count,4*k);
        assert(lower <= words::le_word(local,4*k) <= upper);
    }
    assert forall|k: int| 0 <= k < run.count implies #[trigger]
        words::le_word(before,run.offset+k*run.stride) <= upper by {
        indexed_insert::word_in_slice(before,run.offset as int,run.offset + 4*run.count,4*k);
        assert(lower <= words::le_word(local,4*k) <= upper);
    }
}

/// Lifts the exact changed directory into the complete account image.
#[verifier::spinoff_prover]
proof fn checked_run_delta(before: Seq<u8>, after: Seq<u8>, local: Seq<u8>, patched: Seq<u8>, run: PatchRun, delta: i64)
    requires run.stride == 4, run.count > 0, before.len() == after.len(),
        run_within(run,0,before.len() as int),
        local == before.subrange(run.offset as int,run.offset+4*run.count),
        patched == after.subrange(run.offset as int,run.offset+4*run.count),
        crate::ancestor_pass::shifted(local,patched,delta,run.count as int),
    ensures patch_delta(before,after,run,delta,run.count as int),
{
    reveal(patch_delta);
    assert forall|k: int| 0 <= k < run.count implies #[trigger]
        words::encode_exact(after,run_at(run,k),(words::le_word(before,run_at(run,k))+delta) as u32) by {
        indexed_insert::word_in_slice(before,run.offset as int,run.offset+4*run.count,4*k);
        indexed_insert::word_in_slice(after,run.offset as int,run.offset+4*run.count,4*k);
        words::word_roundtrip(after,run_at(run,k));
        assert(words::le_word(after,run_at(run,k)) == words::le_word(before,run_at(run,k))+delta);
    }
}

/// An exact contiguous destination loan preserves every byte outside its run.
#[verifier::spinoff_prover]
proof fn checked_frame(before: Seq<u8>, after: Seq<u8>, run: PatchRun)
    requires run.stride == 4, run.count > 0, before.len() == after.len(),
        run_within(run,0,before.len() as int),
        forall|p: int| 0 <= p < before.len() && !(run.offset <= p < run.offset+4*run.count)
            ==> #[trigger] after[p] == before[p],
    ensures patch_frame(before,after,run,run.count as int),
{
    reveal(patch_frame);
    assert forall|p: int| 0 <= p < before.len() && !run_changed(run,run.count as int,p)
        implies #[trigger] after[p] == before[p] by {
        if run.offset <= p < run.offset+4*run.count {
            let k = (p-run.offset)/4;
            assert(0 <= k < run.count);
            assert(run_at(run,k) <= p < run_at(run,k)+4);
            assert(run_changed(run,run.count as int,p));
        }
    }
}

/// The old checked partition supplies the interval accepted by the fused pass.
#[verifier::spinoff_prover]
proof fn admitted_source_bounds(before: Seq<u8>, local: Seq<u8>, run: PatchRun, lower: u32, upper: u32, delta: i64)
    requires run.stride==4, run.count>0, run_within(run,0,before.len() as int),
        local==before.subrange(run.offset as int,run.offset+4*run.count),
        run_safe(before,run,delta), run_payload(before,run,upper),
        lower as int==if delta<0 { -delta as int } else { 0int },
    ensures crate::ancestor_pass::ends_within(local,lower,upper),
{
    reveal(run_safe); reveal(run_payload); reveal(patch_safe_at);
    assert forall|k: int| 0<=k<run.count implies lower<=#[trigger] words::le_word(local,4*k)<=upper by {
        run_index_bounds(run,before.len() as int,k);
        indexed_insert::word_in_slice(before,run.offset as int,run.offset+4*run.count,4*k);
        assert(patch_safe_at(before,run,delta,k));
        assert(words::le_word(before,run.offset+k*run.stride)<=upper);
    }
}

/// Valid old ancestor partitions are ghost evidence, never a runtime scan.
proof fn partition_runs(original: Seq<u8>, plan: Plan)
    requires replacement_checked(original,plan), plan.delta!=0,
    ensures forall|i: int| 0<=i<plan.trail.depth ==> #[trigger] run_safe(original,plan.trail.nodes[i].run,plan.delta)
        && run_payload(original,plan.trail.nodes[i].run,plan.trail.nodes[i].payload_len),
{ reveal(replacement_checked); }

/// A private-field subtree request whose ancestor values remain unchecked.
#[doc(hidden)]
#[derive(Debug)]
pub struct CheckedSubtreePlan { inner: Plan<'static> }

/// Numeric and selected-header evidence captured before releasing the byte loan.
pub closed spec fn deferred_admitted(bytes:Seq<u8>,plan:CheckedSubtreePlan)->bool {
    replacement_admitted(bytes,plan.inner) && subtree_plan(plan.inner)
}

impl CheckedSubtreePlan {
    #[verifier::type_invariant]
    closed spec fn valid(self)->bool { self.inner.trail.depth<MAX_DEPTH }

    /// Restores the old source-error order when growth rejects before movement.
    pub fn check_detached_error(&self,bytes:&[u8],poisoned:&mut bool)->(result:Result<()>)
        ensures *old(poisoned) ==> *final(poisoned),
            result.is_ok() ==> *final(poisoned)==*old(poisoned),
    {
        ancestor_edit::check_detached(bytes,&self.inner,poisoned)
    }
    /// Initialized source and destination extent required before reacquisition.
    pub fn required_storage_len(&self)->(result:u32)
        ensures result==deferred_required(*self),
    {self.inner.required}

    /// Replans headers and selected bounds, then checks the same captured geometry.
    /// Ancestor values remain deferred to the poisoned publication pass.
    pub fn begin<'a>(&self,bytes:&'a mut [u8],poisoned:&mut bool)->(result:Result<CheckedSubtreeReady<'a>>)
        ensures result.is_err() ==> final(bytes)@==old(bytes)@,
            result.is_ok() ==> write_phase(ready_value(result.unwrap()),0) && write_original(ready_value(result.unwrap()))==old(bytes)@,
    {
        proof {use_type_invariant(self);}
        let plan=&self.inner;
        if bytes.len()<plan.required as usize || bytes.len()>plan.storage_limit as usize {return Err(ErrorKind::InvalidPlan.into());}
        match envelope(bytes,plan.old_used) {
            Ok(())=>(), Err(error)=>{list_patch::source_error(error,poisoned);return Err(ErrorKind::InvalidPlan.into());},
        }
        let trail=match trace_local(bytes,plan.old_used,&plan.trail.root,&plan.path) {
            Ok(trail)=>trail, Err(error)=>{list_patch::source_error(error,poisoned);return Err(ErrorKind::InvalidPlan.into());},
        };
        let fresh=match plan_headers(bytes,plan.old_used,&plan.path,&trail,plan.replacement_len,plan.storage_limit) {
            Ok(fresh)=>fresh, Err(_)=>return Err(ErrorKind::InvalidPlan.into()),
        };
        if !same_geometry(plan,&fresh) {return Err(ErrorKind::InvalidPlan.into());}
        let inner = in_resolved(bytes,plan.old_used,&plan.path,trail,plan.replacement_len,poisoned)?;
        Ok(CheckedSubtreeReady { inner })
    }
}

/// Captured required extent without exposing private request fields.
pub closed spec fn deferred_required(plan:CheckedSubtreePlan)->u32 {plan.inner.required}

/// Captures a header-admitted subtree request without reading ancestor values.
pub(super) fn defer_in_resolved(bytes:&[u8],used:u32,path:&InlinePath,trail:Trail,input:u32)
    ->(result:Result<CheckedSubtreePlan>)
    requires trail_valid(bytes@,trail.root,*path,trail,used),
        12<=used<=bytes.len()<=u32::MAX, words::le_word(bytes@,8)==used,
    ensures result.is_ok() ==> deferred_admitted(bytes@,result.unwrap()),
{
    let plan=plan_headers(bytes,used,path,&trail,input,u32::MAX)?;
    Ok(CheckedSubtreePlan{inner:plan})
}


#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
pub(super) fn plan_headers(bytes: &[u8], used_len: u32, path: &InlinePath, trail: &Trail,
    replacement_len: u32, storage_limit: u32) -> (result: Result<Plan<'static>>)
    requires trail_valid(bytes@, trail.root, *path, *trail, used_len),
        12 <= used_len <= bytes.len() <= storage_limit, bytes.len() <= u32::MAX,
        words::le_word(bytes@, 8) == used_len,
    ensures result.is_ok() ==> replacement_admitted(bytes@, result.unwrap()) && subtree_plan(result.unwrap())
        && result.unwrap().trail == *trail
        && planned_for(result.unwrap(), used_len, trail.root, *path, replacement_len, storage_limit),
        admission::subtree_ready(bytes@, *trail, replacement_len, storage_limit)
            ==> result == Ok(admission::subtree_plan_for(bytes@, *path, *trail, replacement_len, storage_limit)),
{
    let ghost layout = trail.root;
    let target = trail.nodes[trail.depth];
    if let Some(width) = fixed_width(&trail.selected) {
        if replacement_len != width { return Err(ErrorKind::InvalidBoundary.into()); }
    }
    let delta = replacement_len as i64 - (target.region.end - target.region.start) as i64;
    let new_used = checked_delta(used_len, delta)?;
    let required = if new_used > used_len { new_used } else { used_len };
    if required > storage_limit { return Err(ErrorKind::Capacity.into()); }
    Ok(Plan { key: &[], map: None, variable_map: None, region: target.region, indexed: None, splice: None, list: None, trail: *trail, path: *path,
        discriminator: [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]],
        old_used: used_len, replacement_len, delta, new_used, required, storage_limit })
}

/// Prepares the same checked publication from an existing exclusive resolution.
#[verifier::spinoff_prover]
#[inline(always)]
pub(super) fn in_resolved<'a>(bytes: &'a mut [u8], used: u32, _path: &InlinePath,
    trail: Trail, input: u32, poisoned: &mut bool) -> (result: Result<CheckedSubtreeWrite<'a>>)
    requires trail_valid(old(bytes)@,trail.root,*_path,trail,used),
        12<=used<=old(bytes).len()<=u32::MAX, words::le_word(old(bytes)@,8)==used,
        12<=used as int+input-(trail.nodes[trail.depth as int].region.end-trail.nodes[trail.depth as int].region.start)
            <=old(bytes).len(),
    ensures result.is_err() ==> final(bytes)@==old(bytes)@,
        result.is_ok() ==> write_phase(result.unwrap(),0),
        result.is_ok() ==> write_for(result.unwrap(),old(bytes)@,used,trail.root,*_path,input),
{
    let region=trail.nodes[trail.depth].region;
    if let Some(width)=fixed_width(&trail.selected) {
        if input!=width { return Err(ErrorKind::InvalidBoundary.into()); }
    }
    let delta=input as i64-(region.end-region.start) as i64;
    let new_used=checked_delta(used,delta)?;
    let required=if new_used>used { new_used } else { used };
    let mut runs=[PatchRun { offset:0,stride:4,count:0 };MAX_DEPTH];
    let mut payloads=[0u32;MAX_DEPTH];
    let mut i=0usize;
    while i<trail.depth
        invariant i<=trail.depth<MAX_DEPTH,
            trail_valid(bytes@,trail.root,*_path,trail,used),
            12<=used<=bytes.len()<=u32::MAX, words::le_word(bytes@,8)==used,
            region==trail.nodes[trail.depth as int].region,
            delta==input as int-(region.end-region.start),
            new_used==used as int+delta, required==if new_used>used { new_used } else { used },
            forall|j:int| 0<=j<i ==> #[trigger] runs[j]==trail.nodes[j].run
                && payloads[j]==trail.nodes[j].payload_len,
            forall|j:int| 0<=j<i && runs[j].count>0 ==> #[trigger] bounds_admitted(payloads[j],delta),
            forall|j:int| 0<=j<i && runs[j].stride!=4 ==> #[trigger] run_safe(bytes@,runs[j],delta)
                && (delta!=0 ==> run_payload(bytes@,runs[j],payloads[j])),
        decreases trail.depth-i,
    {
        let ghost previous_runs = runs@;
        let ghost previous_payloads = payloads@;
        runs[i]=trail.nodes[i].run;
        payloads[i]=trail.nodes[i].payload_len;
        proof {
            assert(edge_valid(bytes@,trail.nodes[i as int],_path.steps[i as int],trail.nodes[i as int+1]));
            assert(run_within(runs[i as int],0,bytes.len() as int));
        }
        if runs[i].count>0 {
            let lower=if delta<0 { (-delta) as u32 } else { 0 };
            if lower>payloads[i] || payloads[i] as i64+delta>u32::MAX as i64 {
                *poisoned=true;
                return Err(ErrorKind::ArithmeticOverflow.into());
            }
        }
        if runs[i].stride!=4 {
            match check_run(bytes,runs[i],delta,payloads[i]) {
                Ok(())=>(), Err(error)=>{list_patch::source_error(error,poisoned);return Err(error);},
            }
        }
        proof {
            assert forall|j:int| 0<=j<=i implies #[trigger] runs[j]==trail.nodes[j].run
                && payloads[j]==trail.nodes[j].payload_len by {
                if j < i {
                    assert(runs[j] == previous_runs[j]);
                    assert(payloads[j] == previous_payloads[j]);
                }
            }
        }
        i+=1;
    }
    let ghost plan=admission::subtree_plan_for(bytes@,*_path,trail,input,u32::MAX);
    proof { assert(replacement_admitted(bytes@,plan)); }
    proof {
        assert forall|j:int| 0<=j<trail.depth implies #[trigger] payloads[j]==plan.trail.nodes[j].payload_len by {
            assert(runs[j]==trail.nodes[j].run);
        }
        assert forall|j:int| 0<=j<trail.depth implies #[trigger] runs[j]==plan.trail.nodes[j].run by { }
    }
    let original=Ghost(bytes@);
    let depth=active_depth(&runs,trail.depth,delta);
    proof {
        assert forall|j:int| depth<=j<plan.trail.depth implies delta==0 ||
            #[trigger] plan.trail.nodes[j].run.count==0 by {
            assert(runs[j]==plan.trail.nodes[j].run);
        }
    }
    proof {
        assert(depth<=plan.trail.depth);
        assert forall|j:int| 0<=j<depth implies #[trigger] runs[j]==plan.trail.nodes[j].run by { }
        assert forall|j:int| 0<=j<depth implies #[trigger] payloads[j]==plan.trail.nodes[j].payload_len by { }
        assert forall|j:int| 0<=j<depth && runs[j].count>0 implies
            #[trigger] bounds_admitted(payloads[j],delta) by { }
        assert forall|j:int| 0<=j<depth && runs[j].stride!=4 implies
            #[trigger] run_safe(bytes@,runs[j],delta)
                && (delta!=0 ==> run_payload(bytes@,runs[j],payloads[j])) by { }
        assert(subtree_plan(plan));
    }
    Ok(CheckedSubtreeWrite {bytes,runs,payloads,depth,region,input,delta,
        old_used:used,new_used,required,phase:Ghost(0u8),plan:Ghost(plan),original})
}

/// Checks and patches a contained contiguous run; rejected words remain inside the admitted loan.
#[inline(always)]
#[verifier::spinoff_prover]
fn checked_run(bytes: &mut [u8], run: PatchRun, delta: i64, upper: u32) -> (result: Result<()>)
    requires run.count > 0, run.stride == 4, run_within(run, 0, old(bytes).len() as int),
        old(bytes).len() <= u32::MAX, bounds_admitted(upper, delta),
    ensures patch_frame(old(bytes)@, final(bytes)@, run, run.count as int),
        run_safe(old(bytes)@,run,delta) && run_payload(old(bytes)@,run,upper) ==> result.is_ok(),
        result.is_ok() ==> run_safe(old(bytes)@, run, delta)
            && run_payload(old(bytes)@, run, upper)
            && patch_delta(old(bytes)@, final(bytes)@, run, delta, run.count as int),
    no_unwind
{
    let ghost before = bytes@;
    let length = run.count*4;
    let ghost mut patched = Seq::<u8>::empty();
    let ghost mut local_before = Seq::<u8>::empty();
    let outcome;
    {
        let directory = borrow_destination(bytes,run.offset,length);
        proof { local_before = directory@; }
        let lower = if delta < 0 { (-delta) as u32 } else { 0 };
        proof {
            if run_safe(before,run,delta) && run_payload(before,run,upper) {
                admitted_source_bounds(before,local_before,run,lower,upper,delta);
            }
        }
        outcome = if delta >= 0 {
            crate::ancestor_pass::patch_words::<true>(directory,0,upper,delta,run.offset)
        } else {
            crate::ancestor_pass::patch_words::<false>(directory,lower,upper,delta,run.offset)
        };
        proof {
            patched = directory@;
            if outcome.is_ok() { checked_source(before,local_before,run,lower,upper,delta); }
        }
    }
    proof {
        checked_frame(before,bytes@,run);
        if outcome.is_ok() { checked_run_delta(before,bytes@,local_before,patched,run,delta); }
    }
    outcome
}

/// Ancestor admission exposes only the current run's numeric extent.
pub(super) proof fn admitted_extent(original: Seq<u8>, plan: Plan, i: int)
    requires admitted_geometry(original,plan), 0 <= i < plan.trail.depth,
    ensures run_within(plan.trail.nodes[i].run,0,original.len() as int),
        run_within(plan.trail.nodes[i].run,plan.trail.nodes[i].region.start as int,
            plan.trail.nodes[i+1].region.start as int),
{ }

/// Previously patched ancestors cannot change the next ancestor's source words.
#[verifier::spinoff_prover]
proof fn untouched_run(original: Seq<u8>, before: Seq<u8>, current: Seq<u8>, plan: Plan, i: int)
    requires admitted_geometry(original,plan), 0 <= i < plan.trail.depth,
        before.len() == original.len(), current.len() == before.len(),
        forall|p: int| 0 <= p < target(plan).start ==> #[trigger] before[p] == original[p],
        ancestors_frame(before,current,plan,i),
    ensures forall|k: int| 0 <= k < plan.trail.nodes[i].run.count ==>
        #[trigger] words::le_word(current,run_at(plan.trail.nodes[i].run,k))
            == words::le_word(original,run_at(plan.trail.nodes[i].run,k)),
{
    let run = plan.trail.nodes[i].run;
    assert forall|p: int| run_changed(run,run.count as int,p) implies #[trigger] current[p] == original[p] by {
        let k=choose|k: int| 0<=k<run.count && #[trigger] run_at(run,k)<=p<run_at(run,k)+4;
        run_index_bounds(run,plan.trail.nodes[i+1].region.start as int,k);
        if ancestor_changed(plan,i,p) {
            let j=choose|j: int| 0<=j<i && #[trigger] run_changed(plan.trail.nodes[j].run,plan.trail.nodes[j].run.count as int,p);
            admitted_separation(original,plan,j,i,p,p);
        }
    }
    assert forall|k: int| 0<=k<run.count implies #[trigger]
        words::le_word(current,run_at(run,k)) == words::le_word(original,run_at(run,k)) by {
        run_index_bounds(run,plan.trail.nodes[i+1].region.start as int,k);
        assert forall|p: int| run_at(run,k)<=p<run_at(run,k)+4 implies #[trigger] current[p]==original[p] by {
            assert(run_changed(run,run.count as int,p));
        }
    }
}

/// Identical source words carry the exact arithmetic and payload evidence.
#[verifier::spinoff_prover]
proof fn source_evidence(original: Seq<u8>, source: Seq<u8>, run: PatchRun, delta: i64, upper: u32)
    requires original.len() == source.len(), run_within(run,0,source.len() as int),
        forall|k: int| 0<=k<run.count ==> #[trigger] words::le_word(source,run_at(run,k)) == words::le_word(original,run_at(run,k)),
    ensures run_safe(original,run,delta) == run_safe(source,run,delta),
        run_payload(original,run,upper) == run_payload(source,run,upper),
{
    reveal(run_safe); reveal(run_payload); reveal(patch_safe_at);
    assert forall|k: int| 0<=k<run.count implies #[trigger] patch_safe_at(original,run,delta,k) == patch_safe_at(source,run,delta,k) by {
        run_index_bounds(run,source.len() as int,k);
        assert(words::le_word(source,run_at(run,k)) == words::le_word(original,run_at(run,k)));
    }
    if run_safe(original,run,delta) {
        assert forall|k: int| 0<=k<run.count implies #[trigger] patch_safe_at(source,run,delta,k) by {
            run_index_bounds(run,source.len() as int,k);
            assert(patch_safe_at(original,run,delta,k));
            assert(words::le_word(source,run_at(run,k)) == words::le_word(original,run_at(run,k)));
        }
    }
    if run_safe(source,run,delta) {
        assert forall|k: int| 0<=k<run.count implies #[trigger] patch_safe_at(original,run,delta,k) by {
            run_index_bounds(run,source.len() as int,k);
            assert(patch_safe_at(source,run,delta,k));
            assert(words::le_word(source,run_at(run,k)) == words::le_word(original,run_at(run,k)));
        }
    }
    assert forall|k: int| 0<=k<run.count implies #[trigger] words::le_word(source,run.offset+k*run.stride)
        == words::le_word(original,run.offset+k*run.stride) by {
        run_index_bounds(run,source.len() as int,k);
        assert(words::le_word(source,run_at(run,k)) == words::le_word(original,run_at(run,k)));
    }
    if run_payload(original,run,upper) {
        assert forall|k: int| 0<=k<run.count implies #[trigger] words::le_word(source,run.offset+k*run.stride)<=upper by {
            assert(words::le_word(original,run.offset+k*run.stride)<=upper);
            assert(words::le_word(source,run_at(run,k))==words::le_word(original,run_at(run,k)));
        }
    }
    if run_payload(source,run,upper) {
        assert forall|k: int| 0<=k<run.count implies #[trigger] words::le_word(original,run.offset+k*run.stride)<=upper by {
            assert(words::le_word(source,run.offset+k*run.stride)<=upper);
            assert(words::le_word(source,run_at(run,k))==words::le_word(original,run_at(run,k)));
        }
    }
}

/// An exact source-word equality rebases a patch result without changing any byte.
#[verifier::spinoff_prover]
proof fn source_delta(original: Seq<u8>, source: Seq<u8>, after: Seq<u8>, run: PatchRun, delta: i64)
    requires forall|k: int| 0<=k<run.count ==> #[trigger] words::le_word(source,run_at(run,k)) == words::le_word(original,run_at(run,k)),
        patch_delta(source,after,run,delta,run.count as int),
    ensures patch_delta(original,after,run,delta,run.count as int),
{
    reveal(patch_delta);
    assert forall|k: int| 0<=k<run.count implies #[trigger] words::encode_exact(after,run_at(run,k),
        (words::le_word(original,run_at(run,k))+delta) as u32) by {
        assert(words::le_word(source,run_at(run,k)) == words::le_word(original,run_at(run,k)));
        assert(words::encode_exact(after,run_at(run,k),(words::le_word(source,run_at(run,k))+delta) as u32));
    }
}

/// One run's error frame extends the already completed ancestor prefix.
#[verifier::spinoff_prover]
proof fn append_frame(before: Seq<u8>, previous: Seq<u8>, after: Seq<u8>, plan: Plan, i: int)
    requires 0<=i<plan.trail.depth, ancestors_frame(before,previous,plan,i),
        patch_frame(previous,after,plan.trail.nodes[i].run,plan.trail.nodes[i].run.count as int),
    ensures ancestors_frame(before,after,plan,i+1),
{
    assert forall|p: int| 0<=p<before.len() && !ancestor_changed(plan,i+1,p) implies #[trigger] after[p]==before[p] by {
        assert(!run_changed(plan.trail.nodes[i].run,plan.trail.nodes[i].run.count as int,p));
        if ancestor_changed(plan,i,p) {
            let j=choose|j: int| 0<=j<i && #[trigger] run_changed(plan.trail.nodes[j].run,plan.trail.nodes[j].run.count as int,p);
            assert(ancestor_changed(plan,i+1,p));
        }
    }
}

/// A partial ancestor frame is also a frame outside the complete admitted run set.
#[verifier::spinoff_prover]
proof fn widen_frame(before: Seq<u8>, after: Seq<u8>, plan: Plan, done: int)
    requires 0<=done<=plan.trail.depth, ancestors_frame(before,after,plan,done),
    ensures ancestors_frame(before,after,plan,plan.trail.depth as int),
{
    assert forall|p: int| 0<=p<before.len() && !ancestor_changed(plan,plan.trail.depth as int,p)
        implies #[trigger] after[p]==before[p] by {
        if ancestor_changed(plan,done,p) {
            let j=choose|j: int| 0<=j<done && #[trigger] run_changed(plan.trail.nodes[j].run,plan.trail.nodes[j].run.count as int,p);
            assert(ancestor_changed(plan,plan.trail.depth as int,p));
        }
    }
}

/// A checked run extends exact publication while preserving all earlier ancestor words.
#[verifier::spinoff_prover]
proof fn append_delta(original: Seq<u8>, previous: Seq<u8>, after: Seq<u8>, plan: Plan, i: int)
    requires admitted_geometry(original,plan), 0<=i<plan.trail.depth,
        original.len()==previous.len(), ancestors_delta(original,previous,plan,i),
        patch_delta(original,after,plan.trail.nodes[i].run,plan.delta,plan.trail.nodes[i].run.count as int),
        patch_frame(previous,after,plan.trail.nodes[i].run,plan.trail.nodes[i].run.count as int),
    ensures ancestors_delta(original,after,plan,i+1),
{
    reveal(ancestors_delta); reveal(patch_delta); reveal(patch_frame);
    let run=plan.trail.nodes[i].run;
    assert forall|j: int| 0<=j<i+1 implies #[trigger] patch_delta(original,after,
        plan.trail.nodes[j].run,plan.delta,plan.trail.nodes[j].run.count as int) by {
        let other=plan.trail.nodes[j].run;
        assert forall|k: int| 0<=k<other.count implies #[trigger] words::encode_exact(after,
            run_at(other,k),(words::le_word(original,run_at(other,k))+plan.delta) as u32) by {
            if j<i {
                assert(patch_delta(original,previous,other,plan.delta,other.count as int));
                assert(words::encode_exact(previous,run_at(other,k),(words::le_word(original,run_at(other,k))+plan.delta) as u32));
                assert forall|p: int| run_at(other,k)<=p<run_at(other,k)+4 implies !run_changed(run,run.count as int,p) by {
                    assert(run_changed(other,other.count as int,p));
                    if run_changed(run,run.count as int,p) { admitted_separation(original,plan,j,i,p,p); }
                }
                run_index_bounds(other,plan.trail.nodes[j+1].region.start as int,k);
                assert(after[run_at(other,k)]==previous[run_at(other,k)]);
                assert(after[run_at(other,k)+1]==previous[run_at(other,k)+1]);
                assert(after[run_at(other,k)+2]==previous[run_at(other,k)+2]);
                assert(after[run_at(other,k)+3]==previous[run_at(other,k)+3]);
            } else { assert(j==i); }
        }
    }
}

#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
pub(super) fn patch_subtree_runs(bytes: &mut [u8], runs: &[PatchRun; MAX_DEPTH], payloads: &[u32; MAX_DEPTH], depth: usize, delta: i64,
    Ghost(plan): Ghost<Plan>, Ghost(original): Ghost<Seq<u8>>)
    -> (result: Result<()>)
    requires delta!=0, admitted_geometry(original, plan), old(bytes).len() == original.len(), depth <= plan.trail.depth,
        forall|j:int| depth<=j<plan.trail.depth ==> #[trigger] plan.trail.nodes[j].run.count==0,
        delta == plan.delta, forall|i: int| 0 <= i < depth ==> runs[i] == plan.trail.nodes[i].run,
        forall|p: int| 0 <= p < target(plan).start ==> old(bytes)@[p] == original[p],
        forall|i: int| 0 <= i < depth ==> #[trigger] payloads[i] == plan.trail.nodes[i].payload_len,
        forall|i: int| 0 <= i < depth && runs[i].count > 0 ==> #[trigger] bounds_admitted(payloads[i], delta),
        forall|i: int| 0 <= i < depth && runs[i].stride != 4 ==> #[trigger] run_safe(original, runs[i], delta)
            && (delta != 0 ==> run_payload(original, runs[i], payloads[i])),
        replacement_admitted(original, plan),
    ensures replacement_checked(original,plan) ==> result.is_ok(),
        result.is_ok() ==> replacement_checked(original, plan)
        && ancestors_delta(original, final(bytes)@, plan, plan.trail.depth as int),
        ancestors_frame(old(bytes)@, final(bytes)@, plan, plan.trail.depth as int),
    no_unwind
{
    hide(replacement_admitted); hide(replacement_checked);
    proof { if replacement_checked(original,plan) { partition_runs(original,plan); } }
    let ghost before = bytes@;
    let mut i = 0usize;
    while i < depth
        invariant delta!=0, before == old(bytes)@, i <= depth <= plan.trail.depth < MAX_DEPTH, delta == plan.delta,
        forall|i:int| depth<=i<plan.trail.depth ==> plan.trail.nodes[i].run.count==0,
            replacement_checked(original,plan) ==> forall|j: int| 0<=j<depth ==>
                #[trigger] run_safe(original,plan.trail.nodes[j].run,delta)
                && run_payload(original,plan.trail.nodes[j].run,plan.trail.nodes[j].payload_len),
            forall|j: int| 0 <= j < depth ==> runs[j] == plan.trail.nodes[j].run, admitted_geometry(original, plan),
            bytes.len() == original.len(), before.len() == original.len(),
            forall|p: int| 0 <= p < target(plan).start ==> before[p] == original[p],
            replacement_admitted(original, plan),
            forall|j: int| 0 <= j < depth ==> #[trigger] payloads[j] == plan.trail.nodes[j].payload_len,
            forall|j: int| 0 <= j < depth && runs[j].count > 0 ==> #[trigger] bounds_admitted(payloads[j], delta),
            forall|j: int| 0 <= j < depth && runs[j].stride != 4 ==> #[trigger] run_safe(original, runs[j], delta)
                && (delta != 0 ==> run_payload(original, runs[j], payloads[j])),
            forall|j: int| 0 <= j < i ==> run_safe(original, runs[j], delta)
                && (delta != 0 ==> run_payload(original, runs[j], payloads[j])),
            ancestors_delta(original, bytes@, plan, i as int),
            ancestors_frame(before, bytes@, plan, i as int),
        decreases depth - i,
    {
        let run=runs[i];
        let ghost previous=bytes@;
        proof {
            admitted_extent(original,plan,i as int);
            untouched_run(original,before,previous,plan,i as int);
            source_evidence(original,previous,run,delta,payloads[i as int]);
        }
        if run.count>0 && run.stride==4 {
            match checked_run(bytes,run,delta,payloads[i]) {
                Ok(())=>(),
                Err(error)=>{
                    proof {
                        if replacement_checked(original,plan) {
                            assert(run_safe(original,run,delta));
                            assert(run_payload(original,run,payloads[i as int]));
                            assert(run_safe(previous,run,delta));
                            assert(run_payload(previous,run,payloads[i as int]));
                        }
                        append_frame(before,previous,bytes@,plan,i as int);
                        widen_frame(before,bytes@,plan,i+1);
                    }
                    return Err(error);
                }
            }
        } else {
            proof {
                if run.count>0 { assert(run.stride!=4); }
                assert(run_safe(original,run,delta));
            }
            patch_run(bytes,run,delta);
        }
        proof {
            assert(run_safe(original,run,delta));
            if delta!=0 { assert(run_payload(original,run,payloads[i as int])); }
            source_delta(original,previous,bytes@,run,delta);
            append_delta(original,previous,bytes@,plan,i as int);
            append_frame(before,previous,bytes@,plan,i as int);
        }
        i+=1;
    }
    proof {
        reveal(replacement_checked);
        reveal(replacement_admitted);
        assert forall|j: int| 0 <= j < plan.trail.depth implies run_safe(original, plan.trail.nodes[j].run, plan.delta)
            && (plan.delta != 0 ==> run_payload(original, plan.trail.nodes[j].run, plan.trail.nodes[j].payload_len)) by {
            if j<depth {
                assert(runs[j] == plan.trail.nodes[j].run);
                assert(payloads[j] == plan.trail.nodes[j].payload_len);
            } else {
                assert(plan.trail.nodes[j].run.count==0);
                reveal(run_safe);reveal(run_payload);
            }
        }
        assert(replacement_checked(original, plan));
        assert forall|j:int| 0<=j<plan.trail.depth implies #[trigger]
            patch_delta(original,bytes@,plan.trail.nodes[j].run,delta,plan.trail.nodes[j].run.count as int) by {
            if j>=depth {assert(plan.trail.nodes[j].run.count==0);reveal(patch_delta);}
        }
        assert(ancestors_delta(original,bytes@,plan,plan.trail.depth as int));
        widen_frame(before,bytes@,plan,depth as int);
    }
    Ok(())
}

#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
#[allow(clippy::too_many_arguments)] // The final two arguments exist only in proof state.
fn finish_subtree(bytes: &mut [u8], runs: &[PatchRun; MAX_DEPTH], payloads: &[u32; MAX_DEPTH], depth: usize, delta: i64,
    old_used: u32, new_used: u32, Ghost(plan): Ghost<Plan<'static>>, Ghost(original): Ghost<Seq<u8>>)
    -> (result: Result<()>)
    requires replacement_admitted(original, plan), subtree_plan(plan), payload_frame(original, old(bytes)@, plan),
        plan.required <= old(bytes).len(), depth <= plan.trail.depth, delta == plan.delta,
        old_used == plan.old_used, new_used == plan.new_used,
        delta!=0 ==> forall|i:int| depth<=i<plan.trail.depth ==> plan.trail.nodes[i].run.count==0,
        forall|i: int| 0 <= i < depth ==> runs[i] == plan.trail.nodes[i].run,
        forall|i: int| 0 <= i < depth ==> #[trigger] payloads[i] == plan.trail.nodes[i].payload_len,
        forall|i: int| 0 <= i < depth && runs[i].count > 0 ==> #[trigger] bounds_admitted(payloads[i], delta),
        forall|i: int| 0 <= i < depth && runs[i].stride != 4 ==> #[trigger] run_safe(original, runs[i], delta)
            && (delta != 0 ==> run_payload(original, runs[i], payloads[i])),
    ensures replacement_checked(original,plan) ==> result.is_ok(),
        result.is_ok() ==> replacement_checked(original, plan)
        && replacement_result(original, old(bytes)@, final(bytes)@, plan),
    no_unwind
{
    let ghost encoded = bytes@;
    proof {
        admitted_geometry_proof(original,plan);reveal(subtree_plan);
        if delta>0 {grow_containment(original,plan);} else if delta<0 {shrink_containment(original,plan);}
    }
    if delta != 0 {
        match patch_subtree_runs(bytes, runs, payloads, depth, delta, Ghost(plan), Ghost(original)) {
            Ok(()) => (), Err(error) => return Err(error),
        }
    } else {
        proof {
            admission::idle_runs(original, plan.trail, plan.path, plan.old_used, delta);
            assert(replacement_checked(original, plan));
            replacement_geometry(original, plan);
            ancestors_unchanged(original, bytes@, plan);
        }
    }
    proof { replacement_geometry(original, plan); }
    let ghost patched = bytes@;
    let ghost counted = bytes@;
    if delta < 0 { clear_tail(bytes, new_used, old_used); }
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
    Ok(())
}

/// Prepares a string replacement with a proof-only traversal trail.
/// Checks ingress, selected boundaries, patch arithmetic and initialized capacity.
#[doc(hidden)]
#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline)]
pub fn prepare_checked_byte_replacement<'a>(bytes: &'a mut [u8], discriminator: [u8; 8], minimum: u32,
    checked: crate::CheckedLayout<'static>, path: &InlinePath, input_len: usize, poisoned: &mut bool)
    -> (result: Result<CheckedSubtreeReady<'a>>)
    ensures result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> input_len <= u32::MAX && write_for(ready_value(result.unwrap()), old(bytes)@,
            words::le_word(old(bytes)@, 8) as u32, Layout::Flat { checked }, *path, input_len as u32),
{ hide(run_safe); hide(run_payload);
    let used = match super::tail_edit::account_used(bytes, discriminator, minimum) {
        Ok(used) => used,
        Err(error) => { list_patch::source_error(error, poisoned); return Err(error); },
    };
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
        let selected = match select_checked_child(local, active, path.steps[depth]) {
            Ok(selected) => selected,
            Err(error) => {
                let error = error.rebase(region.start);
                list_patch::source_error(error, poisoned);
                return Err(error);
            },
        };
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
    let patch_depth=active_depth(&runs,depth,delta);
    let mut i = 0usize;
    while i < patch_depth
        invariant -(u32::MAX as int) <= delta <= u32::MAX,
            i <= patch_depth <= depth < MAX_DEPTH, trail.depth == depth, trail.nodes == nodes,
            forall|j:int| patch_depth<=j<depth ==> delta==0 || #[trigger] runs[j].count==0,
            trail_valid(bytes@, Layout::Flat { checked }, *path, trail, used),
            bytes.len() <= u32::MAX,
            forall|j: int| 0 <= j < depth ==> runs[j] == nodes[j].run && payloads[j] == nodes[j].payload_len,
            forall|j: int| 0 <= j < i ==> (runs[j].count > 0 ==> bounds_admitted(payloads[j], delta))
                && (runs[j].stride != 4 ==> run_safe(bytes@, runs[j], delta)
                    && (delta != 0 ==> run_payload(bytes@, runs[j], payloads[j]))),
        decreases patch_depth - i,
    {
        proof {
            assert(edge_valid(bytes@, nodes[i as int], path.steps[i as int], nodes[i as int+1]));
            assert(run_within(runs[i as int], 0, bytes.len() as int));
        }
        if runs[i].count > 0 {
            let lower = if delta < 0 { (-delta) as u32 } else { 0 };
            if lower > payloads[i] || payloads[i] as i64 + delta > u32::MAX as i64 {
                *poisoned = true;
                return Err(ErrorKind::ArithmeticOverflow.into());
            }
        }
        if runs[i].stride != 4 {
            match check_run(bytes, runs[i], delta, payloads[i]) {
                Ok(()) => (), Err(error) => { list_patch::source_error(error, poisoned); return Err(error); },
            }
        }
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
        assert(replacement_admitted(bytes@, plan));
        assert(subtree_plan(plan));
        assert(planned_for(plan, used, Layout::Flat { checked }, *path, input, u32::MAX));
    }
    let original = Ghost(bytes@);
    proof {
        assert forall|j:int| patch_depth<=j<plan.trail.depth implies delta==0 ||
            #[trigger] plan.trail.nodes[j].run.count==0 by {
            assert(runs[j]==plan.trail.nodes[j].run);
        }
    }
    Ok(CheckedSubtreeReady { inner: CheckedSubtreeWrite { bytes, runs, payloads, region, input, delta, old_used: used, new_used, required,
        phase:Ghost(0u8), depth: patch_depth, plan: Ghost(plan), original } })
}

/// Every ancestor word and replacement movement fits header-admitted storage.
#[verifier::spinoff_prover]
proof fn grow_containment(before:Seq<u8>,plan:Plan)
    requires replacement_admitted(before,plan), subtree_plan(plan), plan.delta>0,
        plan.required<=before.len(),
    ensures 12<=plan.region.start<=plan.region.end<=plan.old_used<=plan.required<=before.len()<=u32::MAX,
        plan.region.start<=plan.region.start+plan.replacement_len<=plan.new_used<=plan.required,
        forall|i:int| 0<=i<plan.trail.depth ==> #[trigger] run_within(plan.trail.nodes[i].run,0,before.len() as int),
{
    admitted_geometry_proof(before,plan);
    assert forall|i:int| 0<=i<plan.trail.depth implies
        #[trigger] run_within(plan.trail.nodes[i].run,0,before.len() as int) by {admitted_extent(before,plan,i);}
}

/// Successful checked publication inherits the old exact edit, root and sibling frame.
#[verifier::spinoff_prover]
proof fn grow_refinement(before:Seq<u8>,encoded:Seq<u8>,after:Seq<u8>,plan:Plan)
    requires replacement_checked(before,plan),subtree_plan(plan),plan.delta>0,
        replacement_result(before,encoded,after,plan),
        validation::schema_valid(before.subrange(12,plan.old_used as int),plan.trail.root,MAX_DEPTH as nat,true),
        validation::schema_valid(encoded.subrange(plan.region.start as int,plan.region.start+plan.replacement_len),
            plan.trail.selected,(MAX_DEPTH-plan.trail.depth) as nat,true),
    ensures validation::schema_valid(after.subrange(12,plan.new_used as int),plan.trail.root,MAX_DEPTH as nat,true),
        crate::preservation::framed_path(before.subrange(12,plan.old_used as int),after.subrange(12,plan.new_used as int),
            plan.trail.root,nested::ordinals(plan,0),true),
        after.subrange(12,plan.new_used as int)==nested::nested_apply(before.subrange(12,plan.old_used as int),
            plan.trail.root,nested::ordinals(plan,0),encoded.subrange(plan.region.start as int,plan.region.start+plan.replacement_len)),
{
    subtree_replace_preserves(before,encoded,after,plan);
    nested_frame(before,encoded,after,plan);
    nested_refines(before,encoded,after,plan,MAX_DEPTH as nat,true);
}

/// Every ancestor word and replacement movement fits header-admitted storage.
#[verifier::spinoff_prover]
proof fn shrink_containment(before:Seq<u8>,plan:Plan)
    requires replacement_admitted(before,plan), subtree_plan(plan), plan.delta<0,
        plan.required<=before.len(),
    ensures 12<=plan.region.start<=plan.region.end<=plan.old_used<=plan.required<=before.len()<=u32::MAX,
        plan.region.start<=plan.region.start+plan.replacement_len<=plan.new_used<=plan.required,
        forall|i:int| 0<=i<plan.trail.depth ==> #[trigger] run_within(plan.trail.nodes[i].run,0,before.len() as int),
{
    admitted_geometry_proof(before,plan);
    assert forall|i:int| 0<=i<plan.trail.depth implies
        #[trigger] run_within(plan.trail.nodes[i].run,0,before.len() as int) by {admitted_extent(before,plan,i);}
}

/// Successful checked publication inherits the old exact edit, root and sibling frame.
#[verifier::spinoff_prover]
proof fn shrink_refinement(before:Seq<u8>,encoded:Seq<u8>,after:Seq<u8>,plan:Plan)
    requires replacement_checked(before,plan),subtree_plan(plan),plan.delta<0,
        replacement_result(before,encoded,after,plan),
        validation::schema_valid(before.subrange(12,plan.old_used as int),plan.trail.root,MAX_DEPTH as nat,true),
        validation::schema_valid(encoded.subrange(plan.region.start as int,plan.region.start+plan.replacement_len),
            plan.trail.selected,(MAX_DEPTH-plan.trail.depth) as nat,true),
    ensures validation::schema_valid(after.subrange(12,plan.new_used as int),plan.trail.root,MAX_DEPTH as nat,true),
        crate::preservation::framed_path(before.subrange(12,plan.old_used as int),after.subrange(12,plan.new_used as int),
            plan.trail.root,nested::ordinals(plan,0),true),
        after.subrange(12,plan.new_used as int)==nested::nested_apply(before.subrange(12,plan.old_used as int),
            plan.trail.root,nested::ordinals(plan,0),encoded.subrange(plan.region.start as int,plan.region.start+plan.replacement_len)),
{
    subtree_replace_preserves(before,encoded,after,plan);
    nested_frame(before,encoded,after,plan);
    nested_refines(before,encoded,after,plan,MAX_DEPTH as nat,true);
}

/// A retained list end witnesses growth and shrink publication with nonempty movement.
#[verifier::spinoff_prover]
proof fn ancestor_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,4);
    reveal_with_fuel(crate::layout::descriptor_fields,4);
    reveal_with_fuel(validation::schema_valid,4);
    reveal_with_fuel(validation::children_valid,5);
    reveal_with_fuel(crate::utf8::utf8_valid,3);
    let before=witnesses::account(seq![2u8,0,0,0,1,0,0,0,97,98]);
    let after=witnesses::account(seq![2u8,0,0,0,2,0,0,0,97,97,98]);
    let path=InlinePath{steps:[Step::Index(0);MAX_DEPTH],len:1,overflow:false};
    let selection=Selection{payload_len:2,span:Span{start:8,end:9},layout:Layout::Str,ordinal:0,
        run:PatchRun{offset:4,stride:4,count:1}};
    assert(selection_exact(selection,before.subrange(12,22),witnesses::INDEXED_LIST,Step::Index(0)));
    admission::direct_checked(before,witnesses::INDEXED_LIST,path,22,selection);
    let trail=admission::direct_trail(before,witnesses::INDEXED_LIST,path,22,selection);
    admission::subtree_checked(before,path,trail,2,64);
    let plan=admission::subtree_plan_for(before,path,trail,2,64);
    let encoded=after.update(8,22u8).update(16,1u8);
    assert(replacement_admitted(before,plan));
    assert(patch_delta(before,after,plan.trail.nodes[0].run,1,1));
    assert forall|p:int| 16<=p<20 implies ancestor_changed(plan,1,p) by {
        assert(run_at(plan.trail.nodes[0].run,0)==16);
        assert(run_changed(plan.trail.nodes[0].run,1,p));
    }
    assert(ancestors_delta(before,after,plan,1));
    assert(replacement_result(before,encoded,after,plan));
    grow_containment(before,plan);
    grow_refinement(before,encoded,after,plan);
    let selection=Selection{payload_len:3,span:Span{start:8,end:10},..selection};
    admission::direct_checked(after,witnesses::INDEXED_LIST,path,23,selection);
    let trail=admission::direct_trail(after,witnesses::INDEXED_LIST,path,23,selection);
    admission::subtree_checked(after,path,trail,1,64);
    let plan=admission::subtree_plan_for(after,path,trail,1,64);
    let encoded=before.update(8,23u8).update(16,2u8).update(22,98u8);
    assert(replacement_admitted(after,plan));
    assert(patch_delta(after,before,plan.trail.nodes[0].run,-1i64,1));
    assert forall|p:int| 16<=p<20 implies ancestor_changed(plan,1,p) by {
        assert(run_at(plan.trail.nodes[0].run,0)==16);
        assert(run_changed(plan.trail.nodes[0].run,1,p));
    }
    assert(ancestors_delta(after,before,plan,1));
    assert(replacement_result(after,encoded,before,plan));
    shrink_containment(after,plan);
    shrink_refinement(after,encoded,before,plan);
}

/// Bounded Kani ingress to the production forward pass; no typed graph is modeled.
#[doc(hidden)]
#[cfg_attr(not(kani), allow(dead_code))]
pub fn __ancestor_pass_model(bytes:&mut [u8],grow:bool,amount:u32,upper:u32,poisoned:&mut bool)->(result:Result<()>)
    ensures final(bytes).len()==old(bytes).len(),
        old(bytes).len()==24 && (if grow { amount<=u32::MAX-upper } else { amount<=upper })
            ==> *final(poisoned)==result.is_err(),
    no_unwind
{
    if bytes.len()!=24{return Err(Error::new(ErrorKind::InvalidBoundary,Error::NO_OFFSET));}
    if (grow&&amount>u32::MAX-upper)||(!grow&&amount>upper){return Err(Error::new(ErrorKind::ArithmeticOverflow,Error::NO_OFFSET));}
    let delta=if grow{amount as i64}else{-(amount as i64)};
    *poisoned=true;
    let outcome=checked_run(bytes,PatchRun{offset:4,stride:4,count:4},delta,upper);
    crate::patch_pass::finish_publication(poisoned,outcome)
}

}
