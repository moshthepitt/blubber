//! Balanced field traversal with declaration-order callback sequencing.
// Explicit matches retain panic-free error propagation under Verus.
#![allow(clippy::question_mark)]
#[cfg(verus_only)]
use crate::encoding_product::{grouping, Item};
use crate::encoding_product::{Cursor, Fields, Pair};
use crate::Result;
use vstd::prelude::*;

verus! {
impl<L: Fields, R: Fields> Fields for Pair<L, R> {
    open spec fn items(&self) -> Seq<Item> { self.left.items() + self.right.items() }
    open spec fn trusted_input(&self) -> bool {
        self.left.trusted_input() && self.right.trusted_input()
    }
    open spec fn measurement(&self, total: u32) -> Result<u32> {
        match self.left.measurement(total) {
            Err(error) => Err(error), Ok(next) => self.right.measurement(next),
        }
    }

    #[inline(always)]
    fn measure_from(&self, total: u32) -> (result: Result<u32>)
        ensures result.is_ok() == (total + crate::encoding_product::extent(self.items()) <= u32::MAX),
            result.is_ok() ==> result.unwrap() == total + crate::encoding_product::extent(self.items()),
            result == self.measurement(total),
    {
        proof { grouping(Seq::empty(), arbitrary(), self.left.items(), self.right.items()); }
        let size = match self.left.measure_from(total) {
            Ok(size) => size,
            Err(error) => return Err(error),
        };
        self.right.measure_from(size)
    }

    #[inline(always)]
    fn write(&self, destination: &mut [u8], cursor: Cursor) -> (result: Cursor)
        ensures final(destination).len() == old(destination).len(),
            result == crate::encoding_product::advance(cursor, self.items()),
            final(destination)@ == crate::encoding_product::paint_all(old(destination)@, cursor, self.items()),
    {
        proof { grouping(destination@, cursor, self.left.items(), self.right.items()); }
        let cursor = self.left.write(destination, cursor);
        self.right.write(destination, cursor)
    }
}
}
