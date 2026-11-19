//! Delayed publication loans for direct root variable subtrees.
use super::tail_edit::direct_field;
use super::*;

verus! {
/// Exact string overwrite, optional tail clear and envelope publication.
pub open spec fn string_result(before: Seq<u8>, encoded: Seq<u8>, after: Seq<u8>,
    start: u32, end: u32, used: u32, new_used: u32) -> bool {
    &&& before.len() == encoded.len() == after.len()
    &&& words::le_word(after, 8) == new_used
    &&& after.subrange(start as int, end as int) == encoded.subrange(start as int, end as int)
    &&& forall|i: int| 0 <= i < before.len() ==> {
        &&& new_used <= i < used ==> after[i] == 0
        &&& !(start <= i < end) && !(new_used <= i < used)
            && !(used != new_used && 8 <= i < 12) ==> #[trigger] after[i] == before[i]
    }
}

/// A direct variable-subtree destination with delayed length publication.
/// Checked string and collection admission share this byte-frame implementation.
#[doc(hidden)]
#[must_use = "encode the destination and finish before clearing poison"]
pub struct StringWrite<'a> {
    pub(super) bytes: &'a mut [u8],
    pub(super) start: u32,
    pub(super) end: u32,
    pub(super) used: u32,
    pub(super) new_used: u32,
    pub(super) issued: bool,
    #[cfg_attr(not(verus_only), allow(dead_code))]
    pub(super) original: Ghost<Seq<u8>>,
}

/// Checked encoder and publication coordinates, in account-relative bytes.
pub(super) open spec fn string_coordinates(write: StringWrite) -> (u32, u32, u32, u32) {
    (write.start, write.end, write.used, write.new_used)
}

/// Exact storage retained by a direct string loan.
pub closed spec fn string_storage<'a>(write: StringWrite<'a>) -> &'a mut [u8] { write.bytes }


/// Checked publication phase of a string loan.
pub closed spec fn string_phase(write: StringWrite, issued: bool) -> bool { write.issued == issued }
/// New physical extent if this replacement grows the root.
pub closed spec fn string_growth(write: StringWrite) -> Option<u32> {
    if write.new_used > write.used { Some(write.new_used) } else { None }
}
/// Encoder borrow, unchanged identity and publication geometry.
pub closed spec fn string_loan(before: StringWrite, after: StringWrite, initial: Seq<u8>, encoded: Seq<u8>) -> bool {
    after.issued && after.start == before.start && after.end == before.end
        && after.used == before.used && after.new_used == before.new_used && after.original == before.original
        && initial.len() == before.end - before.start
        && initial == before.original@.subrange(before.start as int,before.end as int)
        && after.bytes@.subrange(before.start as int, before.end as int) == encoded
}
/// Direct string publication's complete byte result.
pub closed spec fn string_finished(write: StringWrite, after: Seq<u8>) -> bool {
    string_result(write.original@, write.bytes@, after, write.start, write.end, write.used, write.new_used)
}
/// Exact selected field and original snapshot for a string loan.
pub closed spec fn string_admitted(write: StringWrite, before: Seq<u8>, layout: Layout,
    path: InlinePath, input_len: usize) -> bool {
    &&& write.original@ == before && write.bytes@ == before
    &&& !path.overflow && path.len == 1 && !write.issued && write.end - write.start == input_len
    &&& 12 <= write.start <= write.end <= write.new_used <= before.len() <= u32::MAX
    &&& 12 <= write.used <= before.len() && words::le_word(before, 8) == write.used
    &&& exists|s: Selection| #[trigger] string_selection_matches(write, before, layout, path, input_len, s)
}

pub(super) open spec fn string_selection_matches(write: StringWrite, before: Seq<u8>, layout: Layout,
    path: InlinePath, input_len: usize, s: Selection) -> bool {
    &&& #[trigger] selection_exact(s, before.subrange(12, write.used as int), layout, path.steps[0])
    &&& matches!(s.layout.head(), LayoutHead::Str)
    &&& write.start == 12 + s.span.start
    &&& write.new_used == write.used + input_len - (s.span.end - s.span.start)
    &&& input_len == s.span.end - s.span.start || (s.run.count == 0 && 12 + s.span.end == write.used)
}

pub(super) open spec fn string_selection(write: StringWrite, before: Seq<u8>, layout: Layout,
    path: InlinePath, input_len: usize) -> Selection {
    choose|s: Selection| string_selection_matches(write, before, layout, path, input_len, s)
}

/// Exposes successful admission facts without exposing the public opaque predicate.
pub(super) proof fn admitted_facts(write: StringWrite, before: Seq<u8>, layout: Layout,
    path: InlinePath, input_len: usize)
    requires string_admitted(write, before, layout, path, input_len),
    ensures !path.overflow, path.len == 1,
        12 <= write.start <= write.end <= write.new_used <= before.len() <= u32::MAX,
        12 <= write.used <= before.len(), words::le_word(before, 8) == write.used,
        write.end - write.start == input_len,
        string_selection_matches(write, before, layout, path, input_len,
            string_selection(write, before, layout, path, input_len)),
{ }

impl<'a> StringWrite<'a> {
    #[verifier::type_invariant]
    closed spec fn valid(self) -> bool {
        &&& self.bytes.len() == self.original@.len() <= u32::MAX
        &&& 12 <= self.start <= self.end <= self.new_used <= self.bytes.len()
        &&& 12 <= self.used <= self.bytes.len()
        &&& self.used == self.new_used || self.end == self.new_used
        &&& !self.issued ==> self.bytes@ == self.original@
        &&& words::le_word(self.original@, 8) == self.used
        &&& forall|i: int| 0 <= i < self.bytes.len() && !(self.start <= i < self.end)
            ==> #[trigger] self.bytes@[i] == self.original@[i]
    }

    /// Returns the new extent only when this replacement grows the root.
    #[inline(always)]
    pub fn growth_len(&self) -> (result: Option<u32>)
        ensures result == string_growth(*self),
    { if self.new_used > self.used { Some(self.new_used) } else { None } }

    /// Lends the complete replacement once; all other bytes stay outside it.
    #[inline(always)]
    pub fn destination(&mut self) -> (result: &mut [u8])
        requires string_phase(*old(self), false),
        ensures string_loan(*old(self), *final(self), result@, final(result)@),
        no_unwind
    {
        proof { use_type_invariant(&*self); }
        if self.issued { vstd::pervasive::__call_panic(&["string destination issued twice"]); }
        // control: string-issued
        self.issued = true;
        borrow_destination(self.bytes, self.start, self.end - self.start)
    }

    /// Clears a shrunk tail and publishes used length after encoding.
    #[inline(always)]
    pub fn finish(self)
        requires string_phase(self, true),
        ensures string_finished(self, final(string_storage(self))@),
        no_unwind
    {
        proof { use_type_invariant(&self); }
        if !self.issued { vstd::pervasive::__call_panic(&["string finish requires destination"]); }
        let ghost original = self.original@;
        let ghost start = self.start;
        let ghost end = self.end;
        let bytes = self.bytes;
        let used = self.used;
        let new_used = self.new_used;
        let ghost encoded = bytes@;
        if new_used != used {
            clear_tail(bytes, new_used, used);
            match crate::write_u32(bytes, 8, new_used) { Ok(()) => (), Err(_) => panic!("checked string used length") };
        }
        proof {
            assert(words::le_word(bytes@, 8) == new_used);
            assert(bytes@.subrange(start as int, end as int) =~= encoded.subrange(start as int, end as int));
            assert forall|i: int| 0 <= i < bytes.len() implies {
                &&& new_used <= i < used ==> bytes@[i] == 0
                &&& !(start <= i < end) && !(new_used <= i < used)
                    && !(used != new_used && 8 <= i < 12) ==> #[trigger] bytes@[i] == original[i]
            } by {
                if !(start <= i < end) && !(new_used <= i < used) && !(used != new_used && 8 <= i < 12) {
                    assert(bytes@[i] == encoded[i]);
                }
            }
            assert(string_result(original, encoded, bytes@, start, end, used, new_used));
        }
    }
}

/// Constructs the shared delayed-publication loan after coordinate admission.
#[inline(always)]
pub(super) fn direct_write<'a>(bytes:&'a mut[u8],start:u32,used:u32,end:u32)->(write:StringWrite<'a>)
    requires 12<=start<=end<=old(bytes).len()<=u32::MAX,
        12<=used<=old(bytes).len(),words::le_word(old(bytes)@,8)==used,
    ensures direct_admitted(write,old(bytes)@,start,used,end),
    no_unwind
{
    StringWrite{original:Ghost(bytes@),bytes,start,end,used,new_used:end,issued:false}
}
pub(super) open spec fn direct_admitted(w:StringWrite,before:Seq<u8>,start:u32,used:u32,end:u32)->bool {
    &&& w.bytes@==before && w.original@==before && !w.issued
    &&& w.start==start && w.used==used && w.end==end && w.new_used==end
}

/// Admits an equal-size root string field or a string at the live tail.
/// Other size changes and capacity misses retain the generic planner.
#[doc(hidden)]
#[inline(always)]
pub fn prepare_string_write<'a>(bytes: &'a mut [u8], discriminator: [u8; 8], minimum: u32,
    layout: &Layout, path: &InlinePath, input_len: usize) -> (result: Result<Option<StringWrite<'a>>>)
    ensures result.is_err() || (result.is_ok() && result.unwrap().is_none()) ==> final(bytes)@ == old(bytes)@,
        result.is_ok() && result.unwrap().is_some()
            ==> string_admitted(result.unwrap().unwrap(), old(bytes)@, *layout, *path, input_len),
{
    let (s, used) = match direct_field(bytes, discriminator, minimum, layout, path)? {
        Some(pair) => pair, None => return Ok(None),
    };
    if s.layout.tag() != 5 { return Ok(None); }
    proof { reveal(crate::layout_head::head_spec); }
    let len = checked_u32(input_len)?;
    let old_len = s.span.end - s.span.start;
    if len != old_len && (s.run.count != 0 || 12 + s.span.end != used) { return Ok(None); }
    let start = 12 + s.span.start;
    let (end, new_used) = if len == old_len { (12 + s.span.end, used) }
        else { let end = checked_delta(start, len as i64)?; (end, end) };
    if new_used as usize > bytes.len() { return Ok(None); }
    let original = Ghost(bytes@);
    let write = StringWrite { bytes, start, end, used, new_used, issued: false, original };
    proof { assert(string_selection_matches(write, original@, *layout, *path, input_len, s)); }
    Ok(Some(write))
}



/// A concrete mutable byte slice inhabits the direct string-loan premises.
#[cfg(verus_only)]
#[verifier::spinoff_prover]
fn string_witness()
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
    raw[0] = 1; raw[8] = 13; raw[12] = 97;
    let original = Ghost(raw@);
    let write = StringWrite { bytes: &mut raw, start: 12, end: 13, used: 13,
        new_used: 13, issued: false, original };
    proof {
        broadcast use vstd::array::group_array_axioms;
        reveal(crate::layout_head::head_spec);
        reveal_with_fuel(crate::layout::descriptor_valid, 4);
        reveal_with_fuel(crate::layout::descriptor_fields, 4);
        reveal_with_fuel(validation::schema_valid, 4);
        reveal_with_fuel(validation::children_valid, 5);
        reveal_with_fuel(crate::utf8::utf8_valid, 3);
        assert(original@ =~= witnesses::account(seq![97u8]));
        let selection = Selection { payload_len: 1, span: Span { start: 0, end: 1 }, layout: Layout::Str, ordinal: 0, run: empty_run() };
        assert(string_selection_matches(write, original@, witnesses::string_root(), witnesses::field_path(), 1, selection));
        string_admission_matches_generic(write, original@, original@, original@, witnesses::string_root(), witnesses::field_path(), 1); }
}

}
