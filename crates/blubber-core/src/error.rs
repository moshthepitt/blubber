//! Stable, allocation-free errors and relative-offset framing.

// Verus generates undocumented ghost enum accessors. Erased Rust retains the
// workspace's missing_docs deny lint for every public runtime item.
#![cfg_attr(verus_only, allow(missing_docs))]

use vstd::prelude::*;

verus! {

/// A wire-format fault or a preserved SDK error code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A Blubber fault at a byte offset, or [`Error::NO_OFFSET`].
    Fault {
        /// The stable fault category.
        kind: ErrorKind,
        /// The first offending byte relative to the supplied region.
        offset: u32,
    },
    /// The full SDK representation, including native program errors.
    Program {
        /// The SDK's lossless error representation.
        code: u64,
    },
}

/// Stable fault discriminants; adapter custom codes are `0xB100 + kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum ErrorKind {
    /// A required header or value extends beyond the supplied bytes.
    Truncated = 1,
    /// The account discriminator differs from the expected schema.
    Discriminator = 2,
    /// The stored used length is invalid.
    UsedLength = 3,
    /// A size, product, delta, or offset is not representable.
    ArithmeticOverflow = 4,
    /// A region boundary is outside its parent or out of order.
    InvalidBoundary = 5,
    /// A boolean byte is neither zero nor one.
    InvalidBool = 6,
    /// A string contains invalid UTF-8.
    InvalidUtf8 = 7,
    /// An option or enum tag is invalid.
    InvalidTag = 8,
    /// Map keys descend in wire order.
    UnsortedKeys = 9,
    /// Adjacent map keys are equal.
    DuplicateKey = 10,
    /// An API index is outside the collection.
    IndexOutOfBounds = 11,
    /// A requested map key is absent.
    MissingKey = 12,
    /// A requested option value is absent.
    MissingValue = 13,
    /// A string edit splits a UTF-8 code point.
    Utf8Boundary = 14,
    /// The backing storage has insufficient capacity.
    Capacity = 15,
    /// A storage allocation failed.
    Allocation = 16,
    /// The schema or selected path exceeds the depth budget.
    DepthExceeded = 17,
    /// The account belongs to another program.
    WrongOwner = 18,
    /// An account required for mutation is read-only.
    NotWritable = 19,
    /// An outstanding runtime borrow conflicts with this operation.
    BorrowConflict = 20,
    /// Account growth exceeds the permitted window.
    GrowthLimit = 21,
    /// Available lamports cannot cover rent.
    RentShortfall = 22,
    /// The program has not authorized the requested action.
    Unauthorized = 23,
    /// A top-up payer fails its preflight checks.
    InvalidPayer = 24,
    /// Accounts that must be distinct share an address.
    DuplicateAccount = 25,
    /// Initialization was requested for an initialized account.
    AlreadyInitialized = 26,
    /// The root or account is closed.
    Closed = 27,
    /// An interrupted edit left the backend poisoned.
    /// Recover through a checked mutable reopen (`load_checked_mut` or
    /// `Store::load_checked`) after repairing invalid bytes. Only a successful
    /// complete scan clears poison; ordinary reopen and `verify` do not.
    Poisoned = 28,
    /// A plan is stale or does not match its buffer.
    InvalidPlan = 29,
    /// A descriptor violates the schema construction rules.
    InvalidSchema = 30,
}

/// The result of a checked Blubber operation.
pub type Result<T> = core::result::Result<T, Error>;

/// A diagnostic is unlocated or lies in the closed interval of a region.
/// Truncation may identify the region's end, including an empty region.
pub open spec fn fault_within(error: Error, length: nat) -> bool {
    match error {
        Error::Fault { offset, .. } => offset == u32::MAX || offset <= length,
        Error::Program { .. } => false,
    }
}

impl Error {
    /// Sentinel for faults with no meaningful byte location.
    pub const NO_OFFSET: u32 = u32::MAX;

    /// Constructs a fault without allocating or formatting.
    pub const fn new(kind: ErrorKind, offset: u32) -> (result: Self)
        ensures result == (Error::Fault { kind, offset }),
        no_unwind
    {
        Self::Fault { kind, offset }
    }

    /// Returns the reserved adapter custom code for a fault.
    ///
    /// Program errors retain their full `u64` representation and return `None`.
    pub const fn custom_code(self) -> (result: Option<u32>)
        ensures result == match self {
            Error::Fault { kind, .. } => Some((0xB100 + kind as u32) as u32),
            Error::Program { .. } => None,
        },
    {
        match self {
            Self::Fault { kind, .. } => Some(0xB100 + kind as u32),
            Self::Program { .. } => None,
        }
    }

    /// Adds a region base once to a relative fault offset.
    ///
    /// Preserves program codes and [`Self::NO_OFFSET`]. An unrepresentable sum
    /// becomes `ArithmeticOverflow` without an offset. A sum equal to the
    /// sentinel is also rejected: it cannot identify a byte in a `u32` region.
    pub const fn rebase(self, base: u32) -> (result: Self)
        ensures result == match self {
            Error::Fault { kind, offset } => {
                if offset == u32::MAX { self }
                else if offset as int + base < u32::MAX {
                    Error::Fault { kind, offset: (offset + base) as u32 }
                } else {
                    Error::Fault { kind: ErrorKind::ArithmeticOverflow, offset: u32::MAX }
                }
            },
            Error::Program { .. } => self,
        },
        no_unwind
    {
        match self {
            Self::Fault { kind, offset } => {
                if offset == Self::NO_OFFSET {
                    self
                } else {
                    match offset.checked_add(base) {
                        Some(absolute) => {
                            if absolute == Self::NO_OFFSET {
                                Self::new(ErrorKind::ArithmeticOverflow, Self::NO_OFFSET)
                            } else {
                                Self::new(kind, absolute)
                            }
                        },
                        None => Self::new(ErrorKind::ArithmeticOverflow, Self::NO_OFFSET),
                    }
                }
            },
            Self::Program { .. } => self,
        }
    }
}

#[cfg(verus_only)]
impl vstd::std_specs::convert::FromSpecImpl<ErrorKind> for Error {
    open spec fn obeys_from_spec() -> bool { true }

    open spec fn from_spec(kind: ErrorKind) -> Self {
        Error::Fault { kind, offset: u32::MAX }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> (result: Self)
        ensures result == (Error::Fault { kind, offset: u32::MAX }),
    {
        Self::new(kind, Self::NO_OFFSET)
    }
}

}

// Formatting is outside the verified kernel and runs only when requested.
impl core::fmt::Display for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::Fault {
                kind,
                offset: Self::NO_OFFSET,
            } => write!(formatter, "{kind:?}"),
            Self::Fault { kind, offset } => write!(formatter, "{kind:?} at byte {offset}"),
            Self::Program { code } => write!(formatter, "program error {code}"),
        }
    }
}

impl core::error::Error for Error {}
