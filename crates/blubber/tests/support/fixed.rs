//! Native layouts shared by host fixtures and production-derive Kani harnesses.

use blubber::Fixed;

/// The native fixed struct from the architecture's application tour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Fixed)]
pub struct Limits {
    /// Whether the limit is active.
    pub enabled: bool,
    /// The configured limit.
    pub limit: u16,
}

/// A native layout with alignment padding and a nested fixed codec.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Fixed)]
pub struct Padded {
    /// A boolean preceding native alignment padding.
    pub flag: bool,
    /// An aligned integer, encoded directly after the flag on the wire.
    pub total: u64,
    /// A copied nested value, without its native padding.
    pub limits: Limits,
    /// Raw trailing bytes.
    pub raw: [u8; 3],
}
