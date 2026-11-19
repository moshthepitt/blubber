//! Header-admitted fixed-entry edits with checked ancestor publication.
#![cfg_attr(verus_only, allow(missing_docs))]
use super::*;
verus! {
/// Admits fixed-entry geometry while deferring contiguous ancestor values.
#[allow(clippy::too_many_arguments)] // Shared checked request and movement geometry.
pub(super) fn entry_headers(bytes: &[u8], used_len: u32, _layout: &Layout, path: &InlinePath,
    trail: Trail, operation: ListEdit, input_len: u32, width: u32, storage_limit: u32)
    -> (result: Result<Plan<'static>>)
    requires trail_valid(bytes@, *_layout, *path, trail, used_len),
        12 <= used_len <= bytes.len() <= storage_limit <= u32::MAX,
        words::le_word(bytes@, 8) == used_len,
        match trail.nodes[trail.depth as int].layout.head() {
            LayoutHead::List { element } => width_spec(element) == Some(width),
            LayoutHead::Map { key, value } => width_spec(value).is_some()
                && (words::le_word(bytes@, trail.nodes[trail.depth as int].region.start as int) > 0
                    || !matches!(operation, ListEdit::Clear | ListEdit::Truncate(_))
                    ==> width == key.width + width_spec(value).unwrap()),
            _ => false,
        },
    ensures result.is_ok() ==> replacement_admitted(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used_len, *_layout, *path, input_len, storage_limit)
        && planned_list_for(result.unwrap(), operation)
        && result.unwrap().trail == trail && result.unwrap().map.is_none()
        && result.unwrap().list.is_some() && result.unwrap().variable_map.is_none()
        && result.unwrap().indexed.is_none() && result.unwrap().splice.is_none(),
        admission::entries_ready(bytes@, trail, operation, input_len, width, storage_limit)
            ==> result == Ok(admission::entries_plan_for(bytes@, *path, trail, operation, input_len, width, storage_limit)),
{
    let node = trail.nodes[trail.depth];
    let count = read_u32(bytes, node.region.start)?;
    proof {
        indexed_insert::word_in_slice(bytes@, node.region.start as int, node.region.end as int, 0);
        let local = node_bytes(bytes@, node);
        assert(crate::edit_layout::header_valid(local, trail.selected));
        assert(local.len() == node.region.end - node.region.start);
        assert(words::le_word(local, 0) == count);
        match trail.selected.head() {
            LayoutHead::List { .. } => { assert(crate::list_layout::header_shape(local, trail.selected));
                assert(local.len() == 4 + count as int * width);
                assert(count == 0 ==> local.len() == 4) by(nonlinear_arith)
                    requires local.len() == 4 + count as int * width;
            },
            LayoutHead::Map { .. } => {
                assert(crate::map_layout::header_shape(local, trail.selected));
                assert(local.len() == 4 + count as int * crate::map_layout::entry_stride(trail.selected));
                assert(count == 0 ==> local.len() == 4) by(nonlinear_arith)
                    requires local.len() == 4 + count as int * crate::map_layout::entry_stride(trail.selected);
                assert(count > 0 ==> width == crate::map_layout::entry_stride(trail.selected));
            },
            _ => {},
        }
        assert(node.region.end == node.region.start + 4 + count as int * width) by(nonlinear_arith)
            requires count == 0 ==> local.len() == 4,
                count > 0 ==> local.len() == 4 + count as int * width,
                local.len() == node.region.end - node.region.start;
    }
    let (first, last, new_count) = match operation {
        ListEdit::Insert(i) => {
            if i > count { return Err(ErrorKind::IndexOutOfBounds.into()); }
            let n = checked_delta(count, 1)?;
            (i, i, n)
        },
        ListEdit::Remove(i) | ListEdit::Replace(i) => {
            if i >= count { return Err(ErrorKind::IndexOutOfBounds.into()); }
            (i, i + 1, if matches!(operation, ListEdit::Remove(_)) { count - 1 } else { count })
        },
        ListEdit::Pop => (if count == 0 { 0u32 } else { count - 1 }, count,
            if count == 0 { 0u32 } else { count - 1 }),
        ListEdit::Truncate(n) => {
            let kept = if n < count { n } else { count };
            (kept, count, kept)
        },
        ListEdit::Clear => (0u32, count, 0u32),
    };
    let expected_input = if matches!(operation, ListEdit::Insert(_) | ListEdit::Replace(_)) { width } else { 0u32 };
    if input_len != expected_input { return Err(ErrorKind::InvalidBoundary.into()); }
    proof {
        assert(first as int * width <= count as int * width) by(nonlinear_arith)
            requires first <= count;
        assert(last as int * width <= count as int * width) by(nonlinear_arith)
            requires last <= count;
    }
    let start = (node.region.start as u64 + 4 + first as u64 * width as u64) as u32;
    let end = (node.region.start as u64 + 4 + last as u64 * width as u64) as u32;
    proof {
        assert(first as int * width <= last as int * width) by(nonlinear_arith)
            requires first <= last;
    }
    let delta = input_len as i64 - (end - start) as i64;
    proof {
        let indices = list_indices(operation, count);
        assert(delta == input_len as int - (indices.1 - indices.0) * width) by(nonlinear_arith)
            requires first == indices.0, last == indices.1,
                start == node.region.start + 4 + first as int * width,
                end == node.region.start + 4 + last as int * width,
                delta == input_len as int - (end - start);
    }
    let new_used = checked_delta(used_len, delta)?;
    let required = if new_used > used_len { new_used } else { used_len };
    if required > storage_limit { return Err(ErrorKind::Capacity.into()); }
    let change = ListChange { operation, width, old_count: count, new_count, first, last };
    let plan = Plan { key: &[], map: None, variable_map: None, region: Span { start, end }, indexed: None, splice: None, list: Some(change), trail, path: *path,
        discriminator: [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]],
        old_used: used_len, replacement_len: input_len, delta, new_used, required, storage_limit };
    proof {
        assert(node.region.end + delta == node.region.start + 4 + new_count * width) by(nonlinear_arith)
            requires node.region.end == node.region.start + 4 + count * width,
                start == node.region.start + 4 + first * width, end == node.region.start + 4 + last * width,
                delta == input_len as int - (end - start),
                new_count == count - (last - first)
                    + if matches!(operation, ListEdit::Insert(_) | ListEdit::Replace(_)) { 1int } else { 0int },
                input_len == if matches!(operation, ListEdit::Insert(_) | ListEdit::Replace(_)) { width } else { 0u32 };
    }
    assert(list_checked(bytes@, plan));
    Ok(plan)
}

#[allow(clippy::too_many_arguments)] // Shared checked request and movement geometry.
/// Checks fixed-map lookup and request geometry before ancestor publication.
pub(super) fn map_headers<'key>(bytes: &[u8], used_len: u32, layout: &Layout, path: &InlinePath,
    trail: Trail, operation: MapEdit, key: &'key [u8], input: u32, storage_limit: u32, hint: Option<crate::Search>)
    -> (result: Result<Plan<'key>>)
    requires trail_valid(bytes@, *layout, *path, trail, used_len),
        12 <= used_len <= bytes.len() <= storage_limit <= u32::MAX,
        words::le_word(bytes@, 8) == used_len,
    ensures result.is_ok() ==> replacement_admitted(bytes@, result.unwrap())
        && planned_for(result.unwrap(), used_len, *layout, *path, input, storage_limit)
        && fixed_map::planned_map_for(result.unwrap(), operation, key@, input)
        && fixed_map::map_previous_exact(bytes@, result.unwrap()) && fixed_map_plan(result.unwrap())
        && result.unwrap().indexed.is_none() && result.unwrap().splice.is_none() && result.unwrap().variable_map.is_none(),
        matches!(operation, MapEdit::Insert) && hint.is_none()
            && admission::map_tail_ready(bytes@, trail, input, key, storage_limit)
            ==> result == Ok(admission::map_tail_plan(bytes@, *path, trail, input, key, storage_limit)),
        forall|index: u32| matches!(operation, MapEdit::Insert) && hint.is_none()
            && admission::map_insert_ready(bytes@, trail, input, key, index, storage_limit)
            ==> result == Ok(admission::map_insert_plan(bytes@, *path, trail, input, key, index, storage_limit)),
{
    hide(crate::search::lex_less);
    hide(crate::search::keys_ordered);
    hide(crate::search::keys_canonical);
    let n = trail.nodes[trail.depth];
    let (key_width, value_width) = if let Layout::Flat {checked} = trail.selected {
        proof { reveal(crate::layout_head::head_spec); }
        if checked.header(0) != 8 {return Err(ErrorKind::InvalidSchema.into());}
        let value = Layout::Flat {checked:checked.only_child()};
        let width = match fixed_width(&value) {
            Some(width) => width, None => return Err(ErrorKind::InvalidSchema.into()),
        };
        (checked.header(6), width)
    } else {match trail.selected.head() {
        LayoutHead::Map { key, value } => (key.width, match fixed_width(&value) {
            Some(width) => width, None => return Err(ErrorKind::InvalidSchema.into()),
        }),
        _ => return Err(ErrorKind::InvalidSchema.into()),
    }};
    let local = &bytes[n.region.start as usize..n.region.end as usize];
    if !trail.local { crate::validate_map_keys(local, &trail.selected).map_err(|e: Error| e.rebase(n.region.start))?; }
    let count = read_u32(local, 0)?;
    let lookup = if matches!(operation, MapEdit::Clear) {
        if !key.is_empty() { return Err(ErrorKind::InvalidBoundary.into()); }
        crate::Search { index: 0, found: false }
    } else { match hint {
        Some(hint) => crate::search::search_selected(local, &trail.selected, key, hint).map_err(|e: Error| e.rebase(n.region.start))?,
        None => if trail.local { crate::search::search_map_local(local, &trail.selected, key).map_err(|e: Error| e.rebase(n.region.start))? } else { crate::search_map(local, &trail.selected, key)? },
    } };
    proof {
        if matches!(operation, MapEdit::Insert) && hint.is_none()
            && admission::map_tail_ready(bytes@, trail, input, key, storage_limit) {
            indexed_insert::word_in_slice(bytes@, n.region.start as int, n.region.end as int, 0);
            admission::tail_lookup(local@, trail.selected, key@, lookup);
        }
    }
    proof {
        assert forall|index: u32| matches!(operation, MapEdit::Insert) && hint.is_none()
            && admission::map_insert_ready(bytes@, trail, input, key, index, storage_limit)
            implies lookup == (crate::Search { index, found: false }) by {
            admission::insert_lookup(local@, trail.selected, key@, index, lookup);
        }
    }
    if matches!(operation, MapEdit::Replace) && !lookup.found { return Err(ErrorKind::MissingKey.into()); }
    let expected = if matches!(operation, MapEdit::Insert | MapEdit::Replace) { value_width } else { 0u32 };
    if input != expected { return Err(ErrorKind::InvalidBoundary.into()); }
    let change = MapChange { operation, key_width, value_width, index: lookup.index, found: lookup.found };
    let entry_op = match operation {
        MapEdit::Insert => if lookup.found { ListEdit::Replace(lookup.index) } else { ListEdit::Insert(lookup.index) },
        MapEdit::Replace => ListEdit::Replace(lookup.index),
        MapEdit::Remove => if lookup.found { ListEdit::Remove(lookup.index) } else { ListEdit::Truncate(count) },
        MapEdit::Clear => ListEdit::Clear,
    };
    // Empty no-ops need no representable stride. A later insertion checks it.
    let width = if count == 0 && matches!(operation, MapEdit::Clear | MapEdit::Remove) { 0u32 }
        else { checked_u32(checked_add(key_width as usize, value_width as usize)?)? };
    let entry_input = if matches!(operation, MapEdit::Insert | MapEdit::Replace) { width } else { 0u32 };
    let base = entry_headers(bytes, used_len, layout, path, trail, entry_op, entry_input, width, storage_limit)?;
    let plan = Plan { key, map: Some(change), ..base };
    proof {
        reveal(replacement_admitted);reveal(fixed_map::map_checked);reveal(list_checked);
        assert(replacement_admitted(bytes@,plan));
        fixed_map::previous_refinement(bytes@,plan);
        fixed_map::attached_request(plan);
        assert(fixed_map::input_len(plan)==input);
        assert(fixed_map::planned_map_for(plan,operation,key@,input));
        assert(planned_for(plan,used_len,*layout,*path,input,storage_limit));
    }
    Ok(plan)
}

#[verifier::spinoff_prover]
fn list_headers(bytes:&[u8],used:u32,path:&InlinePath,trail:Trail,operation:ListEdit,input:u32,limit:u32)->(result:Result<Plan<'static>>)
    requires trail_valid(bytes@,trail.root,*path,trail,used),12<=used<=bytes.len()<=limit<=u32::MAX,words::le_word(bytes@,8)==used,
    ensures result.is_ok() ==> replacement_admitted(bytes@,result.unwrap())
        && planned_for(result.unwrap(),used,trail.root,*path,input,limit)
        && result.unwrap().indexed.is_none() && result.unwrap().splice.is_none() && result.unwrap().variable_map.is_none(),
{
    let element=match trail.selected.head(){LayoutHead::List{element}=>element,_=>return Err(ErrorKind::InvalidSchema.into())};
    let width=match fixed_width(&element){Some(w)=>w,None=>return Err(ErrorKind::InvalidSchema.into())};
    entry_headers(bytes,used,&trail.root,path,trail,operation,input,width,limit)
}

/// Header and interval admission without reading contiguous ancestor values.
pub(super) open spec fn admitted(bytes:Seq<u8>,p:Plan)->bool {
    replacement_admitted(bytes,p)
    && (forall|i:int| 0<=i<p.trail.depth && p.trail.nodes[i].run.count>0 ==>
        #[trigger] subtree_checked::bounds_admitted(p.trail.nodes[i].payload_len,p.delta))
    && (forall|i:int| 0<=i<p.trail.depth && p.trail.nodes[i].run.stride!=4 ==>
        #[trigger] run_safe(bytes,p.trail.nodes[i].run,p.delta)
        && (p.delta!=0 ==> run_payload(bytes,p.trail.nodes[i].run,p.trail.nodes[i].payload_len)))
}

/// Completes numeric/strided admission while leaving contiguous values untouched.
#[verifier::spinoff_prover]
#[cfg_attr(all(target_os="solana",feature="max-depth-16"),inline(never))]
#[cfg_attr(not(all(target_os="solana",feature="max-depth-16")),inline(always))]
pub(super) fn admit_ancestors(bytes:&[u8],p:&Plan)->(result:Result<()>)
    requires replacement_admitted(bytes@,*p),
    ensures result.is_ok() ==> admitted(bytes@,*p),
{
    let mut i=0usize;
    while i<p.trail.depth
        invariant i<=p.trail.depth<MAX_DEPTH,replacement_admitted(bytes@,*p),
            forall|j:int| 0<=j<i && p.trail.nodes[j].run.count>0 ==>
                #[trigger] subtree_checked::bounds_admitted(p.trail.nodes[j].payload_len,p.delta),
            forall|j:int| 0<=j<i && p.trail.nodes[j].run.stride!=4 ==>
                #[trigger] run_safe(bytes@,p.trail.nodes[j].run,p.delta)
                && (p.delta!=0 ==> run_payload(bytes@,p.trail.nodes[j].run,p.trail.nodes[j].payload_len)),
        decreases p.trail.depth-i,
    {
        let node=p.trail.nodes[i];
        if node.run.count>0 {
            let lower=if p.delta<0 {(-p.delta) as u32} else {0};
            if lower>node.payload_len || node.payload_len as i64+p.delta>u32::MAX as i64 {
                return Err(Error::new(ErrorKind::ArithmeticOverflow,node.run.offset));
            }
        }
        if node.run.stride!=4 {
            proof {
                subtree_checked::admitted_geometry_proof(bytes@,*p);
                subtree_checked::admitted_extent(bytes@,*p,i as int);
            }
            check_run(bytes,node.run,p.delta,node.payload_len)?;
        }
        assert forall|j:int| 0<=j<i+1 && p.trail.nodes[j].run.stride!=4 implies
            #[trigger] run_safe(bytes@,p.trail.nodes[j].run,p.delta)
            && (p.delta!=0 ==> run_payload(bytes@,p.trail.nodes[j].run,p.trail.nodes[j].payload_len)) by {
            if j==i {assert(p.trail.nodes[j]==node);}
        }
        i+=1;
    }
    Ok(())
}

/// Retains old source-error precedence when an ordinary pre-write check fails.
#[verifier::spinoff_prover]
#[cfg_attr(target_os="solana",inline(never))]
pub(super) fn scan_prewrite(bytes:&[u8],trail:&Trail,delta:i64,poisoned:&mut bool)->(result:Result<()>)
    requires trail.depth<MAX_DEPTH,bytes.len()<=u32::MAX,-(u32::MAX as int)<=delta<=u32::MAX,
        forall|i:int| 0<=i<trail.depth ==> #[trigger] run_within(trail.nodes[i].run,0,bytes.len() as int),
    ensures *old(poisoned) ==> *final(poisoned),
        result.is_ok() ==> *final(poisoned)==*old(poisoned),
{
    let mut i=0usize;
    while i<trail.depth
        invariant i<=trail.depth<MAX_DEPTH,bytes.len()<=u32::MAX,-(u32::MAX as int)<=delta<=u32::MAX,
            *poisoned==*old(poisoned),
            forall|j:int| 0<=j<trail.depth ==> #[trigger] run_within(trail.nodes[j].run,0,bytes.len() as int),
        decreases trail.depth-i,
    {
        match check_run(bytes,trail.nodes[i].run,delta,trail.nodes[i].payload_len) {
            Ok(())=>(),Err(error)=>{list_patch::source_error(error,poisoned);return Err(error);},
        }
        i+=1;
    }
    Ok(())
}

/// Projects admitted trail bounds without reading an ancestor value.
pub(super) proof fn scan_geometry(bytes:Seq<u8>,p:Plan)
    requires replacement_admitted(bytes,p),
    ensures p.trail.depth<MAX_DEPTH,bytes.len()<=u32::MAX,-(u32::MAX as int)<=p.delta<=u32::MAX,
        forall|i:int| 0<=i<p.trail.depth ==> #[trigger] run_within(p.trail.nodes[i].run,0,bytes.len() as int),
{
    subtree_checked::admitted_geometry_proof(bytes,p);
    assert forall|i:int| 0<=i<p.trail.depth implies #[trigger] run_within(p.trail.nodes[i].run,0,bytes.len() as int) by {
        subtree_checked::admitted_extent(bytes,p,i);
    }
}

/// Rechecks ancestor diagnostics against current headers on a failed backend resize.
pub(super) fn check_detached(bytes:&[u8],p:&Plan,poisoned:&mut bool)->(result:Result<()>)
    ensures *old(poisoned) ==> *final(poisoned),
        result.is_ok() ==> *final(poisoned)==*old(poisoned),
{
    match envelope(bytes,p.old_used){Ok(())=>(),Err(error)=>{list_patch::source_error(error,poisoned);return Err(error);}}
    if bytes.len()>p.storage_limit as usize{return Err(ErrorKind::Capacity.into());}
    if p.trail.depth>=MAX_DEPTH{return Err(ErrorKind::InvalidPlan.into());}
    let selected=p.trail.nodes[p.trail.depth].region;
    if selected.end<selected.start{return Err(ErrorKind::InvalidPlan.into());}
    let input=checked_delta(selected.end-selected.start,p.delta)?;
    let trail=match trace_local(bytes,p.old_used,&p.trail.root,&p.path){Ok(t)=>t,Err(error)=>{list_patch::source_error(error,poisoned);return Err(error);}};
    match plan_in_trail(bytes,p.old_used,&p.path,&trail,input,p.storage_limit) {
        Ok(_)=>Ok(()),Err(error)=>{source_at(error,poisoned);Err(error)},
    }
}

/// Whether publication has any contiguous ancestor values left to check.
#[inline(always)]
fn pending(p:&Plan)->(result:bool)
    requires p.trail.depth<MAX_DEPTH,
    ensures result==(p.delta!=0 && exists|i:int| 0<=i<p.trail.depth
        && #[trigger] p.trail.nodes[i].run.stride==4 && p.trail.nodes[i].run.count>0),
    no_unwind
{
    if p.delta==0{return false;}
    let mut i=0usize;
    while i<p.trail.depth
        invariant i<=p.trail.depth<MAX_DEPTH,p.delta!=0,
            forall|j:int| 0<=j<i ==> p.trail.nodes[j].run.stride!=4 || p.trail.nodes[j].run.count==0,
        decreases p.trail.depth-i,
    {
        let run=p.trail.nodes[i].run;
        if run.stride==4 && run.count>0{return true;}
        i+=1;
    }
    false
}

/// Publishes one admitted plan through the same checked ancestor kernel.
#[verifier::spinoff_prover]
#[cfg_attr(target_os="solana",inline(never))]
pub(super) fn patch_ancestors(bytes:&mut [u8],p:&Plan,Ghost(original):Ghost<Seq<u8>>)->(result:Result<()>)
    requires admitted(original,*p),old(bytes).len()==original.len(),
        (forall|at:int| 0<=at<target(*p).start ==> old(bytes)@[at]==original[at]),p.delta!=0,
    ensures replacement_checked(original,*p) ==> result.is_ok(),
        result.is_ok() ==> replacement_checked(original,*p)
            && ancestors_delta(original,final(bytes)@,*p,p.trail.depth as int),
        ancestors_frame(old(bytes)@,final(bytes)@,*p,p.trail.depth as int),
    no_unwind
{
    proof {subtree_checked::admitted_geometry_proof(original,*p);}
    #[cfg(feature="max-depth-16")]
    let mut runs=[PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0}];
    #[cfg(feature="max-depth-16")]
    let mut payloads=[0u32,0u32,0u32,0u32,0u32,0u32,0u32,0u32,0u32,0u32,0u32,0u32,0u32,0u32,0u32,0u32];
    #[cfg(not(feature="max-depth-16"))]
    let mut runs=[PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0},PatchRun{offset:0,stride:4,count:0}];
    #[cfg(not(feature="max-depth-16"))]
    let mut payloads=[0u32,0u32,0u32,0u32,0u32,0u32,0u32,0u32];
    let mut i=0usize;
    while i<p.trail.depth
        invariant i<=p.trail.depth<MAX_DEPTH,
            forall|j:int| 0<=j<i ==> #[trigger] runs[j]==p.trail.nodes[j].run,
            forall|j:int| 0<=j<i ==> #[trigger] payloads[j]==p.trail.nodes[j].payload_len,
        decreases p.trail.depth-i,
    {runs[i]=p.trail.nodes[i].run;payloads[i]=p.trail.nodes[i].payload_len;i+=1;}
    subtree_checked::patch_subtree_runs(bytes,&runs,&payloads,p.trail.depth,p.delta,Ghost(*p),Ghost(original))
}
#[cfg_attr(all(target_os="solana",feature="max-depth-16"),inline(never))]
#[cfg_attr(not(all(target_os="solana",feature="max-depth-16")),inline(always))]
pub(super) fn finish_replacement(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>) -> (result:Result<()>)
    requires admitted(original,*plan), payload_frame(original, old(bytes)@, *plan),
        plan.required <= old(bytes).len(),
    ensures replacement_checked(original,*plan) ==> result.is_ok(),
        result.is_ok() ==> replacement_checked(original,*plan)
            && replacement_result(original,old(bytes)@,final(bytes)@,*plan),
    no_unwind
{
    if !pending(plan) {
        proof {
            if plan.delta==0 {admission::idle_runs(original,plan.trail,plan.path,plan.old_used,plan.delta);}
            assert forall|j:int| 0<=j<plan.trail.depth implies
                run_safe(original,plan.trail.nodes[j].run,plan.delta)
                && (plan.delta!=0 ==> run_payload(original,plan.trail.nodes[j].run,plan.trail.nodes[j].payload_len)) by {
                let run=plan.trail.nodes[j].run;
                if plan.delta!=0 && run.stride==4 {assert(run.count==0);}
            }
            assert(replacement_checked(original,*plan));
        }
        super::finish_replacement(bytes,plan,Ghost(original));
        return Ok(());
    }
    finish_pending(bytes,plan,Ghost(original))
}

/// Publishes deferred ancestors in a separate SBF frame.
#[cfg_attr(target_os="solana",inline(never))]
#[verifier::spinoff_prover]
fn finish_pending(bytes: &mut [u8], plan: &Plan, Ghost(original): Ghost<Seq<u8>>) -> (result:Result<()>)
    requires admitted(original,*plan), payload_frame(original, old(bytes)@, *plan),
        plan.required <= old(bytes).len(),
    ensures replacement_checked(original,*plan) ==> result.is_ok(),
        result.is_ok() ==> replacement_checked(original,*plan)
            && replacement_result(original,old(bytes)@,final(bytes)@,*plan),
    no_unwind
{
    let ghost encoded = bytes@;
    if plan.delta != 0 {
        match patch_ancestors(bytes,plan,Ghost(original)) {Ok(())=>(),Err(error)=>return Err(error),}
    } else {
        proof {
            admission::idle_runs(original,plan.trail,plan.path,plan.old_used,plan.delta);
            assert(replacement_checked(original,*plan));replacement_geometry(original,*plan);
            ancestors_unchanged(original,bytes@,*plan);
        }
    }
    proof {replacement_geometry(original,*plan);}
    let ghost patched = bytes@;
    if let Some(change) = plan.list {
        if change.new_count != change.old_count {
            let at = plan.trail.nodes[plan.trail.depth].region.start;
            match crate::write_u32(bytes, at, change.new_count) { Ok(()) => (), Err(_) => panic!("checked list count") };
        }
    }
    let ghost counted = bytes@;
    if plan.delta < 0 {
        clear_tail(bytes, plan.new_used, plan.old_used);
    }
    if plan.delta != 0 {
        match crate::write_u32(bytes, 8, plan.new_used) { Ok(()) => (), Err(_) => panic!("checked used length") };
    }
    proof {
        assert forall|p: int| ancestor_changed(*plan, plan.trail.depth as int, p)
            implies bytes@[p] == patched[p] by {
            ancestor_range(original, *plan, p);
            let j = choose|j: int| 0 <= j < plan.trail.depth && #[trigger] run_changed(plan.trail.nodes[j].run,
                plan.trail.nodes[j].run.count as int, p);
            let k = choose|k: int| 0 <= k < plan.trail.nodes[j].run.count
                && #[trigger] run_at(plan.trail.nodes[j].run, k) <= p < run_at(plan.trail.nodes[j].run, k) + 4;
            run_index_bounds(plan.trail.nodes[j].run, plan.trail.nodes[j + 1].region.start as int, k);
            trail_order(original, *plan, j + 1, plan.trail.depth as int);
        }
        ancestors_reframe(original, patched, bytes@, *plan);
    }
    let ghost published = bytes@;
    assert(words::le_word(published, 8) == plan.new_used);
    proof {
        assert forall|p: int| ancestor_changed(*plan, plan.trail.depth as int, p)
            implies bytes@[p] == published[p] by { ancestor_range(original, *plan, p); }
        ancestors_reframe(original, published, bytes@, *plan);
        assert(count_published(published, *plan));
        assert(count_published(bytes@, *plan));
        assert(words::le_word(bytes@, 8) == plan.new_used);
        assert forall|p: int| #![trigger bytes@[p]] 0 <= p < bytes.len() implies {
            &&& (p < 8 || plan.required <= p) ==> bytes@[p] == original[p]
            &&& (12 <= p < target(*plan).start && (plan.delta == 0 || !ancestor_changed(*plan, plan.trail.depth as int, p))
                    && !count_changed(*plan, p))
                ==> bytes@[p] == original[p]
            &&& target(*plan).start <= p < target(*plan).start + plan.replacement_len ==> bytes@[p] == encoded[p]
            &&& target(*plan).start + plan.replacement_len <= p < plan.new_used ==> bytes@[p] == original[p - plan.delta]
            &&& plan.new_used <= p < plan.old_used ==> bytes@[p] == 0
        } by {
            if ancestor_changed(*plan, plan.trail.depth as int, p) { ancestor_range(original, *plan, p); }
        }
        assert(replacement_result(original, encoded, bytes@, *plan));
    }
    Ok(())
}
/// An exclusive fixed-entry preparation with ancestor values deferred to publication.
#[doc(hidden)]
pub struct AncestorPrepared<'a,'key> {
    bytes:&'a mut [u8],plan:Plan<'key>,original:Ghost<Seq<u8>>,
}
/// A detached fixed-entry request, freshly replanned after storage growth.
#[doc(hidden)]
pub struct AncestorPlan<'key> {inner:Plan<'key>}
/// An ordered fixed-entry application with a poisoned publication boundary.
#[doc(hidden)]
#[repr(C)]
pub struct AncestorApply<'a,'key> {
    bytes:&'a mut [u8],plan:Plan<'key>,original:Ghost<Seq<u8>>,#[cfg_attr(not(verus_only), allow(dead_code))]
    phase: Ghost<u8>,
}
/// Prepared application before movement.
#[doc(hidden)]
#[repr(transparent)]
pub struct AncestorReady<'a,'key> { inner: AncestorApply<'a,'key> }
/// Moved application before issuing its destination.
#[doc(hidden)]
#[repr(transparent)]
pub struct AncestorMoved<'a,'key> { inner: AncestorApply<'a,'key> }
/// Proof view of the sealed prepared phase.
pub closed spec fn ready_value<'a,'key>(value: AncestorReady<'a,'key>) -> AncestorApply<'a,'key> { value.inner }
/// Proof view of the sealed moved phase.
pub closed spec fn moved_value<'a,'key>(value: AncestorMoved<'a,'key>) -> AncestorApply<'a,'key> { value.inner }
impl<'a,'key> AncestorReady<'a,'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool { apply_phase(self.inner, 0) }
    /// Consumes preparation and transports retained bytes under sticky poison.
    #[inline(always)]
    #[allow(non_shorthand_field_patterns)]
    pub fn move_regions(self, poisoned: &mut bool) -> (r: AncestorMoved<'a,'key>)
        ensures *final(poisoned), apply_phase(moved_value(r), 1),
        no_unwind
    { proof { use_type_invariant(&self); } let AncestorReady { mut inner } = self; inner.move_inner(poisoned); AncestorMoved { inner } }
}
impl<'a,'key> AncestorMoved<'a,'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool { apply_phase(self.inner, 1) }
    /// Issues the destination once; publication is available on the returned state.
    #[inline(always)]
    #[allow(non_shorthand_field_patterns)]
    pub fn destination(self) -> (r: AncestorApply<'a,'key>)
        ensures apply_phase(r, 2),
        no_unwind
    { proof { use_type_invariant(&self); } let AncestorMoved { mut inner } = self; inner.issue(); inner }
}

/// Exact bytes captured by an ancestor-deferred preparation.
pub closed spec fn prepared_original(p:AncestorPrepared)->Seq<u8>{p.original@}
/// Captured storage for the exclusive preparation.
pub closed spec fn prepared_storage<'a,'key>(p:AncestorPrepared<'a,'key>)->&'a mut [u8]{p.bytes}
/// Captured request and admitted ancestor geometry.
pub closed spec fn prepared_for(p:AncestorPrepared,original:Seq<u8>,used:u32,root:Layout,path:InlinePath,input:u32,limit:u32)->bool {
    p.bytes@==original && p.original@==original && admitted(original,p.plan)
        && planned_for(p.plan,used,root,path,input,limit)
}
/// Opaque plan retained by an immediate preparation.
pub closed spec fn prepared_plan<'a,'key>(p:AncestorPrepared<'a,'key>)->Plan<'key>{p.plan}
/// Exact required extent of an immediate preparation.
pub closed spec fn prepared_required(p:AncestorPrepared)->u32{p.plan.required}
/// Exact optional old value selected before movement.
pub closed spec fn prepared_removed(p:AncestorPrepared,span:Option<Span>)->bool{removed_exact(p.plan,span)}
/// Captured fixed-list operation.
pub closed spec fn prepared_list(p:AncestorPrepared,operation:ListEdit)->bool{planned_list_for(p.plan,operation)}
/// Captured fixed-map operation, query and encoded input width.
pub closed spec fn prepared_map(p:AncestorPrepared,operation:MapEdit,key:Seq<u8>,input:u32)->bool{fixed_map::planned_map_for(p.plan,operation,key,input)}
/// Exact request retained across a detached backend growth boundary.
pub closed spec fn detached_plan(p:AncestorPlan)->Plan{p.inner}
/// Exact required initialized extent of a detached request.
pub closed spec fn detached_required(p:AncestorPlan)->u32{p.inner.required}
/// Exact fresh request identity after detached readiness.
pub closed spec fn detached_started(p:AncestorPlan,a:AncestorApply,before:Seq<u8>)->bool {
    a.original@==before && a.phase@==0 && geometry_equal(p.inner,a.plan)
        && planned_for(a.plan,p.inner.old_used,p.inner.trail.root,p.inner.path,input_size(p.inner),p.inner.storage_limit)
}
/// Exact immediate request identity at the start of movement.
pub closed spec fn prepared_started(p:AncestorPrepared,a:AncestorApply)->bool {
    a.original@==p.original@ && a.plan==p.plan && a.phase@==0
}
/// Movement and encoding cannot change the captured request or original bytes.
pub closed spec fn same_request(before:AncestorApply,after:AncestorApply)->bool {
    before.original==after.original && before.plan==after.plan
}
/// Exact encoder slot and byte correspondence over the retained account loan.
pub closed spec fn apply_loan(before:AncestorApply,after:AncestorApply,initial:Seq<u8>,encoded:Seq<u8>)->bool {
    same_request(before,after) && initial==before.bytes@.subrange(destination_start(before.plan) as int,
        destination_start(before.plan)+input_size(before.plan))
        && after.bytes@.subrange(destination_start(before.plan) as int,destination_start(before.plan)+input_size(before.plan))==encoded
}
/// The old ancestor partition makes publication infallible.
pub closed spec fn valid_partition(a:AncestorApply)->bool {replacement_checked(a.original@,a.plan)}

/// Exact bytes at the start of movement.
pub closed spec fn apply_original(a:AncestorApply)->Seq<u8>{a.original@}
/// Exact storage retained throughout movement and publication.
pub closed spec fn apply_storage<'a,'key>(a:AncestorApply<'a,'key>)->&'a mut [u8]{a.bytes}
/// Phase zero precedes movement, one lends the encoder slot and two publishes.
pub closed spec fn apply_phase(a:AncestorApply,phase:u8)->bool{a.phase@==phase}
/// Exact encoded input width of a fixed-entry application.
pub closed spec fn apply_input_len(a:AncestorApply)->u32{input_size(a.plan)}
/// The final bytes refine the complete old checked replacement contract.
pub closed spec fn apply_finished(a:AncestorApply,after:Seq<u8>)->bool {
    replacement_checked(a.original@,a.plan) && replacement_result(a.original@,a.bytes@,after,a.plan)
        && fixed_map::key_written(a.bytes@,a.plan) && list_model_refines(a.original@,a.bytes@,after,a.plan)
        && (a.plan.map.is_some() ==> fixed_map::map_result(a.original@,a.bytes@,after,a.plan))
}

/// Whether the fixed-map removal selects no existing entry.
pub closed spec fn prepared_missing(p:AncestorPrepared)->bool {
    match p.plan.map { Some(c) => matches!(c.operation,MapEdit::Remove) && !c.found, None => false }
}

impl<'a,'key> AncestorPrepared<'a,'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self)->bool {
        self.bytes@==self.original@ && admitted(self.original@,self.plan)
            && self.plan.indexed.is_none() && self.plan.splice.is_none() && self.plan.variable_map.is_none()
    }
    /// Rechecks old ancestor diagnostics only when another pre-write check failed.
    pub fn check_before_error(&self,poisoned:&mut bool)->(result:Result<()>)
        ensures *old(poisoned) ==> *final(poisoned),
            result.is_ok() ==> *final(poisoned)==*old(poisoned),
    {
        proof{use_type_invariant(self);scan_geometry(self.original@,self.plan);}
        scan_prewrite(self.bytes,&self.plan.trail,self.plan.delta,poisoned)
    }
    /// Initialized storage required for both movement source and destination.
    pub fn prepared_len(&self)->(result:u32)
        ensures result==prepared_required(*self),
    {self.plan.required}
    /// Unchanged storage used to validate a removed value before any write.
    pub fn inspect_prepared(&self)->(result:&[u8])
        ensures result@==prepared_original(*self),
    {proof{use_type_invariant(self);}self.bytes}
    /// Optional old value coordinates, excluding immutable map keys.
    pub fn inspect_removed(&self)->(result:Option<Span>)
        ensures prepared_removed(*self,result),
    {self.plan.removed_span()}
    /// Whether a missing map removal has no bytes to publish.
    pub fn is_missing_fixed_remove(&self)->(result:bool)
        ensures result == prepared_missing(*self),
    {
        match self.plan.map{Some(c)=>matches!(c.operation,MapEdit::Remove)&&!c.found,None=>false}
    }
    /// Releases the account loan while retaining only the request for fresh readiness.
    pub fn detach(self)->(result:AncestorPlan<'key>)
        ensures detached_plan(result)==prepared_plan(self),final(prepared_storage(self))@==prepared_original(self),
    {proof{use_type_invariant(&self);}AncestorPlan{inner:self.plan}}
    /// Starts movement only after the physical window has been checked.
    pub fn begin_prepared(self)->(result:Result<AncestorReady<'a,'key>>)
        ensures result.is_err() ==> final(prepared_storage(self))@==prepared_original(self),
            result.is_ok() ==> prepared_started(self,ready_value(result.unwrap())),
        no_unwind
    {
        proof{use_type_invariant(&self);}
        if self.bytes.len()<self.plan.required as usize{return Err(Error::new(ErrorKind::InvalidPlan,Error::NO_OFFSET));}
        let bytes=self.bytes;let plan=self.plan;let original=self.original;
        Ok(AncestorReady { inner: AncestorApply {bytes,plan,original,phase:Ghost(0u8)} })
    }
}
impl<'key> AncestorPlan<'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self)->bool {
        self.inner.trail.depth<MAX_DEPTH && self.inner.indexed.is_none()
            && self.inner.splice.is_none() && self.inner.variable_map.is_none()
    }
    /// Gives a source fault priority when backend growth rejects before movement.
    pub fn check_detached_error(&self,bytes:&[u8],poisoned:&mut bool)->(result:Result<()>)
        ensures *old(poisoned) ==> *final(poisoned),
            result.is_ok() ==> *final(poisoned)==*old(poisoned),
    {
        check_detached(bytes,&self.inner,poisoned)
    }
    /// Initialized storage required before reacquiring the account.
    pub fn required_storage_len(&self)->(result:u32)
        ensures result==detached_required(*self),
    {self.inner.required}
    /// Repeats header, selection and movement admission against the current buffer.
    pub fn begin<'a>(&self,bytes:&'a mut [u8],poisoned:&mut bool)->(result:Result<AncestorReady<'a,'key>>)
        ensures result.is_err() ==> final(bytes)@==old(bytes)@,
            result.is_ok() ==> detached_started(*self,ready_value(result.unwrap()),old(bytes)@),
    {
        proof{use_type_invariant(self);}
        let p=&self.inner;
        if bytes.len()<p.required as usize || bytes.len()>p.storage_limit as usize{return Err(ErrorKind::InvalidPlan.into());}
        match envelope(bytes,p.old_used){Ok(())=>(),Err(error)=>{list_patch::source_error(error,poisoned);return Err(ErrorKind::InvalidPlan.into());}}
        let trail=match trace_local(bytes,p.old_used,&p.trail.root,&p.path){Ok(t)=>t,Err(error)=>{list_patch::source_error(error,poisoned);return Err(ErrorKind::InvalidPlan.into());}};
        let fresh=if let Some(c)=p.map {
            map_headers(bytes,p.old_used,&p.trail.root,&p.path,trail,c.operation,p.key,fixed_map::input_size_exec(p),p.storage_limit,None)
        } else if let Some(c)=p.list {
            list_headers(bytes,p.old_used,&p.path,trail,c.operation,p.replacement_len,p.storage_limit)
        } else{return Err(ErrorKind::InvalidPlan.into());};
        let fresh=match fresh{Ok(p)=>p,Err(error)=>{source_at(error,poisoned);return Err(ErrorKind::InvalidPlan.into());}};
        if !same_geometry(p,&fresh){return Err(ErrorKind::InvalidPlan.into());}
        match admit_ancestors(bytes,&fresh){Ok(())=>(),Err(error)=>{source_at(error,poisoned);return Err(ErrorKind::InvalidPlan.into());}}
        let original=Ghost(bytes@);
        Ok(AncestorReady { inner: AncestorApply {bytes,plan:fresh,original,phase:Ghost(0u8)} })
    }
}

fn source_at(error:Error,poisoned:&mut bool)
    ensures *old(poisoned) ==> *final(poisoned),
    no_unwind
{
    if let Error::Fault{offset,..}=error {if offset!=Error::NO_OFFSET{list_patch::source_error(error,poisoned);}}
}

impl<'a,'key> AncestorApply<'a,'key> {
    #[verifier::type_invariant]
    closed spec fn valid(self)->bool {
        admitted(self.original@,self.plan) && self.plan.required<=self.bytes.len()
            && self.bytes.len()==self.original@.len() && self.phase@<=3
            && self.plan.indexed.is_none() && self.plan.splice.is_none() && self.plan.variable_map.is_none()
            && (self.phase@==0 ==> self.bytes@==self.original@)
            && (self.phase@==1 || self.phase@==2 ==> payload_frame(self.original@,self.bytes@,self.plan)
                && fixed_map::key_written(self.bytes@,self.plan))
    }
    /// Arms sticky poison before moving the tail or writing a new map key.
    fn move_inner(&mut self,poisoned:&mut bool)
        requires apply_phase(*old(self),0),
        ensures apply_phase(*final(self),1),*final(poisoned),same_request(*old(self),*final(self)),
        no_unwind
    {
        proof{use_type_invariant(&*self);}
        *poisoned=true;proof { self.phase = Ghost(3u8); }
        let p=&self.plan;
        if p.delta!=0 && p.region.end<p.old_used {
            let destination=(p.region.end as i64+p.delta) as u32;
            match crate::move_bytes(self.bytes,Span{start:p.region.end,end:p.old_used},destination){Ok(())=>(),Err(_)=>panic!("admitted fixed-entry movement")}
        }
        proof{fixed_map::occupied_key(self.original@,*p);}
        fixed_map::write_key(self.bytes,p,self.original);
        proof { self.phase = Ghost(1u8); }
    }
    /// Lends only the measured input value, keeping any map key outside the loan.
    fn issue(&mut self)
        requires apply_phase(*old(self), 1),
        ensures apply_phase(*final(self), 2),
        no_unwind
    { proof { use_type_invariant(&*self); } proof { self.phase = Ghost(2u8); } }

    #[cfg_attr(all(target_os = "solana", feature = "max-depth-16"), inline(never))]
    #[cfg_attr(not(all(target_os = "solana", feature = "max-depth-16")), inline(always))]
    pub fn bytes(&mut self)->(result:&mut [u8])
        requires apply_phase(*old(self),2),
        ensures apply_phase(*final(self),2),result.len()==apply_input_len(*old(self)),
            apply_loan(*old(self),*final(self),result@,final(result)@),
        no_unwind
    {
        proof{use_type_invariant(&*self);}
        let start=if self.plan.map.is_some(){fixed_map::value_start_exec(&self.plan)}else{self.plan.region.start};
        let length=if self.plan.map.is_some(){fixed_map::input_size_exec(&self.plan)}else{self.plan.replacement_len};
        proof { self.phase = Ghost(2u8); }
        borrow_destination(self.bytes,start,length)
    }
    /// Checks retained ancestors and publishes counts, tail clearing and used length.
    pub fn finish(self,poisoned:&mut bool)->(result:Result<()>)
        requires apply_phase(self,2),*old(poisoned),
        ensures *final(poisoned)==result.is_err(),valid_partition(self) ==> result.is_ok(),
            result.is_ok() ==> apply_finished(self,final(apply_storage(self))@),
        no_unwind
    {
        proof{use_type_invariant(&self);}
        let bytes=self.bytes;let plan=self.plan;let original=self.original;
        let ghost encoded=bytes@;
        let outcome=finish_replacement(bytes,&plan,original);
        proof {if outcome.is_ok(){
            super::list_refinement(original@,encoded,bytes@,plan);
            fixed_map::refinement(original@,encoded,bytes@,plan);
        }}
        crate::patch_pass::finish_publication(poisoned,outcome)
    }
}

/// Prepares a fixed-width list splice over an existing exact resolution.
#[verifier::spinoff_prover]
pub(super) fn prepare_entries<'a>(bytes:&'a mut [u8],used:u32,path:&InlinePath,trail:Trail,operation:ListEdit,input:u32,limit:u32 )
    ->(result:Result<AncestorPrepared<'a,'static>>)
    requires trail_valid(old(bytes)@,trail.root,*path,trail,used),12<=used<=old(bytes).len()<=u32::MAX,words::le_word(old(bytes)@,8)==used,
    ensures result.is_err() ==> final(bytes)@==old(bytes)@,
        result.is_ok() ==> prepared_for(result.unwrap(),old(bytes)@,used,trail.root,*path,input,limit)
            && prepared_list(result.unwrap(),operation),
{
    if bytes.len()>limit as usize{return Err(ErrorKind::Capacity.into());}
    let element=match trail.selected.head(){LayoutHead::List{element}=>element,_=>return Err(ErrorKind::InvalidSchema.into())};
    let width=match fixed_width(&element){Some(w)=>w,None=>return Err(ErrorKind::InvalidSchema.into())};
    let plan=match entry_headers(bytes,used,&trail.root,path,trail,operation,input,width,limit){Ok(p)=>p,Err(error)=>{return Err(error);}};
    match admit_ancestors(bytes,&plan){Ok(())=>(),Err(error)=>{return Err(error);}}
    let original=Ghost(bytes@);
    Ok(AncestorPrepared{bytes,plan,original})
}

/// Prepares a fixed-value map edit while keeping its key search read-only.
#[allow(clippy::too_many_arguments)]
#[verifier::spinoff_prover]
pub(super) fn prepare_map<'a,'key>(bytes:&'a mut [u8],used:u32,path:&InlinePath,trail:Trail,operation:MapEdit,key:&'key [u8],input:u32,limit:u32,hint:Option<crate::Search> )
    ->(result:Result<AncestorPrepared<'a,'key>>)
    requires trail_valid(old(bytes)@,trail.root,*path,trail,used),12<=used<=old(bytes).len()<=u32::MAX,words::le_word(old(bytes)@,8)==used,
    ensures result.is_err() ==> final(bytes)@==old(bytes)@,
        result.is_ok() ==> prepared_for(result.unwrap(),old(bytes)@,used,trail.root,*path,input,limit)
            && prepared_map(result.unwrap(),operation,key@,input),
{
    if bytes.len()>limit as usize{return Err(ErrorKind::Capacity.into());}
    let plan=match map_headers(bytes,used,&trail.root,path,trail,operation,key,input,limit,hint){Ok(p)=>p,Err(error)=>{return Err(error);}};
    match admit_ancestors(bytes,&plan){Ok(())=>(),Err(error)=>{return Err(error);}}
    let original=Ghost(bytes@);
    Ok(AncestorPrepared{bytes,plan,original})
}

/// Every source word, encoded slot and suffix move fits admitted storage.
#[verifier::spinoff_prover]
proof fn entry_containment(before:Seq<u8>,p:Plan)
    requires admitted(before,p),p.required<=before.len(),
    ensures 12<=p.region.start<=p.region.end<=p.old_used<=p.required<=before.len()<=u32::MAX,
        p.region.start<=p.region.start+p.replacement_len<=p.new_used<=p.required,
        forall|i:int| 0<=i<p.trail.depth ==> #[trigger] run_within(p.trail.nodes[i].run,0,before.len() as int),
{
    scan_geometry(before,p);
}

/// Checked publication retains the original sequence/map edit and root validity.
#[verifier::spinoff_prover]
proof fn entry_refinement(before:Seq<u8>,encoded:Seq<u8>,after:Seq<u8>,p:Plan)
    requires replacement_checked(before,p),p.list.is_some(),
        p.map.is_none() ==> matches!(p.trail.selected.head(),LayoutHead::List{..}),
        replacement_result(before,encoded,after,p),fixed_map::key_written(encoded,p),
        validation::schema_valid(before.subrange(12,p.old_used as int),p.trail.root,MAX_DEPTH as nat,true),
        (p.map.is_none() && matches!(p.list.unwrap().operation,ListEdit::Insert(_)|ListEdit::Replace(_))) ==>
            validation::schema_valid(encoded.subrange(p.region.start as int,p.region.start+p.replacement_len),
                validation::child_layout(p.trail.selected,0),(MAX_DEPTH-p.trail.depth-1) as nat,true),
        (p.map.is_some() && fixed_map::writes(p.map.unwrap())) ==>
            validation::schema_valid(encoded.subrange(fixed_map::value_start(p) as int,fixed_map::value_start(p)+fixed_map::input_len(p)),
                validation::child_layout(p.trail.selected,0),(MAX_DEPTH-p.trail.depth-1) as nat,true),
    ensures validation::schema_valid(after.subrange(12,p.new_used as int),p.trail.root,MAX_DEPTH as nat,true),
        list_model_refines(before,encoded,after,p),
        p.map.is_some() ==> fixed_map::map_result(before,encoded,after,p),
{
    super::list_refinement(before,encoded,after,p);
    fixed_map::refinement(before,encoded,after,p);
    if p.map.is_some(){fixed_map_preserves(before,encoded,after,p);}
    else{fixed_list_preserves(before,encoded,after,p);}
}


/// The shared ancestor theorem also covers fixed-map key-preserving edits.
#[verifier::spinoff_prover]
proof fn map_entry_containment(before:Seq<u8>,p:Plan)
    requires p.map.is_some(),admitted(before,p),p.required<=before.len(),
    ensures 12<=p.region.start<=p.region.end<=p.old_used<=p.required<=before.len()<=u32::MAX,
        p.region.start<=p.region.start+p.replacement_len<=p.new_used<=p.required,
        forall|i:int| 0<=i<p.trail.depth ==> #[trigger] run_within(p.trail.nodes[i].run,0,before.len() as int),
{
    entry_containment(before,p);
}

/// The shared ancestor theorem also covers fixed-map key-preserving edits.
#[verifier::spinoff_prover]
proof fn map_entry_refinement(before:Seq<u8>,encoded:Seq<u8>,after:Seq<u8>,p:Plan)
    requires p.map.is_some(),replacement_checked(before,p),p.list.is_some(),
        p.map.is_none() ==> matches!(p.trail.selected.head(),LayoutHead::List{..}),
        replacement_result(before,encoded,after,p),fixed_map::key_written(encoded,p),
        validation::schema_valid(before.subrange(12,p.old_used as int),p.trail.root,MAX_DEPTH as nat,true),
        (p.map.is_none() && matches!(p.list.unwrap().operation,ListEdit::Insert(_)|ListEdit::Replace(_))) ==>
            validation::schema_valid(encoded.subrange(p.region.start as int,p.region.start+p.replacement_len),
                validation::child_layout(p.trail.selected,0),(MAX_DEPTH-p.trail.depth-1) as nat,true),
        (p.map.is_some() && fixed_map::writes(p.map.unwrap())) ==>
            validation::schema_valid(encoded.subrange(fixed_map::value_start(p) as int,fixed_map::value_start(p)+fixed_map::input_len(p)),
                validation::child_layout(p.trail.selected,0),(MAX_DEPTH-p.trail.depth-1) as nat,true),
    ensures validation::schema_valid(after.subrange(12,p.new_used as int),p.trail.root,MAX_DEPTH as nat,true),
        list_model_refines(before,encoded,after,p),
        p.map.is_some() ==> fixed_map::map_result(before,encoded,after,p),
{
    entry_refinement(before,encoded,after,p);
}
/// Nested nonempty fixed-list and map edits witness the shared ancestor contract.
#[verifier::spinoff_prover]
proof fn list_entry_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,6);
    reveal_with_fuel(crate::layout::descriptor_fields,6);
    reveal_with_fuel(validation::schema_valid,6);
    reveal_with_fuel(validation::children_valid,7);
    reveal_with_fuel(validation::child_valid,6);
    let root=Layout::List{element:&witnesses::FIXED_LIST};
    let before=witnesses::account(seq![2u8,0,0,0,5,0,0,0,1,0,0,0,7,1,0,0,0,9]);
    let after=witnesses::account(seq![2u8,0,0,0,6,0,0,0,2,0,0,0,8,7,1,0,0,0,9]);
    let path=InlinePath{steps:[Step::Index(0);MAX_DEPTH],len:1,overflow:false};
    let selection=Selection{payload_len:10,span:Span{start:8,end:13},layout:witnesses::FIXED_LIST,ordinal:0,run:PatchRun{offset:4,stride:4,count:1}};
    assert(validation::local_valid(before.subrange(12,30),root,true));
    assert(validation::schema_valid(before.subrange(20,25),selection.layout,(MAX_DEPTH-1) as nat,true));
    assert(validation::schema_valid(before.subrange(25,30),selection.layout,(MAX_DEPTH-1) as nat,true));
    assert(validation::child_valid(before.subrange(12,30),root,0,(MAX_DEPTH-1) as nat,true));
    assert(validation::child_valid(before.subrange(12,30),root,1,(MAX_DEPTH-1) as nat,true));
    assert(validation::schema_valid(before.subrange(12,30),root,MAX_DEPTH as nat,true));
    admission::direct_checked(before,root,path,30,selection);
    let trail=admission::direct_trail(before,root,path,30,selection);
    admission::entries_checked(before,path,trail,ListEdit::Insert(0),1,1,64);
    let p=admission::entries_plan_for(before,path,trail,ListEdit::Insert(0),1,1,64);
    let encoded=after.update(8,30u8).update(16,5u8).update(20,1u8);
    assert(admitted(before,p));
    assert(patch_delta(before,after,p.trail.nodes[0].run,1,1));
    assert forall|at:int| 16<=at<20 implies ancestor_changed(p,1,at) by {
        assert(run_at(p.trail.nodes[0].run,0)==16);assert(run_changed(p.trail.nodes[0].run,1,at));
    }
    assert(replacement_result(before,encoded,after,p));
    entry_containment(before,p);entry_refinement(before,encoded,after,p);
}
/// A nested nonempty map witnesses key-preserving checked ancestor publication.
#[verifier::spinoff_prover]
#[verifier::rlimit(120)]
proof fn map_entry_witness()
{
    broadcast use vstd::array::group_array_axioms;
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::layout::descriptor_valid,6);
    reveal_with_fuel(crate::layout::descriptor_fields,6);
    reveal_with_fuel(validation::schema_valid,6);
    reveal_with_fuel(validation::children_valid,7);
    reveal_with_fuel(validation::child_valid,6);
    let root=Layout::List{element:&witnesses::FIXED_MAP};
    let before=witnesses::account(seq![2u8,0,0,0,6,0,0,0,1,0,0,0,1,7,1,0,0,0,3,9]);
    let after=witnesses::account(seq![2u8,0,0,0,8,0,0,0,2,0,0,0,1,7,2,8,1,0,0,0,3,9]);
    let path=InlinePath{steps:[Step::Index(0);MAX_DEPTH],len:1,overflow:false};
    let selection=Selection{payload_len:12,span:Span{start:8,end:14},layout:witnesses::FIXED_MAP,ordinal:0,run:PatchRun{offset:4,stride:4,count:1}};
    assert(validation::local_valid(before.subrange(12,32),root,true));
    assert(validation::schema_valid(before.subrange(20,26),selection.layout,(MAX_DEPTH-1) as nat,true));
    assert(validation::schema_valid(before.subrange(26,32),selection.layout,(MAX_DEPTH-1) as nat,true));
    assert(validation::child_valid(before.subrange(12,32),root,0,(MAX_DEPTH-1) as nat,true));
    assert(validation::child_valid(before.subrange(12,32),root,1,(MAX_DEPTH-1) as nat,true));
    assert(validation::schema_valid(before.subrange(12,32),root,MAX_DEPTH as nat,true));
    admission::direct_checked(before,root,path,32,selection);
    let trail=admission::direct_trail(before,root,path,32,selection);
    admission::entries_checked(before,path,trail,ListEdit::Insert(1),2,2,64);
    let base=admission::entries_plan_for(before,path,trail,ListEdit::Insert(1),2,2,64);
    let p=Plan{key:vstd::array::spec_array_as_slice(&[2u8]),map:Some(MapChange{operation:MapEdit::Insert,key_width:1,value_width:1,index:1,found:false}),..base};
    assert(crate::search::same_prefix(seq![1u8],seq![2u8],0));
    assert(crate::search::lex_less(seq![1u8],seq![2u8]));
    assert(p.key@ =~=seq![2u8]);
    assert(crate::search::search_found(crate::Search{index:1,found:false},before.subrange(20,26),witnesses::FIXED_MAP,p.key@));
    assert(crate::search::encoded_key(before.subrange(20,26),witnesses::FIXED_MAP,0) =~=seq![1u8]);
    assert(crate::search::search_partition(crate::Search{index:1,found:false},before.subrange(20,26),witnesses::FIXED_MAP,p.key@));
    assert(replacement_checked(before,p));assert(admitted(before,p));
    let encoded=after.update(8,32u8).update(16,6u8).update(20,1u8);
    assert(p.region.start==26);
    assert(encoded.subrange(26,27) =~=p.key@);
    assert(fixed_map::key_written(encoded,p));
    assert(patch_delta(before,after,p.trail.nodes[0].run,2,1));
    assert forall|at:int| 16<=at<20 implies ancestor_changed(p,1,at) by {
        assert(run_at(p.trail.nodes[0].run,0)==16);assert(run_changed(p.trail.nodes[0].run,1,at));
    }
    assert(replacement_result(before,encoded,after,p));
    map_entry_containment(before,p);map_entry_refinement(before,encoded,after,p);
}

}
