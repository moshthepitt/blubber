#![forbid(unsafe_code)]
use anchor_lang::prelude::*;
use blubber::{List, Map};
use blubber_anchor::funding;
use blubber_anchor::Blubber;
#[cfg(feature = "checked-policy")]
use blubber_anchor::Checked as Policy;
#[cfg(not(feature = "checked-policy"))]
use blubber_anchor::Lazy as Policy;
declare_id!("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");
#[blubber::schema(account)]
/// Account containing an authority, list and sorted map.
pub struct State {
    /// Stored authority for edits and closure.
    pub authority: Address,
    /// Ordered list values.
    pub values: List<u64>,
    /// Sorted key/value entries.
    pub entries: Map<u32, u32>,
}
impl Owner for State {
    const OWNER: Address = ID;
}
#[derive(Accounts)]
#[instruction(_n: u32)]
/// Creates a signer account and reserves equal physical space.
pub struct Initialize {
    #[account(mut)]
    /// Authorized writable funding signer.
    pub payer: Signer,
    #[account(init, payer = payer, space = 128 + 16 * _n as usize)]
    /// Account selected by this instruction.
    pub state: Blubber<State, Policy>,
    /// Executable System Program.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
/// Authenticates authority before editing the account.
pub struct Modify {
    #[account(mut, funding::payer = payer, funding::system_program = system_program)]
    /// Account selected by this instruction.
    pub state: Blubber<State, Policy>,
    #[account(mut, address = state.authority)]
    /// Authorized writable funding signer.
    pub payer: Signer,
    /// Executable System Program.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
/// Authorizes successful-exit closure to the stored authority.
pub struct Close {
    #[account(mut, close = payer)]
    /// Account selected by this instruction.
    pub state: Blubber<State, Policy>,
    #[account(mut, address = state.authority)]
    /// Authorized writable funding signer.
    pub payer: Signer,
    /// Executable System Program.
    pub system_program: Program<System>,
}
#[program]
/// Equivalent public list/map instruction surface.
pub mod example {
    use super::*;
    #[discrim = 0]
    /// Initializes equivalent list and map contents with physical reservation.
    pub fn initialize(ctx: &mut Context<Initialize>, n: u32) -> Result<()> {
        let values: Vec<u64> = (0..n).map(u64::from).collect();
        let entries: Vec<(u32, u32)> = (0..n).map(|i| (i, i)).collect();
        ctx.accounts.state.initialize(StateInit {
            authority: *ctx.accounts.payer.address(),
            values: &values,
            entries: &entries,
        })?;
        Ok(())
    }
    #[discrim = 1]
    /// Appends one value while preserving the following map entries.
    pub fn push(ctx: &mut Context<Modify>, value: u64) -> Result<()> {
        ctx.accounts.state.edit(|s| s.values_mut().push(value))?;
        Ok(())
    }
    #[discrim = 2]
    /// Inserts or replaces one key in canonical order.
    pub fn insert(ctx: &mut Context<Modify>, key: u32, value: u32) -> Result<()> {
        ctx.accounts
            .state
            .edit(|s| s.entries_mut().insert(key, value).map(|_| ()))?;
        Ok(())
    }
    #[discrim = 3]
    /// Removes a selected list item or map entry.
    pub fn remove(ctx: &mut Context<Modify>, from_map: bool, key: u32) -> Result<()> {
        ctx.accounts.state.edit(|s| {
            if from_map {
                s.entries_mut().remove(&key)?;
            } else {
                s.values_mut().remove(key as usize)?;
            }
            Ok(())
        })?;
        Ok(())
    }
    #[discrim = 4]
    /// Returns the complete account balance to its stored authority.
    pub fn close(_ctx: &mut Context<Close>) -> Result<()> {
        Ok(())
    }
}
