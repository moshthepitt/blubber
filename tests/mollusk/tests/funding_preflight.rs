//! Caught rent errors preserve the lease before runtime rollback could hide writes.
#![forbid(unsafe_code)]
use blubber::SliceStore;
use blubber_borrow_resize::funding_preflight::{
    apply, complete, verify, FundingState, FundingStateInit, TailList, TailListInit, TailMap,
    TailMapInit, OPERATIONS,
};
use mollusk_svm::{result::ProgramResult, Mollusk};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program::ID as SYSTEM;

#[path = "support/invalid_rent.rs"]
mod invalid_rent;

#[test]
fn growing_edits_preflight_prepaid_and_topup_before_writes() {
    let Some(artifact) = std::env::var_os("BLUBBER_FUNDING_ELF") else {
        return;
    };
    let owner = Pubkey::new_from_array([7; 32]);
    let key = Pubkey::new_from_array([8; 32]);
    let payer = Pubkey::new_from_array([9; 32]);
    let system = Pubkey::default();
    let modes = if std::env::var_os("BLUBBER_FUNDING_ALLOC").is_some() {
        3
    } else {
        2
    };
    for profile in 0..3 {
        let mut vm = Mollusk::default();
        vm.feature_set.virtual_address_space_adjustments = profile != 0;
        vm.feature_set.account_data_direct_mapping = profile == 2;
        vm.program_cache =
            mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
        vm.add_program(&owner, artifact.to_str().unwrap().trim_end_matches(".so"));
        for operation in 0..OPERATIONS {
            let mut room = [0; 256];
            let mut store = SliceStore::new(&mut room);
            let used = match operation {
                20 => {
                    TailList::initialize(&mut store, TailListInit { items: &[] })
                        .unwrap()
                        .commit()
                        .unwrap()
                        .used_len
                }
                21 => {
                    TailMap::initialize(&mut store, TailMapInit { items: &[] })
                        .unwrap()
                        .commit()
                        .unwrap()
                        .used_len
                }
                _ => {
                    FundingState::initialize(
                        &mut store,
                        FundingStateInit {
                            items: &[7],
                            labels: &["a"],
                            values: &[(1, "a")],
                            optional: None,
                            fixed: &[(1, 7)],
                            tail: "a",
                        },
                    )
                    .unwrap()
                    .commit()
                    .unwrap()
                    .used_len
                }
            } as usize;
            let initial = &room[..used];

            let mut expected = vec![0; 256];
            expected[..used].copy_from_slice(initial);
            let mut store = SliceStore::new(&mut expected);
            apply(&mut store, operation).unwrap();
            let final_used = complete(&mut store, operation).unwrap().used_len as usize;
            let final_len = if operation == 10 {
                used + 8
            } else {
                final_used
            };
            let minimum = vm.sysvars.rent.minimum_balance(final_len);
            let original_rent = vm.sysvars.rent.minimum_balance(used);
            let deficit = minimum - original_rent;
            assert!(deficit > 0);
            for mode in 0..modes {
                // Prepaid short/exact, TopUp one lamport short/exact.
                for case in 0..4 {
                    let reject = case % 2 == 0;
                    let balance = if case == 1 { minimum } else { original_rent };
                    let payer_balance = if case == 2 { deficit - 1 } else { deficit };
                    let accounts = vec![
                        (
                            key,
                            Account {
                                lamports: balance,
                                data: initial.to_vec(),
                                owner,
                                ..Account::default()
                            },
                        ),
                        (
                            payer,
                            Account {
                                lamports: payer_balance,
                                ..Account::default()
                            },
                        ),
                        mollusk_svm::program::keyed_account_for_system_program(),
                    ];
                    let ix = Instruction {
                        program_id: owner,
                        accounts: vec![
                            AccountMeta::new(key, false),
                            AccountMeta::new(payer, true),
                            AccountMeta::new_readonly(system, false),
                        ],
                        data: vec![42, operation, u8::from(case >= 2), mode],
                    };
                    let result = vm.process_instruction(&ix, &accounts);
                    assert_eq!(
                        result.program_result,
                        ProgramResult::Success,
                        "profile={profile} operation={operation} mode={mode} case={case}"
                    );
                    assert_eq!(
                        result.return_data,
                        [u8::from(reject), u8::from(reject || operation == 10), 1]
                    );
                    let state = result.get_account(&key).unwrap();
                    if reject {
                        assert_eq!(state.data, initial);
                        assert_eq!(state.lamports, balance);
                        assert_eq!(result.get_account(&payer).unwrap().lamports, payer_balance);
                    } else {
                        assert_eq!(state.data, expected[..final_len]);
                        assert_eq!(state.lamports, minimum);
                        assert_eq!(
                            result.get_account(&payer).unwrap().lamports,
                            if case == 3 { 0 } else { payer_balance }
                        );
                        verify(&state.data, operation).unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn fixed_overwrite_does_not_query_out_of_range_rent() {
    let Some(artifact) = std::env::var_os("BLUBBER_FUNDING_ELF") else {
        return;
    };
    let owner = Pubkey::new_from_array([7; 32]);
    let key = Pubkey::new_from_array([8; 32]);
    let payer = Pubkey::new_from_array([9; 32]);
    let mut room = [0; 256];
    let used = FundingState::initialize(
        &mut SliceStore::new(&mut room),
        FundingStateInit {
            items: &[7],
            labels: &["a"],
            values: &[(1, "a")],
            optional: None,
            fixed: &[(1, 7)],
            tail: "a",
        },
    )
    .unwrap()
    .commit()
    .unwrap()
    .used_len as usize;
    let modes = if std::env::var_os("BLUBBER_FUNDING_ALLOC").is_some() {
        3
    } else {
        2
    };
    for profile in 0..3 {
        let mut vm = Mollusk::default();
        vm.feature_set.virtual_address_space_adjustments = profile != 0;
        vm.feature_set.account_data_direct_mapping = profile == 2;
        vm.program_cache =
            mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
        vm.add_program(&owner, artifact.to_str().unwrap().trim_end_matches(".so"));
        for mode in 0..modes {
            let accounts = vec![
                (
                    key,
                    Account {
                        lamports: vm.sysvars.rent.minimum_balance(used),
                        data: room[..used].to_vec(),
                        owner,
                        ..Account::default()
                    },
                ),
                (
                    payer,
                    Account {
                        lamports: 1,
                        ..Account::default()
                    },
                ),
                mollusk_svm::program::keyed_account_for_system_program(),
                invalid_rent::sysvar(&vm),
            ];
            let ix = Instruction {
                program_id: owner,
                accounts: vec![
                    AccountMeta::new(key, false),
                    AccountMeta::new(payer, true),
                    AccountMeta::new_readonly(SYSTEM, false),
                ],
                data: vec![42, 22, 0, mode],
            };
            let result = vm.process_instruction(&ix, &accounts);
            assert_eq!(result.program_result, ProgramResult::Success);
            assert_eq!(result.return_data, [0, 1, 1]);
            assert_eq!(result.resulting_accounts, accounts);

            let mut mismatched = accounts.clone();
            let expected = mismatched[0].1.data[..8].to_vec();
            mismatched[0].1.data[0] ^= 1;
            let found = &mismatched[0].1.data[..8];
            let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
            let diagnostic = format!(
                "schema identity: expected={} found={}",
                hex(&expected),
                hex(found)
            );
            let logger = solana_svm_log_collector::LogCollector::new_ref();
            vm.logger = Some(logger.clone());
            let result = vm.process_instruction(&ix, &mismatched);
            assert!(matches!(result.program_result, ProgramResult::Failure(_)));
            assert!(result.return_data.is_empty());
            assert!(logger
                .borrow()
                .get_recorded_content()
                .iter()
                .any(|line| line == &format!("Program log: {diagnostic}")));
            assert_eq!(result.resulting_accounts, mismatched);
        }
    }
}

#[test]
fn initialization_funding_ignores_the_uninitialized_used_word() {
    let Some(artifact) = std::env::var_os("BLUBBER_FUNDING_ELF") else {
        return;
    };
    let owner = Pubkey::new_from_array([7; 32]);
    let key = Pubkey::new_from_array([8; 32]);
    let payer = Pubkey::new_from_array([9; 32]);
    let mut initial = vec![0xa5; 64];
    initial[..8].fill(0);
    for profile in 0..3 {
        let mut vm = Mollusk::default();
        vm.feature_set.virtual_address_space_adjustments = profile != 0;
        vm.feature_set.account_data_direct_mapping = profile == 2;
        vm.program_cache =
            mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
        vm.add_program(&owner, artifact.to_str().unwrap().trim_end_matches(".so"));
        let rent = vm.sysvars.rent.minimum_balance(24);
        let old_rent = vm.sysvars.rent.minimum_balance(16);
        let deficit = rent - old_rent;
        for case in 0..4 {
            let reject = case % 2 == 0;
            let accounts = vec![
                (
                    key,
                    Account {
                        lamports: if case == 1 { rent } else { old_rent },
                        data: initial.clone(),
                        owner,
                        ..Account::default()
                    },
                ),
                (
                    payer,
                    Account {
                        lamports: if case == 2 { deficit - 1 } else { deficit },
                        ..Account::default()
                    },
                ),
                mollusk_svm::program::keyed_account_for_system_program(),
            ];
            let ix = Instruction {
                program_id: owner,
                accounts: vec![
                    AccountMeta::new(key, false),
                    AccountMeta::new(payer, true),
                    AccountMeta::new_readonly(SYSTEM, false),
                ],
                data: vec![43, u8::from(case >= 2)],
            };
            let result = vm.process_instruction(&ix, &accounts);
            assert_eq!(
                result.program_result,
                ProgramResult::Success,
                "profile={profile} case={case}"
            );
            assert_eq!(result.return_data, [u8::from(reject)]);
            if reject {
                assert_eq!(result.resulting_accounts, accounts);
            } else {
                let state = result.get_account(&key).unwrap();
                assert_eq!(state.lamports, rent);
                assert_eq!(state.data.len(), 24);
                assert_eq!(
                    TailList::load(&state.data).unwrap().items().get(0).unwrap(),
                    42
                );
            }
        }
    }
}
