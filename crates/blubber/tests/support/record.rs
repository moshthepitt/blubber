//! Shared production-derived record shapes for list tests and bounded proofs.

#![forbid(unsafe_code)]

use blubber::{List, Str};

/// A variable record containing borrowed UTF-8 and fixed-width tags.
#[blubber::schema]
pub struct Record {
    /// UTF-8 name.
    pub name: Str,
    /// Counted little-endian tags.
    pub tags: List<u32>,
}

/// A minimal account of variable records.
#[blubber::schema(account)]
pub struct Records {
    /// Records in declaration order.
    pub records: List<Record>,
}
