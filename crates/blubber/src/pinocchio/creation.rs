//! Creation CPIs run before acquiring a Blubber data lease.

use super::{check_funding, check_payer, rent, Funding, MAX_INCREASE, MAX_STORAGE};
use crate::{ErrorKind, Result};
use ::pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, Address,
};
use pinocchio_system::instructions::{Allocate, Assign, CreateAccount, Transfer};

/// Creates an unfunded, empty System account signed by the transaction.
///
/// Both state and payer must be writable signers with no outstanding borrows.
/// The payer must be a distinct, non-executable, empty System account. Checks
/// the executable System Program, actual rent and payer balance before CPI.
/// Requests above 10 MiB return `Capacity`; above 10,240 return `GrowthLimit`.
/// Initialize the borrowed value only after this call succeeds. Propagate any
/// later failure on-chain to roll back creation; there is no host transaction.
pub fn create_account(
    state: &mut AccountView,
    payer: &AccountView,
    system_program: &AccountView,
    owner: &Address,
    space: u32,
) -> Result<()> {
    preflight(state, payer, system_program, space, false)?;
    if !state.is_signer() {
        return Err(ProgramError::MissingRequiredSignature.into());
    }
    let lamports = funding_amount(state, payer, space)?;
    CreateAccount {
        from: payer,
        to: state,
        lamports,
        space: u64::from(space),
        owner,
    }
    .invoke()?;
    Ok(())
}

/// Creates an unfunded, empty System PDA using the complete seeds and bump.
///
/// Derives the address against `owner` before any CPI and rejects a mismatch
/// with `InvalidSeeds`. `owner` must be the calling program: the runtime grants
/// PDA signatures only to that program. Other checks and growth bounds match
/// [`create_account`]; state need not be a transaction signer.
pub fn create_pda(
    state: &mut AccountView,
    payer: &AccountView,
    system_program: &AccountView,
    owner: &Address,
    space: u32,
    seeds_with_bump: &[&[u8]],
) -> Result<()> {
    preflight(state, payer, system_program, space, false)?;
    let seeds = signer_seeds(state, owner, seeds_with_bump)?;
    let signer = Signer::from(&seeds[..seeds_with_bump.len()]);
    let lamports = funding_amount(state, payer, space)?;
    CreateAccount {
        from: payer,
        to: state,
        lamports,
        space: u64::from(space),
        owner,
    }
    .invoke_signed(&[signer])?;
    Ok(())
}

/// Funds, allocates and assigns a prefunded transaction signer.
///
/// Retains existing lamports and transfers only the rent deficit. Checks the
/// writable empty System account, signature and funding before the first CPI.
/// Propagate errors on-chain to roll back the complete CPI sequence.
pub fn allocate_account(
    state: &mut AccountView,
    payer: &AccountView,
    system_program: &AccountView,
    owner: &Address,
    space: u32,
) -> Result<()> {
    preflight(state, payer, system_program, space, true)?;
    if !state.is_signer() {
        return Err(ProgramError::MissingRequiredSignature.into());
    }
    let deficit = funding_amount(state, payer, space)?;
    if deficit != 0 {
        Transfer {
            from: payer,
            to: state,
            lamports: deficit,
        }
        .invoke()?;
    }
    Allocate {
        account: state,
        space: u64::from(space),
    }
    .invoke()?;
    Assign {
        account: state,
        owner,
    }
    .invoke()?;
    Ok(())
}

/// Funds, allocates and assigns an empty System PDA, retaining existing funds.
///
/// Checks the complete seed group, all accounts, bounds and the net rent
/// deficit before the first CPI. Transfers only a positive deficit, then
/// invokes signed Allocate and Assign. Data-bearing or program-owned accounts
/// return `AlreadyInitialized`. Other preconditions match [`create_pda`].
/// Propagate errors on-chain for transaction rollback across the CPI sequence.
pub fn allocate_pda(
    state: &mut AccountView,
    payer: &AccountView,
    system_program: &AccountView,
    owner: &Address,
    space: u32,
    seeds_with_bump: &[&[u8]],
) -> Result<()> {
    preflight(state, payer, system_program, space, true)?;
    let seeds = signer_seeds(state, owner, seeds_with_bump)?;
    let signer = Signer::from(&seeds[..seeds_with_bump.len()]);
    let deficit = funding_amount(state, payer, space)?;
    if deficit != 0 {
        Transfer {
            from: payer,
            to: state,
            lamports: deficit,
        }
        .invoke()?;
    }
    Allocate {
        account: state,
        space: u64::from(space),
    }
    .invoke_signed(core::slice::from_ref(&signer))?;
    Assign {
        account: state,
        owner,
    }
    .invoke_signed(&[signer])?;
    Ok(())
}

fn preflight(
    state: &AccountView,
    payer: &AccountView,
    system_program: &AccountView,
    space: u32,
    allow_funded: bool,
) -> Result<()> {
    check_funding(
        state,
        Funding::TopUp {
            payer,
            system_program,
        },
    )?;
    preflight_accounts(state, payer, space, allow_funded)
}

// Shared preflight for callers supplying the allocation instruction.
/// Internal checked account operation for runtime adapters.
#[doc(hidden)]
pub fn preflight_accounts(
    state: &AccountView,
    payer: &AccountView,
    space: u32,
    allow_funded: bool,
) -> Result<()> {
    check_payer(state, payer)?;
    if state.address() == &pinocchio_system::ID {
        return Err(ErrorKind::DuplicateAccount.into());
    }
    if !state.is_writable() {
        return Err(ErrorKind::NotWritable.into());
    }
    if state.executable() {
        return Err(ErrorKind::Unauthorized.into());
    }
    state
        .check_borrow_mut()
        .map_err(|_| ErrorKind::BorrowConflict)?;
    if !state.owned_by(&pinocchio_system::ID)
        || !state.is_data_empty()
        || (!allow_funded && state.lamports() != 0)
    {
        return Err(ErrorKind::AlreadyInitialized.into());
    }
    creation_space(space)?;
    Ok(())
}

fn creation_space(space: u32) -> Result<()> {
    if space as usize > MAX_STORAGE {
        return Err(ErrorKind::Capacity.into());
    }
    // The safe SDK exposes no saved-original-length getter. Empty System
    // accounts always have at least this caller reservation; never probe or
    // resize a System-owned account to discover a larger one before CPI.
    if space as usize > MAX_INCREASE {
        return Err(ErrorKind::GrowthLimit.into());
    }
    Ok(())
}

/// Internal checked account operation for runtime adapters.
#[doc(hidden)]
pub fn funding_amount(state: &AccountView, payer: &AccountView, space: u32) -> Result<u64> {
    let minimum = rent::minimum_balance(space as usize)?;
    let deficit = minimum.saturating_sub(state.lamports());
    payer
        .lamports()
        .checked_sub(deficit)
        .ok_or(ErrorKind::RentShortfall)?;
    Ok(deficit)
}

/// Internal checked account operation for runtime adapters.
#[doc(hidden)]
pub fn signer_seeds<'a>(
    state: &AccountView,
    owner: &Address,
    seeds: &[&'a [u8]],
) -> Result<[Seed<'a>; 16]> {
    let address =
        Address::create_program_address(seeds, owner).map_err(|_| ProgramError::InvalidSeeds)?;
    if &address != state.address() {
        return Err(ProgramError::InvalidSeeds.into());
    }
    // Address derivation above checks the SDK's 16-seed/32-byte limits.
    let mut signer = core::array::from_fn(|_| Seed::from(&[][..]));
    for (out, bytes) in signer.iter_mut().zip(seeds) {
        *out = Seed::from(*bytes);
    }
    Ok(signer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_limits_check_the_absolute_cap_before_the_cpi_window() {
        for size in [0, 12, MAX_INCREASE] {
            assert_eq!(creation_space(size as u32), Ok(()));
        }
        for size in [MAX_INCREASE + 1, MAX_STORAGE] {
            assert_eq!(
                creation_space(size as u32),
                Err(ErrorKind::GrowthLimit.into())
            );
        }
        for size in [MAX_STORAGE + 1, u32::MAX as usize] {
            assert_eq!(creation_space(size as u32), Err(ErrorKind::Capacity.into()));
        }
    }
}
