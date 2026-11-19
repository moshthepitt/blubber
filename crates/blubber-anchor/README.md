# Blubber account adapter for Anchor

`blubber-anchor` is an unpublished workspace crate for native account fields.
It uses the exact SDK revision
`7d68eb325f297013a5e0903ddd698fe26218f5a3`, which supplies safe mutable load.
The published `2.0.0-rc.2` SDK supports the raw-account recipe below but lacks
the native adapter's required safe mutable load. The adapter is separate from
the three published Blubber crates.

## Choose a load policy

`Blubber<State>` authenticates owner, schema identity and envelope, then checks
copied fixed fields before constraints. Variable fields rely on valid
current-schema writer history and are checked when touched.
`Blubber<State, Checked>` scans the entire active root on load.
Import `Blubber` and `Checked` from `blubber_anchor`. `verify-on-load` adds
full scans to lazy loads and initialization. `verify()` scans a ready
root and preserves terminal access state. See the [trust model](../../docs/trust-model.md).

Check application authority in constraints. Ownership and schema identity do
not decide which signer may change state. The schema implements
`blubber_anchor::sdk::Owner`; copied fixed fields are available for constraints,
and variable data stays in read/edit closures.

## Published SDK raw recipe

For a program laid out like `programs/anchor-v2-raw`, use:

```toml
[dependencies]
anchor-lang = "2.0.0-rc.2"
blubber = { path = "../../crates/blubber", default-features = false, features = ["derive", "pinocchio"] }
```

The local path selects this checkout. These version strings use Cargo's default
compatible range. Pin the program's complete dependency graph in its lockfile.
The [raw fixture](../../programs/anchor-v2-raw/) exercises this extracted recipe:

```rust
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

}
```

Scoped helpers authenticate owner and identity. `with_read`/`with_edit` use
lazy entry; their checked twins scan before the callback. `with_edit_options`
and `with_edit_checked_options` accept explicit `EditOptions`. Default headroom
is `UpTo(0)`, including when funding is TopUp. Propagate errors: successful
callbacks commit once, while errors preserve completed edits and skip settlement.

## Native program

For a program laid out like `programs/anchor-v2-demo`, use:

```toml
[dependencies]
blubber-anchor = { path = "../../crates/blubber-anchor", version = "0.1.0" }
anchor-lang = { git = "https://github.com/solana-foundation/anchor", rev = "7d68eb325f297013a5e0903ddd698fe26218f5a3" }
blubber = { path = "../../crates/blubber", default-features = false, features = ["derive", "alloc", "pinocchio"] }
solana-address = { version = "2.8.0", default-features = false }
solana-instruction = { version = "3.5.1", default-features = false }
```

The [native fixture](../../programs/anchor-v2-demo/) and its lockfile bind the
exact revision. The docs lane compiles this program and the lifecycle additions
below directly from these fences.

<!-- anchor-native -->
```rust
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
```

Initialization starts in New state. Call `initialize(init)` before typed access
or successful exit. `initialize_if_new(init)` ignores input on reuse; ownership,
application authority, physical-space and rent checks still apply. Lazy
initialization can omit its post-write scan for a fully covered encoder graph;
checked policy and `verify-on-load` retain it. `State::space(reserve)` includes
the empty root and twelve-byte envelope. Generic nested schemas are supported;
account roots are concrete.

`read` scopes borrowed data to the callback. `edit` uses Prepaid funding unless
both `funding::payer` and `funding::system_program` are bound. Bindings enable
TopUp with at most one net transfer; default headroom remains `UpTo(0)`.
`edit_with(EditOptions { ... })` selects an explicit budget or refund. Opening
allocation is retained unless `shrink_to_fit` explicitly clears that floor.
Final trim is best-effort; rent uses actual physical length. `shrink_error` is
diagnostic, while settlement failures return errors. Propagate callback and
settlement failures for transaction rollback.

## Close and exact realloc

Use one sizing authority per handler. An exact realloc constraint sizes and
settles before the handler, forces subsequent edits to `UpTo(0)` and preserves
that exact allocation. Managed-growth handlers use the lease engine. Close
requires an authorized distinct writable recipient, checked balance addition
and released data borrows; it invalidates typed access and transfers once.

<!-- anchor-lifecycle -->
```rust
#[derive(Accounts)]
/// Closes state and returns its full balance to the stored authority.
pub struct Close {
    #[account(mut, close = authority)]
    /// Authenticated state; closed after successful handler return.
    pub state: Blubber<State>,
    #[account(mut, address = state.authority)]
    /// Authorizes close and receives its lamports.
    pub authority: Signer,
}

/// Use as a program handler; the constraint performs the close at exit.
pub fn close(_ctx: &mut Context<Close>) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
#[instruction(_new_space: u32)]
/// Selects exact allocation and settles rent before the handler.
pub struct Resize {
    #[account(mut, realloc = _new_space as usize,
        realloc_payer = authority, realloc_zero = true)]
    /// The requested allocation must fit the complete used encoding.
    pub state: Blubber<State>,
    #[account(mut, address = state.authority)]
    /// Pays a rent deficit or receives all excess above final rent.
    pub authority: Signer,
    /// Executable System Program for a possible rent top-up.
    pub system_program: Program<System>,
}

/// Use as a program handler; exact sizing has already completed on entry.
pub fn resize(_ctx: &mut Context<Resize>, _new_space: u32) -> Result<()> {
    Ok(())
}
```

## CPI and retained handle slots

Finish reads and edits before `with_cpi` or `with_cpi_read`. Each requires
`&mut state`, releases the data guard for a scoped checked framework handle,
then fully reloads on every returned path, including callback/preflight errors.
Failed reacquisition/reload and destructive poison disable typed access
terminally. Preserve the original operation error if cleanup also fails.

Native loads need a working allocator. Each load retains an eight-byte handle
slot on SBF; initialization, edits, realloc and CPI handoffs retain another
slot when they pass preflight. OnDemand growth attempts add slots, including
failed attempts. Count all reopens, failed loads and other allocations together.
Drop releases exclusion; forgetting a wrapper retains it until invocation
teardown. The handle slots do not contain account bytes. A 32 KiB bump heap
has at most 4,095 slots before any other allocations or alignment costs.

## Client bytes and IDL

Format 1 is opaque to the stock account decoder. Instruction metadata remains
available, but clients need the Blubber schema/descriptor to decode account
bytes. Do not interpret the packed payload as native Rust memory or a Borsh
account. Account identity binds the canonical schema and semantic epoch.
See [identity and migration](../../docs/trust-model.md#account-identity) and
[SPEC.md](../../SPEC.md) for the format.

## Native costs

Retained whole-instruction measurements on platform-tools v1.53 / Mollusk
0.16.2; each cell is v0/v3 CU. Off/on selects `verify-on-load`. All three memory
mappings run complete-byte, allocation and rent checks. These use the default
runtime meter and 32 KiB heap, separately from the 256 KiB container comparison.
Sources: [v0](../../benchmarks/anchor-v2-ergo-v0.csv),
[v3](../../benchmarks/anchor-v2-ergo-v3.csv) and
[retained receipt](../../verification/results/policy/anchor-costs.log).

| Profile | n | Operation | Lazy off | Checked off | Lazy on | Checked on | Borsh | Slab |
|---|---|---|---|---|---|---|---|---|
| 0 | 10 | init | 3,531/3,527 | 6,006/5,970 | 8,485/8,418 | 8,485/8,418 | 3,462/3,460 | 2,107/2,105 |
| 0 | 10 | push | 3,781/3,789 | 6,253/6,229 | 11,224/11,136 | 11,224/11,136 | 2,956/2,956 | 533/533 |
| 0 | 10 | insert | 2,636/2,625 | 5,108/5,065 | 10,165/10,058 | 10,165/10,058 | 3,081/3,081 | 575/575 |
| 0 | 10 | remove_list | 3,475/3,474 | 6,055/6,022 | 11,255/11,158 | 11,255/11,158 | 3,100/3,100 | 521/521 |
| 0 | 10 | remove_map | 4,001/3,985 | 6,581/6,533 | 11,669/11,557 | 11,669/11,557 | 3,160/3,160 | 610/610 |
| 0 | 10 | close | 690/690 | 3,172/3,140 | 3,172/3,140 | 3,172/3,140 | 2,281/2,281 | 330/330 |
| 0 | 100 | init | 8,661/8,657 | 20,856/20,820 | 33,055/32,988 | 33,055/32,988 | 12,743/12,738 | 5,707/5,705 |
| 0 | 100 | push | 3,781/3,789 | 15,973/15,949 | 40,384/40,296 | 40,384/40,296 | 37,060/37,060 | 533/533 |
| 0 | 100 | insert | 2,636/2,625 | 14,828/14,785 | 39,325/39,218 | 39,325/39,218 | 37,325/37,325 | 617/617 |
| 0 | 100 | remove_list | 3,594/3,593 | 15,894/15,861 | 40,534/40,437 | 40,534/40,437 | 37,344/37,344 | 521/521 |
| 0 | 100 | remove_map | 4,001/3,985 | 16,301/16,253 | 40,829/40,717 | 40,829/40,717 | 37,474/37,474 | 646/646 |
| 0 | 100 | close | 690/690 | 12,892/12,860 | 12,892/12,860 | 12,892/12,860 | 34,192/34,192 | 330/330 |
| 1 | 10 | init | 3,531/3,527 | 6,006/5,970 | 8,485/8,418 | 8,485/8,418 | 3,462/3,460 | 2,107/2,105 |
| 1 | 10 | push | 3,781/3,789 | 6,253/6,229 | 11,224/11,136 | 11,224/11,136 | 2,956/2,956 | 533/533 |
| 1 | 10 | insert | 2,636/2,625 | 5,108/5,065 | 10,165/10,058 | 10,165/10,058 | 3,081/3,081 | 575/575 |
| 1 | 10 | remove_list | 3,475/3,474 | 6,055/6,022 | 11,255/11,158 | 11,255/11,158 | 3,100/3,100 | 521/521 |
| 1 | 10 | remove_map | 4,001/3,985 | 6,581/6,533 | 11,669/11,557 | 11,669/11,557 | 3,160/3,160 | 610/610 |
| 1 | 10 | close | 690/690 | 3,172/3,140 | 3,172/3,140 | 3,172/3,140 | 2,281/2,281 | 330/330 |
| 1 | 100 | init | 8,661/8,657 | 20,856/20,820 | 33,055/32,988 | 33,055/32,988 | 12,743/12,738 | 5,707/5,705 |
| 1 | 100 | push | 3,781/3,789 | 15,973/15,949 | 40,384/40,296 | 40,384/40,296 | 37,060/37,060 | 533/533 |
| 1 | 100 | insert | 2,636/2,625 | 14,828/14,785 | 39,325/39,218 | 39,325/39,218 | 37,325/37,325 | 617/617 |
| 1 | 100 | remove_list | 3,594/3,593 | 15,894/15,861 | 40,534/40,437 | 40,534/40,437 | 37,344/37,344 | 521/521 |
| 1 | 100 | remove_map | 4,001/3,985 | 16,301/16,253 | 40,829/40,717 | 40,829/40,717 | 37,474/37,474 | 646/646 |
| 1 | 100 | close | 690/690 | 12,892/12,860 | 12,892/12,860 | 12,892/12,860 | 34,192/34,192 | 330/330 |
| 2 | 10 | init | 3,531/3,527 | 6,006/5,970 | 8,485/8,418 | 8,485/8,418 | 3,462/3,460 | 2,107/2,105 |
| 2 | 10 | push | 3,781/3,789 | 6,253/6,229 | 11,224/11,136 | 11,224/11,136 | 2,956/2,956 | 533/533 |
| 2 | 10 | insert | 2,636/2,625 | 5,108/5,065 | 10,165/10,058 | 10,165/10,058 | 3,081/3,081 | 575/575 |
| 2 | 10 | remove_list | 3,475/3,474 | 6,055/6,022 | 11,255/11,158 | 11,255/11,158 | 3,100/3,100 | 521/521 |
| 2 | 10 | remove_map | 4,001/3,985 | 6,581/6,533 | 11,669/11,557 | 11,669/11,557 | 3,160/3,160 | 610/610 |
| 2 | 10 | close | 690/690 | 3,172/3,140 | 3,172/3,140 | 3,172/3,140 | 2,281/2,281 | 330/330 |
| 2 | 100 | init | 8,661/8,657 | 20,856/20,820 | 33,055/32,988 | 33,055/32,988 | 12,743/12,738 | 5,707/5,705 |
| 2 | 100 | push | 3,781/3,789 | 15,973/15,949 | 40,384/40,296 | 40,384/40,296 | 37,060/37,060 | 533/533 |
| 2 | 100 | insert | 2,636/2,625 | 14,828/14,785 | 39,325/39,218 | 39,325/39,218 | 37,325/37,325 | 617/617 |
| 2 | 100 | remove_list | 3,594/3,593 | 15,894/15,861 | 40,534/40,437 | 40,534/40,437 | 37,344/37,344 | 521/521 |
| 2 | 100 | remove_map | 4,001/3,985 | 16,301/16,253 | 40,829/40,717 | 40,829/40,717 | 37,474/37,474 | 646/646 |
| 2 | 100 | close | 690/690 | 12,892/12,860 | 12,892/12,860 | 12,892/12,860 | 34,192/34,192 | 330/330 |

The repository contains the [load](../../programs/anchor-v2-load/),
[initialization](../../programs/anchor-v2-demo/) and
[editing](../../programs/anchor-v2-ergo/) example programs.