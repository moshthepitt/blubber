//! Concrete failed map preflight for the replay observation witness.

use crate::encoding_replay::Input;
use crate::{Error, ErrorKind, Result};
use vstd::prelude::*;

verus! {

/// Duplicate-key map input used only by the proof's negative branch.
pub struct RejectedMap<'a> {
    /// Borrowed pairs; the witness supplies two equal keys.
    pub values: &'a [([u8; 1], [u8; 1])],
}

impl Input for RejectedMap<'_> {
    open spec fn trusted_input(&self) -> bool {
        self.values.len() == 2 && self.values@[0].0@ == self.values@[1].0@
    }
    open spec fn bytes(&self) -> Seq<u8> { Seq::empty() }
    open spec fn measurement(&self) -> Result<u32> {
        Err(Error::Fault { kind: ErrorKind::DuplicateKey, offset: u32::MAX })
    }
    fn measure(&self) -> (result: Result<u32>)
        ensures result == self.measurement(),
            result.is_ok() ==> result.unwrap() == self.bytes().len(),
    {
        proof { reveal_with_fuel(crate::encoding_map::fold, 4); }
        let source = crate::encoding_map_fixed_inputs::FixedInputs {
            values: self.values,
        };
        crate::encoding_map::measure(&source, 1, false, Some(1))
    }
    fn encode(&self, destination: &mut [u8])
        ensures final(destination)@ == self.bytes(),
    { crate::encoding_text::encode_bytes(&[], destination); }
}

}
