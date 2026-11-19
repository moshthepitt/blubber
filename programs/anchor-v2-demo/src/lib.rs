//! Native Anchor v2 accounts with borrowed initialization and checked CPI.
#![forbid(unsafe_code)]

use anchor_lang::prelude::*;
use blubber::{
    pinocchio::{Funding, Headroom},
    CommitInfo, List, Str,
};
use blubber_anchor::{funding, Blubber, EditOptions};

declare_id!("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");

#[blubber::schema(account)]
/// Format 1 state; fixed fields also form the copied constraint prefix.
pub struct State {
    /// Signer authorized to edit and fund this state.
    pub authority: Address,
    /// Application counter, incremented by append.
    pub counter: u64,
    /// Borrowed strings remain accessible only through scoped readers.
    pub values: List<Str>,
}
impl Owner for State {
    const OWNER: Address = ID;
}

#[derive(Accounts)]
#[instruction(_reserve: u32)]
/// Creates a signer account and reserves bytes beyond the empty encoding.
pub struct Initialize {
    #[account(mut)]
    /// Pays creation rent and becomes the stored authority.
    pub payer: Signer,
    #[account(init, payer = payer, space = State::space(_reserve as usize)?)]
    /// New until the handler supplies its borrowed initializer.
    pub state: Blubber<State>,
    /// Executable System Program used by creation.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
#[instruction(_reserve: u32)]
/// Creates or reuses the payer's PDA without resetting existing state.
pub struct Reuse {
    #[account(mut)]
    /// Authorizes reuse as well as creation.
    pub payer: Signer,
    #[account(init_if_needed, payer = payer, space = State::space(_reserve as usize)?,
        seeds = [b"state", payer.address().as_ref()], bump,
        constraint = state.is_new() || state.authority == *payer.address())]
    /// Existing state must have this exact physical allocation on reuse.
    pub state: Blubber<State>,
    /// Executable System Program used only when creation is needed.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
/// Binds managed rent funding to the authorized signer once.
pub struct Modify {
    #[account(mut, funding::payer = authority, funding::system_program = system_program)]
    /// Authenticated state with genuine exclusive SDK exclusion.
    pub state: Blubber<State>,
    #[account(mut, address = state.authority)]
    /// Copied fixed-prefix address constraint authenticates this signer.
    pub authority: Signer,
    /// Executable System Program for a possible final rent deficit.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
/// Explicit-funding alternative for handlers that select their own policy.
pub struct Explicit {
    #[account(mut)]
    /// Authenticated mutable state without retained funding bindings.
    pub state: Blubber<State>,
    #[account(mut, address = state.authority)]
    /// Authorizes edits and any net rent transfer.
    pub authority: Signer,
    /// Executable System Program passed explicitly to Store.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
/// Reads the opaque account through its checked schema.
pub struct Read {
    /// Shared SDK exclusion lasts until this wrapper is dropped.
    pub state: Blubber<State>,
}

/// Executable identity used by the self-CPI demonstration.
pub struct DemoProgram;
impl Id for DemoProgram {
    fn id() -> Address {
        ID
    }
}

#[derive(Accounts)]
/// Commits an edit, lends a checked readonly CPI handle, then reloads.
pub struct Relay {
    #[account(mut)]
    /// Exclusive guard is released only during the scoped CPI.
    pub state: Blubber<State>,
    #[account(address = state.authority)]
    /// Authorizes the edit before CPI.
    pub authority: Signer,
    /// Executable callee, supplied in the transaction's account list.
    pub program: Program<DemoProgram>,
}

#[event]
/// Actual allocation and net rent settlement from one successful edit.
pub struct Settled {
    /// Canonical envelope and root length.
    pub used: u32,
    /// Actual physical length after best-effort trim.
    pub storage: u32,
    /// Lamports transferred by the final funding CPI.
    pub topped_up: u64,
    /// Lamports returned to an explicitly selected recipient.
    pub refunded: u64,
}

#[error_code]
/// Application errors supplement blubber's stable fault codes.
pub enum DemoError {
    /// The caller's expected count did not match the checked account.
    #[msg("Unexpected state counter")]
    UnexpectedCounter,
}

fn report(receipt: CommitInfo) -> Result<()> {
    // Settlement uses storage_len even if physical trimming failed.
    emit!(Settled {
        used: receipt.used_len,
        storage: receipt.storage_len,
        topped_up: receipt.topped_up,
        refunded: receipt.refunded,
    });
    Ok(())
}

#[program]
/// Instructions exercised by the docs lane on both SBF architectures.
pub mod native {
    use super::*;

    #[discrim = 0]
    /// Initializes borrowed input after Anchor has allocated the signer target.
    pub fn initialize(ctx: &mut Context<Initialize>, reserve: u32) -> Result<()> {
        let _ = reserve; // Consumed by the space constraint.
        let receipt = ctx.accounts.state.initialize(StateInit {
            authority: *ctx.accounts.payer.address(),
            counter: 0,
            values: &[],
        })?;
        report(receipt)
    }

    #[discrim = 1]
    /// Creates the PDA or preserves the selected policy on reuse.
    pub fn reuse(ctx: &mut Context<Reuse>, reserve: u32) -> Result<()> {
        let _ = reserve;
        if let Some(receipt) = ctx.accounts.state.initialize_if_new(StateInit {
            authority: *ctx.accounts.payer.address(),
            counter: 0,
            values: &[],
        })? {
            report(receipt)?;
        }
        Ok(())
    }

    #[discrim = 2]
    /// Appends with explicit headroom, TopUp funding and one commit.
    pub fn append(ctx: &mut Context<Modify>, text: String) -> Result<()> {
        // Text and indexed-directory growth can exceed the creation reserve.
        let (_, receipt) = ctx.accounts.state.edit_with(
            EditOptions {
                funding: Funding::TopUp {
                    payer: ctx.accounts.authority.account(),
                    system_program: ctx.accounts.system_program.account(),
                },
                headroom: Headroom::Max,
                refund: None,
            },
            |s| {
                let next = s
                    .counter()
                    .checked_add(1)
                    .ok_or(blubber::ErrorKind::ArithmeticOverflow)?;
                s.values_mut().push(text.as_str())?;
                s.counter_mut().set(next)
            },
        )?;
        report(receipt)
    }

    #[discrim = 3]
    /// Reserves or explicitly trims using the explicit funding API.
    pub fn allocation(ctx: &mut Context<Explicit>, reserve: u32, trim: bool) -> Result<()> {
        let (_, receipt) = ctx.accounts.state.edit_with(
            EditOptions {
                funding: Funding::TopUp {
                    payer: ctx.accounts.authority.account(),
                    system_program: ctx.accounts.system_program.account(),
                },
                headroom: Headroom::Max,
                refund: None,
            },
            |s| {
                if trim {
                    s.shrink_to_fit()?;
                } else {
                    s.reserve(reserve)?;
                }
                Ok(())
            },
        )?;
        report(receipt)
    }

    #[discrim = 4]
    /// Checks a scalar and validates the variable children through a read scope.
    pub fn read(ctx: &mut Context<Read>, expected: u64) -> Result<()> {
        let counter = ctx.accounts.state.read(|s| {
            s.validate()?;
            Ok(s.counter())
        })?;
        require!(counter == expected, DemoError::UnexpectedCounter);
        Ok(())
    }

    #[discrim = 5]
    /// Completes an edit before invoking the generated readonly callee.
    pub fn relay(ctx: &mut Context<Relay>, expected: u64) -> Result<()> {
        ctx.accounts.state.edit(|s| s.counter_mut().set(expected))?;
        ctx.accounts.state.with_cpi_read(|state| {
            checked::cpi::read(
                CpiContext::new(&ID, checked::cpi::accounts::Read { state }),
                expected,
            )
        })?;
        require!(
            ctx.accounts.state.counter == expected,
            DemoError::UnexpectedCounter
        );
        Ok(())
    }
}

/// Generated CPI binding to the actual read instruction above.
pub mod checked {
    use super::*;
    #[program(interface, program_id = ID)]
    /// Interface calls share their bodies with the real callee.
    pub mod callee {
        use super::*;
        #[discrim = 4]
        /// Checks the fully validated state after the caller commits.
        pub fn read(ctx: &mut Context<Read>, expected: u64) -> Result<()> {
            native::read(ctx, expected)
        }
    }
}
