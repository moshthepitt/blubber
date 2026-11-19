#![forbid(unsafe_code)]
use anchor_lang::{prelude::*, AccountRealloc};
use std::collections::BTreeMap;
declare_id!("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");
#[account(borsh)]
/// Account containing an authority, list and sorted map.
pub struct State {
    /// Stored authority for edits and closure.
    pub authority: Address,
    /// Ordered list values.
    pub values: Vec<u64>,
    /// Sorted key/value entries.
    pub entries: BTreeMap<u32, u32>,
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
    pub state: BorshAccount<State>,
    /// Executable System Program.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
/// Authenticates authority before editing the account.
pub struct Modify {
    #[account(mut)]
    /// Account selected by this instruction.
    pub state: BorshAccount<State>,
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
    pub state: BorshAccount<State>,
    #[account(mut, address = state.authority)]
    /// Authorized writable funding signer.
    pub payer: Signer,
    /// Executable System Program.
    pub system_program: Program<System>,
}
fn grow(state: &mut BorshAccount<State>, payer: &AccountView) -> Result<()> {
    let needed = 48 + 8 * (state.values.len() + state.entries.len());
    if needed > state.account().data_len() {
        state.realloc_account(needed, *payer, true)?;
    }
    Ok(())
}
#[program]
/// Equivalent public list/map instruction surface.
pub mod example {
    use super::*;
    #[discrim = 0]
    /// Initializes equivalent list and map contents with physical reservation.
    pub fn initialize(ctx: &mut Context<Initialize>, n: u32) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.authority = *ctx.accounts.payer.address();
        state.values = (0..n).map(u64::from).collect();
        state.entries = (0..n).map(|i| (i, i)).collect();
        Ok(())
    }
    #[discrim = 1]
    /// Appends one value while preserving the following map entries.
    pub fn push(ctx: &mut Context<Modify>, value: u64) -> Result<()> {
        ctx.accounts.state.values.push(value);
        grow(&mut ctx.accounts.state, ctx.accounts.payer.account())
    }
    #[discrim = 2]
    /// Inserts or replaces one key in canonical order.
    pub fn insert(ctx: &mut Context<Modify>, key: u32, value: u32) -> Result<()> {
        ctx.accounts.state.entries.insert(key, value);
        grow(&mut ctx.accounts.state, ctx.accounts.payer.account())
    }
    #[discrim = 3]
    /// Removes a selected list item or map entry.
    pub fn remove(ctx: &mut Context<Modify>, from_map: bool, key: u32) -> Result<()> {
        let state = &mut ctx.accounts.state;
        if from_map {
            state.entries.remove(&key);
        } else {
            require!((key as usize) < state.values.len(), Error::InvalidArgument);
            state.values.remove(key as usize);
        }
        Ok(())
    }
    #[discrim = 4]
    /// Returns the complete account balance to its stored authority.
    pub fn close(_ctx: &mut Context<Close>) -> Result<()> {
        Ok(())
    }
}
