//! Primitive map sources used by executable non-vacuity witnesses.

use crate::encoding_map::{Entries, Entry};
use crate::encoding_product::Source;
use core::cmp::Ordering;
use vstd::prelude::*;
use vstd::string::StringSliceAdditionalSpecFns;

verus! {

/// Borrowed raw-byte keys and UTF-8 values, including empty values.
pub struct TextInputs<'a> {
    /// Existing immutable key/value pairs.
    pub values: &'a [([u8; 1], &'a str)],
}

impl<'a> Entries for TextInputs<'a> {
    type Child = crate::encoding_text::Text<'a>;
    open spec fn entries(&self) -> Seq<Entry> {
        Seq::new(self.values.len() as nat, |i: int| Entry { key: self.values@[i].0@,
            value: self.values@[i].1.spec_bytes() })
    }
    open spec fn trusted_input(&self) -> bool {
        forall|i:int| 0 <= i < self.values.len() ==> self.values@[i].1.spec_bytes().len() <= u32::MAX
    }
    open spec fn measurement(&self, index: int) -> crate::Result<u32> {
        Ok(self.values@[index].1.spec_bytes().len() as u32)
    }
    open spec fn comparison(&self, index: int) -> Ordering {
        if self.values@[index - 1].0@[0] < self.values@[index].0@[0] { Ordering::Less }
        else if self.values@[index - 1].0@[0] == self.values@[index].0@[0] { Ordering::Equal }
        else { Ordering::Greater }
    }
    fn count(&self) -> (count: usize)
        ensures count == self.entries().len(),
    { self.values.len() }
    fn value(&self, index: usize) -> (child: Self::Child)
        ensures child.trusted_input(), child.bytes() == self.entries()[index as int].value,
            child.measurement() == self.measurement(index as int),
    { crate::encoding_text::text(self.values[index].1) }
    fn compare(&self, index: usize) -> (result: Ordering)
        ensures result == self.comparison(index as int),
            crate::search::lex_total(result, self.entries()[index as int - 1].key, self.entries()[index as int].key),
    { crate::encoding_leaf::compare_array(&self.values[index - 1].0, &self.values[index].0) }
    fn write_key(&self, index: usize, destination: &mut [u8])
        ensures final(destination)@ == self.entries()[index as int].key,
    { crate::encoding_leaf::encode_array(&self.values[index].0, destination); }
}

}
