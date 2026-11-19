//! Native load/alias fixtures and matched scalar Account baselines.
#![forbid(unsafe_code)]
use anchor_lang::prelude::*;
use anchor_lang::AccountRealloc;
use blubber::{
    pinocchio::{Funding, Headroom},
    List, Map, Str,
};
use blubber_anchor::{funding, EditOptions, HANDLE_SLOT_BYTES};

use blubber_anchor::Blubber;
#[cfg(not(feature = "lazy-policy"))]
use blubber_anchor::Checked as Policy;
#[cfg(feature = "lazy-policy")]
use blubber_anchor::Lazy as Policy;

declare_id!("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");

mod close_baseline;
use close_baseline::*;

mod compatibility;
use compatibility::*;

mod cpi_fixture;
use cpi_fixture::*;

#[blubber::schema(account)]
/// Native acceptance fixture for State.
pub struct State {
    /// Account or schema field used by this acceptance fixture.
    pub authority: Address,
    /// Account or schema field used by this acceptance fixture.
    pub counter: u64,
    /// Account or schema field used by this acceptance fixture.
    pub flag: bool,
    /// Account or schema field used by this acceptance fixture.
    pub text: Str,
    /// Account or schema field used by this acceptance fixture.
    pub values: List<Str>,
}
impl Owner for State {
    const OWNER: Address = ID;
}

#[blubber::schema(account)]
/// Native acceptance fixture for Counter.
pub struct Counter {
    /// Account or schema field used by this acceptance fixture.
    pub authority: Address,
    /// Account or schema field used by this acceptance fixture.
    pub counter: u64,
}
impl Owner for Counter {
    const OWNER: Address = ID;
}

#[blubber::schema(account)]
/// Variable containers used by the managed-edit oracle.
pub struct Book {
    /// Authorizes the funding signer.
    pub authority: Address,
    /// Copied constraint prefix refreshed after edits.
    pub counter: u64,
    /// Indexed list payload.
    pub values: List<Str>,
    /// Sorted indexed map payload.
    pub entries: Map<u32, Str>,
}
impl Owner for Book {
    const OWNER: Address = ID;
}

#[derive(Accounts)]
/// Binds funding once through Anchor's full-view constraint protocol.
pub struct Managed {
    #[account(mut, funding::payer = payer, funding::system_program = system_program)]
    /// Fully validated managed state.
    pub state: Blubber<Book, Policy>,
    #[account(mut, address = state.authority)]
    /// Authorized TopUp signer.
    pub payer: Signer,
    /// Validated executable System Program.
    pub system_program: Program<System>,
    #[account(mut, address = Address::new_from_array([10; 32]))]
    /// Application-authorized refund destination.
    pub refund: UncheckedAccount,
}

#[derive(Accounts)]
/// Raw handles allow explicit-funding rejection and alias controls.
pub struct ManagedRaw {
    #[account(mut)]
    /// Fully validated state.
    pub state: Blubber<Book, Policy>,
    /// Explicit funding payer, checked by Store.
    pub payer: UncheckedAccount,
    /// Explicit System Program, checked by Store.
    pub system_program: UncheckedAccount,
    /// Explicit refund recipient, authorized by the test instruction.
    pub refund: UncheckedAccount,
}

fn edit_book(s: &mut BookMut<'_>, count: u32, width: u32, mode: u8) -> blubber::Result<()> {
    let text = "x".repeat(width as usize);
    s.counter_mut().set(19)?;
    for i in 0..count {
        s.values_mut().push(text.as_str())?;
        s.entries_mut().insert(i, text.as_str())?;
    }
    if mode == 1 {
        s.values_mut().clear()?;
        s.entries_mut().clear()?;
    }
    if mode == 2 {
        s.reserve(700)?;
    }
    if mode == 3 || mode == 4 || mode == 5 {
        s.shrink_to_fit()?;
    }
    if mode == 4 {
        return Err(blubber::ErrorKind::Capacity.into());
    }
    Ok(())
}

fn check_balances(
    state: &Blubber<Book, Policy>,
    payer: &AccountView,
    refund: &AccountView,
    before: (u64, u64, u64),
    receipt: blubber::CommitInfo,
) -> Result<()> {
    require!(
        before.1.checked_sub(payer.lamports()) == Some(receipt.topped_up),
        Error::InvalidAccountData
    );
    require!(
        refund.lamports().checked_sub(before.2) == Some(receipt.refunded),
        Error::InvalidAccountData
    );
    require!(
        before
            .0
            .checked_add(receipt.topped_up)
            .and_then(|n| n.checked_sub(receipt.refunded))
            == Some(state.account().lamports()),
        Error::InvalidAccountData
    );
    Ok(())
}

fn check_receipt(state: &Blubber<Book, Policy>, receipt: blubber::CommitInfo) -> Result<()> {
    require!(
        receipt.storage_len as usize == state.account().data_len(),
        Error::InvalidAccountData
    );
    require!(
        receipt.used_len <= receipt.storage_len && state.counter == 19,
        Error::InvalidAccountData
    );
    // No author check of shrink_error: actual-size settlement is authoritative.
    require!(
        state.account().try_borrow().is_err(),
        Error::InvalidAccountData
    );
    state.read(|s| s.validate())?;
    Ok(())
}

#[derive(Accounts)]
#[instruction(_new_space: u32)]
/// Reallocates once in constraints, then edits within that exact allocation.
pub struct ExactResize {
    #[account(mut, realloc = _new_space as usize, realloc_payer = payer,
        realloc_zero = true, funding::payer = payer,
        funding::system_program = system_program)]
    /// Native state retaining its exact constraint-selected allocation.
    pub state: Blubber<State, Policy>,
    #[account(mut, address = state.authority)]
    /// Authorizes both net rent top-up and excess-rent refund.
    pub payer: Signer,
    /// Executable program required by the checked funding CPI.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
/// Closes only after the authorized handler succeeds.
pub struct CloseState {
    #[account(mut, close = recipient)]
    /// Ready mutable account closed at successful exit.
    pub state: Blubber<State, Policy>,
    #[account(mut, address = state.authority)]
    /// Authorizes closure and any preceding exact resize.
    pub payer: Signer,
    #[account(mut, address = Address::new_from_array([10; 32]))]
    /// Application-authorized close recipient.
    pub recipient: UncheckedAccount,
    /// Executable program available for exact resize funding.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
/// Raw views allow direct capability rejection and terminal-state controls.
pub struct LifecycleRaw {
    /// Checked by the native wrapper inside the handler.
    pub state: UncheckedAccount,
    /// Checked by the capability inside the handler.
    pub payer: UncheckedAccount,
    /// Application/test-authorized recipient for direct-close controls.
    pub recipient: UncheckedAccount,
    /// Program available for checked funding CPI.
    pub system_program: UncheckedAccount,
}

#[blubber::schema(account)]
/// Exercises native edits at the full copied-prefix cap.
pub struct Wide {
    /// Copied fixed prefix.
    pub prefix: [u8; 1024],
    /// Variable tail.
    pub values: List<Str>,
}
impl Owner for Wide {
    const OWNER: Address = ID;
}
#[derive(Accounts)]
/// Writable native account at the copied-prefix cap.
pub struct WideEdit {
    #[account(mut)]
    /// SDK-guarded variable account with a 1024-byte prefix.
    pub state: Blubber<Wide, Policy>,
}
#[derive(Accounts)]
#[instruction(_space: u32)]
/// Applies funding bindings to a newly allocated Book.
pub struct ManagedInitialize {
    #[account(mut)]
    /// Pays creation rent and authorizes later funding.
    pub payer: Signer,
    #[account(init, payer = payer, space = _space as usize,
        funding::payer = payer, funding::system_program = system_program)]
    /// New wrapper initialized before the managed edit.
    pub state: Blubber<Book, Policy>,
    /// Executable System Program for both stages.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
/// Native acceptance fixture for Read.
pub struct Read {
    /// Account or schema field used by this acceptance fixture.
    pub state: Blubber<State, Policy>,
}
#[derive(Accounts)]
/// Checks copied fixed fields before entering a handler with lazy variable data.
pub struct PrefixRead {
    /// Authenticated account using the selected native policy.
    pub state: Blubber<State, Policy>,
    #[account(address = state.authority)]
    /// Constraint consumes the checked copied authority.
    pub authority: Signer,
}
#[derive(Accounts)]
/// Native acceptance fixture for Modify.
pub struct Modify {
    #[account(mut)]
    /// Account or schema field used by this acceptance fixture.
    pub state: Blubber<State, Policy>,
    #[account(address = state.authority)]
    /// Account or schema field used by this acceptance fixture.
    pub authority: Signer,
}
#[derive(Accounts)]
/// Native acceptance fixture for RR.
pub struct RR {
    /// Account or schema field used by this acceptance fixture.
    pub first: Blubber<State, Policy>,
    /// Account or schema field used by this acceptance fixture.
    pub second: Blubber<State, Policy>,
}
#[derive(Accounts)]
/// Native acceptance fixture for RM.
pub struct RM {
    /// Account or schema field used by this acceptance fixture.
    pub first: Blubber<State, Policy>,
    #[account(mut)]
    /// Account or schema field used by this acceptance fixture.
    pub second: Blubber<State, Policy>,
}
#[derive(Accounts)]
/// Native acceptance fixture for MR.
pub struct MR {
    #[account(mut)]
    /// Account or schema field used by this acceptance fixture.
    pub first: Blubber<State, Policy>,
    /// Account or schema field used by this acceptance fixture.
    pub second: Blubber<State, Policy>,
}
#[derive(Accounts)]
/// Native acceptance fixture for MM.
pub struct MM {
    #[account(mut)]
    /// Account or schema field used by this acceptance fixture.
    pub first: Blubber<State, Policy>,
    #[account(mut)]
    /// Account or schema field used by this acceptance fixture.
    pub second: Blubber<State, Policy>,
}
#[derive(Accounts)]
/// Native acceptance fixture for NestedCase.
pub struct NestedCase {
    /// Account or schema field used by this acceptance fixture.
    pub inner: anchor_lang::Nested<Read>,
    #[account(mut)]
    /// Account or schema field used by this acceptance fixture.
    pub second: Blubber<State, Policy>,
}
#[derive(Accounts)]
/// Native acceptance fixture for Raw.
pub struct Raw {
    /// Account or schema field used by this acceptance fixture.
    pub state: UncheckedAccount,
}
#[derive(Accounts)]
/// Native acceptance fixture for Mixed.
pub struct Mixed {
    #[account(mut)]
    /// Account or schema field used by this acceptance fixture.
    pub state: Blubber<State, Policy>,
    /// Account or schema field used by this acceptance fixture.
    pub raw: UncheckedAccount,
}
#[derive(Accounts)]
/// Native acceptance fixture for ScalarRead.
pub struct ScalarRead {
    /// Account or schema field used by this acceptance fixture.
    pub state: Blubber<Counter, Policy>,
}
#[derive(Accounts)]
/// Native acceptance fixture for ScalarWrite.
pub struct ScalarWrite {
    #[account(mut)]
    /// Account or schema field used by this acceptance fixture.
    pub state: Blubber<Counter, Policy>,
    #[account(address = state.authority)]
    /// Account or schema field used by this acceptance fixture.
    pub authority: Signer,
}
#[account]
/// Native acceptance fixture for Baseline.
pub struct Baseline {
    /// Account or schema field used by this acceptance fixture.
    pub authority: Address,
    /// Account or schema field used by this acceptance fixture.
    pub counter: u64,
}
#[derive(Accounts)]
/// Native acceptance fixture for BaselineRead.
pub struct BaselineRead {
    /// Account or schema field used by this acceptance fixture.
    pub state: Account<Baseline>,
}
#[derive(Accounts)]
/// Native acceptance fixture for BaselineWrite.
pub struct BaselineWrite {
    #[account(mut)]
    /// Account or schema field used by this acceptance fixture.
    pub state: Account<Baseline>,
    #[account(address = state.authority)]
    /// Account or schema field used by this acceptance fixture.
    pub authority: Signer,
}

#[derive(Accounts)]
#[instruction(_space: u32)]
/// Signer target allocated by Anchor before the borrowed initializer.
pub struct Initialize {
    #[account(mut)]
    /// Distinct transaction signer funding the exact allocation.
    pub payer: Signer,
    #[account(init, payer = payer, space = _space as usize)]
    /// New state, unavailable until initialized in the handler.
    pub state: Blubber<State, Policy>,
    /// Validated executable System Program.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
#[instruction(_space: u32)]
/// PDA target with canonical seeds supplied by Anchor.
pub struct InitializePda {
    #[account(mut)]
    /// Transaction signer funding the allocation.
    pub payer: Signer,
    #[account(init, payer = payer, space = _space as usize, seeds = [b"native-init"], bump)]
    /// Newly allocated PDA.
    pub state: Blubber<State, Policy>,
    /// Validated executable System Program.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
#[instruction(_space: u32)]
/// Reuses only fully validated, authorized state at the exact physical space.
pub struct Ensure {
    #[account(mut)]
    /// Application authority on both branches.
    pub payer: Signer,
    #[account(init_if_needed, payer = payer, space = _space as usize,
        constraint = state.is_new() || state.authority == *payer.address())]
    /// New or existing state; the short-circuit avoids New prefix access.
    pub state: Blubber<State, Policy>,
    /// Validated executable System Program.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
/// SDK-hook rejection controls unavailable through ordinary typed constraints.
pub struct InitRaw {
    #[account(mut)]
    /// Payer is checked by the wrapper's shared preflight.
    pub payer: UncheckedAccount,
    #[account(mut)]
    /// Target is checked by the wrapper's shared preflight.
    pub state: UncheckedAccount,
    /// The helper requires the runtime System Program.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
#[instruction(_space: u32)]
/// Reuse the canonical PDA without reinitializing it.
pub struct EnsurePda {
    #[account(mut)]
    /// Application authority on both branches.
    pub payer: Signer,
    #[account(init_if_needed, payer = payer, space = _space as usize,
        seeds = [b"native-init"], bump,
        constraint = state.is_new() || state.authority == *payer.address())]
    /// New or fully validated existing PDA.
    pub state: Blubber<State, Policy>,
    /// Validated executable System Program.
    pub system_program: Program<System>,
}

#[derive(Accounts)]
#[instruction(_space: u32)]
/// Payer seeds reach the hook and are explicitly unsupported in this version.
pub struct PdaPayer {
    #[account(mut, seeds = [b"native-payer"], bump)]
    /// System PDA needing a program signature for funding.
    pub payer: SystemAccount,
    #[account(init, payer = payer, space = _space as usize)]
    /// Signer target.
    pub state: Blubber<State, Policy>,
    /// Validated executable System Program.
    pub system_program: Program<System>,
}

fn finish_init(
    state: &mut Blubber<State, Policy>,
    payer: Address,
    space: u32,
    mode: u8,
) -> Result<()> {
    let was_new = state.is_new();
    let before_counter = if was_new { 7 } else { state.counter };
    if mode == 1 {
        // Successful handler still fails exit when initialization is omitted.
        return Ok(());
    }
    if was_new {
        require!(state.read(|_| Ok(())).is_err(), Error::InvalidAccountData);
        require!(state.edit(|_| Ok(())).is_err(), Error::InvalidAccountData);
        if mode == 4 {
            let too_long = "x".repeat(300);
            let error = state.initialize(StateInit {
                authority: payer,
                counter: 99,
                flag: true,
                text: &too_long,
                values: &[],
            });
            require!(
                error == Err(blubber::Error::from(blubber::ErrorKind::Capacity).into()),
                Error::InvalidAccountData
            );
            require!(
                state.is_new() && state.read(|_| Ok(())).is_err(),
                Error::InvalidAccountData
            );
        }
    }
    let text = if mode == 2 {
        "x".repeat(300)
    } else {
        "hello".into()
    };
    let init = StateInit {
        authority: payer,
        counter: 7,
        flag: true,
        text: &text,
        values: &["a", "bc"],
    };
    let receipt = state.initialize_if_new(init)?;
    require!(receipt.is_some() == was_new, Error::InvalidAccountData);
    if let Some(receipt) = receipt {
        require!(receipt.storage_len == space, Error::InvalidAccountData);
        require!(
            receipt.topped_up == 0 && receipt.refunded == 0,
            Error::InvalidAccountData
        );
        require!(receipt.shrink_error.is_none(), Error::InvalidAccountData);
    }
    require!(
        !state.is_new() && state.counter == before_counter,
        Error::InvalidAccountData
    );
    let second = state.initialize(StateInit {
        authority: payer,
        counter: 99,
        flag: false,
        text: "reset",
        values: &[],
    });
    require!(
        second == Err(blubber::Error::from(blubber::ErrorKind::AlreadyInitialized).into()),
        Error::InvalidAccountData
    );
    // The opening reserve stays usable after initialization and subsequent
    // fixed-capacity growth. Propagate errors to roll back the allocation CPI.
    if mode == 3 {
        state.edit(|s| s.text_mut().set("longer text"))?;
    }
    Ok(())
}

#[program]
pub mod load_fixture {
    use super::*;
    #[discrim = 57]
    /// Reads only a copied constraint prefix, optionally verifying the whole root.
    pub fn prefix_read(ctx: &mut Context<PrefixRead>, verify: bool) -> Result<()> {
        require!(
            ctx.accounts.state.counter == 7 && ctx.accounts.state.flag,
            Error::InvalidAccountData
        );
        if verify {
            ctx.accounts.state.verify()?;
        }
        Ok(())
    }
    #[discrim = 56]
    /// Raw regrowth cannot restore a closed native envelope.
    pub fn regrow_closed(ctx: &mut Context<LifecycleRaw>, space: u32) -> Result<()> {
        use anchor_lang::pinocchio::Resize as _;
        let mut account = *ctx.accounts.state.account();
        require!(account.data_len() == 12, Error::InvalidAccountData);
        account.resize(space as usize)?;
        require!(
            account.try_borrow()?[12..].iter().all(|b| *b == 0),
            Error::InvalidAccountData
        );
        require!(
            Blubber::<State, Policy>::load(account).is_err(),
            Error::InvalidAccountData
        );
        Ok(())
    }
    #[discrim = 52]
    /// Creates a stock Account only when its initial counter is zero.
    pub fn reuse_account(ctx: &mut Context<ReuseAccount>) -> Result<()> {
        if ctx.accounts.state.counter == 0 {
            ctx.accounts.state.authority = *ctx.accounts.payer.address();
            ctx.accounts.state.counter = 7;
        }
        Ok(())
    }
    #[discrim = 53]
    /// Authorized successful-exit stock Account close.
    pub fn close_account(_ctx: &mut Context<CloseAccount>) -> Result<()> {
        Ok(())
    }
    #[discrim = 54]
    /// Creates a stock Slab only when its initial counter is zero.
    pub fn reuse_slab(ctx: &mut Context<ReuseSlab>) -> Result<()> {
        if ctx.accounts.state.counter == 0 {
            ctx.accounts.state.authority = *ctx.accounts.payer.address();
            ctx.accounts.state.counter = 7;
        }
        Ok(())
    }
    #[discrim = 55]
    /// Authorized successful-exit stock Slab close.
    pub fn close_slab(_ctx: &mut Context<CloseSlab>) -> Result<()> {
        Ok(())
    }

    #[discrim = 40]
    /// Fully validated collection read with an observable checksum.
    pub fn series_read(ctx: &mut Context<SeriesRead>, kind: u8, n: u32) -> Result<()> {
        compatibility::read(&ctx.accounts.state, kind, n)
    }
    #[discrim = 41]
    /// Measures one managed operation and one successful exit.
    pub fn series_edit(
        ctx: &mut Context<SeriesWrite>,
        kind: u8,
        n: u32,
        operation: u8,
    ) -> Result<()> {
        compatibility::edit(
            &mut ctx.accounts.state,
            kind,
            n,
            operation,
            Funding::TopUp {
                payer: ctx.accounts.payer.account(),
                system_program: ctx.accounts.system_program.account(),
            },
        )
    }
    #[discrim = 42]
    /// Reads the stock slab or serialized map with the same checksum.
    pub fn serialized_read(ctx: &mut Context<SerializedRead>, kind: u8, n: u32) -> Result<()> {
        let state = &ctx.accounts.state;
        require!(kind == 1, Error::InvalidInstructionData);
        let sum: u64 = state
            .entries
            .iter()
            .map(|(k, v)| u64::from(*k) + u64::from(*v))
            .sum();
        let expected = u64::from(n) * u64::from(n.saturating_sub(1)) / 2;
        require!(
            sum == expected * if kind == 0 { 1 } else { 2 },
            Error::InvalidAccountData
        );
        Ok(())
    }
    #[discrim = 43]
    /// Edits the stock counted slab or sorted serialized map.
    pub fn serialized_edit(
        ctx: &mut Context<SerializedWrite>,
        kind: u8,
        n: u32,
        operation: u8,
    ) -> Result<()> {
        require!(kind == 1, Error::InvalidInstructionData);
        let state = &mut ctx.accounts.state;
        match operation {
            0 => state.counter = 11,
            1 => {
                let at = state
                    .entries
                    .binary_search_by_key(&n, |(key, _)| *key)
                    .unwrap_err();
                state.entries.insert(at, (n, n));
            }
            2 => (),
            _ => return Err(Error::InvalidInstructionData),
        }
        Ok(())
    }
    #[discrim = 44]
    /// Creates and fills the native collection with borrowed input.
    pub fn series_init(
        ctx: &mut Context<CreateSeries>,
        space: u32,
        kind: u8,
        n: u32,
    ) -> Result<()> {
        let _ = space;
        compatibility::initialize(
            &mut ctx.accounts.state,
            *ctx.accounts.payer.address(),
            kind,
            n,
        )
    }
    #[discrim = 45]
    /// Creates the stock collection at the same space and rent.
    pub fn serialized_init(
        ctx: &mut Context<CreateSerialized>,
        space: u32,
        kind: u8,
        n: u32,
    ) -> Result<()> {
        let _ = space;
        let state = &mut ctx.accounts.state;
        state.authority = *ctx.accounts.payer.address();
        state.counter = 7;
        state.entries = if kind == 1 {
            (0..n).map(|i| (i, i)).collect()
        } else {
            Vec::new()
        };
        Ok(())
    }

    #[discrim = 46]
    /// Reads every slab item with the same checksum as the packed list.
    pub fn slab_read(ctx: &mut Context<SlabRead>, kind: u8, n: u32) -> Result<()> {
        require!(
            kind == 0 && ctx.accounts.state.len() == n as usize,
            Error::InvalidAccountData
        );
        let sum: u64 = ctx
            .accounts
            .state
            .iter()
            .map(|v| u64::from_le_bytes(*v))
            .sum();
        require!(
            sum == u64::from(n) * u64::from(n.saturating_sub(1)) / 2,
            Error::InvalidAccountData
        );
        Ok(())
    }
    #[discrim = 47]
    /// Updates the genuine slab header or appends into prepaid capacity.
    pub fn slab_edit(ctx: &mut Context<SlabWrite>, kind: u8, n: u32, operation: u8) -> Result<()> {
        require!(kind == 0, Error::InvalidInstructionData);
        match operation {
            0 => ctx.accounts.state.counter = 11,
            1 => {
                ctx.accounts.state.try_push(u64::from(n).to_le_bytes())?;
            }
            2 => (),
            _ => return Err(Error::InvalidInstructionData),
        }
        Ok(())
    }
    #[discrim = 48]
    /// Creates a slab with identical authority, logical contents and rent.
    pub fn slab_init(ctx: &mut Context<CreateSlab>, space: u32, kind: u8, n: u32) -> Result<()> {
        let _ = space;
        require!(kind == 0, Error::InvalidInstructionData);
        let state = &mut ctx.accounts.state;
        state.authority = *ctx.accounts.payer.address();
        state.counter = 7;
        for i in 0..n {
            state.try_push(u64::from(i).to_le_bytes())?;
        }
        Ok(())
    }
    #[discrim = 49]
    /// Measures validated native opening and exit without a read callback.
    pub fn series_open(ctx: &mut Context<SeriesRead>, kind: u8, n: u32) -> Result<()> {
        let _ = (kind, n);
        require!(ctx.accounts.state.counter == 7, Error::InvalidAccountData);
        Ok(())
    }
    #[discrim = 50]
    /// Measures readonly slab opening and exit with a copied scalar check.
    pub fn slab_open(ctx: &mut Context<SlabRead>, kind: u8, n: u32) -> Result<()> {
        let _ = (kind, n);
        require!(ctx.accounts.state.counter == 7, Error::InvalidAccountData);
        Ok(())
    }
    #[discrim = 51]
    /// Measures serialized-map opening and exit without an item traversal.
    pub fn serialized_open(ctx: &mut Context<SerializedRead>, kind: u8, n: u32) -> Result<()> {
        let _ = (kind, n);
        require!(ctx.accounts.state.counter == 7, Error::InvalidAccountData);
        Ok(())
    }
    #[discrim = 39]
    /// Transaction-readonly account releases and restores its shared guard.
    pub fn cpi_readonly(ctx: &mut Context<CpiReadonlyCall>) -> Result<()> {
        cpi_fixture::readonly(ctx)
    }
    #[discrim = 34]
    /// Exercises generated and checked SDK CPI through scoped native guards.
    pub fn cpi_call(ctx: &mut Context<CpiCall>, scenario: u8, arg: u32) -> Result<()> {
        cpi_fixture::call(ctx, scenario, arg)
    }
    #[discrim = 35]
    /// Fully typed readonly callee, also used by generated CPI bindings.
    pub fn cpi_read(ctx: &mut Context<Read>, expected: u64) -> Result<()> {
        cpi_fixture::callee::cpi_read(ctx, expected)
    }
    #[discrim = 36]
    /// Authorized typed mutation observes the caller's completed edit.
    pub fn cpi_write(ctx: &mut Context<Modify>, expected: u64) -> Result<()> {
        cpi_fixture::callee::cpi_write(ctx, expected)
    }
    #[discrim = 37]
    /// Safe raw SDK damage, resize and returned-error callee controls.
    pub fn cpi_raw(ctx: &mut Context<CpiRaw>, mode: u8, size: u32) -> Result<()> {
        cpi_fixture::callee::cpi_raw(ctx, mode, size)
    }
    #[discrim = 38]
    /// Nested invocations retain each ancestor's saved growth ceiling.
    pub fn cpi_nested(ctx: &mut Context<CpiCall>, size: u32) -> Result<()> {
        cpi_fixture::nested(ctx, size)
    }
    #[discrim = 0]
    /// Exercises the named native account operation.
    pub fn read(ctx: &mut Context<Read>, expected: u64) -> Result<()> {
        let value = ctx.accounts.state.read(|s| {
            if s.text() != "hello" || s.values().get(1) != Some("bc") {
                return Err(blubber::ErrorKind::InvalidSchema.into());
            }
            Ok(s.counter())
        })?;
        require!(
            value == expected && ctx.accounts.state.counter == expected,
            Error::InvalidAccountData
        );
        Ok(())
    }
    #[discrim = 1]
    /// Exercises the named native account operation.
    pub fn update(ctx: &mut Context<Modify>, value: u64) -> Result<()> {
        let (_, receipt) = ctx.accounts.state.edit(|s| s.counter_mut().set(value))?;
        require!(
            ctx.accounts.state.counter == value && receipt.storage_len == 256,
            Error::InvalidAccountData
        );
        require!(
            ctx.accounts.state.read(|s| Ok(s.counter()))? == value,
            Error::InvalidAccountData
        );
        Ok(())
    }
    #[discrim = 2]
    /// Exercises the named native account operation.
    pub fn rr(_ctx: &mut Context<RR>) -> Result<()> {
        Ok(())
    }
    #[discrim = 3]
    /// Exercises the named native account operation.
    pub fn rm(_ctx: &mut Context<RM>) -> Result<()> {
        Ok(())
    }
    #[discrim = 4]
    /// Exercises the named native account operation.
    pub fn mr(_ctx: &mut Context<MR>) -> Result<()> {
        Ok(())
    }
    #[discrim = 5]
    /// Exercises the named native account operation.
    pub fn mm(_ctx: &mut Context<MM>) -> Result<()> {
        Ok(())
    }
    #[discrim = 6]
    /// Exercises the named native account operation.
    pub fn nested(_ctx: &mut Context<NestedCase>) -> Result<()> {
        Ok(())
    }
    #[discrim = 7]
    /// Exercises the named native account operation.
    pub fn raw_first(ctx: &mut Context<Raw>, mutable: bool) -> Result<()> {
        let mut view = *ctx.accounts.state.account();
        if mutable {
            let copy = view;
            let _guard = view.try_borrow_mut()?;
            require!(
                Blubber::<State, Policy>::load(copy).is_err(),
                Error::InvalidAccountData
            );
            require!(
                Blubber::<State, Policy>::load_mut(copy).is_err(),
                Error::InvalidAccountData
            );
        } else {
            let _guard = view.try_borrow()?;
            require!(
                Blubber::<State, Policy>::load(view).is_ok(),
                Error::InvalidAccountData
            );
            require!(
                Blubber::<State, Policy>::load_mut(view).is_err(),
                Error::InvalidAccountData
            );
        }
        Ok(())
    }
    #[discrim = 8]
    /// Exercises the named native account operation.
    pub fn typed_first(ctx: &mut Context<Mixed>) -> Result<()> {
        let mut view = *ctx.accounts.raw.account();
        require!(
            view.try_borrow().is_err() && view.try_borrow_mut().is_err(),
            Error::InvalidAccountData
        );
        Ok(())
    }
    #[discrim = 9]
    /// Exercises the named native account operation.
    pub fn drop_forget(ctx: &mut Context<Raw>, mutable: bool, forget: bool) -> Result<()> {
        let mut view = *ctx.accounts.state.account();
        let wrapper = if mutable {
            Blubber::<State, Policy>::load_mut(view)?
        } else {
            Blubber::<State, Policy>::load(view)?
        };
        if forget {
            core::mem::forget(wrapper);
            require!(view.try_borrow_mut().is_err(), Error::InvalidAccountData);
            require!(
                view.try_borrow().is_ok() != mutable,
                Error::InvalidAccountData
            );
        } else {
            drop(wrapper);
            let _guard = view.try_borrow_mut()?;
        }
        Ok(())
    }
    #[discrim = 10]
    /// Exercises the named native account operation.
    pub fn repeated_load(ctx: &mut Context<Raw>) -> Result<()> {
        require!(HANDLE_SLOT_BYTES == 8, Error::InvalidAccountData);
        for _ in 0..16 {
            let state = Blubber::<State, Policy>::load(*ctx.accounts.state.account())?;
            require!(state.counter == 7, Error::InvalidAccountData);
        }
        Ok(())
    }
    #[discrim = 11]
    /// Exercises the named native account operation.
    pub fn remaining_alias(ctx: &mut Context<Read>) -> Result<()> {
        let remaining = ctx.remaining_accounts();
        let view = *remaining.first().ok_or(Error::NotEnoughAccountKeys)?;
        require!(
            Blubber::<State, Policy>::load_mut(view).is_err(),
            Error::InvalidAccountData
        );
        Ok(())
    }
    #[discrim = 12]
    /// Exercises the named native account operation.
    pub fn readonly_edit(ctx: &mut Context<Read>) -> Result<()> {
        require!(
            ctx.accounts.state.edit(|_| Ok(())).is_err(),
            Error::InvalidAccountData
        );
        require!(ctx.accounts.state.counter == 7, Error::InvalidAccountData);
        Ok(())
    }
    #[discrim = 13]
    /// Exercises the named native account operation.
    pub fn callback_error(ctx: &mut Context<Modify>) -> Result<()> {
        let error = ctx.accounts.state.edit::<()>(|s| {
            s.counter_mut().set(99)?;
            Err(blubber::ErrorKind::Capacity.into())
        });
        require!(
            error.is_err() && ctx.accounts.state.counter == 99,
            Error::InvalidAccountData
        );
        Err(Error::Custom(77))
    }
    #[discrim = 16]
    /// Exercises the named native account operation.
    pub fn scalar_read(ctx: &mut Context<ScalarRead>, expected: u64) -> Result<()> {
        require!(
            ctx.accounts.state.read(|s| Ok(s.counter()))? == expected,
            Error::InvalidAccountData
        );
        Ok(())
    }
    #[discrim = 17]
    /// Exercises the named native account operation.
    pub fn scalar_write(ctx: &mut Context<ScalarWrite>, value: u64) -> Result<()> {
        ctx.accounts.state.edit(|s| s.counter_mut().set(value))?;
        Ok(())
    }
    #[discrim = 18]
    /// Exercises the named native account operation.
    pub fn baseline_read(ctx: &mut Context<BaselineRead>, expected: u64) -> Result<()> {
        require!(
            ctx.accounts.state.counter == expected,
            Error::InvalidAccountData
        );
        Ok(())
    }
    #[discrim = 19]
    /// Exercises the named native account operation.
    pub fn baseline_write(ctx: &mut Context<BaselineWrite>, value: u64) -> Result<()> {
        ctx.accounts.state.counter = value;
        Ok(())
    }
    #[discrim = 20]
    /// Allocates a signer target, then encodes borrowed data exactly once.
    pub fn initialize(ctx: &mut Context<Initialize>, space: u32, mode: u8) -> Result<()> {
        finish_init(
            &mut ctx.accounts.state,
            *ctx.accounts.payer.address(),
            space,
            mode,
        )
    }
    #[discrim = 21]
    /// Allocates a canonical PDA, including prefunded targets.
    pub fn initialize_pda(ctx: &mut Context<InitializePda>, space: u32, mode: u8) -> Result<()> {
        finish_init(
            &mut ctx.accounts.state,
            *ctx.accounts.payer.address(),
            space,
            mode,
        )
    }
    #[discrim = 22]
    /// Creates or reuses without resetting existing state.
    pub fn ensure(ctx: &mut Context<Ensure>, space: u32, mode: u8) -> Result<()> {
        finish_init(
            &mut ctx.accounts.state,
            *ctx.accounts.payer.address(),
            space,
            mode,
        )
    }
    #[discrim = 23]
    /// Tests direct hook preflight, including explicit seed/owner rejection.
    pub fn init_raw(ctx: &mut Context<InitRaw>, space: u32, mode: u8) -> Result<()> {
        use anchor_lang::AccountInitialize;
        let bad_seeds: &[&[u8]] = &[b"wrong", &[0]];
        let wrong_bump: &[&[u8]] = &[b"native-init", &[0]];
        let owner = if mode == 2 {
            Address::new_from_array([4; 32])
        } else {
            ID
        };
        let mut state = Blubber::<State, Policy>::create_and_initialize(
            ctx.accounts.payer.account(),
            ctx.accounts.state.account(),
            space as usize,
            &owner,
            &(),
            if mode == 4 {
                Some(wrong_bump)
            } else {
                (mode == 1).then_some(bad_seeds)
            },
            (mode == 3).then_some(bad_seeds),
        )?;
        finish_init(&mut state, *ctx.accounts.payer.address(), space, 0)?;
        state.exit()
    }
    #[discrim = 24]
    /// Rejects a constrained PDA payer before allocating the target.
    pub fn pda_payer(ctx: &mut Context<PdaPayer>, space: u32) -> Result<()> {
        finish_init(
            &mut ctx.accounts.state,
            *ctx.accounts.payer.address(),
            space,
            0,
        )
    }
    #[discrim = 25]
    /// Creates or reuses the target PDA without resetting its contents.
    pub fn ensure_pda(ctx: &mut Context<EnsurePda>, space: u32, mode: u8) -> Result<()> {
        finish_init(
            &mut ctx.accounts.state,
            *ctx.accounts.payer.address(),
            space,
            mode,
        )
    }
    #[discrim = 26]
    /// Uses namespaced payer/System bindings and one managed commit.
    pub fn managed(ctx: &mut Context<Managed>, count: u32, width: u32, mode: u8) -> Result<()> {
        let before = (
            ctx.accounts.state.account().lamports(),
            ctx.accounts.payer.account().lamports(),
            ctx.accounts.refund.account().lamports(),
        );
        let funding = Funding::TopUp {
            payer: ctx.accounts.payer.account(),
            system_program: ctx.accounts.system_program.account(),
        };
        let (_, receipt) = if mode == 7 || mode == 8 {
            let edit = |s: &mut BookMut<'_>| {
                require!(
                    s.values_mut().push(&"x".repeat(width as usize))
                        == Err(blubber::ErrorKind::Capacity.into()),
                    Error::InvalidAccountData
                );
                s.verify()
            };
            if mode == 7 {
                ctx.accounts.state.edit(edit)?
            } else {
                ctx.accounts.state.edit_with(funding, edit)?
            }
        } else {
            // The managed growth fixture builds variable-size lists and maps
            // beyond its opening reserve; request SDK capacity explicitly.
            ctx.accounts.state.edit_with(
                EditOptions {
                    funding,
                    headroom: Headroom::Max,
                    refund: None,
                },
                |s| edit_book(s, count, width, mode),
            )?
        };
        check_balances(
            &ctx.accounts.state,
            ctx.accounts.payer.account(),
            ctx.accounts.refund.account(),
            before,
            receipt,
        )?;
        if mode == 7 || mode == 8 {
            // Capacity rejection preserves the original counter and complete bytes.
            return ctx.accounts.state.read(|s| s.verify());
        }
        check_receipt(&ctx.accounts.state, receipt)
    }
    #[discrim = 27]
    /// Selects all headroom modes, optional refund and prepaid settlement.
    pub fn managed_explicit(
        ctx: &mut Context<ManagedRaw>,
        count: u32,
        width: u32,
        mode: u8,
        headroom: u8,
        prepaid: bool,
        refund: bool,
    ) -> Result<()> {
        let headroom = match headroom {
            0 => Headroom::Max,
            1 => Headroom::UpTo(0),
            2 => Headroom::UpTo(512),
            3 => Headroom::OnDemand,
            _ => return Err(Error::InvalidArgument),
        };
        let funding = if prepaid {
            Funding::Prepaid
        } else {
            Funding::TopUp {
                payer: ctx.accounts.payer.account(),
                system_program: ctx.accounts.system_program.account(),
            }
        };
        let before = (
            ctx.accounts.state.account().lamports(),
            ctx.accounts.payer.account().lamports(),
            ctx.accounts.refund.account().lamports(),
        );
        let mut recipient = *ctx.accounts.refund.account();
        let result = ctx.accounts.state.edit_with(
            EditOptions {
                funding,
                headroom,
                refund: refund.then_some(&mut recipient),
            },
            |s| {
                if mode == 6 {
                    s.values_mut().push(&"x".repeat(width as usize))
                } else {
                    edit_book(s, count, width, mode)
                }
            },
        );
        if mode == 6 {
            require!(
                result == Err(blubber::Error::from(blubber::ErrorKind::RentShortfall).into()),
                Error::InvalidAccountData
            );
            require!(ctx.accounts.state.counter == 7, Error::InvalidAccountData);
            ctx.accounts.state.read(|root| {
                assert_eq!(root.values().len(), 0);
                assert_eq!(root.entries().len(), 0);
                root.verify()
            })?;
            return Ok(());
        }
        if mode == 4 {
            require!(
                result == Err(blubber::Error::from(blubber::ErrorKind::Capacity).into()),
                Error::InvalidAccountData
            );
            // Ordinary callback errors reacquire and refresh completed prefix edits.
            require!(ctx.accounts.state.counter == 19, Error::InvalidAccountData);
            return Err(Error::Custom(81));
        }
        let (_, receipt) = result?;
        check_balances(
            &ctx.accounts.state,
            ctx.accounts.payer.account(),
            ctx.accounts.refund.account(),
            before,
            receipt,
        )?;
        check_receipt(&ctx.accounts.state, receipt)?;
        if mode == 5 {
            return Err(Error::Custom(82));
        }
        Ok(())
    }
    #[discrim = 28]
    /// Repeated managed sessions bound retained slots across same-size edits.
    pub fn managed_repeated(ctx: &mut Context<ManagedRaw>, count: u32) -> Result<()> {
        for _ in 0..count {
            let (_, receipt) = ctx.accounts.state.edit_with(
                EditOptions {
                    funding: Funding::Prepaid,
                    headroom: Headroom::UpTo(0),
                    refund: None,
                },
                |s| s.counter_mut().set(19),
            )?;
            check_receipt(&ctx.accounts.state, receipt)?;
        }
        Ok(())
    }
    #[discrim = 29]
    /// Checks the managed Store handoff at the maximum public prefix size.
    pub fn wide_edit(ctx: &mut Context<WideEdit>) -> Result<()> {
        let (_, receipt) = ctx.accounts.state.edit(|s| s.values_mut().push("wide"))?;
        require!(
            receipt.storage_len as usize == ctx.accounts.state.account().data_len(),
            Error::InvalidAccountData
        );
        for (index, byte) in ctx.accounts.state.prefix.iter().enumerate() {
            require!(*byte == (index % 251) as u8, Error::InvalidAccountData);
        }
        Ok(())
    }
    #[discrim = 30]
    /// Checks that init-hook bindings survive initialization and avoid another transfer.
    pub fn managed_initialize(ctx: &mut Context<ManagedInitialize>, space: u32) -> Result<()> {
        let initial = ctx.accounts.state.initialize(BookInit {
            authority: *ctx.accounts.payer.address(),
            counter: 7,
            values: &[],
            entries: &[],
        })?;
        require!(initial.storage_len == space, Error::InvalidAccountData);
        let (_, receipt) = ctx.accounts.state.edit(|s| edit_book(s, 1, 16, 0))?;
        require!(
            receipt.topped_up == 0 && receipt.refunded == 0,
            Error::InvalidAccountData
        );
        check_receipt(&ctx.accounts.state, receipt)
    }
    #[discrim = 31]
    /// Checks exact capacity, default/bound/explicit editing and reservation.
    pub fn exact_resize(ctx: &mut Context<ExactResize>, new_space: u32, mode: u8) -> Result<()> {
        require!(
            ctx.accounts.state.account().data_len() == new_space as usize,
            Error::InvalidAccountData
        );
        let options = EditOptions {
            funding: Funding::Prepaid,
            headroom: if mode == 2 {
                Headroom::OnDemand
            } else {
                Headroom::Max
            },
            refund: None,
        };
        let edit = |s: &mut StateMut<'_>| {
            s.counter_mut().set(19)?;
            if mode == 1 || mode == 2 {
                // A conflicting explicit growth policy cannot escape exact sizing.
                s.reserve(new_space)?;
            }
            if mode == 7 {
                s.shrink_to_fit()?;
                return Err(blubber::ErrorKind::Capacity.into());
            }
            assert!(!s.shrink_to_fit()?);
            Ok(())
        };
        let result = if mode == 2 {
            ctx.accounts.state.edit_with(options, edit)
        } else {
            ctx.accounts.state.edit(edit)
        };
        if mode == 1 || mode == 2 || mode == 7 {
            require!(result.is_err(), Error::InvalidAccountData);
            require!(
                ctx.accounts.state.account().data_len() == new_space as usize,
                Error::InvalidAccountData
            );
            require!(ctx.accounts.state.counter == 19, Error::InvalidAccountData);
            return Err(Error::Custom(83));
        }
        let (_, receipt) = result?;
        require!(
            receipt.storage_len == new_space && receipt.topped_up == 0 && receipt.refunded == 0,
            Error::InvalidAccountData
        );
        require!(ctx.accounts.state.counter == 19, Error::InvalidAccountData);
        if mode == 3 {
            return Err(Error::Custom(84));
        }
        if mode == 4 {
            let before = ctx.accounts.payer.account().lamports();
            AccountRealloc::realloc_account(
                &mut ctx.accounts.state,
                new_space as usize,
                *ctx.accounts.payer.account(),
                false,
            )?;
            require!(
                ctx.accounts.payer.account().lamports() == before,
                Error::InvalidAccountData
            );
        }
        if mode == 5 || mode == 6 {
            let used = receipt.used_len;
            AccountRealloc::realloc_account(
                &mut ctx.accounts.state,
                used as usize,
                *ctx.accounts.payer.account(),
                false,
            )?;
            let target = if mode == 5 {
                new_space as usize
            } else {
                new_space as usize + 10_241
            };
            AccountRealloc::realloc_account(
                &mut ctx.accounts.state,
                target,
                *ctx.accounts.payer.account(),
                true,
            )?;
        }
        Ok(())
    }

    #[discrim = 32]
    /// Successful exit closes; returned handler errors bypass the close hook.
    pub fn close_state(ctx: &mut Context<CloseState>, mode: u8) -> Result<()> {
        if mode == 1 {
            return Err(Error::Custom(85));
        }
        if mode == 2 {
            let used = ctx.accounts.state.read(|s| Ok(s.used_len() as usize))?;
            AccountRealloc::realloc_account(
                &mut ctx.accounts.state,
                used,
                *ctx.accounts.payer.account(),
                true,
            )?;
        }
        ctx.accounts.state.edit(|s| s.counter_mut().set(19))?;
        Ok(())
    }

    #[discrim = 33]
    /// Direct lifecycle hooks check readonly, alias, borrow and terminal states.
    pub fn lifecycle_raw(ctx: &mut Context<LifecycleRaw>, mode: u8, new_space: u32) -> Result<()> {
        let account = *ctx.accounts.state.account();
        let mut state = if mode == 1 {
            Blubber::<State, Policy>::load(account)?
        } else {
            Blubber::<State, Policy>::load_mut(account)?
        };
        if mode == 4 {
            state.edit(|s| s.counter_mut().set(19))?;
            return AccountRealloc::realloc_account(
                &mut state,
                new_space as usize,
                *ctx.accounts.payer.account(),
                false,
            );
        }
        if mode == 5 || mode == 6 {
            return AccountRealloc::realloc_account(
                &mut state,
                new_space as usize,
                *ctx.accounts.payer.account(),
                mode == 5,
            );
        }
        let guard = if mode == 3 {
            Some(ctx.accounts.recipient.account().try_borrow()?)
        } else {
            None
        };
        AccountClose::close(&mut state, *ctx.accounts.recipient.account())?;
        drop(guard);
        let poisoned = Error::from(blubber::Error::from(blubber::ErrorKind::Poisoned));
        require!(
            state.read(|s| Ok(s.counter())) == Err(poisoned.clone()),
            Error::InvalidAccountData
        );
        require!(
            state.edit(|s| s.counter_mut().set(1)) == Err(poisoned.clone()),
            Error::InvalidAccountData
        );
        require!(
            state.exit() == Err(poisoned.clone()),
            Error::InvalidAccountData
        );
        require!(
            AccountClose::close(&mut state, *ctx.accounts.recipient.account())
                == Err(poisoned.clone()),
            Error::InvalidAccountData
        );
        require!(
            AccountRealloc::realloc_account(&mut state, 256, *ctx.accounts.payer.account(), true)
                == Err(poisoned),
            Error::InvalidAccountData
        );
        require!(account.data_len() == 12, Error::InvalidAccountData);
        require!(account.owned_by(&ID), Error::InvalidAccountData);
        require!(
            Blubber::<State, Policy>::load(account).is_err(),
            Error::InvalidAccountData
        );
        Ok(())
    }
}
