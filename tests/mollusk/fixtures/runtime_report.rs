//! Unsubtracted measurements emitted after the shared runtime assertions pass.
#![forbid(unsafe_code)]

use solana_account::Account;
use solana_pubkey::Pubkey;

pub(crate) fn record(
    emit: bool,
    (name, profile, scope): (&str, u8, &str),
    cu: u64,
    before: &[(Pubkey, Account)],
    after: &[(Pubkey, Account)],
) {
    if !emit {
        return;
    }
    let state = Pubkey::new_from_array([8; 32]);
    let payer = Pubkey::new_from_array([9; 32]);
    let find = |accounts: &[(Pubkey, Account)], key| {
        accounts
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, a)| (a.data.len(), a.lamports))
            .unwrap_or_default()
    };
    let (before_len, before_balance) = find(before, state);
    let (after_len, after_balance) = find(after, state);
    let total =
        |accounts: &[(Pubkey, Account)]| accounts.iter().map(|(_, a)| a.data.len()).sum::<usize>();
    println!("runtime,{name}_profile{profile},{profile},{scope},{cu},{before_len},{after_len},{},{},{before_balance},{after_balance},{},{}", total(before), total(after), find(before, payer).1, find(after, payer).1);
}
