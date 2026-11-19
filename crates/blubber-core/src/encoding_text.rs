//! Borrowed Rust strings and their independent wire-validity bridge.

use crate::encoding::{private, Encoder};
#[cfg(verus_only)]
use crate::{Layout, MAX_DEPTH};
use vstd::prelude::*;
#[cfg(verus_only)]
use vstd::string::StringSliceAdditionalSpecFns;

verus! {

/// Borrowed UTF-8 without a count word or terminator.
pub struct Text<'a> {
    source: &'a str,
}

impl<'a> private::Sealed for Text<'a> {}

impl<'a> Encoder for Text<'a> {
    closed spec fn trusted_input(&self) -> bool { self.source.spec_bytes().len() <= u32::MAX }
    closed spec fn bytes(&self) -> Seq<u8> { self.source.spec_bytes() }

    #[inline(always)]
    fn measure(&self) -> (n: usize)
        ensures n == self.bytes().len(),
    { self.source.len() }

    #[inline(always)]
    fn encode(&self, destination: &mut [u8])
        ensures final(destination)@ == self.bytes(),
    {
        encode_bytes(self.source.as_bytes(), destination);
        proof { encoder_string_valid(self, destination@, MAX_DEPTH as nat); }
    }
}

/// Borrows an entire Rust string; preflight admits its byte extent.
#[inline(always)]
pub fn text(source: &str) -> (value: Text<'_>)
    ensures value.bytes() == source.spec_bytes(),
        value.trusted_input() == (source.spec_bytes().len() <= u32::MAX),
    no_unwind
{ Text { source } }

/// Writes a preflighted string through its borrowed primitive encoder.
///
/// # Panics
/// Its byte length must fit `u32` and equal the destination length.
#[inline(always)]
pub fn encode_text(source: &str, destination: &mut [u8])
    requires source.spec_bytes().len() <= u32::MAX,
        old(destination).len() == source.spec_bytes().len(),
    ensures final(destination)@ == source.spec_bytes(),
    no_unwind
{ text(source).encode(destination); }

/// Rust string bytes satisfy the independent wire UTF-8 and schema relations.
pub proof fn encoder_string_valid(source: &Text<'_>, after: Seq<u8>, budget: nat)
    requires source.trusted_input(), after == source.bytes(), 0 < budget <= MAX_DEPTH,
    ensures crate::validation::schema_valid(after, Layout::Str, budget, true),
{
    vstd::utf8::encode_utf8_valid_utf8(source.source@);
    assert(after.subrange(0, after.len() as int) =~= after);
    crate::encoding_utf8::string_bridge(after, 0);
    reveal(crate::layout_head::head_spec);
    reveal_with_fuel(crate::validation::children_valid, 2);
}

/// Concrete multibyte string witness through the actual text encoder.
#[cfg(verus_only)]
pub(crate) fn string_witness()
    ensures true,
{
    proof { reveal_strlit("\u{80}"); }
    let value = text("\u{80}"); let mut output = [19u8; 2];
    proof {
        reveal_with_fuel(vstd::utf8::encode_utf8, 3);
        assert(value.bytes().len() == 2);
    }
    value.encode(&mut output);
    proof { encoder_string_valid(&value, output@, 1); }
}


/// Copies an exact byte slice without callbacks or allocation.
///
/// Ordinary inlining retains slice alias information for dynamic copies while
/// allowing constant source widths to specialize in their callers.
///
/// # Panics
/// The destination length must equal the source length; rejection precedes writes.
#[inline]
pub fn encode_bytes(source: &[u8], destination: &mut [u8])
    requires old(destination).len() == source.len(),
    ensures final(destination)@ == source@,
    no_unwind
{
    assert!(destination.len() == source.len());
    let mut i = 0;
    while i < source.len()
        invariant i <= source.len(), destination.len() == source.len(),
            forall|j: int| 0 <= j < i ==> destination@[j] == source@[j],
        decreases source.len() - i,
    {
        destination[i] = source[i];
        i += 1;
    }
    assert(destination@ =~= source@);
}

}
