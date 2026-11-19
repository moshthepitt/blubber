//! Published rc.2 accounts with scoped blubber views and rent-funded growth.
#![forbid(unsafe_code)]

use anchor_lang::prelude::*;
use blubber::{pinocchio::{EditOptions, Funding, Headroom}, List};

declare_id!("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");

#[blubber::schema(account)]
pub struct State {
    pub authority: Address,
    pub counter: u64,
    pub values: List<u64>,
}

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub state: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Modify {
    #[account(mut, owner = ID)]
    pub state: UncheckedAccount,
    pub authority: Signer,
}

#[derive(Accounts)]
pub struct Grow {
    #[account(mut, owner = ID)]
    pub state: UncheckedAccount,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Read {
    #[account(owner = ID)]
    pub state: UncheckedAccount,
}

#[program]
pub mod raw {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        let mut view = *ctx.accounts.state.account();
        State::create(
            &mut view,
            ctx.accounts.payer.account(),
            ctx.accounts.system_program.account(),
            &ID,
            StateInit {
                authority: *ctx.accounts.payer.account().address(),
                counter: 0,
                values: &[],
            },
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn push(ctx: &mut Context<Grow>, value: u64) -> Result<()> {
        let mut view = *ctx.accounts.state.account();
        State::with_edit_options(
            &mut view,
            &ID,
            // One fixed u64 append exceeds the exact creation allocation.
            EditOptions {
                funding: Funding::TopUp {
                    payer: ctx.accounts.payer.account(),
                    system_program: ctx.accounts.system_program.account(),
                },
                headroom: Headroom::UpTo(8),
                refund: None,
            },
            |state| {
                require!(
                    state.authority() == *ctx.accounts.payer.account().address(),
                    blubber::ErrorKind::Unauthorized
                );
                state.values_mut().push(value)
            },
        )?;
        Ok(())
    }

    #[discrim = 2]
    pub fn read(ctx: &mut Context<Read>, expected: u64) -> Result<()> {
        State::with_read(ctx.accounts.state.account(), &ID, |state| {
            require!(
                state.values().get(0).unwrap_or(0) == expected,
                blubber::ErrorKind::Unauthorized
            );
            Ok(())
        })?;
        Ok(())
    }

    #[discrim = 3]
    pub fn set_counter(ctx: &mut Context<Modify>, value: u64) -> Result<()> {
        let mut view = *ctx.accounts.state.account();
        State::with_edit(&mut view, &ID, Funding::Prepaid, |state| {
            require!(
                state.authority() == *ctx.accounts.authority.account().address(),
                blubber::ErrorKind::Unauthorized
            );
            state.counter_mut().set(value)
        })?;
        Ok(())
    }

    // Test rollback after an actual committed write, before instruction success.
    #[discrim = 4]
    pub fn fail_after_edit(ctx: &mut Context<Modify>) -> Result<()> {
        set_counter(ctx, 99)?;
        Err(Error::Custom(77))
    }

    #[discrim = 5]
    pub fn fail_after_growth(ctx: &mut Context<Grow>, value: u64) -> Result<()> {
        push(ctx, value)?;
        Err(Error::Custom(77))
    }

    #[discrim = 6]
    pub fn reserve_prepaid(ctx: &mut Context<Modify>, additional: u32) -> Result<()> {
        let mut view = *ctx.accounts.state.account();
        State::with_edit_options(&mut view, &ID, EditOptions {
            funding: Funding::Prepaid,
            headroom: Headroom::UpTo(additional as usize), // Explicit reservation beyond opening storage.
            refund: None,
        }, |state| {
            require!(
                state.authority() == *ctx.accounts.authority.account().address(),
                blubber::ErrorKind::Unauthorized
            );
            state.reserve(additional)
        })?;
        Ok(())
    }
}
