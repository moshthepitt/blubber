//! Runtime-only checks for account guards, discovery and resize semantics.

#![cfg_attr(target_os = "solana", no_std)]
#![forbid(unsafe_code)]

use blubber::raw::Resize as _;
use blubber::{
    pinocchio::{Funding, Headroom, ReadStore, Store},
    ErrorKind, ReadStorage, Resize,
};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult, Resize as SdkResize};

#[cfg(target_os = "solana")]
pinocchio::program_entrypoint!(process_instruction, 4);
#[cfg(all(target_os = "solana", not(feature = "alloc")))]
pinocchio::no_allocator!();
#[cfg(all(target_os = "solana", feature = "alloc"))]
pinocchio::default_allocator!();
#[cfg(target_os = "solana")]
pinocchio::nostd_panic_handler!();

pub mod funding_preflight;

use pinocchio::account::MAX_PERMITTED_DATA_INCREASE as INCREASE;
const MAX: usize = 10_485_760;

#[blubber::schema(account, discriminator = [76, 69, 65, 83, 69, 48, 48, 49])]
struct Lease {
    value: u32,
}

/// Runs assertions against accounts supplied by the real SBF entrypoint.
pub fn process_instruction(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    if ix.first() == Some(&43) {
        return funding_preflight::run_initialize(id, accounts, ix);
    }
    if ix.first() == Some(&42) {
        return funding_preflight::run(id, accounts, ix);
    }
    #[cfg(feature = "runtime")]
    if ix.first() == Some(&80) {
        return boundaries::run(id, accounts, ix);
    }
    if ix.first() == Some(&70) {
        return scoped::run(id, accounts, ix);
    }
    if matches!(ix.first(), Some(60 | 61)) {
        return tour::run(id, accounts, ix);
    }
    if ix.first() == Some(&50) {
        return initialize::run(id, accounts, ix);
    }
    if ix.first() == Some(&40) {
        return rent::run(id, accounts, ix);
    }
    #[cfg(feature = "alloc")]
    let opcode = *ix.first().ok_or(ProgramError::InvalidInstructionData)?;
    #[cfg(feature = "alloc")]
    if (20..=31).contains(&opcode) || opcode == 34 {
        return on_demand::run(id, accounts, opcode);
    }
    d16(id, accounts, ix)
}

#[cfg(feature = "runtime")]
mod boundaries;

mod initialize;
mod rent;
mod scoped;
mod tour;

// Package-local copies keep rust-analyzer's module ownership unambiguous.
// The host tour contract checks both files against the demo byte for byte.
#[path = "tour_exercise.rs"]
mod exercise;
#[path = "tour_state.rs"]
mod state;

#[inline(never)]
fn d16(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let opcode = *ix.first().ok_or(ProgramError::InvalidInstructionData)?;
    if opcode == 16 {
        let (first, rest) = accounts.split_at_mut(1);
        let one = &mut first[0];
        let two = rest.first_mut().ok_or(ProgramError::NotEnoughAccountKeys)?;
        assert_eq!(one.address(), two.address());
        let original = one.data_len();
        let read = ReadStore::new(one, id)?;
        assert!(
            matches!(Store::new(two, id, Funding::Prepaid), Err(e) if e == ErrorKind::BorrowConflict.into())
        );
        assert_eq!(two.data_len(), original);
        drop(read);
        let mut write = Store::new(one, id, Funding::Prepaid)?;
        assert!(matches!(ReadStore::new(two, id), Err(e) if e == ErrorKind::BorrowConflict.into()));
        assert!(
            matches!(Store::new(two, id, Funding::Prepaid), Err(e) if e == ErrorKind::BorrowConflict.into())
        );
        write.commit(original as u32)?;
        return Ok(());
    }
    let account = accounts
        .first_mut()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    // These raw lease controls require Max to test probe windows, spare retention,
    // reopens and Drop after speculative growth. Default edits are tested in scoped.
    let original = account.data_len();
    match opcode {
        19 => {
            let target = if ix.get(1) == Some(&0) {
                original + 501
            } else {
                original - 37
            };
            account.resize(target)?;
        }
        32 | 33 => {
            use pinocchio::instruction::{InstructionAccount, InstructionView};
            let direction = u8::from(opcode == 33);
            pinocchio::cpi::invoke(
                &InstructionView {
                    program_id: id,
                    accounts: &[InstructionAccount::writable(account.address())],
                    data: &[19, direction],
                },
                &[&*account],
            )?;
            let current = if opcode == 32 {
                original + 501
            } else {
                original - 37
            };
            assert_eq!(account.data_len(), current);
            let mut store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            assert_eq!(
                store.growth_limit(),
                if opcode == 32 {
                    original + INCREASE
                } else {
                    current + INCREASE
                }
            );
            assert_eq!(
                store.resize(store.growth_limit() + 1),
                Err(ErrorKind::GrowthLimit.into())
            );
            store.commit(original as u32)?;
        }
        0 => {
            let mut store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            let expected = (original + INCREASE).min(MAX);
            assert_eq!(store.storage_len(), expected);
            assert_eq!(store.growth_limit(), expected);
            assert!(store.bytes()[original..].iter().all(|b| *b == 0));
            let old = store.bytes()[12];
            store.bytes_mut()[12] = old + 1;
            store.resize(original - 1)?;
            assert_eq!(store.storage_len(), expected);
            let beyond = if expected == MAX {
                ErrorKind::Capacity
            } else {
                ErrorKind::GrowthLimit
            };
            assert_eq!(store.check_resize(expected + 1), Err(beyond.into()));
            let receipt = store.commit(original as u32)?;
            assert_eq!(receipt.storage_len as usize, original);
            assert_eq!(receipt.shrink_error, None);
            assert!(store.is_closed());
            assert_eq!(store.commit(original as u32), Err(ErrorKind::Closed.into()));
            drop(store);
            assert_eq!(account.data_len(), original);
            assert_eq!(account.try_borrow()?[12], old + 1);
        }
        1 => {
            let mut first = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            first.bytes_mut()[12] = 99;
            first.commit((original + 317) as u32)?;
            drop(first);
            let mut second =
                Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            assert_eq!(second.growth_limit(), original + INCREASE);
            assert_eq!(second.bytes()[12], 99);
            assert_eq!(
                second.check_resize(original + INCREASE + 1),
                Err(ErrorKind::GrowthLimit.into())
            );
            second.commit((original + INCREASE) as u32)?;
            drop(second);
            // No remaining growth: all search probes fail and current is kept.
            let mut third = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            assert_eq!(third.growth_limit(), original + INCREASE);
            third.commit(original as u32)?;
        }
        2 => {
            let shrunk = original - 37;
            let mut first = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            first.commit(shrunk as u32)?;
            drop(first);
            let mut second =
                Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            assert_eq!(second.growth_limit(), shrunk + INCREASE);
            assert_eq!(
                second.check_resize(shrunk + INCREASE + 1),
                Err(ErrorKind::GrowthLimit.into())
            );
            second.commit(shrunk as u32)?;
            drop(second);
            // Demonstrate the SDK itself permits the larger original window.
            account.resize(original + INCREASE)?;
            account.resize(shrunk)?;
        }
        3 => {
            let n = u32::from_le_bytes(
                ix.get(1..5)
                    .ok_or(ProgramError::InvalidInstructionData)?
                    .try_into()
                    .unwrap(),
            ) as usize;
            let mut store =
                Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::UpTo(n))?;
            let target = (original + n.min(INCREASE)).min(MAX);
            assert_eq!(store.storage_len(), target);
            assert_eq!(store.growth_limit(), target);
            assert!(store.storage_len() - original <= n);
            let kind = if n < INCREASE || target == MAX {
                ErrorKind::Capacity
            } else {
                ErrorKind::GrowthLimit
            };
            assert_eq!(store.check_resize(target + 1), Err(kind.into()));
            store.commit(original as u32)?;
        }
        4 => {
            let mut store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            store.reserve(original as u32, 73)?;
            assert_eq!(store.storage_len(), original + INCREASE);
            let receipt = store.commit(original as u32)?;
            assert_eq!(receipt.storage_len as usize, original + 73);
            drop(store);
            let mut store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            store.reserve(original as u32, 81)?;
            assert!(store.shrink_to_fit(original as u32)?);
            assert_eq!(
                store.commit(original as u32)?.storage_len as usize,
                original
            );
        }
        5 | 17 | 18 => {
            let mut store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            if opcode != 5 {
                let used = if opcode == 17 { 0u32 } else { 11u32 };
                store.bytes_mut()[8..12].copy_from_slice(&used.to_le_bytes());
                drop(store);
                assert_eq!(account.data_len(), original + INCREASE);
                assert!(!account.is_borrowed());
                account.resize(original)?;
                return Ok(());
            }
            store.bytes_mut()[12] = 42;
            drop(store);
            assert_eq!(account.data_len(), original);
            assert_eq!(account.try_borrow()?[12], 42);
            assert!(!account.is_borrowed());
        }
        6 => {
            // Invalid envelope: guard still drops, physical trim is skipped.
            let mut store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            store.bytes_mut()[8..12].fill(0xff);
            drop(store);
            assert_eq!(account.data_len(), original + INCREASE);
            assert!(!account.is_borrowed());
            account.resize(original)?;
        }
        7 => {
            let mut alias = *account;
            let store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            core::mem::forget(store);
            assert_eq!(alias.data_len(), original + INCREASE);
            assert!(alias.is_borrowed_mut());
            assert!(
                matches!(ReadStore::new(&alias, id), Err(e) if e == ErrorKind::BorrowConflict.into())
            );
            assert!(
                matches!(Store::new(&mut alias, id, Funding::Prepaid), Err(e) if e == ErrorKind::BorrowConflict.into())
            );
            assert_eq!(
                alias.resize(original),
                Err(ProgramError::AccountBorrowFailed)
            );
        }
        8 => {
            let mut alias = *account;
            let one = ReadStore::new(account, id)?;
            let two = ReadStore::new(&alias, id)?;
            assert_eq!(one.bytes().len(), original);
            assert_eq!(two.bytes(), one.bytes());
            drop(two);
            assert!(
                matches!(Store::new(&mut alias, id, Funding::Prepaid), Err(e) if e == ErrorKind::BorrowConflict.into())
            );
            assert_eq!(alias.data_len(), original);
            drop(one);
            let mut store = Store::new(&mut alias, id, Funding::Prepaid)?;
            assert!(
                matches!(ReadStore::new(account, id), Err(e) if e == ErrorKind::BorrowConflict.into())
            );
            assert!(
                matches!(Store::new(account, id, Funding::Prepaid), Err(e) if e == ErrorKind::BorrowConflict.into())
            );
            store.commit(original as u32)?;
        }
        9 => {
            // Pinocchio semantic canary, using only genuine runtime memory.
            account.resize(original + INCREASE)?;
            account.try_borrow_mut()?.fill(0xa5);
            assert_eq!(
                account.resize(original + INCREASE + 1),
                Err(ProgramError::InvalidRealloc)
            );
            assert_eq!(account.data_len(), original + INCREASE);
            assert!(account.try_borrow()?.iter().all(|b| *b == 0xa5));
            account.resize(original)?;
        }
        10 => {
            let mut store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            // Exercise the generated root through the sealed adapter.
            let mut root = Lease::load_mut(&mut store)?;
            root.value_mut().set(77)?;
            root.commit()?;
        }
        11 => {
            let mut alias = *account;
            let mut store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            store.set_poisoned(true);
            assert_eq!(
                store.check_resize(original),
                Err(ErrorKind::Poisoned.into())
            );
            assert_eq!(
                store.commit(original as u32),
                Err(ErrorKind::Poisoned.into())
            );
            assert!(store.is_closed());
            assert!(!alias.is_borrowed());
            alias.resize(original)?;
        }
        12 => {
            let mut alias = *account;
            let mut store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
            assert_eq!(
                store.commit((original + INCREASE + 1) as u32),
                Err(ErrorKind::UsedLength.into())
            );
            assert!(store.is_closed());
            assert!(!alias.is_borrowed());
            alias.resize(original)?;
        }
        13 => {
            let _store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::Max)?;
        }
        14 => {
            let _store = ReadStore::new(account, id)?;
        }
        15 => {
            account.resize(original + 501)?;
            let mut store =
                Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::UpTo(INCREASE))?;
            assert_eq!(store.growth_limit(), original + INCREASE);
            assert!(store.storage_len() - (original + 501) <= INCREASE);
            assert_eq!(
                store.check_resize(original + INCREASE + 1),
                Err(ErrorKind::GrowthLimit.into())
            );
            store.commit(original as u32)?;
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    Ok(())
}

#[cfg(feature = "alloc")]
mod on_demand;
