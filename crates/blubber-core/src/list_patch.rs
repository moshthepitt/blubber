//! Header-admitted indexed edits with fallible directory publication.
#![cfg_attr(verus_only, allow(missing_docs))]
#![allow(non_shorthand_field_patterns)]
use super::*;

verus! {

/// A detached indexed edit. Untouched source ends have no runtime certificate.
#[doc(hidden)]
pub struct CheckedListPlan<'key> { inner: Plan<'key> }

impl<'key> CheckedListPlan<'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool { self.inner.trail.depth < MAX_DEPTH }
}

impl<'key> CheckedListPlan<'key> {
    /// Restores ancestor diagnostics when backend growth fails before transport.
    pub fn check_detached_error(&self,bytes:&[u8],poisoned:&mut bool)->(result:Result<()>)
        ensures *old(poisoned) ==> *final(poisoned),
            result.is_ok() ==> *final(poisoned)==*old(poisoned),
    {
        ancestor_edit::check_detached(bytes,&self.inner,poisoned)
    }
    /// Initialized window required for either forward growth or shrink movement.
    #[inline(always)]
    pub fn required_len(&self) -> (result: u32)
        ensures result == plan_required(*self),
    { self.inner.required }

    /// Reacquires header and ancestor admission after an exclusive backend resize.
    /// Untouched list ends are checked only by the subsequent movement pass.
    pub fn begin<'a>(&self, bytes: &'a mut [u8], poisoned: &mut bool) -> (result: Result<CheckedListReady<'a,'key>>)
        ensures result.is_err() ==> final(bytes)@ == old(bytes)@,
            result.is_ok() ==> apply_phase(ready_value(result.unwrap()), 0)
                && apply_original(ready_value(result.unwrap())) == old(bytes)@,
    {
        proof { use_type_invariant(self); }
        let p = &self.inner;
        let (operation, input) = if let Some(c) = p.indexed { (ListEdit::Insert(c.index), c.input_len) }
            else if let Some(c) = p.splice { (c.operation, c.input_len) }
            else if let Some(c) = p.variable_map { (ListEdit::Clear, c.input_len) }
            else { return Err(ErrorKind::InvalidPlan.into()); };
        let fresh = recheck_plan(bytes, p, operation, input, poisoned)?;
        if !same_geometry(p, &fresh.inner) { return Err(ErrorKind::InvalidPlan.into()); }
        if fresh.inner.required as usize > bytes.len() { return Err(ErrorKind::Capacity.into()); }
        let original = Ghost(bytes@);
        Ok(CheckedListReady { inner: CheckedListApply { bytes, plan: fresh, phase: Ghost(0), original } })
    }
}

/// Rechecks numeric geometry in a separate frame for the sixteen-level build.
#[cfg_attr(target_os = "solana", inline(never))]
#[cfg_attr(not(target_os = "solana"), inline(always))]
fn recheck_plan<'key>(bytes: &[u8], p: &Plan<'key>, operation: ListEdit, input: u32,
    poisoned: &mut bool) -> (result: Result<CheckedListPlan<'key>>)
    requires p.trail.depth < MAX_DEPTH,
    ensures result.is_ok() ==> admitted(bytes@, result.unwrap()),
{
    match envelope(bytes, p.old_used) { Ok(()) => (), Err(error) => { source_error(error, poisoned); return Err(error); } }
    let trail = match trace_local(bytes, p.old_used, &p.trail.root, &p.path) {
        Ok(trail) => trail, Err(error) => { source_error(error, poisoned); return Err(error); },
    };
    if let Some(c) = p.variable_map {
        plan_checked_map(bytes, p.old_used, &p.path, trail, c.change.operation, p.key,
            input, p.storage_limit, Some(crate::Search { index: c.change.index, found: c.change.found }), poisoned)
    } else { plan_checked(bytes, p.old_used, &p.path, trail, operation, input, p.storage_limit, poisoned) }
}

/// Checked header, selected interval, movement extents and ancestor admission.
pub closed spec fn admitted(bytes: Seq<u8>, plan: CheckedListPlan) -> bool {
    let p = plan.inner;
    &&& ancestor_edit::admitted(bytes,base(p)) && subtree_plan(base(p))
    &&& if let Some(c) = p.indexed {
        p.splice.is_none() && p.variable_map.is_none() && indexed_insert::checked::admitted(bytes, p, c)
    } else if let Some(c) = p.splice {
        p.variable_map.is_none() && indexed_remove::checked::admitted(bytes, p, c)
    } else if p.variable_map.is_some() { indexed_map::bounded(bytes, p) } else { false }
}

spec fn base(plan: Plan) -> Plan { Plan { indexed: None, splice: None, variable_map: None, ..plan } }

/// Only numeric/header evidence is retained by immediate preparation.
#[doc(hidden)]
pub struct CheckedListPrepared<'a, 'key> {
    bytes: &'a mut [u8],
    plan: CheckedListPlan<'key>,
    original: Ghost<Seq<u8>>,
}

/// Exact bytes retained by the preparation.
pub closed spec fn prepared_storage<'a, 'key>(p: CheckedListPrepared<'a, 'key>) -> &'a mut [u8] { p.bytes }
/// Exact byte sequence before admission.
pub closed spec fn prepared_original(p: CheckedListPrepared) -> Seq<u8> { p.original@ }
/// The offset-only plan captured before any write.
pub closed spec fn prepared_plan<'a, 'key>(p: CheckedListPrepared<'a, 'key>) -> CheckedListPlan<'key> { p.plan }

#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[allow(clippy::too_many_arguments)]
fn plan_checked(bytes: &[u8], used: u32, path: &InlinePath, trail: Trail,
    operation: ListEdit, input: u32, limit: u32, poisoned: &mut bool) -> (result: Result<CheckedListPlan<'static>>)
    requires trail_valid(bytes@, trail.root, *path, trail, used),
        12 <= used <= bytes.len() <= u32::MAX, words::le_word(bytes@, 8) == used,
    ensures result.is_ok() ==> admitted(bytes@, result.unwrap()),
{
    reveal(admitted);
    let region = trail.nodes[trail.depth].region;
    if bytes.len() > limit as usize { return Err(ErrorKind::Capacity.into()); }
    if let ListEdit::Insert(_) = operation {
        let c = match indexed_insert::checked::prepare(bytes, region, &trail.selected, operation, input) {
            Ok(c) => c, Err(error) => { source_error(error, poisoned); return Err(error); },
        };
        let len = checked_delta(region.end - region.start, c.table_growth as i64 + input as i64)?;
        let mut plan = match subtree_checked::plan_headers(bytes,used,path,&trail,len,limit) {
            Ok(plan) => plan,
            Err(error) => {
                if let Error::Fault { offset, .. } = error {
                    if offset != Error::NO_OFFSET { source_error(error, poisoned); }
                }
                return Err(error);
            },
        };
        match ancestor_edit::admit_ancestors(bytes,&plan){Ok(())=>(),Err(error)=>{source_error(error,poisoned);return Err(error);}}
        proof { replacement_extent(bytes@,plan); }
        plan.indexed = Some(c);
        Ok(CheckedListPlan { inner: plan })
    } else {
        if !matches!(operation, ListEdit::Replace(_)) && input != 0 { return Err(ErrorKind::InvalidBoundary.into()); }
        let mut c = match indexed_remove::checked::prepare(bytes, region, &trail.selected, operation, 0) {
            Ok(c) => c, Err(error) => { source_error(error, poisoned); return Err(error); },
        };
        c.input_len = input;
        let delta = input as i64 - (c.end_offset - c.offset) as i64 - c.table_shrink as i64;
        let len = checked_delta(region.end - region.start, delta)?;
        let mut plan = match subtree_checked::plan_headers(bytes,used,path,&trail,len,limit) {
            Ok(plan) => plan,
            Err(error) => {
                if let Error::Fault { offset, .. } = error {
                    if offset != Error::NO_OFFSET { source_error(error, poisoned); }
                }
                return Err(error);
            },
        };
        match ancestor_edit::admit_ancestors(bytes,&plan){Ok(())=>(),Err(error)=>{source_error(error,poisoned);return Err(error);}}
        proof { replacement_extent(bytes@,plan); }
        plan.splice = Some(c);
        Ok(CheckedListPlan { inner: plan })
    }
}


#[verifier::spinoff_prover]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(not(all(target_os = "solana", feature = "max-depth-16")), inline(always))]
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_in_resolved<'a>(bytes: &'a mut [u8], used: u32, path: &InlinePath,
    trail: Trail, operation: ListEdit, input: u32, limit: u32, poisoned: &mut bool) -> (result: Result<CheckedListPrepared<'a, 'static>>)
    requires trail_valid(old(bytes)@, trail.root, *path, trail, used),
        12 <= used <= old(bytes).len() <= u32::MAX, words::le_word(old(bytes)@, 8) == used,
    ensures result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> admitted(old(bytes)@, prepared_plan(result.unwrap()))
            && prepared_original(result.unwrap()) == old(bytes)@,
{
    let plan = plan_checked(bytes, used, path, trail, operation, input, limit, poisoned)?;
    let original = Ghost(bytes@);
    Ok(CheckedListPrepared { bytes, plan, original })
}

#[verifier::spinoff_prover]
#[allow(clippy::too_many_arguments)]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(not(all(target_os = "solana", feature = "max-depth-16")), inline(always))]
fn plan_checked_map<'key>(bytes: &[u8], used: u32, path: &InlinePath, trail: Trail,
    operation: MapEdit, key: &'key [u8], input: u32, limit: u32, hint: Option<crate::Search>,
    poisoned: &mut bool) -> (result: Result<CheckedListPlan<'key>>)
    requires trail_valid(bytes@, trail.root, *path, trail, used),
        12 <= used <= bytes.len() <= u32::MAX, words::le_word(bytes@, 8) == used,
    ensures result.is_ok() ==> admitted(bytes@, result.unwrap()),
{
    reveal(admitted); reveal(base); reveal(replacement_checked); reveal(replacement_admitted); reveal(subtree_plan);
    broadcast use vstd::array::group_array_axioms;
    let region = trail.nodes[trail.depth].region;
    if bytes.len() > limit as usize { return Err(ErrorKind::Capacity.into()); }
    let c = match indexed_map::prepare_checked(bytes, region, &trail.selected, operation, key, input, hint) {
        Ok(c) => c, Err(error) => {
            if let Error::Fault { kind, offset } = error {
                let source = if offset != Error::NO_OFFSET { true }
                    else if matches!(kind, ErrorKind::ArithmeticOverflow) {
                        crate::map_layout::partition_map_header(&bytes[region.start as usize..region.end as usize], &trail.selected).is_err()
                    } else { false };
                if source { source_error(error, poisoned); }
            }
            return Err(error);
        },
    };
    let delta = c.table_delta + input as i64 - (c.end_offset - c.offset) as i64;
    let len = if matches!(operation, MapEdit::Clear) { 4u32 }
        else { checked_delta(region.end - region.start, delta)? };
    let mut plan = match subtree_checked::plan_headers(bytes,used,path,&trail,len,limit) {
        Ok(plan) => plan, Err(error) => {
            if let Error::Fault { offset, .. } = error {
                if offset != Error::NO_OFFSET { source_error(error, poisoned); }
            }
            return Err(error);
        },
    };
    match ancestor_edit::admit_ancestors(bytes,&plan){Ok(())=>(),Err(error)=>{source_error(error,poisoned);return Err(error);}}
    proof { replacement_extent(bytes@,plan); }
    plan.key=key;
    proof {
        reveal(fixed_map::map_checked);reveal(indexed_map::checked);
        reveal(indexed_insert::indexed_checked);reveal(indexed_remove::splice_checked);reveal(list_checked);
        assert(ancestor_edit::admitted(bytes@,plan));
    }
    let ghost plain=plan;
    plan.variable_map=Some(c);
    let inner=plan;
    proof {
        assert(base(inner)==plain);
        assert(ancestor_edit::admitted(bytes@,base(inner)));
        assert(subtree_plan(base(inner)));
        assert(c.payload + c.table_delta == region.start + 4 + c.new_count * c.stride) by(nonlinear_arith)
            requires c.payload == region.start + 4 + c.count * c.stride,
                c.table_delta == (c.new_count as int - c.count) * c.stride;
        if matches!(operation, MapEdit::Clear) {
            assert(c.new_count == 0 && c.offset == 0 && c.input_len == 0);
            assert(c.end_offset == region.end - c.payload);
            assert(c.table_delta == -(c.count as int * c.stride)) by(nonlinear_arith)
                requires c.table_delta == (c.new_count as int - c.count) * c.stride, c.new_count == 0;
            assert(delta == 4 + region.start - region.end);
        }
        assert(inner.delta == delta);
        assert(indexed_map::header_geometry(bytes@, inner));
        assert(indexed_map::bounded(bytes@, inner));
    }
    Ok(CheckedListPlan { inner })
}

/// Prepares a lazy indexed-map edit through the shared checked pass.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(not(all(target_os = "solana", feature = "max-depth-16")), inline(always))]
pub(super) fn prepare_map_in_resolved<'a, 'key>(bytes: &'a mut [u8], used: u32, path: &InlinePath,
    trail: Trail, operation: MapEdit, key: &'key [u8], input: u32, limit: u32,
    hint: Option<crate::Search>, poisoned: &mut bool) -> (result: Result<CheckedListPrepared<'a, 'key>>)
    requires trail_valid(old(bytes)@, trail.root, *path, trail, used),
        12 <= used <= old(bytes).len() <= u32::MAX, words::le_word(old(bytes)@, 8) == used,
    ensures result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> admitted(old(bytes)@, prepared_plan(result.unwrap()))
            && prepared_original(result.unwrap()) == old(bytes)@,
{
    let plan = plan_checked_map(bytes, used, path, trail, operation, key, input, limit, hint, poisoned)?;
    let original = Ghost(bytes@);
    Ok(CheckedListPrepared { bytes, plan, original })
}

/// Arms source faults without changing the pre-write frame of caller errors.
pub(super) fn source_error(error: Error, poisoned: &mut bool)
    ensures *final(poisoned) == (*old(poisoned) || matches!(error, Error::Fault {
        kind: ErrorKind::Truncated | ErrorKind::ArithmeticOverflow | ErrorKind::InvalidBoundary
            | ErrorKind::Discriminator | ErrorKind::UsedLength | ErrorKind::InvalidBool
            | ErrorKind::UnsortedKeys | ErrorKind::DuplicateKey, .. })),
    no_unwind
{
    if matches!(error, Error::Fault { kind: ErrorKind::Truncated | ErrorKind::ArithmeticOverflow
        | ErrorKind::InvalidBoundary | ErrorKind::Discriminator | ErrorKind::UsedLength
        | ErrorKind::InvalidBool | ErrorKind::UnsortedKeys | ErrorKind::DuplicateKey, .. }) {
        *poisoned = true;
    }
}

impl<'a, 'key> CheckedListPrepared<'a, 'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool { self.bytes@ == self.original@ && admitted(self.original@, self.plan) }

    /// Checks old ancestor errors only after another pre-write check rejects.
    #[cfg_attr(target_os="solana",inline(never))]
    pub fn check_before_error(&self,poisoned:&mut bool)->(result:Result<()>)
        ensures *old(poisoned) ==> *final(poisoned),
            result.is_ok() ==> *final(poisoned)==*old(poisoned),
    {
        proof {use_type_invariant(self);ancestor_edit::scan_geometry(self.original@,base(self.plan.inner));}
        ancestor_edit::scan_prewrite(self.bytes,&self.plan.inner.trail,self.plan.inner.delta,poisoned)
    }
    /// Required initialized extent, checked before writes and funding.
    #[inline(always)]
    pub fn prepared_len(&self) -> (result: u32)
        ensures result == plan_required(prepared_plan(*self)),
    { self.plan.inner.required }

    /// Original account bytes for removed/export validation.
    #[inline(always)]
    pub fn inspect(&self) -> (result: &[u8])
        ensures result@ == prepared_original(*self),
    { proof { use_type_invariant(self); } self.bytes }

    /// The selected removed value, excluding pop on an empty list and suffix edits.
    pub fn removed(&self) -> (result: Option<Span>)
        ensures result == removed_span(prepared_plan(*self)),
        no_unwind
    {
        proof { use_type_invariant(self); reveal(admitted); }
        match self.plan.inner.splice {
            Some(c) => if matches!(c.operation, ListEdit::Remove(_) | ListEdit::Replace(_) | ListEdit::Pop) && c.first < c.last {
                Some(Span { start: c.payload + c.offset, end: c.payload + c.end_offset })
            } else { None },
            None => match self.plan.inner.variable_map {
                Some(c) => if c.change.found && !matches!(c.change.operation, MapEdit::Clear) {
                    Some(Span { start: c.payload + c.offset, end: c.payload + c.end_offset })
                } else { None },
                None => None,
            },
        }
    }

    /// Releases the borrow for a checked backend extension.
    pub fn detach(self) -> (result: CheckedListPlan<'key>)
        ensures result == prepared_plan(self), final(prepared_storage(self))@ == prepared_original(self),
        no_unwind
    { proof { use_type_invariant(&self); } self.plan }

    /// Begins the fallible pass over the same exclusive bytes, without rescanning.
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(not(all(target_os = "solana", feature = "max-depth-16")), inline(always))]
    pub fn begin_immediate(self) -> (result: Result<CheckedListReady<'a,'key>>)
        ensures result.is_err() ==> final(prepared_storage(self))@ == prepared_original(self),
            result.is_ok() ==> apply_phase(ready_value(result.unwrap()), 0)
                && apply_original(ready_value(result.unwrap())) == prepared_original(self),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        if self.plan.inner.required as usize > self.bytes.len() { return Err(Error::new(ErrorKind::Capacity, Error::NO_OFFSET)); }
        let CheckedListPrepared { bytes, plan, original } = self;
        Ok(CheckedListReady { inner: CheckedListApply { bytes, plan, phase: Ghost(0), original } })
    }
}

/// Removed span interpreted only from admitted numeric coordinates.
pub closed spec fn removed_span(plan: CheckedListPlan) -> Option<Span> {
    match plan.inner.splice {
        Some(c) => if matches!(c.operation, ListEdit::Remove(_) | ListEdit::Replace(_) | ListEdit::Pop) && c.first < c.last {
            Some(Span { start: (c.payload + c.offset) as u32, end: (c.payload + c.end_offset) as u32 })
        } else { None },
        None => match plan.inner.variable_map {
            Some(c) => if c.change.found && !matches!(c.change.operation, MapEdit::Clear) {
                Some(Span { start: (c.payload + c.offset) as u32, end: (c.payload + c.end_offset) as u32 })
            } else { None }, None => None,
        },
    }
}

/// Ordered application; an error consumes the byte loan under sticky poison.
#[doc(hidden)]
#[must_use = "complete publication before clearing poison"]
#[repr(C)]
pub struct CheckedListApply<'a, 'key> {
    bytes: &'a mut [u8],
    plan: CheckedListPlan<'key>,
    phase: Ghost<u8>,
    original: Ghost<Seq<u8>>,
}

/// Prepared application before movement.
#[doc(hidden)]
#[repr(transparent)]
pub struct CheckedListReady<'a,'key> { inner: CheckedListApply<'a,'key> }
/// Moved application before issuing its destination.
#[doc(hidden)]
#[repr(transparent)]
pub struct CheckedListMoved<'a,'key> { inner: CheckedListApply<'a,'key> }
/// Proof view of the sealed prepared phase.
pub closed spec fn ready_value<'a,'key>(value: CheckedListReady<'a,'key>) -> CheckedListApply<'a,'key> { value.inner }
/// Proof view of the sealed moved phase.
pub closed spec fn moved_value<'a,'key>(value: CheckedListMoved<'a,'key>) -> CheckedListApply<'a,'key> { value.inner }
impl<'a,'key> CheckedListReady<'a,'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool { apply_phase(self.inner, 0) }
    /// Consumes preparation and transports retained bytes under sticky poison.
    #[inline(always)]
    pub fn move_regions(self, poisoned: &mut bool) -> (r: Result<CheckedListMoved<'a,'key>>)
        ensures *final(poisoned), r.is_ok() ==> apply_phase(moved_value(r.unwrap()), 1),
        no_unwind
    { proof { use_type_invariant(&self); } let CheckedListReady { mut inner } = self; match inner.move_inner(poisoned) { Ok(()) => Ok(CheckedListMoved { inner }), Err(error) => Err(error) } }
}
impl<'a,'key> CheckedListMoved<'a,'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool { apply_phase(self.inner, 1) }
    /// Issues the destination once; publication is available on the returned state.
    #[inline(always)]
    pub fn destination(self) -> (r: CheckedListApply<'a,'key>)
        ensures apply_phase(r, 2),
        no_unwind
    { proof { use_type_invariant(&self); } let CheckedListMoved { mut inner } = self; inner.issue(); inner }
}

/// Phase zero admits movement; one admits an encoder loan; two admits publication.
pub closed spec fn apply_phase(a: CheckedListApply, phase: u8) -> bool { a.phase@ == phase }
/// Original bytes captured before movement.
pub closed spec fn apply_original(a: CheckedListApply) -> Seq<u8> { a.original@ }
/// Required initialized window for a detached list plan.
pub closed spec fn plan_required(p: CheckedListPlan) -> u32 { p.inner.required }
/// Exact input width of an ordered list application.
pub closed spec fn apply_input_len(a: CheckedListApply) -> int { input_len(a.plan) }
/// Current storage retained by the apply.
pub closed spec fn apply_storage<'a, 'key>(a: CheckedListApply<'a, 'key>) -> &'a mut [u8] { a.bytes }

spec fn moved(original: Seq<u8>, after: Seq<u8>, plan: CheckedListPlan) -> bool {
    if let Some(c) = plan.inner.indexed {
        indexed_insert::checked::complete(original, after, plan.inner, c)
    } else if let Some(c) = plan.inner.splice {
        indexed_remove::checked::complete(original, after, plan.inner, c)
    } else if plan.inner.variable_map.is_some() {
        indexed_map::movement(original, after, plan.inner)
    } else { false }
}

spec fn input_start(p: CheckedListPlan) -> int {
    if let Some(c) = p.inner.indexed { c.payload + c.table_growth + c.offset }
    else if let Some(c) = p.inner.splice { c.payload - c.table_shrink + c.offset }
    else { indexed_map::destination(p.inner) as int }
}

spec fn input_len(p: CheckedListPlan) -> int {
    if let Some(c) = p.inner.indexed { c.input_len as int }
    else if let Some(c) = p.inner.splice { c.input_len as int }
    else { p.inner.variable_map.unwrap().input_len as int }
}

#[verifier::spinoff_prover]
proof fn loan_framed(original: Seq<u8>, before: Seq<u8>, p: CheckedListPlan)
    requires admitted(original, p), moved(original, before, p), p.inner.required <= before.len(),
    ensures forall|after: Seq<u8>| after.len() == before.len()
        && (forall|i: int| 0 <= i < before.len() && !(input_start(p) <= i < input_start(p) + input_len(p))
            ==> after[i] == before[i]) ==> #[trigger] moved(original, after, p),
{
    hide(words::le_word);
    reveal(admitted); reveal(with_insert); reveal(with_splice); reveal(target); reveal(payload_frame);
    assert forall|after: Seq<u8>| after.len() == before.len()
        && (forall|i: int| 0 <= i < before.len() && !(input_start(p) <= i < input_start(p) + input_len(p))
            ==> after[i] == before[i]) implies #[trigger] moved(original, after, p) by {
        same_word(before, after, p.inner.region.start as int);
        if let Some(c) = p.inner.indexed {
            assert forall|j: int| 0 <= j < c.count implies
                #[trigger] words::le_word(after, p.inner.region.start + 4 + 4 * j) == indexed_insert::new_end(original, p.inner, j) by {
                let at = p.inner.region.start + 4 + 4 * j;
                assert forall|i: int| at <= i < at + 4 implies after[i] == before[i] by { }
                same_word(before, after, at);
            }
        } else if let Some(c) = p.inner.splice {
            assert forall|j: int| 0 <= j < indexed_remove::words_for(c.new_count as int) implies
                #[trigger] words::le_word(after, p.inner.region.start + 4 + 4 * j) == indexed_remove::new_end(original, p.inner, j) by {
                let at = p.inner.region.start + 4 + 4 * j;
                assert forall|i: int| at <= i < at + 4 implies after[i] == before[i] by { }
                same_word(before, after, at);
            }
        } else if p.inner.variable_map.is_some() {
            indexed_map::header_movement_framed(original, before, p.inner);
        }
    }
}

proof fn same_word(before: Seq<u8>, after: Seq<u8>, at: int)
    requires before.len() == after.len(), 0 <= at, at + 4 <= before.len(),
        forall|p: int| at <= p < at + 4 ==> after[p] == before[p],
    ensures words::le_word(after, at) == words::le_word(before, at),
{ }

/// A tail list with no stored ends has no retained byte movement.
#[verifier::spinoff_prover]
proof fn stationary_splice(bytes: Seq<u8>, p: CheckedListPlan, c: IndexedSplice)
    requires admitted(bytes, p), p.inner.indexed.is_none(), p.inner.splice == Some(c),
        p.inner.required <= bytes.len(), c.count <= 1, c.payload + c.end_offset == p.inner.old_used,
    ensures moved(bytes, bytes, p),
{
    reveal(admitted); reveal(with_splice); reveal(payload_frame); reveal(target);
    assert(c.table_shrink == 0 && c.new_count <= 1);
}

/// Reattaches a strict splice certificate only under its ghost partition premise.
#[verifier::spinoff_prover]
proof fn strict_splice(original: Seq<u8>, p: CheckedListPlan, c: IndexedSplice)
    requires admitted(original,p),replacement_checked(original,base(p.inner)), p.inner.indexed.is_none(), p.inner.splice == Some(c),
        indexed_remove::parameters(original, p.inner.region, c),
    ensures replacement_checked(original, p.inner),
{
    broadcast use vstd::array::group_array_axioms;
    reveal(admitted); reveal(base); reveal(replacement_checked); reveal(target);
    reveal(list_checked); reveal(subtree_plan); reveal(fixed_map::map_checked);
    reveal(indexed_map::checked); reveal(indexed_insert::indexed_checked);
    reveal(indexed_remove::splice_checked);
    let a = p.inner;
    assert(trail_valid(original, a.trail.root, a.path, a.trail, a.old_used));
    assert(list_checked(original, a));
    assert(fixed_map::map_checked(original, a));
    assert(indexed_map::checked(original, a));
    assert(indexed_insert::indexed_checked(original, a));
    assert(indexed_remove::splice_checked(original, a));
    assert(a.trail.nodes[a.trail.depth as int].region.start <= target(a).start <= target(a).end
        <= a.trail.nodes[a.trail.depth as int].region.end);
    assert(12 <= a.old_used <= original.len() <= a.storage_limit);
    assert(words::le_word(original, 8) == a.old_used);
    assert(a.discriminator == base(a).discriminator);
    assert forall|j: int| 0 <= j < 8 implies #[trigger] a.discriminator[j] == original[j] by {
        assert(base(a).discriminator[j] == original[j]);
    }
    assert(a.delta == a.replacement_len as int - (target(a).end - target(a).start));
    assert(a.new_used == a.old_used + a.delta);
    assert(12 <= a.new_used <= a.required <= a.storage_limit);
    assert(a.required == if a.new_used > a.old_used { a.new_used } else { a.old_used });
    assert(target(a).start + a.replacement_len == target(a).end + a.delta);
    assert(target(a).start <= target(a).start + a.replacement_len <= a.new_used);
    assert forall|i: int| 0 <= i < a.trail.depth implies
        run_safe(original, a.trail.nodes[i].run, a.delta)
        && (a.delta != 0 ==> run_payload(original, a.trail.nodes[i].run, a.trail.nodes[i].payload_len)) by { }
}

#[verifier::spinoff_prover]
proof fn strict_map(original: Seq<u8>, p: CheckedListPlan, c: IndexedMap)
    requires admitted(original,p),replacement_checked(original,base(p.inner)), p.inner.indexed.is_none(), p.inner.splice.is_none(), p.inner.variable_map == Some(c),
        indexed_map::parameters(original, p.inner.region, fixed_map::node(p.inner).layout, c, p.inner.key@),
    ensures replacement_checked(original, p.inner),
{
    broadcast use vstd::array::group_array_axioms;
    reveal(admitted); reveal(base); reveal(replacement_checked); reveal(target);
    reveal(list_checked); reveal(subtree_plan); reveal(fixed_map::map_checked);
    reveal(indexed_map::checked); reveal(indexed_insert::indexed_checked);
    reveal(indexed_remove::splice_checked);
    let a = p.inner;
    assert(trail_valid(original, a.trail.root, a.path, a.trail, a.old_used));
    assert(list_checked(original, a));
    assert(fixed_map::map_checked(original, a));
    assert(indexed_map::checked(original, a));
    assert(indexed_insert::indexed_checked(original, a));
    assert(indexed_remove::splice_checked(original, a));
    assert(a.trail.nodes[a.trail.depth as int].region.start <= target(a).start <= target(a).end
        <= a.trail.nodes[a.trail.depth as int].region.end);
    assert(12 <= a.old_used <= original.len() <= a.storage_limit);
    assert(words::le_word(original, 8) == a.old_used);
    assert(a.discriminator == base(a).discriminator);
    assert forall|j: int| 0 <= j < 8 implies #[trigger] a.discriminator[j] == original[j] by {
        assert(base(a).discriminator[j] == original[j]);
    }
    assert(a.delta == a.replacement_len as int - (target(a).end - target(a).start));
    assert(a.new_used == a.old_used + a.delta);
    assert(12 <= a.new_used <= a.required <= a.storage_limit);
    assert(a.required == if a.new_used > a.old_used { a.new_used } else { a.old_used });
    assert(target(a).start + a.replacement_len == target(a).end + a.delta);
    assert(target(a).start <= target(a).start + a.replacement_len <= a.new_used);
    assert forall|i: int| 0 <= i < a.trail.depth implies
        run_safe(original, a.trail.nodes[i].run, a.delta)
        && (a.delta != 0 ==> run_payload(original, a.trail.nodes[i].run, a.trail.nodes[i].payload_len)) by { }
}

/// A valid original ancestor partition makes final publication infallible.
pub closed spec fn valid_ancestor_partition(a:CheckedListApply)->bool {
    replacement_checked(a.original@,base(a.plan.inner))
}

/// The final result uses the retained strict specifications, conditional on the
/// original partition. The executable admission supplies no such premise.
pub closed spec fn refines(a: CheckedListApply, after: Seq<u8>) -> bool {
    let p = a.plan.inner;
    &&& if let Some(c) = p.indexed {
        indexed_insert::parameters(a.original@, p.region, c) ==>
            indexed_insert::insert_result(a.original@, a.bytes@, after, p)
            && indexed_insert::sequence_insert(a.original@, a.bytes@, after, p)
    } else if let Some(c) = p.splice {
        indexed_remove::parameters(a.original@, p.region, c) ==>
            indexed_remove::splice_result(a.original@, a.bytes@, after, p)
            && indexed_remove::sequence_splice(a.original@, a.bytes@, after, p)
    } else if let Some(c) = p.variable_map {
        indexed_map::parameters(a.original@, p.region, fixed_map::node(p).layout, c, p.key@) ==>
            indexed_map::edit_result(a.original@, a.bytes@, after, p)
            && indexed_map::map_result(a.original@, a.bytes@, after, p)
    } else { false }
}

impl<'a, 'key> CheckedListApply<'a, 'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        &&& admitted(self.original@, self.plan)
        &&& self.bytes.len() == self.original@.len() && self.plan.inner.required <= self.bytes.len()
        &&& self.phase@ <= 3
        &&& self.phase@ == 0 ==> self.bytes@ == self.original@
        &&& self.phase@ == 1 || self.phase@ == 2 ==> moved(self.original@, self.bytes@, self.plan)
    }

    /// Checks each retained end at its transport, keeping used length unpublished.
    #[verifier::spinoff_prover]
    #[inline(always)]
    fn move_inner(&mut self, poisoned: &mut bool) -> (result: Result<()>)
        requires apply_phase(*old(self), 0),
        ensures *final(poisoned), result.is_ok() ==> apply_phase(*final(self), 1),
        no_unwind
    {
        proof { use_type_invariant(&*self); reveal(admitted); }
        self.phase = Ghost(3);
        let result = if let Some(c) = self.plan.inner.indexed {
            indexed_insert::checked::move_checked(self.bytes, &self.plan.inner, c, poisoned)
        } else if let Some(c) = self.plan.inner.splice {
            if c.count <= 1 && c.payload + c.end_offset == self.plan.inner.old_used {
                *poisoned = true;
                proof { stationary_splice(self.original@, self.plan, c); }
                Ok(())
            } else {
                indexed_remove::checked::move_checked(self.bytes, &self.plan.inner, c, poisoned)
            }
        } else if self.plan.inner.variable_map.is_some() {
            indexed_map::checked::move_checked(self.bytes, &self.plan.inner, poisoned)
        } else { *poisoned = true; Err(ErrorKind::InvalidPlan.into()) };
        if result.is_ok() { self.phase = Ghost(1); }
        result
    }

    /// Lends only the new value after a complete successful directory pass.
    /// The caller must use movement, destination and publication in that order.
    #[verifier::spinoff_prover]
    fn issue(&mut self)
        requires apply_phase(*old(self), 1),
        ensures apply_phase(*final(self), 2),
        no_unwind
    { proof { use_type_invariant(&*self); } self.phase = Ghost(2); }

    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(not(all(target_os = "solana", feature = "max-depth-16")), inline(always))]
    pub fn bytes(&mut self) -> (result: &mut [u8])
        requires apply_phase(*old(self), 2),
        ensures apply_phase(*final(self), 2), result.len() == apply_input_len(*old(self)),
        no_unwind
    {
        proof { use_type_invariant(&*self); reveal(admitted); }
        self.phase = Ghost(3);
        let (start, len) = if let Some(c) = self.plan.inner.indexed {
            (c.payload + c.table_growth + c.offset, c.input_len)
        } else if let Some(c) = self.plan.inner.splice {
            (c.payload - c.table_shrink + c.offset, c.input_len)
        } else if let Some(c) = self.plan.inner.variable_map {
            (indexed_map::destination_exec(&self.plan.inner), c.input_len)
        } else { panic!("admitted list change") };
        proof { loan_framed(self.original@, self.bytes@, self.plan); }
        let bytes = borrow_destination(self.bytes, start, len);
        self.phase = Ghost(2);
        bytes
    }

    /// Publishes the count and ancestors, then used length, and clears the tail.
    #[verifier::spinoff_prover]
    #[inline(always)]
    pub fn finish(self)->(result:Result<()>)
        requires apply_phase(self, 2),
        ensures valid_ancestor_partition(self) ==> result.is_ok(),
            result.is_ok() ==> refines(self,final(apply_storage(self))@),
        no_unwind
    {
        hide(words::le_word);
        proof {
            use_type_invariant(&self); reveal(admitted); reveal(refines); reveal(base);
            reveal(replacement_result); reveal(count_changed); reveal(target); reveal(destination_start); reveal(input_size);
            reveal(indexed_insert::indexed_checked); reveal(indexed_remove::splice_checked);
            reveal(with_insert); reveal(with_splice); reveal(ancestor_changed); reveal(ancestors_delta);
            reveal(subtree_plan); reveal(list_checked); reveal(fixed_map::map_checked); reveal(indexed_map::checked);
        }
        let CheckedListApply { bytes, plan, original, .. } = self;
        let mut p = plan.inner;
        let ghost tagged = p;
        let ghost encoded = bytes@;
        let (count, old_count) = if let Some(c) = p.indexed { (c.count + 1, c.count) }
            else if let Some(c) = p.splice { (c.new_count, c.count) }
            else if let Some(c) = p.variable_map { (c.new_count, c.count) }
            else { panic!("admitted list change") };
        if count != old_count {
            match crate::write_u32(bytes, p.region.start, count) {
                Ok(()) => (), Err(_) => panic!("contained list count"),
            }
        }
        let ghost counted = bytes@;
        p.indexed = None;
        p.splice = None;
        p.variable_map = None;
        match ancestor_edit::finish_replacement(bytes,&p,original) {Ok(())=>(),Err(error)=>return Err(error),}
        proof {
            assert forall|i: int| p.region.start + 4 <= i < p.region.start + p.replacement_len implies
                #[trigger] bytes@[i] == encoded[i] by { }
            same_word(counted, bytes@, p.region.start as int);
            if let Some(c) = tagged.indexed {
                if indexed_insert::parameters(original@, tagged.region, c) {
                    reveal(replacement_checked); reveal(indexed_insert::indexed_checked);
                    assert(replacement_checked(original@, tagged));
                    assert forall|j: int| 0 <= j < c.count implies
                        #[trigger] words::le_word(bytes@, tagged.region.start + 4 + 4 * j) == indexed_insert::new_end(original@, tagged, j) by {
                        same_word(encoded, bytes@, tagged.region.start + 4 + 4 * j);
                    }
                    assert(indexed_insert::payload_preserved(original@, bytes@, tagged));
                    let at = destination_start(tagged) as int;
                    assert(bytes@.subrange(at, at + c.input_len) =~= encoded.subrange(at, at + c.input_len));
                    assert(ancestors_delta(original@, bytes@, tagged, tagged.trail.depth as int));
                    assert(indexed_insert::insert_result(original@, encoded, bytes@, tagged));
                    indexed_insert::sequence_refinement(original@, encoded, bytes@, tagged);
                }
            } else if let Some(c) = tagged.splice {
                if indexed_remove::parameters(original@, tagged.region, c) {
                    reveal(replacement_checked); reveal(indexed_remove::splice_checked);
                    assert(indexed_remove::geometry(original@, tagged));
                    assert(list_checked(original@, tagged));
                    assert(fixed_map::map_checked(original@, tagged));
                    assert(indexed_map::checked(original@, tagged));
                    assert(indexed_insert::indexed_checked(original@, tagged));
                    assert(replacement_checked(original@, base(tagged)));
                    strict_splice(original@, plan, c);
                    assert forall|j: int| 0 <= j < indexed_remove::words_for(c.new_count as int) implies
                        #[trigger] words::le_word(bytes@, tagged.region.start + 4 + 4 * j) == indexed_remove::new_end(original@, tagged, j) by {
                        same_word(encoded, bytes@, tagged.region.start + 4 + 4 * j);
                    }
                    assert(indexed_remove::payload_preserved(original@, bytes@, tagged));
                    let at = destination_start(tagged) as int;
                    assert(bytes@.subrange(at, at + c.input_len) =~= encoded.subrange(at, at + c.input_len));
                    assert(ancestors_delta(original@, bytes@, tagged, tagged.trail.depth as int));
                    assert(indexed_remove::splice_result(original@, encoded, bytes@, tagged));
                    indexed_remove::sequence_refinement(original@, encoded, bytes@, tagged);
                }
            } else if let Some(c) = tagged.variable_map {
                if indexed_map::parameters(original@, tagged.region, fixed_map::node(tagged).layout, c, tagged.key@) {
                    strict_map(original@, plan, c);
                    indexed_map::checked::publication_refinement(original@, encoded, counted, bytes@, tagged);
                }
            }
        }
        Ok(())
    }
}

/// Every directory address and payload movement fits admitted storage on any bytes.
proof fn insert_containment(bytes: Seq<u8>, p: CheckedListPlan, c: IndexedInsert)
    requires admitted(bytes, p), p.inner.indexed == Some(c), p.inner.required <= bytes.len(),
    ensures 12 <= p.inner.region.start < c.payload <= p.inner.region.end <= p.inner.required <= bytes.len() <= u32::MAX,
        forall|k: int| 1 <= k < c.count ==> #[trigger] crate::patch_pass::directory_slot(p.inner.region.start as int, c.payload as int, k)
            && crate::patch_pass::directory_slot(p.inner.region.start as int, c.payload + c.table_growth, k + 1),
{
    reveal(admitted);
}

/// A valid old partition makes every reverse transport check succeed.
#[verifier::spinoff_prover]
proof fn insert_transport_refinement(bytes: Seq<u8>, p: CheckedListPlan, c: IndexedInsert, k: int)
    requires admitted(bytes, p), p.inner.indexed == Some(c), p.inner.required <= bytes.len(),
        indexed_insert::parameters(bytes, p.inner.region, c), c.index < k < c.count,
    ensures c.offset <= words::le_word(bytes, p.inner.region.start + 4 * k)
            <= indexed_insert::boundary(bytes, p.inner.region.start as int, c.payload as int,
                p.inner.region.end as int, c.count as int, k + 1),
        0 <= words::le_word(bytes, p.inner.region.start + 4 * k) + c.input_len <= u32::MAX,
{
    reveal(admitted); reveal(indexed_insert::parameters);
    indexed_insert::boundary_order(bytes, p.inner.region, c, c.index as int, k);
    indexed_insert::boundary_order(bytes, p.inner.region, c, k, k + 1);
}

/// Every compacted source and destination lies in the admitted directory.
proof fn splice_containment(bytes: Seq<u8>, p: CheckedListPlan, c: IndexedSplice)
    requires admitted(bytes, p), p.inner.indexed.is_none(), p.inner.splice == Some(c), p.inner.required <= bytes.len(),
    ensures 12 <= p.inner.region.start < c.payload <= p.inner.region.end <= p.inner.required <= bytes.len() <= u32::MAX,
        forall|j: int| c.first <= j < indexed_remove::words_for(c.new_count as int) ==>
            p.inner.region.start + 4 + 4 * j + 4 <= c.payload - c.table_shrink
            && p.inner.region.start + 4 * indexed_remove::source_boundary(c, j) + 4 <= c.payload,
{
    reveal(admitted);
}

/// A valid old partition makes every forward transport check succeed.
#[verifier::spinoff_prover]
proof fn splice_transport_refinement(bytes: Seq<u8>, p: CheckedListPlan, c: IndexedSplice, j: int)
    requires admitted(bytes, p), p.inner.indexed.is_none(), p.inner.splice == Some(c), p.inner.required <= bytes.len(),
        indexed_remove::parameters(bytes, p.inner.region, c), c.first <= j < indexed_remove::words_for(c.new_count as int),
    ensures c.end_offset <= indexed_insert::boundary(bytes, p.inner.region.start as int, c.payload as int,
            p.inner.region.end as int, c.count as int, indexed_remove::source_boundary(c, j)) <= p.inner.region.end - c.payload,
        0 <= indexed_insert::boundary(bytes, p.inner.region.start as int, c.payload as int,
            p.inner.region.end as int, c.count as int, indexed_remove::source_boundary(c, j))
            + indexed_remove::payload_delta(c) <= u32::MAX,
{
    reveal(admitted); reveal(indexed_remove::parameters);
    indexed_remove::boundary_order(bytes, p.inner.region, c, c.last as int, indexed_remove::source_boundary(c, j));
}

/// Nonempty concrete insert and remove passes discharge all transport premises.
#[verifier::spinoff_prover]
proof fn checked_pass_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid, 4);
    reveal_with_fuel(crate::layout::descriptor_fields, 4);
    reveal(admitted); reveal(base); reveal(replacement_checked); reveal(subtree_plan);
    let before = witnesses::account(seq![3u8,0,0,0,1,0,0,0,2,0,0,0,97,98,99]);
    let c = IndexedInsert { index: 0, count: 3, payload: 24, offset: 0, table_growth: 4, input_len: 1 };
    let p = CheckedListPlan { inner: Plan { indexed: Some(c), ..witnesses::replacement(before, witnesses::INDEXED_LIST, 20) } };
    insert_containment(before, p, c);
    insert_transport_refinement(before, p, c, 2);
    let c = IndexedSplice { operation: ListEdit::Remove(0), count: 3, new_count: 2,
        first: 0, last: 1, payload: 24, offset: 0, end_offset: 1, table_shrink: 4, input_len: 0 };
    let p = CheckedListPlan { inner: Plan { splice: Some(c), ..witnesses::replacement(before, witnesses::INDEXED_LIST, 10) } };
    splice_containment(before, p, c);
    splice_transport_refinement(before, p, c, 0);
}

}
