//! Concrete borrowed inputs for generic list non-vacuity checks.
use crate::encoding_list::Elements;
use crate::encoding_product::{Item, Source};
use crate::Layout;
use vstd::prelude::*;
use vstd::string::StringSliceAdditionalSpecFns;

verus! {
/// Concrete borrowed strings include zero-length elements and equal end words.
pub struct TextInputs<'a> {
    /// Borrowed UTF-8 elements.
    pub values: &'a [&'a str],
}

impl<'a> Elements for TextInputs<'a> {
    type Child = crate::encoding_text::Text<'a>;
    open spec fn items(&self) -> Seq<Item> {
        Seq::new(self.values.len() as nat, |i: int| Item { bytes: self.values@[i].spec_bytes(),
            fixed: false, layout: Layout::Str })
    }
    open spec fn trusted_input(&self) -> bool {
        forall|i: int| 0 <= i < self.values.len() ==> self.values@[i].spec_bytes().len() <= u32::MAX
    }
    open spec fn measurement(&self, index: int) -> crate::Result<u32> {
        Ok(self.values@[index].spec_bytes().len() as u32)
    }
    fn count(&self) -> (count: usize)
        ensures count == self.items().len(),
    { self.values.len() }
    fn at(&self, index: usize) -> (child: Self::Child)
        ensures child.trusted_input(), child.measurement() == self.measurement(index as int),
            child.bytes() == self.items()[index as int].bytes,
            child.fixed() == self.items()[index as int].fixed,
            child.layout() == self.items()[index as int].layout,
    { crate::encoding_text::text(self.values[index]) }
}


}
