//! Allocation-free repeated input for count and directory failure witnesses.
use crate::encoding_list::Elements;
use crate::encoding_product::{Item, Source};
use crate::Layout;
use vstd::prelude::*;
use vstd::string::StringSliceAdditionalSpecFns;

verus! {
/// Repeated immutable text with a machine-sized count and no materialized list.
pub struct RepeatedInputs<'a> {
    /// Number of element views.
    pub count: usize,
    /// Text projected at each ordinal.
    pub value: &'a str,
}

impl<'a> Elements for RepeatedInputs<'a> {
    type Child = crate::encoding_text::Text<'a>;
    open spec fn items(&self) -> Seq<Item> {
        Seq::new(self.count as nat, |i: int| Item { bytes: self.value.spec_bytes(),
            fixed: false, layout: Layout::Str })
    }
    open spec fn trusted_input(&self) -> bool { self.value.spec_bytes().len() <= u32::MAX }
    open spec fn measurement(&self, index: int) -> crate::Result<u32> {
        Ok(self.value.spec_bytes().len() as u32)
    }
    fn count(&self) -> (count: usize)
        ensures count == self.items().len(),
    { self.count }
    fn at(&self, index: usize) -> (child: Self::Child)
        ensures child.trusted_input(), child.measurement() == self.measurement(index as int),
            child.bytes() == self.items()[index as int].bytes,
            child.fixed() == self.items()[index as int].fixed,
            child.layout() == self.items()[index as int].layout,
    { crate::encoding_text::text(self.value) }
}
}
