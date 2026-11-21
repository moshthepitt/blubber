//! Creation CPIs run before acquiring a Blubber data lease.

use super::{check_funding, minimum_balance, Funding, MAX_PERMITTED_DATA_INCREASE, MAX_STORAGE};
use crate::{ErrorKind, Result};
use ::pinocchio::{
    address::{address_eq, MAX_SEEDS},
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, Address,
};
use pinocchio_system::instructions::{
    CreateAccount, CreateAccountAllowPrefund, Funding as SystemFunding,
};

/// Creates the system account before the generated writer acquires a data lease.
#[doc(hidden)]
#[inline(never)]
pub fn create(
    state: &AccountView,
    payer: &AccountView,
    system_program: &AccountView,
    owner: &Address,
    space: u32,
    prefunded: bool,
    seeds_with_bump: Option<&[&[u8]]>,
) -> Result<()> {
    preflight(state, payer, system_program, space, prefunded)?;
    let seeds;
    let signer;
    let signers = if let Some(group) = seeds_with_bump {
        seeds = signer_seeds(state, owner, group)?;
        signer = [Signer::from(&seeds[..group.len()])];
        &signer[..]
    } else {
        if !state.is_signer() {
            return Err(ProgramError::MissingRequiredSignature.into());
        }
        &[]
    };
    let lamports = funding_amount(state, payer, space)?;
    if prefunded {
        CreateAccountAllowPrefund {
            to: state,
            owner,
            space: u64::from(space),
            funding: (lamports != 0).then_some(SystemFunding {
                from: payer,
                lamports,
            }),
        }
        .invoke_signed(signers)?;
    } else {
        CreateAccount {
            from: payer,
            to: state,
            lamports,
            space: u64::from(space),
            owner,
        }
        .invoke_signed(signers)?;
    }
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
    preflight_accounts(state, space, allow_funded)
}

fn preflight_accounts(state: &AccountView, space: u32, allow_funded: bool) -> Result<()> {
    if address_eq(state.address(), &pinocchio_system::ID) {
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
    if space as usize > MAX_PERMITTED_DATA_INCREASE {
        return Err(ErrorKind::GrowthLimit.into());
    }
    Ok(())
}

fn funding_amount(state: &AccountView, payer: &AccountView, space: u32) -> Result<u64> {
    let minimum = minimum_balance(space as usize)?;
    let deficit = minimum.saturating_sub(state.lamports());
    payer
        .lamports()
        .checked_sub(deficit)
        .ok_or(ErrorKind::RentShortfall)?;
    Ok(deficit)
}

fn signer_seeds<'a>(
    state: &AccountView,
    owner: &Address,
    seeds: &[&'a [u8]],
) -> Result<[Seed<'a>; MAX_SEEDS]> {
    let address =
        Address::create_program_address(seeds, owner).map_err(|_| ProgramError::InvalidSeeds)?;
    if !address_eq(&address, state.address()) {
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
        for size in [0, 12, MAX_PERMITTED_DATA_INCREASE] {
            assert_eq!(creation_space(size as u32), Ok(()));
        }
        for size in [MAX_PERMITTED_DATA_INCREASE + 1, MAX_STORAGE] {
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
