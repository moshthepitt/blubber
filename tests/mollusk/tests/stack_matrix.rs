//! Execute every retained stack-matrix call graph through the real SBF runtime.

#![forbid(unsafe_code)]

use mollusk_svm::{result::Check, Mollusk};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

#[test]
fn all_depths_widths_and_edit_families_execute_within_sbf_frames() {
    let owner = Pubkey::new_from_array([7; 32]);
    let state = Pubkey::new_from_array([8; 32]);
    for depth in [8, 16] {
        for width in ["narrow", "medium", "wide"] {
            for mode in ["no-alloc", "alloc"] {
                let mut mollusk = Mollusk::default();
                mollusk.add_program(
                    &owner,
                    &format!("blubber_stack_matrix_{depth}_{width}_{mode}"),
                );
                let ix = |data| Instruction {
                    program_id: owner,
                    accounts: vec![AccountMeta::new(state, false)],
                    data,
                };
                let initial = mollusk.process_and_validate_instruction(
                    &ix(vec![0]),
                    &[(
                        state,
                        Account {
                            lamports: mollusk.sysvars.rent.minimum_balance(10_240),
                            data: vec![0; 10_240],
                            owner,
                            ..Account::default()
                        },
                    )],
                    &[Check::success()],
                );
                for (family, count) in [(1, 8), (2, 11), (3, 11), (4, 9), (5, 9), (6, 1)] {
                    for op in 0..count {
                        let result = mollusk.process_and_validate_instruction(
                            &ix(vec![family, op]),
                            &initial.resulting_accounts,
                            &[Check::success(), Check::inner_instruction_count(0)],
                        );
                        let account = result.get_account(&state).unwrap();
                        assert!(
                            account.lamports
                                >= mollusk.sysvars.rent.minimum_balance(account.data.len())
                        );
                        assert_eq!(account.owner, owner);
                    }
                }
            }
        }
    }
}
