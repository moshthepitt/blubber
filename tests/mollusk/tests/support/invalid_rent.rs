//! Overrides the program's rent sysvar while keeping the bank's rent valid.

#![forbid(unsafe_code)]

use mollusk_svm::Mollusk;
use solana_account::Account;
use solana_pubkey::Pubkey;

pub(super) fn sysvar(mollusk: &Mollusk) -> (Pubkey, Account) {
    const OUT_OF_RANGE_RATE: u64 = 1_759_197_129_868;
    let (key, mut account) = mollusk.sysvars.keyed_account_for_rent_sysvar();
    account.data[..8].copy_from_slice(&OUT_OF_RANGE_RATE.to_le_bytes());
    (key, account)
}
