//! Owned format-1 vocabulary, independent of production descriptors.

use std::{collections::BTreeMap, fmt};

/// A rejected reference encoding, decoding, schema, or edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Invalid metadata, zero fixed width, or a schema beyond the depth budget.
    InvalidSchema,
    /// A value, key, or operation does not match its schema.
    TypeMismatch,
    /// An integer does not fit its declared width.
    OutOfRange,
    /// An exact leaf, fixed layout, or header has the wrong length.
    InvalidLength,
    /// A directory does not partition the exact payload.
    InvalidBoundary,
    /// A boolean byte is outside zero and one.
    InvalidBool,
    /// An option tag is outside zero and one.
    InvalidTag,
    /// A string is not UTF-8.
    InvalidUtf8,
    /// Encoded map keys are duplicated or descending.
    KeyOrder,
    /// A count, offset, or total size is outside the u32 wire domain.
    Overflow,
    /// An account discriminator is zero or differs from the expected bytes.
    Discriminator,
    /// An encoding exceeds the caller's capacity limit.
    Capacity,
    /// A list or field index is outside its allowed range.
    Index,
    /// A path selects an absent option payload or map entry.
    Missing,
    /// A path segment cannot select a child of this node.
    InvalidPath,
    /// A caller callback returned its chosen error after completing its edit.
    Callback,
    /// The session was already poisoned before this operation.
    Poisoned,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSchema => "invalid reference schema or depth budget",
            Self::TypeMismatch => "value or operation does not match schema",
            Self::OutOfRange => "integer outside its declared width",
            Self::InvalidLength => "incorrect exact encoding length",
            Self::InvalidBoundary => "invalid payload partition",
            Self::InvalidBool => "invalid boolean byte",
            Self::InvalidTag => "invalid option tag",
            Self::InvalidUtf8 => "invalid UTF-8",
            Self::KeyOrder => "map keys are not strictly increasing",
            Self::Overflow => "encoding exceeds the wire domain",
            Self::Discriminator => "invalid or mismatched discriminator",
            Self::Capacity => "encoding exceeds capacity",
            Self::Index => "index out of bounds",
            Self::Missing => "selected value is absent",
            Self::InvalidPath => "path does not match schema",
            Self::Callback => "caller callback error",
            Self::Poisoned => "session already poisoned",
        })
    }
}

impl std::error::Error for Error {}

/// Supported integer widths, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntWidth {
    /// Eight bits.
    W1,
    /// Sixteen bits.
    W2,
    /// Thirty-two bits.
    W4,
    /// Sixty-four bits.
    W8,
    /// One hundred twenty-eight bits.
    W16,
}

impl IntWidth {
    /// Returns the encoded byte width.
    pub const fn bytes(self) -> usize {
        match self {
            Self::W1 => 1,
            Self::W2 => 2,
            Self::W4 => 4,
            Self::W8 => 8,
            Self::W16 => 16,
        }
    }

    pub(crate) fn check_unsigned(self, value: u128) -> Result<(), Error> {
        if self == Self::W16 || value < (1u128 << (8 * self.bytes())) {
            Ok(())
        } else {
            Err(Error::OutOfRange)
        }
    }

    pub(crate) fn check_signed(self, value: i128) -> Result<(), Error> {
        if self != Self::W16 {
            let limit = 1i128 << (8 * self.bytes() - 1);
            if !(-limit..limit).contains(&value) {
                return Err(Error::OutOfRange);
            }
        }
        Ok(())
    }
}

/// A built-in map-key domain, with native logical ordering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeySchema {
    /// Unsigned integer encoded big-endian.
    Unsigned(IntWidth),
    /// Signed integer encoded big-endian with its sign bit flipped.
    Signed(IntWidth),
    /// Boolean encoded as zero or one.
    Bool,
    /// Thirty-two raw address bytes.
    Pubkey,
    /// A positive number of raw bytes.
    Array(u32),
}

/// A native owned key; all keys in one map must match its [`KeySchema`].
///
/// Variant ordering is irrelevant to the wire: mixed domains are rejected.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    /// Unsigned integer in its declared domain.
    Unsigned(u128),
    /// Signed integer in its declared domain.
    Signed(i128),
    /// Boolean key.
    Bool(bool),
    /// Address key.
    Pubkey([u8; 32]),
    /// Raw array key.
    Array(Vec<u8>),
}

impl KeySchema {
    /// Returns the checked fixed encoded width.
    pub fn width(&self) -> Result<usize, Error> {
        match self {
            Self::Unsigned(width) | Self::Signed(width) => Ok(width.bytes()),
            Self::Bool => Ok(1),
            Self::Pubkey => Ok(32),
            Self::Array(0) => Err(Error::InvalidSchema),
            Self::Array(width) => Ok(*width as usize),
        }
    }
}

/// An owned schema in declaration order, with no production layout metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Schema {
    /// Unsigned little-endian scalar.
    Unsigned(IntWidth),
    /// Signed two's-complement little-endian scalar.
    Signed(IntWidth),
    /// Exactly one zero or one byte.
    Bool,
    /// Exactly thirty-two raw bytes.
    Pubkey,
    /// A positive, fixed-length raw array.
    Array(u32),
    /// Exact raw UTF-8, without a length word.
    Str,
    /// Packed fields in declaration order; fixed fields move before variables.
    Struct(Vec<Schema>),
    /// Counted elements, using fixed strides or elided terminal ends.
    List(Box<Schema>),
    /// Native ordered keys with fixed-stride or indexed values.
    Map(KeySchema, Box<Schema>),
    /// A tag and only the active payload; always variable-kind.
    Option(Box<Schema>),
}

/// An owned logical value; struct fields remain in declaration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// Unsigned scalar, checked against its schema width.
    Unsigned(u128),
    /// Signed scalar, checked against its schema width.
    Signed(i128),
    /// Boolean scalar.
    Bool(bool),
    /// Address scalar.
    Pubkey([u8; 32]),
    /// Raw fixed array, distinct from a counted byte list.
    Array(Vec<u8>),
    /// Owned UTF-8 string.
    Str(String),
    /// Fields in declaration order, including fixed and variable children.
    Struct(Vec<Value>),
    /// Logical element sequence.
    List(Vec<Value>),
    /// Unique native keys in total order.
    Map(BTreeMap<Key, Value>),
    /// Optional active child.
    Option(Option<Box<Value>>),
}

pub(crate) fn wire_size(size: usize) -> Result<usize, Error> {
    u32::try_from(size).map_err(|_| Error::Overflow)?;
    Ok(size)
}

pub(crate) fn add(a: usize, b: usize) -> Result<usize, Error> {
    wire_size(a.checked_add(b).ok_or(Error::Overflow)?)
}

pub(crate) fn mul(a: usize, b: usize) -> Result<usize, Error> {
    wire_size(a.checked_mul(b).ok_or(Error::Overflow)?)
}

impl Schema {
    /// Default root-to-leaf node budget, including inactive schema branches.
    pub const DEFAULT_DEPTH: usize = 8;

    /// Largest supported node budget, corresponding to `max-depth-16`.
    pub const MAX_DEPTH: usize = 16;

    /// Checks every branch, even for empty collections and absent options.
    ///
    /// The root consumes one unit. Structs require 1..=65535 fields; fixed
    /// widths and minimum composite sizes must fit u32. Budgets are 1..=16.
    pub fn check(&self, max_depth: usize) -> Result<(), Error> {
        if !(1..=Self::MAX_DEPTH).contains(&max_depth) {
            return Err(Error::InvalidSchema);
        }
        self.info(max_depth).map(|_| ())
    }

    /// Returns a fixed width, or `None` for a variable-kind schema.
    pub fn fixed_width(&self) -> Result<Option<usize>, Error> {
        self.info(Self::DEFAULT_DEPTH).map(|(fixed, _)| fixed)
    }

    /// Returns the smallest encoding after checking the requested depth budget.
    pub fn minimum_size(&self, max_depth: usize) -> Result<usize, Error> {
        self.check(max_depth)?;
        self.info(max_depth).map(|(_, minimum)| minimum)
    }

    fn info(&self, budget: usize) -> Result<(Option<usize>, usize), Error> {
        if budget == 0 {
            return Err(Error::InvalidSchema);
        }
        let width = match self {
            Self::Unsigned(width) | Self::Signed(width) => width.bytes(),
            Self::Bool => 1,
            Self::Pubkey => 32,
            Self::Array(0) => return Err(Error::InvalidSchema),
            Self::Array(width) => *width as usize,
            Self::Str => return Ok((None, 0)),
            Self::Option(child) | Self::List(child) | Self::Map(_, child) => {
                child.info(budget - 1)?;
                if let Self::Map(key, _) = self {
                    key.width()?;
                }
                return Ok((
                    None,
                    if matches!(self, Self::Option(_)) {
                        1
                    } else {
                        4
                    },
                ));
            }
            Self::Struct(fields) => {
                if fields.is_empty() || fields.len() > u16::MAX as usize {
                    return Err(Error::InvalidSchema);
                }
                let mut minimum = 0;
                let mut variables: usize = 0;
                for field in fields {
                    let (fixed, child_min) = field.info(budget - 1)?;
                    minimum = add(minimum, child_min)?;
                    variables += usize::from(fixed.is_none());
                }
                let size = add(minimum, mul(4, variables.saturating_sub(1))?)?;
                return Ok(((variables == 0).then_some(size), size));
            }
        };
        Ok((Some(width), width))
    }

    // Used only after checking the entire schema at the entry point.
    pub(crate) fn fixed(&self) -> Option<usize> {
        match self {
            Self::Unsigned(width) | Self::Signed(width) => Some(width.bytes()),
            Self::Bool => Some(1),
            Self::Pubkey => Some(32),
            Self::Array(width) => Some(*width as usize),
            Self::Struct(fields) => fields.iter().map(Self::fixed).sum(),
            _ => None,
        }
    }
}
