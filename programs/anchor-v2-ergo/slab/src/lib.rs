#![forbid(unsafe_code)]
use anchor_lang::{accounts::Slab, prelude::*};
declare_id!("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");
#[account]
/// Copied Slab header separating list and map slots.
pub struct Header {
    /// Stored authority for edits and closure.
    pub authority: Address,
    /// Number of leading slots belonging to the list.
    pub list_len: u64,
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
    pub state: Slab<Header, [u8; 8]>,
    /// Executable System Program.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
/// Authenticates authority before editing the account.
pub struct Modify {
    #[account(mut)]
    /// Account selected by this instruction.
    pub state: Slab<Header, [u8; 8]>,
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
    pub state: Slab<Header, [u8; 8]>,
    #[account(mut, address = state.authority)]
    /// Authorized writable funding signer.
    pub payer: Signer,
    /// Executable System Program.
    pub system_program: Program<System>,
}
fn grow(state: &mut Slab<Header, [u8; 8]>, payer: &AccountView) -> Result<()> {
    if state.is_full() {
        let capacity = u32::try_from(state.len() + 1).map_err(|_| Error::ArithmeticOverflow)?;
        state.resize_to_capacity(capacity)?;
        state.top_up(payer)?;
    }
    Ok(())
}
fn entry(key: u32, value: u32) -> [u8; 8] {
    let mut slot = [0; 8];
    slot[..4].copy_from_slice(&key.to_le_bytes());
    slot[4..].copy_from_slice(&value.to_le_bytes());
    slot
}
fn list_len(state: &Slab<Header, [u8; 8]>) -> Result<usize> {
    let count = usize::try_from(state.list_len).map_err(|_| Error::InvalidAccountData)?;
    require!(count <= state.len(), Error::InvalidAccountData);
    Ok(count)
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
        state.list_len = u64::from(n);
        for i in 0..n {
            state.try_push(u64::from(i).to_le_bytes())?;
        }
        for i in 0..n {
            state.try_push(entry(i, i))?;
        }
        Ok(())
    }
    #[discrim = 1]
    /// Appends one value while preserving the following map entries.
    pub fn push(ctx: &mut Context<Modify>, value: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let index = list_len(state)?;
        grow(state, ctx.accounts.payer.account())?;
        state.try_push(value.to_le_bytes())?;
        state.as_mut_slice()[index..].rotate_right(1);
        state.list_len += 1;
        Ok(())
    }
    #[discrim = 2]
    /// Inserts or replaces one key in canonical order.
    pub fn insert(ctx: &mut Context<Modify>, key: u32, value: u32) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let start = list_len(state)?;
        let found = state.as_slice()[start..].binary_search_by_key(&key, |slot| {
            u32::from_le_bytes(slot[..4].try_into().unwrap())
        });
        match found {
            Ok(index) => state.as_mut_slice()[start + index] = entry(key, value),
            Err(index) => {
                grow(state, ctx.accounts.payer.account())?;
                state.try_push(entry(key, value))?;
                state.as_mut_slice()[start + index..].rotate_right(1);
            }
        }
        Ok(())
    }
    #[discrim = 3]
    /// Removes a selected list item or map entry.
    pub fn remove(ctx: &mut Context<Modify>, from_map: bool, key: u32) -> Result<()> {
        let state = &mut ctx.accounts.state;
        let start = list_len(state)?;
        let index = if from_map {
            match state.as_slice()[start..].binary_search_by_key(&key, |slot| {
                u32::from_le_bytes(slot[..4].try_into().unwrap())
            }) {
                Ok(index) => start + index,
                Err(_) => return Ok(()),
            }
        } else {
            require!((key as usize) < start, Error::InvalidArgument);
            state.list_len -= 1;
            key as usize
        };
        state.as_mut_slice()[index..].rotate_left(1);
        state.pop();
        Ok(())
    }
    #[discrim = 4]
    /// Returns the complete account balance to its stored authority.
    pub fn close(_ctx: &mut Context<Close>) -> Result<()> {
        Ok(())
    }
}
