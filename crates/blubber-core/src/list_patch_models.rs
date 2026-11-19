//! Fixed-geometry QA entry points calling the production checked movers.
#![cfg_attr(verus_only, allow(missing_docs))]
use super::*;

verus! {

spec fn coordinates(p: Plan, insert: bool, end: u32) -> bool {
    &&& p.region == (Span { start: 12, end: 32 })
    &&& p.old_used == 32
    &&& p.delta == if insert { 5int } else { -(4 + end) }
    &&& p.new_used == if insert { 37int } else { 28 - end }
    &&& p.required == if insert { 37int } else { 32int }
    &&& p.replacement_len == if insert { 25int } else { 16 - end }
}

/// Builds numeric movement coordinates; parent admission is outside this model.
fn numeric_plan(insert: bool, end: u32) -> (p: Plan<'static>)
    requires end <= 4,
    ensures coordinates(p, insert, end),
{
    let region = Span { start: 12, end: 32 };
    let node = Node { payload_len: 0, region, #[cfg(verus_only)] layout: Layout::Str, ordinal: 0, run: PatchRun { offset: 0, stride: 4, count: 0 } };
    Plan { key: &[], map: None, variable_map: None, indexed: None, splice: None, list: None,
        region, trail: Trail { root: Layout::Str, selected: Layout::Str, local: true,
            nodes: [node; MAX_DEPTH], depth: 0 }, path: InlinePath { steps: [Step::Some; MAX_DEPTH], len: 0, overflow: false }, discriminator: [0;8],
        old_used: 32, delta: if insert { 5i64 } else { -(4 + end as i64) },
        new_used: if insert { 37 } else { 28 - end },
        replacement_len: if insert { 25 } else { 16 - end }, required: if insert { 37 } else { 32 },
        storage_limit: 40 }
}

/// Runs the production reverse pass on arbitrary ends in a four-item list.
#[doc(hidden)]
pub fn __list_insert_model(bytes: &mut [u8], poisoned: &mut bool) -> (result: Result<()>)
    requires old(bytes).len() == 40, words::le_word(old(bytes)@, 12) == 4,
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        forall|i: int| 0 <= i < 12 ==> final(bytes)@[i] == old(bytes)@[i],
{
    let plan = numeric_plan(true, 0);
    let c = IndexedInsert { index: 0, count: 4, payload: 28, offset: 0, table_growth: 4, input_len: 1 };
    indexed_insert::checked::move_checked(bytes, &plan, c, poisoned)
}

/// Runs the actual forward removal pass, including a later retained-word fault.
#[doc(hidden)]
pub fn __list_splice_model(bytes: &mut [u8], poisoned: &mut bool) -> (result: Result<()>)
    requires old(bytes).len() == 40, words::le_word(old(bytes)@, 12) == 4,
    ensures *final(poisoned), final(bytes).len() == old(bytes).len(),
        forall|i: int| 0 <= i < 12 ==> final(bytes)@[i] == old(bytes)@[i],
{
    *poisoned = true;
    let end = match read_u32(bytes, 16) { Ok(end) => end, Err(error) => return Err(error) };
    if end > 4 { return Err(Error::new(ErrorKind::InvalidBoundary, 12)); }
    let plan = numeric_plan(false, end);
    let c = IndexedSplice { operation: ListEdit::Remove(0), count: 4, new_count: 3,
        first: 0, last: 1, payload: 28, offset: 0, end_offset: end, table_shrink: 4, input_len: 0 };
    indexed_remove::checked::move_checked(bytes, &plan, c, poisoned)
}

}
