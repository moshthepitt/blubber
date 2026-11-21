//! Account declarations shared by the SBF fixture and host acceptance checks.
use blubber::{Address, List, Map, Str};

/// Fixed scalar fixture.
#[blubber::schema(account)]
pub struct Number {
    /// Scalar edited in place.
    pub value: u64,
}
/// Raw UTF-8 fixture.
#[blubber::schema(account)]
pub struct Text {
    /// Parent-delimited string.
    pub value: Str,
}
/// Fixed-stride list fixture.
#[blubber::schema(account)]
pub struct Numbers {
    /// Counted fixed-stride values.
    pub value: List<u64>,
}
/// Public-key map fixture.
#[blubber::schema(account)]
pub struct PublicKeyMap {
    /// Sorted fixed-value entries.
    pub value: Map<Address, u64>,
}
/// SDK Address map fixture.
#[blubber::schema(account)]
pub struct AddressMap {
    /// Sorted fixed-value entries.
    pub value: Map<pinocchio::Address, u64>,
}
// Keep the original acceptance program's monomorphization set unchanged.
#[cfg(any(feature = "matrix", not(target_os = "solana")))]
mod scaling {
    use super::*;
    /// Every built-in fixed codec, packed independently of native padding.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, blubber::Fixed)]
    pub struct ScalarValues {
        /// Stored u8 value.
        pub small: u8,
        /// Stored u16 value.
        pub medium: u16,
        /// Stored u32 value.
        pub word: u32,
        /// Stored u64 value.
        pub wide: u64,
        /// Stored u128 value.
        pub huge: u128,
        /// Stored i8 value.
        pub signed_small: i8,
        /// Stored i16 value.
        pub signed_medium: i16,
        /// Stored i32 value.
        pub signed_word: i32,
        /// Stored i64 value.
        pub signed_wide: i64,
        /// Stored i128 value.
        pub signed_huge: i128,
        /// Stored bool value.
        pub enabled: bool,
        /// Stored [u8; 17] value.
        pub raw: [u8; 17],
        /// Stored Address value.
        pub key: Address,
        /// Stored pinocchio::Address value.
        pub address: pinocchio::Address,
    }
    /// All scalar setters and fixed-option projection fixture.
    #[blubber::schema(account)]
    pub struct ScalarKinds {
        /// Packed scalar values.
        pub value: ScalarValues,
        /// Optional fixed-width value.
        pub fixed_option: Option<u32>,
    }
    /// Indexed-list scaling fixture.
    #[blubber::schema(account)]
    pub struct Words {
        /// Parent-delimited UTF-8 elements.
        pub value: List<Str>,
    }
    /// Indexed-value map scaling fixture.
    #[blubber::schema(account)]
    pub struct KeyWords {
        /// Raw public keys and indexed UTF-8 values.
        pub value: Map<Address, Str>,
    }
    /// Address-keyed indexed-value map scaling fixture.
    #[blubber::schema(account)]
    pub struct AddressWords {
        /// SDK keys with indexed UTF-8 values.
        pub value: Map<pinocchio::Address, Str>,
    }
    /// Four-node ancestry fixture, including the account and string.
    #[blubber::schema(account)]
    pub struct Depth4 {
        /// Binary indexed lists leading to the edited string.
        pub value: List<List<Str>>,
        /// Unchanged outer suffix.
        pub tail: Str,
    }
    /// Four indexed-list layers over UTF-8.
    pub type FourLists = List<List<List<List<Str>>>>;

    /// Eight-node ancestry fixture, including the account and string.
    #[blubber::schema(account)]
    pub struct Depth8 {
        /// Six binary lists leading to the edited string.
        pub value: List<List<FourLists>>,
        /// Unchanged outer suffix.
        pub tail: Str,
    }
    /// Option mutation scaling fixture.
    #[blubber::schema(account)]
    pub struct MaybeText {
        /// Present UTF-8 string.
        pub value: Option<Str>,
    }
}
#[cfg(any(feature = "matrix", not(target_os = "solana")))]
pub use scaling::*;
