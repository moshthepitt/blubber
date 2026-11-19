use blubber::{List, Map, Pubkey, Str};

/// Native limits encoded without Rust padding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, blubber::Fixed)]
pub struct Limits {
    /// Whether edits are enabled.
    pub enabled: bool,
    /// Application item limit.
    pub limit: u16,
}

/// Named record with numeric tags.
#[blubber::schema]
pub struct Record {
    /// UTF-8 record name.
    pub name: Str,
    /// Numeric record tags.
    pub tags: List<u32>,
}

/// The complete format-1 tour account.
#[blubber::schema(account)]
pub struct MyAccount {
    /// Authority allowed to run the exercise.
    pub authority: Pubkey,
    /// Example counter.
    pub counter: u64,
    /// Variable-size records.
    pub records: List<Record>,
    /// Balances ordered by public key.
    pub balances: Map<Pubkey, u64>,
    /// Labels ordered by numeric key.
    pub labels: Map<u16, Str>,
    /// Uncounted UTF-8 note.
    pub note: Str,
    /// Counted binary attachment.
    pub attachment: List<u8>,
    /// Optional UTF-8 memo.
    pub memo: Option<Str>,
    /// Copied native limits.
    pub limits: Limits,
}

/// Borrows static inputs and copies the account's stored `authority`.
pub fn seed(authority: Pubkey) -> MyAccountInit<'static> {
    const RECORDS: &[RecordInit<'static>] = &[
        RecordInit {
            name: "one",
            tags: &[1],
        },
        RecordInit {
            name: "two",
            tags: &[2, 3],
        },
    ];
    MyAccountInit {
        authority,
        counter: 0,
        records: RECORDS,
        balances: &[],
        labels: &[(1, "first")],
        note: "ready",
        attachment: &[],
        memo: None,
        limits: Limits {
            enabled: true,
            limit: 100,
        },
    }
}
