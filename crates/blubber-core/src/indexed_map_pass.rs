//! Fallible map directory transport over header-admitted bytes.
#![allow(clippy::question_mark)]
use super::*;
verus! {

/// Actual movement outcome, independent of the runtime admission of old ends.
pub(in crate::replace_plan) open spec fn pass_result(before: Seq<u8>, after: Seq<u8>, plan: Plan, result: Result<()>) -> bool {
    &&& after.len() == before.len()
    &&& forall|p: int| 0 <= p < plan.region.start + 4 ==> #[trigger] after[p] == before[p]
    &&& result.is_ok() ==> movement(before, after, plan)
    &&& parameters(before, plan.region, fixed_map::node(plan).layout, plan.variable_map.unwrap(), plan.key@) ==> result.is_ok()
}

#[verifier::spinoff_prover]
#[inline(always)]
pub(in crate::replace_plan) fn move_checked(bytes: &mut [u8], plan: &Plan, poisoned: &mut bool) -> (result: Result<()>)
    requires bounded(old(bytes)@, *plan), plan.variable_map.is_some(), plan.required <= old(bytes).len(),
    ensures pass_result(old(bytes)@, final(bytes)@, *plan, result), *final(poisoned),
    no_unwind
{
       hide(map_layout::map_shape);

    *poisoned = true;

    let ghost original = bytes@;
    let c = match plan.variable_map { Some(c) => c, None => panic!("checked edit plan") };
    let q = (c.payload as i64 + c.table_delta) as u32;
    let split = c.payload + c.offset;
    let end = c.payload + c.end_offset;
    if matches!(c.change.operation, MapEdit::Insert) && !c.change.found {
        // Preserve B and the outside suffix, then A, before enlarging D.
        if split < plan.old_used {
            match crate::move_bytes(bytes, Span { start: split, end: plan.old_used }, (split as i64 + plan.delta) as u32) { Ok(value) => value, Err(error) => return Err(error) };
        }
        if c.offset != 0 {
            match crate::move_bytes(bytes, Span { start: c.payload, end: split }, q) { Ok(value) => value, Err(error) => return Err(error) };
        }
        let ghost payload = bytes@;
        proof { assert(payload_preserved(original, payload, *plan)); assert(payload_frame(original, payload, *plan)); }
        // Directory shifting depends only on its old source bytes. A ghost
        // reframe supplies the unchanged metadata without validating moved data.
        shift_ready_directory(bytes, plan, Ghost(original));
        match fill_checked(bytes, plan, poisoned, Ghost(original)) { Ok(()) => (), Err(error) => return Err(error) };
        proof {
            assert(payload_preserved(original, bytes@, *plan));
            assert(payload_frame(original, bytes@, *plan));
        }
    } else {
        // Consume old trailing key/end records before A can overwrite them.
        shift_directory(bytes, plan);
        match fill_checked(bytes, plan, poisoned, Ghost(original)) { Ok(()) => (), Err(error) => return Err(error) };
        let ghost directory = bytes@;
        proof {
            assert forall|p: int| q <= p < original.len() implies #[trigger] directory[p] == original[p] by { }
        }
        if c.table_delta != 0 && c.offset != 0 {
            match crate::move_bytes(bytes, Span { start: c.payload, end: split }, q) { Ok(value) => value, Err(error) => return Err(error) };
        }
        let ghost prefix = bytes@;
        if plan.delta != 0 && end < plan.old_used {
            match crate::move_bytes(bytes, Span { start: end, end: plan.old_used }, q + c.offset + c.input_len) { Ok(value) => value, Err(error) => return Err(error) };
        }
        proof {
            directory_reframe(original, directory, bytes@, *plan, c.new_count as int);
            assert(q + c.offset + c.input_len - plan.delta == end);
            assert forall|p: int| q <= p < q + c.offset implies #[trigger] bytes@[p] == original[p - c.table_delta] by {
                assert(directory[p - c.table_delta] == original[p - c.table_delta]);
                if c.table_delta != 0 && c.offset != 0 { assert(prefix[p] == directory[p - c.table_delta]); }
                else { assert(prefix[p] == directory[p]); }
                assert(bytes@[p] == prefix[p]);
            }
            assert forall|p: int| q + c.offset + c.input_len <= p < plan.new_used implies #[trigger] bytes@[p] == original[p - plan.delta] by {
                assert(end <= p - plan.delta < plan.old_used);
                assert(prefix[p - plan.delta] == directory[p - plan.delta]);
                assert(directory[p - plan.delta] == original[p - plan.delta]);
            }
            assert(payload_preserved(original, bytes@, *plan));
        }
    }
    proof {
        assert forall|p: int| 0 <= p < original.len() && (p < plan.region.start || plan.required <= p)
            implies bytes@[p] == original[p] by { }
        assert(payload_preserved(original, bytes@, *plan));
        assert(payload_frame(original, bytes@, *plan));
        assert(words::le_word(bytes@, plan.region.start as int) == c.count);
        assert(directory_done(original, bytes@, *plan, c.new_count as int));
    }
    Ok(())
}

/// Checks each consumed old end at the destination where it is published.
#[verifier::spinoff_prover]
#[allow(clippy::question_mark)]
fn fill_checked(bytes: &mut [u8], plan: &Plan, poisoned: &mut bool, Ghost(original): Ghost<Seq<u8>>)
    -> (result: Result<()>)
    requires bounded(original, *plan), plan.required <= old(bytes).len(),
        old(bytes).len() == original.len(), directory_ready(original, old(bytes)@, *plan),
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        directory_frame(old(bytes)@, final(bytes)@, *plan),
        result.is_ok() ==> directory_done(original, final(bytes)@, *plan, plan.variable_map.unwrap().new_count as int),
        parameters(original, plan.region, fixed_map::node(*plan).layout, plan.variable_map.unwrap(), plan.key@) ==> result.is_ok(),
    no_unwind
{
    hide(words::le_word);
     hide(map_layout::map_shape);
    *poisoned = true;
    let c = match plan.variable_map { Some(c) => c, None => panic!("checked edit plan") };
    let ghost before = bytes@;
    let mut i = c.change.index;
    proof {
        header_suffix_initialized(original, bytes@, *plan);
        assert(i <= c.new_count);
        table_index(plan.region.start + 4, c.stride, c.new_count, i as int);
    }
    if i < c.new_count && matches!(c.change.operation, MapEdit::Insert | MapEdit::Replace) {
        let at = (plan.region.start as u64 + 4 + i as u64 * c.stride as u64) as u32;
        proof {
            table_index(plan.region.start + 4, c.stride, c.new_count, i as int);
            assert(at + c.stride <= c.payload + c.table_delta) by(nonlinear_arith)
                requires at == plan.region.start + 4 + i * c.stride, i < c.new_count,
                    c.payload + c.table_delta == plan.region.start + 4 + c.new_count * c.stride;
        }
        let ghost iteration = bytes@;
        if matches!(c.change.operation, MapEdit::Insert) && !c.change.found {
            write_query(bytes, at, plan.key);
        }
        let end = match crate::patch_pass::checked_end(c.offset, 0, plan.region.end - c.payload, c.input_len as i64, at + c.change.key_width) { Ok(value) => value, Err(error) => return Err(error) };
        proof { assert(end as int == new_end(original, *plan, i as int)); }
        crate::words::write_contained_word(bytes, (at + c.change.key_width) as usize, end);
        proof {
            directory_reframe(original, iteration, bytes@, *plan, i as int);
            assert forall|j: int| #![trigger bytes@.subrange(plan.region.start + 4 + j * c.stride, plan.region.start + 4 + j * c.stride + c.change.key_width)]
                #![trigger words::le_word(bytes@, plan.region.start + 4 + j * c.stride + c.change.key_width)]
                0 <= j < i + 1 implies {
                let a = plan.region.start + 4 + j * c.stride;
                &&& bytes@.subrange(a, a + c.change.key_width) == new_key(original, *plan, j)
                &&& words::le_word(bytes@, a + c.change.key_width) == new_end(original, *plan, j)
            } by {
                let a = plan.region.start + 4 + j * c.stride;
                if j < i {
                    assert(directory_done(original, bytes@, *plan, i as int));
                } else {
                    if !inserts(c) || j != c.change.index {
                        let s = plan.region.start + 4 + source(c, j) * c.stride;
                        assert(iteration.subrange(a, a + c.stride) == original.subrange(s, s + c.stride));
                        record_prefix(original, s, iteration, a, c.stride as int, c.change.key_width as int);
                        assert(bytes@.subrange(a, a + c.change.key_width) =~= new_key(original, *plan, j));
                    }
                }
            }
        }
        proof {
            assert forall|j: int| #![trigger bytes@.subrange(plan.region.start + 4 + j*c.stride, plan.region.start + 4 + j*c.stride + c.stride)]
                i + 1 <= j < c.new_count && !(inserts(c) && j == c.change.index) implies {
                let a = plan.region.start + 4 + j*c.stride;
                let s = plan.region.start + 4 + source(c,j)*c.stride;
                bytes@.subrange(a,a+c.stride) == original.subrange(s,s+c.stride)
            } by {
                let a = plan.region.start + 4 + j*c.stride;
                let source_at = plan.region.start + 4 + source(c,j)*c.stride;
                table_index(plan.region.start + 4,c.stride,c.new_count,j);
                assert(at+c.stride <= a) by(nonlinear_arith)
                    requires at == plan.region.start + 4 + i*c.stride, a == plan.region.start + 4 + j*c.stride,
                        i < j, c.stride > 0;
                assert(bytes@.subrange(a,a+c.stride) =~= iteration.subrange(a,a+c.stride));
                assert(iteration.subrange(a,a+c.stride) == original.subrange(source_at,source_at+c.stride));
            }
        }
        i += 1;
    }
    proof {
        assert(directory_done(original, bytes@, *plan, i as int));
        assert(directory_frame(before, bytes@, *plan));
        assert forall|j: int| #![trigger bytes@.subrange(plan.region.start + 4 + j * c.stride, plan.region.start + 4 + j * c.stride + c.stride)]
            i <= j < c.new_count ==> {
                let a = plan.region.start + 4 + j * c.stride;
                let source_at = plan.region.start + 4 + source(c, j) * c.stride;
                bytes@.subrange(a, a + c.stride) == original.subrange(source_at, source_at + c.stride)
            } by { }
    }
    patch_suffix_checked(bytes, plan, i, poisoned, Ghost(original))
}

#[verifier::spinoff_prover]
#[verifier::rlimit(160)]
#[inline(always)]
fn patch_suffix_checked(bytes: &mut [u8], plan: &Plan, first: u32, poisoned: &mut bool, Ghost(original): Ghost<Seq<u8>>) -> (result: Result<()>)
    requires bounded(original, *plan), plan.variable_map.is_some(),
        old(bytes).len() == original.len(), plan.required <= old(bytes).len(),
        plan.variable_map.unwrap().change.index <= first <= plan.variable_map.unwrap().new_count,
        first < plan.variable_map.unwrap().new_count ==> !(writes(plan.variable_map.unwrap()) && first == plan.variable_map.unwrap().change.index),
        directory_done(original, old(bytes)@, *plan, first as int),
        forall|j: int| #![trigger old(bytes)@.subrange(plan.region.start + 4 + j * plan.variable_map.unwrap().stride,
            plan.region.start + 4 + j * plan.variable_map.unwrap().stride + plan.variable_map.unwrap().stride)]
            first <= j < plan.variable_map.unwrap().new_count ==> {
                let c = plan.variable_map.unwrap();
                let a = plan.region.start + 4 + j * c.stride;
                let source_at = plan.region.start + 4 + source(c, j) * c.stride;
                old(bytes)@.subrange(a, a + c.stride) == original.subrange(source_at, source_at + c.stride)
            },
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        directory_frame(old(bytes)@, final(bytes)@, *plan),
        result.is_ok() ==> directory_done(original, final(bytes)@, *plan, plan.variable_map.unwrap().new_count as int),
        parameters(original, plan.region, fixed_map::node(*plan).layout, plan.variable_map.unwrap(), plan.key@) ==> result.is_ok(),
    no_unwind
{
    hide(words::le_word); hide(search::search_found); hide(search::search_partition);
    *poisoned = true;
    let c = match plan.variable_map { Some(c) => c, None => panic!("checked edit plan") };
    let i = first;
    let ghost before = bytes@;
    if i < c.new_count {
        proof { table_index(plan.region.start + 4, c.stride, c.new_count, i as int); }
        let at = (plan.region.start as u64 + 4 + i as u64 * c.stride as u64) as u32;
        let table_end = (c.payload as i64 + c.table_delta) as u32;
        let length = table_end - at;
        proof {
            assert(length == (c.new_count - i) * c.stride) by(nonlinear_arith)
                requires length == table_end - at, table_end == plan.region.start + 4 + c.new_count * c.stride,
                    at == plan.region.start + 4 + i * c.stride;
        }
        let delta = c.input_len as i64 - (c.end_offset - c.offset) as i64;
        let ghost prior = bytes@;
        let ghost patched;
        let transported;
        {
            let suffix = borrow_destination(bytes, at, length);
            proof {
                if parameters(original, plan.region, fixed_map::node(*plan).layout, c, plan.key@) {
                    assert(crate::patch_pass::directory_safe(suffix@, c.change.key_width as usize, c.stride as usize,
                        (c.new_count - i) as usize, c.end_offset, (plan.region.end - c.payload) as u32, delta)) by {
                        assert forall|k: int| 0 <= k < c.new_count - i implies {
                            let end = #[trigger] words::le_word(suffix@, k * c.stride + c.change.key_width);
                            &&& c.end_offset <= end <= plan.region.end - c.payload
                            &&& k > 0 ==> words::le_word(suffix@, (k - 1) * c.stride + c.change.key_width) <= end
                            &&& 0 <= end + delta <= u32::MAX
                        } by {
                            let j = i + k;
                            let a = plan.region.start + 4 + j * c.stride;
                            let source_at = plan.region.start + 4 + source(c, j) * c.stride;
                            table_index(plan.region.start + 4, c.stride, c.new_count, j);
                            table_index(plan.region.start + 4, c.stride, c.count, source(c, j));
                            assert(prior.subrange(a,a+c.stride) == original.subrange(source_at,source_at+c.stride));
                            record_word(original, source_at, prior, a, c.stride as int, c.change.key_width as int);
                            assert(a == at + k*c.stride) by(nonlinear_arith)
                                requires a == plan.region.start + 4 + j*c.stride,
                                    at == plan.region.start + 4 + i*c.stride, k == j-i;
                            word_in_slice(prior, at as int, at + length, k*c.stride+c.change.key_width);
                            assert(words::le_word(suffix@,k*c.stride+c.change.key_width) == boundary(original,plan.region.start as int,c,source(c,j)+1));
                            assert(c.change.index + (if c.change.found {1int} else {0int}) <= source(c,j)+1 <= c.count);
                            assert(c.end_offset == boundary(original,plan.region.start as int,c,c.change.index+(if c.change.found {1int} else {0int})));
                            assert(c.end_offset <= words::le_word(suffix@,k*c.stride+c.change.key_width) <= plan.region.end-c.payload);
                            assert(plan.region.end-c.payload+delta == plan.region.end+plan.delta-(c.payload+c.table_delta));
                            assert(0 <= words::le_word(suffix@,k*c.stride+c.change.key_width)+delta <= u32::MAX);
                            if k > 0 {
                                let previous_a = a - c.stride;
                                let previous_source = plan.region.start + 4 + source(c,j-1)*c.stride;
                                table_index(plan.region.start+4,c.stride,c.count,source(c,j-1));
                                table_index(plan.region.start+4,c.stride,c.new_count,j-1);
                                assert(previous_a == at+(k-1)*c.stride) by(nonlinear_arith)
                                    requires previous_a == a-c.stride, a == at+k*c.stride;
                                assert(previous_a == plan.region.start+4+(j-1)*c.stride) by(nonlinear_arith)
                                    requires previous_a == a-c.stride, a == plan.region.start+4+j*c.stride;
                                assert(prior.subrange(previous_a,previous_a+c.stride) == original.subrange(previous_source,previous_source+c.stride));
                                assert(0 <= (k-1)*c.stride+c.change.key_width && (k-1)*c.stride+c.change.key_width+4 <= length) by(nonlinear_arith)
                                    requires 0<k<c.new_count-i, length==(c.new_count-i)*c.stride, c.stride as int==c.change.key_width+4, c.change.key_width>=0;
                                record_word(original,previous_source,prior,previous_a,c.stride as int,c.change.key_width as int);
                                word_in_slice(prior,at as int,at+length,(k-1)*c.stride+c.change.key_width);
                                assert(words::le_word(suffix@,(k-1)*c.stride+c.change.key_width) == boundary(original,plan.region.start as int,c,source(c,j-1)+1));
                                assert(0 <= source(c,j-1)+1 <= source(c,j)+1 <= c.count);
                            }
                        }
                    }
                }
            }
            transported = if c.change.key_width == 32 {
                crate::patch_pass::transport_directory::<32, 36>(suffix, c.change.key_width as usize,
                c.stride as usize, (c.new_count - i) as usize, c.end_offset,
                plan.region.end - c.payload, delta, at, poisoned)
            } else {
                crate::patch_pass::transport_directory::<0, 0>(suffix, c.change.key_width as usize,
                c.stride as usize, (c.new_count - i) as usize, c.end_offset,
                plan.region.end - c.payload, delta, at, poisoned)
            };
            proof { patched = suffix@; }
        }
        if let Err(error) = transported { return Err(error); }
        proof {
            directory_reframe(original, prior, bytes@, *plan, i as int);
            assert forall|j: int| #![trigger bytes@.subrange(plan.region.start + 4 + j * c.stride, plan.region.start + 4 + j * c.stride + c.change.key_width)]
                #![trigger words::le_word(bytes@, plan.region.start + 4 + j * c.stride + c.change.key_width)]
                0 <= j < c.new_count implies {
                    let a = plan.region.start + 4 + j * c.stride;
                    &&& bytes@.subrange(a, a + c.change.key_width) == new_key(original, *plan, j)
                    &&& words::le_word(bytes@, a + c.change.key_width) == new_end(original, *plan, j)
                } by {
                    if j < i {
                        assert(directory_done(original, bytes@, *plan, i as int));
                    } else {
                        let a = plan.region.start + 4 + j * c.stride;
                        let source_at = plan.region.start + 4 + source(c, j) * c.stride;
                        table_index(plan.region.start + 4, c.stride, c.new_count, j);
                        table_index(plan.region.start + 4, c.stride, c.count, source(c, j));
                        let k = j - i;
                        assert(a == at + k * c.stride) by(nonlinear_arith)
                        requires a == plan.region.start + 4 + j * c.stride,
                            at == plan.region.start + 4 + i * c.stride, k == j - i;
                        word_in_slice(prior, at as int, at + length, k * c.stride + c.change.key_width);
                        word_in_slice(bytes@, at as int, at + length, k * c.stride + c.change.key_width);
                        record_word(original, source_at, prior, a, c.stride as int, c.change.key_width as int);
                        record_prefix(original, source_at, prior, a, c.stride as int, c.change.key_width as int);
                        assert forall|p: int| 0 <= p < c.change.key_width implies #[trigger] bytes@[a + p] == prior[a + p] by {
                            assert(patched[k * c.stride + p] == prior.subrange(at as int, at + length)[k * c.stride + p]);
                        }
                        assert forall|p: int| 0 <= p < c.change.key_width implies
                            #[trigger] bytes@.subrange(a, a + c.change.key_width)[p]
                                == prior.subrange(a, a + c.change.key_width)[p] by {
                            assert(bytes@[a + p] == prior[a + p]);
                        }
                        assert(bytes@.subrange(a, a + c.change.key_width) =~= prior.subrange(a, a + c.change.key_width));
                    }
                }
            assert(directory_done(original, bytes@, *plan, c.new_count as int));
            assert forall|p: int| 0 <= p < bytes.len() && !(plan.region.start + 4 <= p < c.payload + c.table_delta)
                implies #[trigger] bytes@[p] == before[p] by {
                assert(!(at <= p < at + length));
                assert(prior[p] == before[p]);
            }
            assert(directory_frame(before, bytes@, *plan));
        }
    }    Ok(())
}

/// Transfers the strict map specification across shared used-last publication.
#[verifier::spinoff_prover]
pub(in crate::replace_plan) proof fn publication_refinement(original: Seq<u8>, encoded: Seq<u8>, counted: Seq<u8>, after: Seq<u8>, plan: Plan)
    requires replacement_checked(original, plan), plan.variable_map.is_some(),
        movement(original, encoded, plan), counted.len() == encoded.len(),
        words::le_word(counted, plan.region.start as int) == plan.variable_map.unwrap().new_count,
        forall|i: int| 0 <= i < counted.len() && !(plan.region.start <= i < plan.region.start + 4) ==> counted[i] == encoded[i],
        replacement_result(original, counted, after, Plan { variable_map: None, ..plan }),
    ensures edit_result(original, encoded, after, plan), map_result(original, encoded, after, plan),
{
    hide(words::le_word);
    broadcast use vstd::array::group_array_axioms;
    checked_geometry(original, plan);
    reveal(replacement_result); reveal(target); reveal(count_changed); reveal(ancestor_changed); reveal(ancestors_delta);
    let c = plan.variable_map.unwrap();
    directory_reframe(original, encoded, after, plan, c.new_count as int);
    word_equal(counted, plan.region.start as int, after, plan.region.start as int);
    assert(payload_preserved(original, after, plan));
    assert(after.subrange(destination(plan) as int, destination(plan) + c.input_len)
        =~= encoded.subrange(destination(plan) as int, destination(plan) + c.input_len));
    let base = Plan { variable_map: None, ..plan };
    assert(ancestors_delta(original, after, base, base.trail.depth as int));
    assert forall|j: int| 0 <= j < plan.trail.depth implies #[trigger] patch_delta(original, after,
        plan.trail.nodes[j].run, plan.delta, plan.trail.nodes[j].run.count as int) by {
        assert(plan.trail.nodes[j].run == base.trail.nodes[j].run);
        assert(patch_delta(original, after, base.trail.nodes[j].run, base.delta, base.trail.nodes[j].run.count as int));
    }
    assert(ancestors_delta(original, after, plan, plan.trail.depth as int));
    assert forall|p: int| 0 <= p < after.len() && (p < 8 || plan.required <= p
        || (12 <= p < plan.region.start && (plan.delta == 0 || !ancestor_changed(plan, plan.trail.depth as int, p))))
        implies #[trigger] after[p] == original[p] by { }
    assert(edit_result(original, encoded, after, plan));
    refinement(original, encoded, after, plan);
}

spec fn slot_fits(plan: Plan, i: int) -> bool {
    let c = plan.variable_map.unwrap();
    &&& plan.region.start + 4 <= plan.region.start + 4 + i * c.stride
    &&& plan.region.start + 4 + i * c.stride + c.change.key_width + 4 <= c.payload + c.table_delta <= plan.required
    &&& !(inserts(c) && i == c.change.index) ==> 0 <= source(c, i) < c.count
}

proof fn directory_containment(bytes: Seq<u8>, plan: Plan)
    requires bounded(bytes, plan), plan.required <= bytes.len(),
    ensures plan.region.start + 4 <= plan.variable_map.unwrap().payload <= plan.old_used <= bytes.len() <= u32::MAX,
        0 <= destination(plan) <= destination(plan) + plan.variable_map.unwrap().input_len <= plan.new_used <= bytes.len(),
        forall|i: int| 0 <= i < plan.variable_map.unwrap().new_count ==> #[trigger] slot_fits(plan, i),
{
    hide(words::le_word); hide(search::search_found); hide(search::search_partition); hide(map_layout::header_shape);
    let c = plan.variable_map.unwrap();
    assert forall|i: int| 0 <= i < c.new_count implies #[trigger] slot_fits(plan, i) by {
        table_index(plan.region.start + 4, c.stride, c.new_count, i);
    }
}

/// Arbitrary admitted bytes retain finite, contained insert movement coordinates.
proof fn insert_containment(bytes: Seq<u8>, plan: Plan)
    requires bounded(bytes, plan), plan.required <= bytes.len(),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Insert),
    ensures 12 <= plan.region.start < plan.variable_map.unwrap().payload <= plan.old_used <= plan.required <= bytes.len() <= u32::MAX,
        0 <= destination(plan) <= destination(plan) + plan.variable_map.unwrap().input_len <= plan.new_used,
        forall|i: int| 0 <= i < plan.variable_map.unwrap().new_count ==>
            #[trigger] slot_fits(plan, i),
{
    hide(words::le_word); hide(search::search_found); hide(search::search_partition); hide(map_layout::header_shape);
    directory_containment(bytes, plan);
}

/// A ghost valid-partition premise makes the checked insert pass exact and successful.
proof fn insert_refinement(before: Seq<u8>, after: Seq<u8>, plan: Plan, result: Result<()>)
    requires bounded(before, plan), pass_result(before, after, plan, result),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Insert),
        parameters(before, plan.region, fixed_map::node(plan).layout, plan.variable_map.unwrap(), plan.key@),
    ensures result.is_ok(), movement(before, after, plan),
{
    hide(bounded); hide(parameters); hide(movement); hide(pass_result);
    reveal(pass_result);
}

/// Nonempty admission and a concrete successful outcome witness the insert route.
#[verifier::spinoff_prover]
proof fn insert_witness()
{
    reveal(crate::layout_head::head_spec);
    broadcast use vstd::array::group_array_axioms;
    let before = witnesses::account(seq![1u8,0,0,0,2,1,0,0,0,97]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Insert, key_width: 1, value_width: 1, index: 0, found: true },
        count: 1, new_count: 1, stride: 5, payload: 21, offset: 0, end_offset: 1,
        input_len: 1, table_delta: 0 };
    let plan = Plan { key: vstd::array::spec_array_as_slice(&[2u8]), variable_map: Some(c),
        ..witnesses::replacement(before, witnesses::INDEXED_MAP, 10) };
    assert(plan.key@ =~= seq![2u8]);
    let local = before.subrange(plan.region.start as int, plan.region.end as int);
    assert(crate::search::encoded_key(local, witnesses::INDEXED_MAP, 0) =~= seq![2u8]);
    assert(crate::search::search_found(crate::Search { index: 0, found: true }, local, witnesses::INDEXED_MAP, plan.key@));
    assert(crate::search::search_partition(crate::Search { index: 0, found: true }, local, witnesses::INDEXED_MAP, plan.key@));
    assert(parameters(before, plan.region, witnesses::INDEXED_MAP, c, plan.key@));
    assert(bounded(before, plan));
    assert(movement(before, before, plan));
    insert_containment(before, plan);
    insert_refinement(before, before, plan, Ok(()));
}

/// Arbitrary admitted bytes retain finite, contained replace movement coordinates.
proof fn replace_containment(bytes: Seq<u8>, plan: Plan)
    requires bounded(bytes, plan), plan.required <= bytes.len(),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Replace),
    ensures 12 <= plan.region.start < plan.variable_map.unwrap().payload <= plan.old_used <= plan.required <= bytes.len() <= u32::MAX,
        0 <= destination(plan) <= destination(plan) + plan.variable_map.unwrap().input_len <= plan.new_used,
        forall|i: int| 0 <= i < plan.variable_map.unwrap().new_count ==>
            #[trigger] slot_fits(plan, i),
{
    hide(words::le_word); hide(search::search_found); hide(search::search_partition); hide(map_layout::header_shape);
    directory_containment(bytes, plan);
}

/// A ghost valid-partition premise makes the checked replace pass exact and successful.
proof fn replace_refinement(before: Seq<u8>, after: Seq<u8>, plan: Plan, result: Result<()>)
    requires bounded(before, plan), pass_result(before, after, plan, result),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Replace),
        parameters(before, plan.region, fixed_map::node(plan).layout, plan.variable_map.unwrap(), plan.key@),
    ensures result.is_ok(), movement(before, after, plan),
{
    hide(bounded); hide(parameters); hide(movement); hide(pass_result);
    reveal(pass_result);
}

/// Nonempty admission and a concrete successful outcome witness the replace route.
#[verifier::spinoff_prover]
proof fn replace_witness()
{
    reveal(crate::layout_head::head_spec);
    broadcast use vstd::array::group_array_axioms;
    let before = witnesses::account(seq![1u8,0,0,0,2,1,0,0,0,97]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Replace, key_width: 1, value_width: 1, index: 0, found: true },
        count: 1, new_count: 1, stride: 5, payload: 21, offset: 0, end_offset: 1,
        input_len: 1, table_delta: 0 };
    let plan = Plan { key: vstd::array::spec_array_as_slice(&[2u8]), variable_map: Some(c),
        ..witnesses::replacement(before, witnesses::INDEXED_MAP, 10) };
    assert(plan.key@ =~= seq![2u8]);
    let local = before.subrange(plan.region.start as int, plan.region.end as int);
    assert(crate::search::encoded_key(local, witnesses::INDEXED_MAP, 0) =~= seq![2u8]);
    assert(crate::search::search_found(crate::Search { index: 0, found: true }, local, witnesses::INDEXED_MAP, plan.key@));
    assert(crate::search::search_partition(crate::Search { index: 0, found: true }, local, witnesses::INDEXED_MAP, plan.key@));
    assert(parameters(before, plan.region, witnesses::INDEXED_MAP, c, plan.key@));
    assert(bounded(before, plan));
    assert(movement(before, before, plan));
    replace_containment(before, plan);
    replace_refinement(before, before, plan, Ok(()));
}

/// Arbitrary admitted bytes retain finite, contained remove movement coordinates.
proof fn remove_containment(bytes: Seq<u8>, plan: Plan)
    requires bounded(bytes, plan), plan.required <= bytes.len(),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Remove),
    ensures 12 <= plan.region.start < plan.variable_map.unwrap().payload <= plan.old_used <= plan.required <= bytes.len() <= u32::MAX,
        0 <= destination(plan) <= destination(plan) + plan.variable_map.unwrap().input_len <= plan.new_used,
        forall|i: int| 0 <= i < plan.variable_map.unwrap().new_count ==>
            #[trigger] slot_fits(plan, i),
{
    hide(words::le_word); hide(search::search_found); hide(search::search_partition); hide(map_layout::header_shape);
    directory_containment(bytes, plan);
}

/// A ghost valid-partition premise makes the checked remove pass exact and successful.
proof fn remove_refinement(before: Seq<u8>, after: Seq<u8>, plan: Plan, result: Result<()>)
    requires bounded(before, plan), pass_result(before, after, plan, result),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Remove),
        parameters(before, plan.region, fixed_map::node(plan).layout, plan.variable_map.unwrap(), plan.key@),
    ensures result.is_ok(), movement(before, after, plan),
{
    hide(bounded); hide(parameters); hide(movement); hide(pass_result);
    reveal(pass_result);
}

/// Nonempty admission and a concrete successful outcome witness the remove route.
#[verifier::spinoff_prover]
proof fn remove_witness()
{
    reveal(crate::layout_head::head_spec);
    broadcast use vstd::array::group_array_axioms;
    let before = witnesses::account(seq![1u8,0,0,0,2,0,0,0,0]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Remove, key_width: 1, value_width: 0, index: 0, found: true },
        count: 1, new_count: 0, stride: 5, payload: 21, offset: 0, end_offset: 0,
        input_len: 0, table_delta: -5i64 };
    let plan = Plan { key: vstd::array::spec_array_as_slice(&[2u8]), variable_map: Some(c),
        ..witnesses::replacement(before, witnesses::INDEXED_MAP, 4) };
    assert(plan.key@ =~= seq![2u8]);
    let local = before.subrange(plan.region.start as int, plan.region.end as int);
    assert(crate::search::encoded_key(local, witnesses::INDEXED_MAP, 0) =~= seq![2u8]);
    assert(crate::search::search_found(crate::Search { index: 0, found: true }, local, witnesses::INDEXED_MAP, plan.key@));
    assert(crate::search::search_partition(crate::Search { index: 0, found: true }, local, witnesses::INDEXED_MAP, plan.key@));
    assert(parameters(before, plan.region, witnesses::INDEXED_MAP, c, plan.key@));
    assert(bounded(before, plan));
    assert(movement(before, before, plan));
    remove_containment(before, plan);
    remove_refinement(before, before, plan, Ok(()));
}

/// Arbitrary admitted bytes retain finite, contained clear movement coordinates.
proof fn clear_containment(bytes: Seq<u8>, plan: Plan)
    requires bounded(bytes, plan), plan.required <= bytes.len(),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Clear),
    ensures 12 <= plan.region.start < plan.variable_map.unwrap().payload <= plan.old_used <= plan.required <= bytes.len() <= u32::MAX,
        0 <= destination(plan) <= destination(plan) + plan.variable_map.unwrap().input_len <= plan.new_used,
        forall|i: int| 0 <= i < plan.variable_map.unwrap().new_count ==>
            #[trigger] slot_fits(plan, i),
{
    hide(words::le_word); hide(search::search_found); hide(search::search_partition); hide(map_layout::header_shape);
    directory_containment(bytes, plan);
}

/// A ghost valid-partition premise makes the checked clear pass exact and successful.
proof fn clear_refinement(before: Seq<u8>, after: Seq<u8>, plan: Plan, result: Result<()>)
    requires bounded(before, plan), pass_result(before, after, plan, result),
        matches!(plan.variable_map.unwrap().change.operation, MapEdit::Clear),
        parameters(before, plan.region, fixed_map::node(plan).layout, plan.variable_map.unwrap(), plan.key@),
    ensures result.is_ok(), movement(before, after, plan),
{
    hide(bounded); hide(parameters); hide(movement); hide(pass_result);
    reveal(pass_result);
}

/// Nonempty admission and a concrete successful outcome witness the clear route.
#[verifier::spinoff_prover]
#[verifier::rlimit(600)]
proof fn clear_witness()
{
    reveal(crate::layout_head::head_spec);
    broadcast use vstd::array::group_array_axioms;
    let before = witnesses::account(seq![1u8,0,0,0,2,0,0,0,0]);
    let c = IndexedMap { change: MapChange { operation: MapEdit::Clear, key_width: 1, value_width: 0, index: 0, found: false },
        count: 1, new_count: 0, stride: 5, payload: 21, offset: 0, end_offset: 0,
        input_len: 0, table_delta: -5i64 };
    let plan = Plan { key: &[], map: None, variable_map: Some(c), indexed: None,
        splice: None, list: None, region: Span { start: 12, end: 21 },
        trail: witnesses::root_trail(witnesses::INDEXED_MAP, 21), path: witnesses::path(),
        discriminator: [1u8,0,0,0,0,0,0,0], old_used: 21, replacement_len: 4, delta: -5i64,
        new_used: 16, required: 21, storage_limit: before.len() as u32 };
    witnesses::single_map_shape(before.subrange(12,21));
    let local=before.subrange(12,21);
    assert(local =~= seq![1u8,0,0,0,2,0,0,0,0]);
    assert(words::le_word(before,12)==1);
    assert(words::le_word(before,17)==0);
    assert(map_layout::value_width(witnesses::INDEXED_MAP).is_none());
    assert(map_layout::entry_stride(witnesses::INDEXED_MAP)==5);
    assert(map_layout::header_shape(local,witnesses::INDEXED_MAP));
    assert(plan.key@.len()==0);
    assert(match witnesses::INDEXED_MAP.head() { LayoutHead::Map{key,value}=>key.width==1 && width_spec(value).is_none(),_=>false });
    assert(boundary(before,12,c,1)==0);
    assert(c.table_delta==(c.new_count as int-c.count)*c.stride);
    assert(header_parameters(before, plan.region, witnesses::INDEXED_MAP, c, plan.key@));
    assert forall|k:int| 0<=k<=c.count implies
        0<=#[trigger] boundary(before,plan.region.start as int,c,k)<=plan.region.end-c.payload by {
        assert(k==0 || k==1);
    }
    assert forall|i:int,j:int| 0<=i<=j<=c.count implies
        #[trigger] boundary(before,plan.region.start as int,c,i)<=#[trigger] boundary(before,plan.region.start as int,c,j) by {
        assert(i==0 || i==1);assert(j==0 || j==1);
    }
    assert(parameters(before, plan.region, witnesses::INDEXED_MAP, c, plan.key@));
    assert(bounded(before, plan));
    assert(movement(before, before, plan));
    clear_containment(before, plan);
    clear_refinement(before, before, plan, Ok(()));
}
}
