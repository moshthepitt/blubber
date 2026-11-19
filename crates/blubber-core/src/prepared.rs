//! Exclusive resolution and preparation over one unchanged byte borrow.

#![cfg_attr(verus_only, allow(missing_docs))]

use super::*;

verus! {

/// Opaque immediate-edit selection over an exclusively borrowed account.
///
/// Created only by checked resolution. Inspection cannot mutate bytes; consuming
/// preparation reuses this trail instead of resolving the path a second time.
/// This internal cross-crate bridge is not a persisted or detached witness.
///
/// The original bytes cannot be changed while a resolution is retained:
///
/// ```compile_fail
/// # #![forbid(unsafe_code)]
/// use blubber_core::{resolve_edit, InlinePath, Layout};
/// let mut bytes = [0; 16];
/// bytes[8..12].copy_from_slice(&16u32.to_le_bytes());
/// let resolved = resolve_edit(&mut bytes, 16, &Layout::Str, &InlinePath::new()).unwrap();
/// bytes[12] = 1;
/// let _ = resolved.selection();
/// ```
#[doc(hidden)]
pub struct Resolved<'a> {
    bytes: &'a mut [u8],
    trail: Trail,
    path: InlinePath,
    used: u32,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    original: Ghost<Seq<u8>>,
}

/// Exact storage retained by a resolution.
pub closed spec fn resolved_storage<'a>(resolved: Resolved<'a>) -> &'a mut [u8] { resolved.bytes }

/// Exact original account bytes, including spare capacity.
pub closed spec fn resolved_original(resolved: Resolved) -> Seq<u8> { resolved.original@ }

/// Checked selection and exact root/path identity; no leaf admission is implied.
pub closed spec fn resolved_for(resolved: Resolved, bytes: Seq<u8>, used: u32,
    layout: Layout, path: InlinePath) -> bool {
    resolved.original@ == bytes && resolved.bytes@ == bytes && resolved.used == used
        && resolved.path == path && resolved.trail.root == layout
        && trail_valid(bytes, layout, path, resolved.trail, used)
}

/// Exact selected interval and metadata.
pub closed spec fn resolved_selection(resolved: Resolved) -> (Span, Layout) {
    let node = resolved.trail.nodes[resolved.trail.depth as int];
    (node.region, node.layout)
}

/// Preparation preserves the resolved root/path and the caller's size policy.
pub closed spec fn resolved_request(resolved: Resolved, plan: Plan, input: u32, limit: u32) -> bool {
    planned_for(plan, resolved.used, resolved.trail.root, resolved.path, input, limit)
}

/// Whether this resolution contains retained contiguous ancestor ends.
pub closed spec fn resolved_has_ancestor_words(resolved:Resolved)->bool {
    exists|i:int| 0<=i<resolved.trail.depth && #[trigger] resolved.trail.nodes[i].run.count>0
        && resolved.trail.nodes[i].run.stride==4
}

/// Whether a contiguous ancestor run reaches a requested retained-word count.
pub closed spec fn resolved_has_ancestor_run(resolved:Resolved,minimum:u32)->bool {
    exists|i:int| 0<=i<resolved.trail.depth && #[trigger] resolved.trail.nodes[i].run.count>=minimum
        && resolved.trail.nodes[i].run.stride==4
}

/// Fixed-entry ancestor preparation retains the exact resolved request.
pub closed spec fn resolved_ancestor_request(resolved:Resolved,prepared:AncestorPrepared,input:u32,limit:u32)->bool {
    ancestor_edit::prepared_for(prepared,resolved.original@,resolved.used,resolved.trail.root,resolved.path,input,limit)
}

/// Immediate preparation retains the exact resolved request.
pub closed spec fn resolved_subtree_request(resolved: Resolved, write: SubtreeWrite, input: u32) -> bool {
    subtree_write::write_for(write, resolved.original@, resolved.used, resolved.trail.root, resolved.path, input)
}

/// The deferred writer retains this exact resolved request.
pub closed spec fn resolved_checked_request(resolved:Resolved,write:subtree_checked::CheckedSubtreeWrite,input:u32)->bool {
    subtree_checked::write_for(write,resolved.original@,resolved.used,resolved.trail.root,resolved.path,input)
}

/// Exact extent needed for both source and replacement destination.
pub closed spec fn resolved_subtree_required(resolved:Resolved,input:u32)->int {
    let size=resolved.used as int+input-(resolved.trail.nodes[resolved.trail.depth as int].region.end
        -resolved.trail.nodes[resolved.trail.depth as int].region.start);
    if size>resolved.used {size} else {resolved.used as int}
}

/// The final replacement fits this unchanged physical buffer.
pub closed spec fn resolved_subtree_fits(resolved: Resolved, input: u32) -> bool {
    resolved.used as int + input - (resolved.trail.nodes[resolved.trail.depth as int].region.end
        - resolved.trail.nodes[resolved.trail.depth as int].region.start) <= resolved.bytes.len()
}

/// Resolves an immediate edit without changing any byte.
///
/// Checks the envelope, metadata, local headers and selected adjacent ends.
/// Untouched directory entries and global map order require full validation.
/// Local edits preserve validity conditional on a valid initial root and input;
/// malformed directories can alias sibling regions within the account. Retains
/// the exclusive borrow until preparation or detached-plan extraction.
#[doc(hidden)]
#[cfg_attr(target_os = "solana", inline(never))]
pub fn resolve_edit<'a>(bytes: &'a mut [u8], used: u32, layout: &Layout, path: &InlinePath)
    -> (result: Result<Resolved<'a>>)
    ensures result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> resolved_for(result.unwrap(), old(bytes)@, used, *layout, *path),
{
    envelope(bytes, used)?;
    let trail = trace_local(bytes, used, layout, path)?;
    let original = Ghost(bytes@);
    Ok(Resolved { bytes, trail, path: *path, used, original })
}

/// Exact identity and minimum root extent for a typed account ingress.
pub open spec fn account_identity(bytes: Seq<u8>, discriminator: [u8; 8], minimum: u32) -> bool {
    bytes.len() >= 12 && words::le_word(bytes, 8) >= 12 + minimum
        && forall|i: int| 0 <= i < 8 ==> #[trigger] bytes[i] == discriminator[i]
}

/// Exact admission of a typed account envelope, including representable extents.
pub open spec fn account_admitted(bytes: Seq<u8>, discriminator: [u8; 8], minimum: u32) -> bool {
    &&& exists|i: int| 0 <= i < 8 && #[trigger] discriminator[i] != 0
    &&& 12 <= bytes.len() <= u32::MAX
    &&& 12 + minimum <= u32::MAX
    &&& account_identity(bytes, discriminator, minimum)
    &&& words::le_word(bytes, 8) <= bytes.len()
}

/// Checks typed account ingress and resolves its path in one exclusive borrow.
///
/// Preserves lazy-load diagnostic order: nonzero expected identity, physical
/// wire extent, envelope truncation, discriminator and minimum used length.
/// The selected structural path is checked before caller input is measured.
#[doc(hidden)]
#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
pub fn resolve_account_edit<'a>(bytes: &'a mut [u8], discriminator: [u8; 8], minimum: u32,
    layout: &Layout, path: &InlinePath) -> (result: Result<Resolved<'a>>)
    ensures result.is_err() ==> final(bytes)@ == old(bytes)@,
        result.is_ok() ==> resolved_for(result.unwrap(), old(bytes)@,
            words::le_word(old(bytes)@, 8) as u32, *layout, *path)
            && account_identity(old(bytes)@, discriminator, minimum),
{
    if discriminator == [0u8; 8] { return Err(ErrorKind::InvalidSchema.into()); }
    checked_u32(bytes.len())?;
    if bytes.len() < 12 { return Err(Error::new(ErrorKind::Truncated, 0)); }
    if bytes[0] != discriminator[0] || bytes[1] != discriminator[1]
        || bytes[2] != discriminator[2] || bytes[3] != discriminator[3]
        || bytes[4] != discriminator[4] || bytes[5] != discriminator[5]
        || bytes[6] != discriminator[6] || bytes[7] != discriminator[7] {
        return Err(Error::new(ErrorKind::Discriminator, 0));
    }
    let minimum_used = checked_delta(12, minimum as i64)?;
    let used = read_u32(bytes, 8)?;
    if used < minimum_used || used as usize > bytes.len() {
        return Err(Error::new(ErrorKind::UsedLength, 8));
    }
    let trail = trace_local(bytes, used, layout, path)?;
    let original = Ghost(bytes@);
    Ok(Resolved { bytes, trail, path: *path, used, original })
}

#[allow(non_shorthand_field_patterns)] // Verus expands destructuring shorthand.
impl<'a> Resolved<'a> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        &&& self.bytes@ == self.original@
        &&& 12 <= self.used <= self.bytes.len() <= u32::MAX
        &&& words::le_word(self.original@, 8) == self.used
        &&& trail_valid(self.original@, self.trail.root, self.path, self.trail, self.used)
    }

    /// Inspects the exact selected span and its certified metadata.
    #[inline(always)]
    pub fn selection(&self) -> (result: (Span, Layout))
        ensures result == resolved_selection(*self),
            12 <= result.0.start <= result.0.end <= resolved_original(*self).len(),
    {
        proof { use_type_invariant(self); }
        let node = self.trail.nodes[self.trail.depth];
        (node.region, self.trail.selected)
    }

    /// Borrows unchanged account bytes read-only, including spare capacity.
    #[inline(always)]
    pub fn inspect_resolved(&self) -> (result: &[u8])
        ensures result@ == resolved_original(*self),
    {
        proof { use_type_invariant(self); }
        self.bytes
    }

    /// Lends the selected interval for an equal-length overwrite.
    ///
    /// The envelope, every boundary and all spare bytes remain outside the loan.
    /// The caller validates its replacement encoding before writing.
    #[inline(always)]
    pub fn into_destination(self) -> (result: &'a mut [u8])
        ensures result@ == resolved_original(self).subrange(resolved_selection(self).0.start as int,
                resolved_selection(self).0.end as int),
            result.len() == resolved_selection(self).0.end - resolved_selection(self).0.start,
            final(resolved_storage(self)).len() == resolved_original(self).len(),
            final(resolved_storage(self))@.subrange(resolved_selection(self).0.start as int,
                resolved_selection(self).0.end as int) == final(result)@,
            crate::fixed_edit::overwrite_frame(resolved_original(self), final(resolved_storage(self))@,
                resolved_selection(self).0),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        let span = self.trail.nodes[self.trail.depth].region;
        let bytes = self.bytes;
        borrow_destination(bytes, span.start, span.end - span.start)
    }

    /// Checks physical capacity without consuming or modifying this resolution.
    #[inline(always)]
    pub fn subtree_fits(&self, input: u32) -> (result: bool)
        ensures result == resolved_subtree_fits(*self, input),
    {
        proof { use_type_invariant(self); }
        let region = self.trail.nodes[self.trail.depth].region;
        input as usize <= self.bytes.len() - self.used as usize + (region.end - region.start) as usize
    }

    /// Prepares a compact immediate replacement over initialized capacity.
    #[allow(non_shorthand_field_patterns)] // Verus expands destructuring shorthand.
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
    pub fn prepare_subtree(self, input: u32) -> (result: Result<SubtreeWrite<'a>>)
        ensures result.is_err() ==> final(resolved_storage(self))@ == resolved_original(self),
            result.is_ok() ==> resolved_subtree_request(self, result.unwrap(), input),
    {
        proof { use_type_invariant(&self); }
        if !self.subtree_fits(input) { return Err(ErrorKind::Capacity.into()); }
        let Resolved { bytes, used, path, trail, .. } = self;
        subtree_write::in_resolved(bytes, used, &path, trail, input)
    }

    /// Gives the original ancestor fault priority over an ordinary pre-write error.
    pub fn check_ancestor_error(&self,input:u32,poisoned:&mut bool)->Result<()> {
        proof{use_type_invariant(self);}
        match plan_in_trail(self.bytes,self.used,&self.path,&self.trail,input,u32::MAX) {
            Ok(_)=>Ok(()),Err(error)=>{
                if let Error::Fault{offset,..}=error {if offset!=Error::NO_OFFSET{list_patch::source_error(error,poisoned);}}
                Err(error)
            },
        }
    }

    /// Returns whether a forward checked ancestor pass can replace a source scan.
    #[inline(always)]
    pub fn has_ancestor_words(&self)->(result:bool)
        ensures result==resolved_has_ancestor_words(*self),
        no_unwind
    {
        proof {use_type_invariant(self);}
        let mut i=0usize;
        while i<self.trail.depth
            invariant i<=self.trail.depth<MAX_DEPTH,
                forall|j:int| 0<=j<i ==> self.trail.nodes[j].run.count==0 || self.trail.nodes[j].run.stride!=4,
            decreases self.trail.depth-i,
        {
            let run=self.trail.nodes[i].run;
            if run.count>0 && run.stride==4 {return true;}
            i+=1;
        }
        false
    }

    /// Returns whether one contiguous ancestor run has at least `minimum` words.
    ///
    /// `minimum` must be nonzero.
    #[inline(always)]
    pub fn has_ancestor_run(&self,minimum:u32)->(result:bool)
        requires minimum>0,
        ensures result==resolved_has_ancestor_run(*self,minimum),
        no_unwind
    {
        proof {use_type_invariant(self);}
        if self.trail.depth==1 {
            let run=self.trail.nodes[0].run;
            return run.count>=minimum && run.stride==4;
        }
        let mut i=0usize;
        while i<self.trail.depth
            invariant i<=self.trail.depth<MAX_DEPTH,
                forall|j:int| 0<=j<i ==> self.trail.nodes[j].run.count<minimum || self.trail.nodes[j].run.stride!=4,
            decreases self.trail.depth-i,
        {
            let run=self.trail.nodes[i].run;
            if run.count>=minimum && run.stride==4 {return true;}
            i+=1;
        }
        false
    }

    /// Defers contiguous ancestor-end checks until poisoned publication.
    /// Public checked preparation retains its complete read-only error frame.
    pub fn prepare_subtree_checked(self, input: u32, poisoned: &mut bool)
        -> (result: Result<subtree_checked::CheckedSubtreeReady<'a>>)
        ensures result.is_err() ==> final(resolved_storage(self))@==resolved_original(self),
            result.is_ok() ==> resolved_checked_request(self,subtree_checked::ready_value(result.unwrap()),input),
    {
        proof {use_type_invariant(&self);}
        if !self.subtree_fits(input) {return Err(ErrorKind::Capacity.into());}
        let Resolved {bytes,used,path,trail,..}=self;
        let inner = subtree_checked::in_resolved(bytes,used,&path,trail,input,poisoned)?;
        Ok(subtree_checked::seal_ready(inner))
    }

    /// Captures a header-only request for growth followed by fresh checked admission.
    pub fn defer_subtree(self,input:u32)->(result:Result<subtree_checked::CheckedSubtreePlan>)
        ensures final(resolved_storage(self))@==resolved_original(self),
            result.is_ok() ==> subtree_checked::deferred_admitted(resolved_original(self),result.unwrap()),
    {
        proof {use_type_invariant(&self);}
        let Resolved{bytes,used,path,trail,..}=self;
        subtree_checked::defer_in_resolved(bytes,used,&path,trail,input)
    }

    /// Computes the initialized extent without scanning ancestor values.
    pub fn subtree_required(&self,input:u32)->(result:Result<u32>)
        ensures result.is_ok() ==> result.unwrap()==resolved_subtree_required(*self,input),
    {
        proof {use_type_invariant(self);}
        let region=self.trail.nodes[self.trail.depth].region;
        if let Some(width)=fixed_width(&self.trail.selected) {
            if input!=width {return Err(ErrorKind::InvalidBoundary.into());}
        }
        let delta=input as i64-(region.end-region.start) as i64;
        let new_used=checked_delta(self.used,delta)?;
        Ok(if new_used>self.used {new_used} else {self.used})
    }

    /// Prepares a subtree replacement using this exact resolution.
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
    pub fn prepare_replace(self, input: u32, limit: u32) -> (result: Result<Prepared<'a, 'static>>)
        ensures prepared_result(result, resolved_original(self)),
            result.is_err() ==> final(resolved_storage(self))@ == resolved_original(self),
            result.is_ok() ==> subtree_plan(prepared_plan(result.unwrap()))
                && resolved_request(self, prepared_plan(result.unwrap()), input, limit),
    {
        proof { use_type_invariant(&self); }
        if self.bytes.len() > limit as usize { return Err(ErrorKind::Capacity.into()); }
        let plan = plan_in_trail(self.bytes, self.used, &self.path, &self.trail, input, limit);
        let bytes = self.bytes;
        capture_checked(bytes, plan)
    }

    /// Defers retained contiguous ancestors for a lazy fixed-width list edit.
    #[cfg_attr(target_os="solana",inline(never))]
    pub fn prepare_list_ancestors(self,operation:ListEdit,input:u32,limit:u32)->(result:Result<AncestorPrepared<'a,'static>>)
        ensures result.is_err() ==> final(resolved_storage(self))@==resolved_original(self),
            result.is_ok() ==> resolved_ancestor_request(self,result.unwrap(),input,limit)
                && ancestor_edit::prepared_list(result.unwrap(),operation),
    {
        proof{use_type_invariant(&self);}
        let Resolved{bytes,used,path,trail,..}=self;
        ancestor_edit::prepare_entries(bytes,used,&path,trail,operation,input,limit)
    }

    /// Defers ancestor values while retaining the complete read-only fixed-map key search.
    #[cfg_attr(target_os="solana",inline(never))]
    pub fn prepare_map_ancestors<'key>(self,operation:MapEdit,key:&'key [u8],input:u32,limit:u32,hint:Option<crate::Search>)
        ->(result:Result<AncestorPrepared<'a,'key>>)
        ensures result.is_err() ==> final(resolved_storage(self))@==resolved_original(self),
            result.is_ok() ==> resolved_ancestor_request(self,result.unwrap(),input,limit)
                && ancestor_edit::prepared_map(result.unwrap(),operation,key@,input),
    {
        proof{use_type_invariant(&self);}
        let Resolved{bytes,used,path,trail,..}=self;
        ancestor_edit::prepare_map(bytes,used,&path,trail,operation,key,input,limit,hint)
    }

    /// Prepares a list edit without repeating resolution.
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
    pub fn prepare_list(self, operation: ListEdit, input: u32, limit: u32)
        -> (result: Result<Prepared<'a, 'static>>)
        ensures prepared_result(result, resolved_original(self)),
            result.is_err() ==> final(resolved_storage(self))@ == resolved_original(self),
            result.is_ok() ==> planned_list_for(prepared_plan(result.unwrap()), operation)
                && resolved_request(self, prepared_plan(result.unwrap()), input, limit),
    {
        proof { use_type_invariant(&self); }
        if self.bytes.len() > limit as usize { return Err(ErrorKind::Capacity.into()); }
        let root = self.trail.root;
        let plan = list_in_trail(self.bytes, self.used, &root, &self.path, self.trail, operation, input, limit);
        let bytes = self.bytes;
        capture_checked(bytes, plan)
    }

    /// Admits indexed list geometry without reading untouched source ends.
    #[allow(non_shorthand_field_patterns)]
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(not(all(target_os = "solana", feature = "max-depth-16")), inline(always))]
    pub fn prepare_list_checked(self, operation: ListEdit, input: u32, limit: u32, poisoned: &mut bool)
        -> (result: Result<CheckedListPrepared<'a, 'static>>)
        ensures result.is_err() ==> final(resolved_storage(self))@ == resolved_original(self),
            result.is_ok() ==> list_patch::admitted(resolved_original(self), list_patch::prepared_plan(result.unwrap())),
    {
        proof { use_type_invariant(&self); }
        let Resolved { bytes, used, path, trail, .. } = self;
        list_patch::prepare_in_resolved(bytes, used, &path, trail, operation, input, limit, poisoned)
    }

    /// Admits an indexed map without certifying unvisited source ends.
    #[allow(clippy::too_many_arguments, non_shorthand_field_patterns)]
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(not(all(target_os = "solana", feature = "max-depth-16")), inline(always))]
    pub fn prepare_map_checked<'key>(self, operation: MapEdit, key: &'key [u8], input: u32,
        limit: u32, hint: Option<crate::Search>, poisoned: &mut bool)
        -> (result: Result<CheckedListPrepared<'a, 'key>>)
        ensures result.is_err() ==> final(resolved_storage(self))@ == resolved_original(self),
            result.is_ok() ==> list_patch::admitted(resolved_original(self), list_patch::prepared_plan(result.unwrap()))
                && list_patch::prepared_original(result.unwrap()) == resolved_original(self),
    {
        proof { use_type_invariant(&self); }
        let Resolved { bytes, used, path, trail, .. } = self;
        list_patch::prepare_map_in_resolved(bytes, used, &path, trail, operation, key, input, limit, hint, poisoned)
    }

    /// Prepares a sorted map edit, retaining the immutable external query.
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
    pub fn prepare_map<'key>(self, operation: MapEdit, key: &'key [u8], input: u32, limit: u32)
        -> (result: Result<Prepared<'a, 'key>>)
        ensures prepared_result(result, resolved_original(self)),
            result.is_err() ==> final(resolved_storage(self))@ == resolved_original(self),
            result.is_ok() ==> planned_map(prepared_plan(result.unwrap()), operation, key@, input)
                && previous_map_exact(resolved_original(self), prepared_plan(result.unwrap()))
                && resolved_request(self, prepared_plan(result.unwrap()), input, limit),
    {
        proof { use_type_invariant(&self); }
        if self.bytes.len() > limit as usize { return Err(ErrorKind::Capacity.into()); }
        let root = self.trail.root;
        let plan = map_in_trail(self.bytes, self.used, &root, &self.path, self.trail, operation, key, input, limit, None);
        let bytes = self.bytes;
        capture_checked(bytes, plan)
    }
    /// Prepares a retained entry, rechecking its ordinal on these exact bytes.
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
    pub fn prepare_entry<'key>(self, operation: MapEdit, key: &'key [u8], input: u32, limit: u32, hint: crate::Search)
        -> (result: Result<Prepared<'a, 'key>>)
        ensures prepared_result(result, resolved_original(self)),
            result.is_err() ==> final(resolved_storage(self))@ == resolved_original(self),
            result.is_ok() ==> planned_map(prepared_plan(result.unwrap()), operation, key@, input)
                && previous_map_exact(resolved_original(self), prepared_plan(result.unwrap()))
                && resolved_request(self, prepared_plan(result.unwrap()), input, limit),
    {
        proof { use_type_invariant(&self); }
        if self.bytes.len() > limit as usize { return Err(ErrorKind::Capacity.into()); }
        let root = self.trail.root;
        let plan = map_in_trail(self.bytes, self.used, &root, &self.path, self.trail, operation, key, input, limit, Some(hint));
        let bytes = self.bytes;
        capture_checked(bytes, plan)
    }
}

/// Opaque checked plan and the exact exclusive bytes on which it was prepared.
///
/// Inspection is read-only. Consume `begin_prepared` with sufficient capacity,
/// or `detach` before backend growth and then use detached readiness. No mutable
/// byte access or plan substitution is exposed before the Apply transition.
///
/// Inspection cannot mutate account bytes:
///
/// ```compile_fail
/// # #![forbid(unsafe_code)]
/// use blubber_core::{resolve_edit, InlinePath, Layout};
/// let mut bytes = [0; 16];
/// bytes[8..12].copy_from_slice(&16u32.to_le_bytes());
/// let prepared = resolve_edit(&mut bytes, 16, &Layout::Str, &InlinePath::new())
///     .unwrap().prepare_replace(4, 16).unwrap();
/// prepared.inspect_prepared()[12] = 1;
/// ```
#[doc(hidden)]
#[must_use = "apply the prepared edit or explicitly detach it before growth"]
pub struct Prepared<'a, 'key> {
    bytes: &'a mut [u8],
    plan: Plan<'key>,
    original: Ghost<Seq<u8>>,
}

/// Exact immutable plan captured by preparation.
pub closed spec fn prepared_plan<'key>(prepared: Prepared<'_, 'key>) -> Plan<'key> { prepared.plan }

/// Exact original bytes captured by preparation.
pub closed spec fn prepared_original(prepared: Prepared) -> Seq<u8> { prepared.original@ }

/// A fixed-map removal whose checked lookup found no key.
pub closed spec fn missing_fixed_remove(prepared: Prepared) -> bool {
    prepared.plan.map.is_some() && matches!(prepared.plan.map.unwrap().operation, MapEdit::Remove)
        && !prepared.plan.map.unwrap().found
}

/// The checked generic result leaves the entire physical buffer unchanged.
pub closed spec fn removal_identity(prepared: Prepared) -> bool {
    replacement_result(prepared.original@, prepared.original@, prepared.original@, prepared.plan)
}

/// Exact storage retained by preparation.
pub closed spec fn prepared_storage<'a>(prepared: Prepared<'a, '_>) -> &'a mut [u8] { prepared.bytes }

/// Exact unchanged bytes and all checked movement/patch facts.
pub closed spec fn prepared_result(result: Result<Prepared>, original: Seq<u8>) -> bool {
    result.is_ok() ==> result.unwrap().bytes@ == original
        && result.unwrap().original@ == original
        && replacement_checked(original, result.unwrap().plan)
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
fn capture<'a, 'key>(bytes: &'a mut [u8], plan: Plan<'key>) -> (result: Prepared<'a, 'key>)
    requires replacement_checked(old(bytes)@, plan),
    ensures result.bytes@ == old(bytes)@, result.original@ == old(bytes)@, result.plan == plan,
    no_unwind
{
    let original = Ghost(bytes@);
    Prepared { bytes, plan, original }
}

#[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
#[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
#[allow(clippy::question_mark)] // Verus has no no_unwind contract for FromResidual.
fn capture_checked<'a, 'key>(bytes: &'a mut [u8], plan: Result<Plan<'key>>)
    -> (result: Result<Prepared<'a, 'key>>)
    requires plan.is_ok() ==> replacement_checked(old(bytes)@, plan.unwrap()),
    ensures prepared_result(result, old(bytes)@),
        result.is_ok() == plan.is_ok(),
        result.is_ok() ==> prepared_plan(result.unwrap()) == plan.unwrap(),
        result.is_err() ==> result.err() == plan.err() && final(bytes)@ == old(bytes)@,
    no_unwind
{
    let checked = match plan { Ok(plan) => plan, Err(error) => return Err(error) };
    Ok(capture(bytes, checked))
}

impl<'a, 'key> Prepared<'a, 'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        self.bytes@ == self.original@ && replacement_checked(self.original@, self.plan)
    }

    /// Returns the required initialized storage length, including the old tail.
    #[inline(always)]
    pub fn prepared_len(&self) -> (result: u32)
        ensures result == required_len(prepared_plan(*self)),
    { self.plan.required_storage_len() }

    /// Identifies an absent fixed-map removal, with the generic identity result.
    #[inline(always)]
    pub fn is_missing_fixed_remove(&self) -> (result: bool)
        ensures result == missing_fixed_remove(*self),
            result ==> removal_identity(*self),
        no_unwind
    {
        proof { use_type_invariant(self); }
        if let Some(change) = self.plan.map {
            if matches!(change.operation, MapEdit::Remove) && !change.found {
                proof { missing_fixed_remove_identity(self.original@, self.plan); }
                return true;
            }
        }
        false
    }

    /// Inspects only the selected old value; never lends mutable storage.
    #[inline(always)]
    pub fn inspect_removed(&self) -> (result: Option<Span>)
        ensures removed_exact(prepared_plan(*self), result),
    { self.plan.removed_span() }

    /// Borrows unchanged account bytes for checked old-value decoding.
    #[inline(always)]
    pub fn inspect_prepared(&self) -> (result: &[u8])
        ensures result@ == prepared_original(*self),
    {
        proof { use_type_invariant(self); }
        self.bytes
    }

    /// Releases the byte borrow for growth; its plan needs detached readiness.
    #[inline(always)]
    pub fn detach(self) -> (result: Plan<'key>)
        ensures result == prepared_plan(self), final(prepared_storage(self))@ == prepared_original(self),
            replacement_checked(prepared_original(self), result),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        self.plan
    }

    /// Consumes exact preparation into phase zero without a readiness replan.
    ///
    /// Insufficient capacity returns `InvalidPlan` with all bytes unchanged.
    /// Ordinary borrowing excludes all mutation since preparation, so every
    /// movement/patch fact holds over the captured current bytes.
    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(all(target_os = "solana", not(feature = "max-depth-16")), inline(always))]
    pub fn begin_prepared(self) -> (result: Result<Apply<'a, 'key>>)
        ensures replace_failure(result, prepared_original(self), final(prepared_storage(self))@),
            apply_started(result, prepared_plan(self), prepared_original(self)),
            result.is_ok() ==> apply_phase(result.unwrap(), 0),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        if self.bytes.len() < self.plan.required as usize { return Err(Error::new(ErrorKind::InvalidPlan, Error::NO_OFFSET)); }
        let bytes = self.bytes;
        let plan = self.plan;
        let original = self.original;
        Ok(Apply { bytes, plan, original, phase: 0 })
    }
}

}
