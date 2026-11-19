//! Complete format-1 tour schema and its borrowed golden input.

#![forbid(unsafe_code)]

use blubber::{List, Map, Pubkey, Str};

/// Native limits with a three-byte packed codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, blubber::Fixed)]
pub struct Limits {
    /// Whether edits are enabled.
    pub enabled: bool,
    /// Item limit.
    pub limit: u16,
}

/// A record whose first end is stored and last end is implicit.
#[blubber::schema]
pub struct Record {
    /// UTF-8 name.
    pub name: Str,
    /// Counted numeric tags.
    pub tags: List<u32>,
}

/// The account in SPEC.md's worked demo hex.
#[blubber::schema(account)]
pub struct MyAccount {
    /// Account authority.
    pub authority: Pubkey,
    /// Example counter.
    pub counter: u64,
    /// Variable records.
    pub records: List<Record>,
    /// Fixed map values.
    pub balances: Map<Pubkey, u64>,
    /// Variable map values.
    pub labels: Map<u16, Str>,
    /// Uncounted UTF-8.
    pub note: Str,
    /// Counted raw bytes.
    pub attachment: List<u8>,
    /// Optional UTF-8.
    pub memo: Option<Str>,
    /// Copied native limits.
    pub limits: Limits,
}

/// Borrowed fixture with references to promoted immutable arrays.
pub fn input() -> MyAccountInit<'static> {
    const INPUT: MyAccountInit<'static> = MyAccountInit {
        authority: Pubkey::from_bytes([0x11; 32]),
        counter: 1,
        records: &[RecordInit {
            name: "A",
            tags: &[7, 9],
        }],
        balances: &[(Pubkey::from_bytes([0x22; 32]), 5)],
        labels: &[(1, "x"), (256, "")],
        note: "ok",
        attachment: &[0xaa, 0xbb],
        memo: Some("m"),
        limits: Limits {
            enabled: true,
            limit: 2,
        },
    };
    INPUT
}

/// Parses the independently specified account bytes, including four spare bytes.
pub fn golden() -> Vec<u8> {
    include_str!("../fixtures/demo.hex")
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}
