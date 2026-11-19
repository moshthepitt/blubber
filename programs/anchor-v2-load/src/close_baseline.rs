//! Pinned Anchor Account and Slab close/init-if-needed baselines.
use super::*;
use anchor_lang::accounts::Slab;

#[derive(Accounts)]
/// Stock Account creation or reuse with the native fixture's PDA seeds.
pub struct ReuseAccount {
    #[account(mut)]
    /// Pays creation and supplies the initial authority.
    pub payer: Signer,
    #[account(init_if_needed, payer = payer, space = 256, seeds = [b"native-init"], bump)]
    /// Fixed native Anchor layout.
    pub state: Account<Baseline>,
    /// Used on creation only.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
/// Stock Account successful-exit close with the same authority check.
pub struct CloseAccount {
    #[account(mut, close = recipient)]
    /// Account closed at successful exit.
    pub state: Account<Baseline>,
    #[account(mut, address = state.authority)]
    /// Authorizes close.
    pub payer: Signer,
    #[account(mut)]
    /// Receives the entire balance.
    pub recipient: UncheckedAccount,
}
#[derive(Accounts)]
/// Stock Slab creation or reuse with the same PDA and allocation.
pub struct ReuseSlab {
    #[account(mut)]
    /// Pays creation and supplies the initial authority.
    pub payer: Signer,
    #[account(init_if_needed, payer = payer, space = 256, seeds = [b"native-init"], bump)]
    /// Fixed native header with an unused slot tail.
    pub state: Slab<Baseline, [u8; 8]>,
    /// Used on creation only.
    pub system_program: Program<System>,
}
#[derive(Accounts)]
/// Stock Slab successful-exit close with the same authority check.
pub struct CloseSlab {
    #[account(mut, close = recipient)]
    /// Account closed at successful exit.
    pub state: Slab<Baseline, [u8; 8]>,
    #[account(mut, address = state.authority)]
    /// Authorizes close.
    pub payer: Signer,
    #[account(mut)]
    /// Receives the entire balance.
    pub recipient: UncheckedAccount,
}
