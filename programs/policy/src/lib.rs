//! W1/W2/W4 point access with matched wire layouts, keys and checksums.
#![cfg_attr(target_os = "solana", no_std)]
#![forbid(unsafe_code)]

use blubber::{pinocchio::ReadStore, List, Map, Pubkey, Str};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

/// Bounded corrupt histories, compiled only for the dedicated T7 fixture.
#[cfg(feature = "corrupt")]
pub mod corrupt;

/// Fixed-list workload from the comparison fixture.
#[blubber::schema(account)]
pub struct Numbers {
    /// Counted integers.
    pub value: List<u64>,
}
/// Fixed-map workload from the comparison fixture.
#[blubber::schema(account)]
pub struct PublicKeyMap {
    /// Sorted raw keys and copied values.
    pub value: Map<Pubkey, u64>,
}
/// Indexed-map workload from the comparison fixture.
#[blubber::schema(account)]
pub struct KeyWords {
    /// Sorted raw keys and indexed eight-byte strings.
    pub value: Map<Pubkey, Str>,
}

#[cfg(all(target_os = "solana", not(feature = "corrupt")))]
pinocchio::program_entrypoint!(run, 1);
#[cfg(all(target_os = "solana", feature = "corrupt"))]
pinocchio::program_entrypoint!(run, 2);
#[cfg(target_os = "solana")]
pinocchio::no_allocator!();
#[cfg(target_os = "solana")]
pinocchio::nostd_panic_handler!();

// Keep the comparison's full-byte key and UTF-8 checksum work.
fn middle_key(n: usize) -> Pubkey {
    let mut key = [42; 32];
    key[..4].copy_from_slice(&(2 * (n / 2 + 1) as u32).to_be_bytes());
    Pubkey::from_bytes(key)
}

/// Runs first and repeated typed loads under one genuine shared guard.
///
/// Instruction `[policy, workload, operation, count_lo, count_hi]` uses policies
/// 1/2/13 for lazy/checked/raw-trusted; adding 20 or 40 repeats twice or eight
/// times. Operation zero admits only; W1 operation two reads the middle item;
/// W2/W4 operation one looks up that key. Totals include owner/guard/return work.
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    #[cfg(feature = "corrupt")]
    if let [0x77, rest @ ..] = ix {
        return corrupt::run(id, accounts, rest);
    }
    let [policy, workload, op, lo, hi] = *ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let n = u16::from_le_bytes([lo, hi]) as usize;
    if n > 1000 || account.owner() != id || !account.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }
    let (policy, repeat) = if policy >= 40 {
        (policy - 40, 8)
    } else if policy >= 20 {
        (policy - 20, 2)
    } else {
        (policy, 1)
    };
    if !matches!(policy, 1 | 2 | 13) {
        return Err(ProgramError::InvalidInstructionData);
    }
    macro_rules! repeats {
        ($policy:literal, $workload:literal) => {
            match repeat {
                1 => point::<$policy, 1, $workload>(id, account, op, n)?,
                2 => point::<$policy, 2, $workload>(id, account, op, n)?,
                8 => point::<$policy, 8, $workload>(id, account, op, n)?,
                _ => unreachable!(),
            }
        };
    }
    macro_rules! execute {
        ($workload:literal) => {
            match policy {
                1 => repeats!(1, $workload),
                2 => repeats!(2, $workload),
                13 => repeats!(13, $workload),
                _ => unreachable!(),
            }
        };
    }
    let checksum = match workload {
        1 => execute!(1),
        2 => execute!(2),
        4 => execute!(4),
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    pinocchio::cpi::set_return_data(&checksum.to_le_bytes());
    Ok(())
}

#[inline(always)]
fn point<const POLICY: u8, const REPEAT: usize, const WORKLOAD: u8>(
    id: &Address,
    account: &mut AccountView,
    op: u8,
    n: usize,
) -> Result<u64, ProgramError> {
    let store = ReadStore::new(account, id)?;
    let key = middle_key(n);
    let mut checksum = 0u64;
    macro_rules! load {
        ($schema:ty) => {
            match POLICY {
                1 => store.load::<$schema>()?,
                2 => store.load_checked::<$schema>()?,
                _ => <$schema>::load_trusted(store.bytes())?,
            }
        };
    }
    for _ in 0..REPEAT {
        checksum += match WORKLOAD {
            1 => {
                let root = load!(Numbers);
                if op == 0 {
                    0
                } else {
                    root.value().get(n / 2).unwrap_or(0)
                }
            }
            2 => {
                let root = load!(PublicKeyMap);
                if op == 0 {
                    0
                } else {
                    root.value().get(&key).unwrap_or(0)
                }
            }
            4 => {
                let root = load!(KeyWords);
                if op == 0 {
                    0
                } else {
                    root.value()
                        .get(&key)
                        .map(|s| s.bytes().map(u64::from).sum())
                        .unwrap_or(0)
                }
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        };
    }
    Ok(checksum)
}
