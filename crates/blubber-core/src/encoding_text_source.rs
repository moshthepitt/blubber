//! Verified primitive source adapter for generic product and option writers.
use crate::encoding_product::Source;
#[cfg(verus_only)]
use crate::Layout;
use crate::Result;
use vstd::prelude::*;

verus! {
impl<'a> Source for crate::encoding_text::Text<'a> {
    open spec fn trusted_input(&self) -> bool { crate::encoding::Encoder::trusted_input(self)
        && crate::encoding::Encoder::bytes(self).len() <= u32::MAX }
    open spec fn bytes(&self) -> Seq<u8> { crate::encoding::Encoder::bytes(self) }
    open spec fn measurement(&self) -> Result<u32> { Ok(self.bytes().len() as u32) }
    open spec fn fixed(&self) -> bool { false }
    open spec fn layout(&self) -> Layout { Layout::Str }
    #[inline(always)]
    fn is_fixed(&self) -> (result: bool)
        ensures result == self.fixed(),
    { false }
    #[inline(always)]
    fn measure(&self) -> (result: Result<u32>)
        ensures result.is_ok() == (self.bytes().len() <= u32::MAX),
            result.is_ok() ==> result.unwrap() == self.bytes().len(),
    {
        let size = crate::encoding::Encoder::measure(self);
        Ok(size as u32)
    }
    #[inline(always)]
    fn encode(&self, destination: &mut [u8])
        ensures final(destination)@ == self.bytes(),
    { crate::encoding::Encoder::encode(self, destination); }
}
}
