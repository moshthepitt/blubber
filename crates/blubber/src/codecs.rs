//! Explicit fixed-value codecs, independent of native layout and SDK types.

use crate::{Error, ErrorKind, Result};
use blubber_core::{KeyDomain, KeyLayout};

/// A copied value with a positive, byte-aligned, fixed-width wire encoding.
///
/// Implementations must accept exactly [`Self::SIZE`] bytes, validate every bit
/// domain, and write every encoded byte. Native padding never enters the wire.
/// Built-ins use little-endian integers; map keys have a separate encoding.
/// The `derive` feature also exports a derive macro with this name:
///
/// ```
/// # #[cfg(feature = "derive")] {
/// use blubber::Fixed;
/// #[derive(Clone, Copy, Debug, PartialEq, Eq, Fixed)]
/// struct Limits { enabled: bool, limit: u16 }
/// let value = Limits { enabled: true, limit: 100 };
/// assert_eq!(Limits::SIZE, 3);
/// assert_eq!(value.encode(), [1, 100, 0]);
/// assert_eq!(Limits::decode(&[1, 100, 0]), Ok(value));
/// # }
/// ```
pub trait Fixed: Sized + Copy + 'static {
    /// Selects the exact fixed encoder contract, including every nested field.
    ///
    /// A handwritten true value is a trusted implementer contract, in the same
    /// class as derive projection glue: pure deterministic measurement, exact
    /// encoding into the measured destination, schema-valid output and replay.
    /// It is not a proof about that implementation. A violation may cause
    /// errors, poison or wrong data in that program's own accounts, never
    /// undefined behavior: all library paths remain safe Rust.
    /// Encoded widths and domains agree with Schema metadata. Built-ins are
    /// true; fixed derives AND all child markers. Default false stays checked.
    #[doc(hidden)]
    const __ENCODER_AUDITED: bool = false;
    /// Exact encoded width, in bytes.
    const SIZE: u32;
    /// Owned bytes returned without allocating.
    type Encoded: AsRef<[u8]>;

    /// Decodes an exact region, rejecting short, trailing, or invalid bytes.
    ///
    /// Short regions return `Truncated` at their end; trailing bytes return
    /// `InvalidBoundary` at the expected end. Invalid scalar bools fault at zero;
    /// derived structs rebase field faults to their packed wire positions.
    fn decode(bytes: &[u8]) -> Result<Self>;

    /// Decodes an exact fixed-walk loan, with the same checked result as `decode`.
    #[doc(hidden)]
    #[inline(always)]
    fn __decode_region(bytes: &[u8]) -> Result<Self> {
        Self::decode(bytes)
    }

    /// Encodes the complete value without native padding or allocation.
    fn encode(self) -> Self::Encoded;
}

/// A fixed key whose canonical byte order agrees with its Rust total order.
///
/// Integers use big-endian bytes, with the sign bit flipped for signed keys.
/// Raw arrays and [`Pubkey`] retain their bytes. Fixed structs are not keys.
pub trait MapKey: Fixed + Ord {
    /// Selects the canonical key encoder contract for the complete key graph.
    ///
    /// A handwritten true value is a trusted implementer contract, in the same
    /// class as derive projection glue: pure deterministic measurement, exact
    /// encoding into the measured destination, schema-valid output and replay.
    /// It is not a proof about that implementation. A violation may cause
    /// errors, poison or wrong data in that program's own accounts, never
    /// undefined behavior: all library paths remain safe Rust.
    /// Key bytes also have exact width, canonical domains and injective order
    /// agreement. Built-ins are true; default false keeps containing maps checked.
    #[doc(hidden)]
    const __KEY_ENCODER_AUDITED: bool = false;
    /// Permits replay of pure deterministic Rust key ordering comparisons.
    ///
    /// A handwritten true value is a trusted implementer contract, in the same
    /// class as derive projection glue: pure deterministic measurement, exact
    /// encoding into the measured destination, schema-valid output and replay.
    /// It is not a proof about that implementation. A violation may cause
    /// errors, poison or wrong data in that program's own accounts, never
    /// undefined behavior: all library paths remain safe Rust.
    /// `Ord` must agree with canonical key bytes. Built-ins are true; default
    /// false propagates to containing maps before any new key hook runs.
    #[doc(hidden)]
    const __ORDER_REUSABLE: bool = false;
    /// Stable key codec contract for automatic account identities.
    ///
    /// Handwritten keys must supply a `WireKey` with an explicit codec namespace
    /// and epoch. The default rejects automatic account identity generation.
    const KEY_DESCRIPTOR: Option<crate::identity::WireKey> = None;
    /// Width and canonical byte domain used by the core map validator.
    const KEY_LAYOUT: KeyLayout;
    /// Owned canonical key bytes, produced without allocating.
    type KeyBytes: AsRef<[u8]>;

    /// Encodes a key so lexicographic comparison agrees with [`Ord`].
    fn encode_key(self) -> Self::KeyBytes;

    /// Decodes an exact canonical key region, with [`Fixed::decode`] errors.
    fn decode_key(bytes: &[u8]) -> Result<Self>;
}

fn exact_array<const N: usize>(bytes: &[u8]) -> Result<[u8; N]> {
    bytes.try_into().map_err(|_| {
        let (kind, offset) = if bytes.len() < N {
            (ErrorKind::Truncated, bytes.len())
        } else {
            (ErrorKind::InvalidBoundary, N)
        };
        Error::new(kind, u32::try_from(offset).unwrap_or(Error::NO_OFFSET))
    })
}

macro_rules! integer {
    ($unsigned:ty, $signed:ty, $size:literal) => {
        impl Fixed for $unsigned {
            const __ENCODER_AUDITED: bool = true;
            const SIZE: u32 = $size;
            type Encoded = [u8; $size];

            fn decode(bytes: &[u8]) -> Result<Self> {
                Ok(Self::from_le_bytes(exact_array(bytes)?))
            }

            #[inline(always)]
            fn __decode_region(bytes: &[u8]) -> Result<Self> {
                Ok(Self::from_le_bytes(exact_array(bytes)?))
            }

            #[inline(always)]
            fn encode(self) -> Self::Encoded {
                blubber_core::encoding_leaf::array_bytes(self.to_le_bytes())
            }
        }

        impl MapKey for $unsigned {
            const __KEY_ENCODER_AUDITED: bool = true;
            const __ORDER_REUSABLE: bool = true;
            const KEY_DESCRIPTOR: Option<crate::identity::WireKey> =
                Some(crate::identity::WireKey {
                    layout: Self::KEY_LAYOUT,
                    codec: None,
                });
            const KEY_LAYOUT: KeyLayout = KeyLayout {
                width: Self::SIZE,
                domain: KeyDomain::Unsigned,
            };
            type KeyBytes = [u8; $size];

            #[inline(always)]
            fn encode_key(self) -> Self::KeyBytes {
                blubber_core::encoding_leaf::array_bytes(self.to_be_bytes())
            }

            #[inline(always)]
            fn decode_key(bytes: &[u8]) -> Result<Self> {
                Ok(Self::from_be_bytes(exact_array(bytes)?))
            }
        }

        impl Fixed for $signed {
            const __ENCODER_AUDITED: bool = true;
            const SIZE: u32 = $size;
            type Encoded = [u8; $size];

            fn decode(bytes: &[u8]) -> Result<Self> {
                Ok(Self::from_le_bytes(exact_array(bytes)?))
            }

            #[inline(always)]
            fn __decode_region(bytes: &[u8]) -> Result<Self> {
                Ok(Self::from_le_bytes(exact_array(bytes)?))
            }

            #[inline(always)]
            fn encode(self) -> Self::Encoded {
                blubber_core::encoding_leaf::array_bytes(self.to_le_bytes())
            }
        }

        impl MapKey for $signed {
            const __KEY_ENCODER_AUDITED: bool = true;
            const __ORDER_REUSABLE: bool = true;
            const KEY_DESCRIPTOR: Option<crate::identity::WireKey> =
                Some(crate::identity::WireKey {
                    layout: Self::KEY_LAYOUT,
                    codec: None,
                });
            const KEY_LAYOUT: KeyLayout = KeyLayout {
                width: Self::SIZE,
                domain: KeyDomain::Signed,
            };
            type KeyBytes = [u8; $size];

            #[inline(always)]
            fn encode_key(self) -> Self::KeyBytes {
                let mut bytes = self.to_be_bytes();
                bytes[0] ^= 0x80;
                bytes
            }

            #[inline(always)]
            fn decode_key(bytes: &[u8]) -> Result<Self> {
                let mut bytes = exact_array::<$size>(bytes)?;
                bytes[0] ^= 0x80;
                Ok(Self::from_be_bytes(bytes))
            }
        }
    };
}

integer!(u8, i8, 1);
integer!(u16, i16, 2);
integer!(u32, i32, 4);
integer!(u64, i64, 8);
integer!(u128, i128, 16);

impl Fixed for bool {
    const __ENCODER_AUDITED: bool = true;
    const SIZE: u32 = 1;
    type Encoded = [u8; 1];

    fn decode(bytes: &[u8]) -> Result<Self> {
        match exact_array::<1>(bytes)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::new(ErrorKind::InvalidBool, 0)),
        }
    }

    #[inline(always)]
    fn encode(self) -> Self::Encoded {
        blubber_core::encoding_leaf::bool_bytes(self)
    }
}

impl MapKey for bool {
    const __KEY_ENCODER_AUDITED: bool = true;
    const __ORDER_REUSABLE: bool = true;
    const KEY_DESCRIPTOR: Option<crate::identity::WireKey> = Some(crate::identity::WireKey {
        layout: Self::KEY_LAYOUT,
        codec: None,
    });
    const KEY_LAYOUT: KeyLayout = KeyLayout {
        width: Self::SIZE,
        domain: KeyDomain::Bool,
    };
    type KeyBytes = [u8; 1];

    #[inline(always)]
    fn encode_key(self) -> Self::KeyBytes {
        self.encode()
    }

    #[inline(always)]
    fn decode_key(bytes: &[u8]) -> Result<Self> {
        Self::decode(bytes)
    }
}

impl<const N: usize> Fixed for [u8; N] {
    const __ENCODER_AUDITED: bool = true;
    const SIZE: u32 = {
        assert!(N > 0, "fixed arrays must have positive width");
        assert!(N <= u32::MAX as usize, "fixed array width exceeds u32");
        N as u32
    };
    type Encoded = Self;

    fn decode(bytes: &[u8]) -> Result<Self> {
        let _ = Self::SIZE;
        exact_array(bytes)
    }

    #[inline(always)]
    fn encode(self) -> Self::Encoded {
        let _ = Self::SIZE;
        blubber_core::encoding_leaf::array_bytes(self)
    }
}

impl<const N: usize> MapKey for [u8; N] {
    const __KEY_ENCODER_AUDITED: bool = true;
    const __ORDER_REUSABLE: bool = true;
    const KEY_DESCRIPTOR: Option<crate::identity::WireKey> = Some(crate::identity::WireKey {
        layout: Self::KEY_LAYOUT,
        codec: None,
    });
    const KEY_LAYOUT: KeyLayout = KeyLayout {
        width: Self::SIZE,
        domain: KeyDomain::Raw,
    };
    type KeyBytes = Self;

    #[inline(always)]
    fn encode_key(self) -> Self::KeyBytes {
        self.encode()
    }

    #[inline(always)]
    fn decode_key(bytes: &[u8]) -> Result<Self> {
        Self::decode(bytes)
    }
}

/// An SDK-independent public key, ordered lexicographically by its 32 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pubkey([u8; 32]);

impl Pubkey {
    /// Constructs a key from its complete raw encoding.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the complete raw encoding.
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl AsRef<[u8]> for Pubkey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Fixed for Pubkey {
    const __ENCODER_AUDITED: bool = true;
    const SIZE: u32 = 32;
    type Encoded = [u8; 32];

    #[inline(always)]
    fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(Self(exact_array(bytes)?))
    }

    #[inline(always)]
    fn encode(self) -> Self::Encoded {
        blubber_core::encoding_leaf::array_bytes(self.0)
    }
}

impl MapKey for Pubkey {
    const __KEY_ENCODER_AUDITED: bool = true;
    const __ORDER_REUSABLE: bool = true;
    const KEY_DESCRIPTOR: Option<crate::identity::WireKey> = Some(crate::identity::WireKey {
        layout: Self::KEY_LAYOUT,
        codec: None,
    });
    const KEY_LAYOUT: KeyLayout = KeyLayout {
        width: Self::SIZE,
        domain: KeyDomain::Raw,
    };
    type KeyBytes = [u8; 32];

    #[inline(always)]
    fn encode_key(self) -> Self::KeyBytes {
        blubber_core::encoding_leaf::array_bytes(self.0)
    }

    #[inline(always)]
    fn decode_key(bytes: &[u8]) -> Result<Self> {
        Self::decode(bytes)
    }
}

#[cfg(feature = "pinocchio")]
impl From<::pinocchio::Address> for Pubkey {
    fn from(address: ::pinocchio::Address) -> Self {
        Self(address.to_bytes())
    }
}

#[cfg(feature = "pinocchio")]
impl From<Pubkey> for ::pinocchio::Address {
    fn from(key: Pubkey) -> Self {
        Self::new_from_array(key.0)
    }
}

#[cfg(feature = "pinocchio")]
impl Fixed for ::pinocchio::Address {
    const __ENCODER_AUDITED: bool = true;
    const SIZE: u32 = 32;
    type Encoded = [u8; 32];

    #[inline(always)]
    fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(Self::new_from_array(exact_array(bytes)?))
    }

    #[inline(always)]
    fn encode(self) -> Self::Encoded {
        blubber_core::encoding_leaf::array_bytes(self.to_bytes())
    }
}

#[cfg(feature = "pinocchio")]
impl MapKey for ::pinocchio::Address {
    const __KEY_ENCODER_AUDITED: bool = true;
    const __ORDER_REUSABLE: bool = true;
    const KEY_DESCRIPTOR: Option<crate::identity::WireKey> = Some(crate::identity::WireKey {
        layout: Self::KEY_LAYOUT,
        codec: None,
    });
    const KEY_LAYOUT: KeyLayout = <Pubkey as MapKey>::KEY_LAYOUT;
    type KeyBytes = [u8; 32];

    #[inline(always)]
    fn encode_key(self) -> Self::KeyBytes {
        blubber_core::encoding_leaf::array_bytes(self.to_bytes())
    }

    #[inline(always)]
    fn decode_key(bytes: &[u8]) -> Result<Self> {
        Self::decode(bytes)
    }
}
