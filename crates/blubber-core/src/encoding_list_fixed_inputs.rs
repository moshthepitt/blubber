//! Concrete borrowed inputs for generic list non-vacuity checks.
use crate::encoding_list::Elements;
use crate::encoding_product::{Item, Source};
use crate::Layout;
use vstd::prelude::*;

verus! {
/// Concrete borrowed raw-byte inputs for the no-premise list witnesses.
pub struct FixedInputs<'a> {
    /// Borrowed raw-byte elements.
    pub values: &'a [[u8; 1]],
}

impl<'a> Elements for FixedInputs<'a> {
    type Child = crate::encoding_leaf::Array<'a, 1>;
    open spec fn items(&self) -> Seq<Item> {
        Seq::new(self.values.len() as nat, |i: int| Item { bytes: self.values@[i]@,
            fixed: true, layout: Layout::Leaf(crate::Leaf::Raw(1)) })
    }
    open spec fn trusted_input(&self) -> bool { true }
    open spec fn measurement(&self, index: int) -> crate::Result<u32> { Ok(1) }
    fn count(&self) -> (count: usize)
        ensures count == self.items().len(),
    { self.values.len() }
    fn at(&self, index: usize) -> (child: Self::Child)
        ensures child.trusted_input(), child.measurement() == self.measurement(index as int),
            child.bytes() == self.items()[index as int].bytes,
            child.fixed() == self.items()[index as int].fixed,
            child.layout() == self.items()[index as int].layout,
    { crate::encoding_leaf::raw(&self.values[index]) }
}


}
