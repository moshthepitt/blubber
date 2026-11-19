//! Shared transaction contexts, caller growth ceilings and bank rent checks.
#![forbid(unsafe_code)]

use super::runtime_report::record;
use blubber::{Error, ErrorKind};
use mollusk_svm::{
    result::{types::TransactionProgramResult, Check},
    Mollusk,
};
use solana_account::Account;
use solana_instruction::{error::InstructionError, AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const OWNER: Pubkey = Pubkey::new_from_array([7; 32]);
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);
const PAYER: Pubkey = Pubkey::new_from_array([9; 32]);
const SYSTEM: Pubkey = Pubkey::new_from_array([0; 32]);
const INITIAL: usize = 128;
const WINDOW: usize = 10_240;
const MAX: usize = 10_485_760;

fn runtime(profile: u8) -> Mollusk {
    let mut vm = Mollusk::default();
    vm.feature_set.virtual_address_space_adjustments = profile != 0;
    vm.feature_set.account_data_direct_mapping = profile == 2;
    vm.program_cache =
        mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
    vm.add_program(&OWNER, "blubber_borrow_resize_runtime");
    vm
}

fn account(key: Pubkey, owner: Pubkey, len: usize, lamports: u64) -> (Pubkey, Account) {
    let mut data = vec![0; len];
    if len >= 13 {
        data[..8].copy_from_slice(b"LEASE001");
        data[8..12].copy_from_slice(&(len as u32).to_le_bytes());
        data[12] = 19;
    }
    (
        key,
        Account {
            lamports,
            data,
            owner,
            ..Account::default()
        },
    )
}

fn instruction(action: u8, amount: usize) -> Instruction {
    let mut data = vec![80, action];
    data.extend_from_slice(&(amount as u32).to_le_bytes());
    Instruction {
        program_id: OWNER,
        accounts: vec![
            AccountMeta::new(STATE, false),
            AccountMeta::new_readonly(OWNER, false),
        ],
        data,
    }
}

fn inputs(balance: u64) -> Vec<(Pubkey, Account)> {
    vec![
        account(STATE, OWNER, INITIAL, balance),
        account(PAYER, SYSTEM, 0, 100_000_000_000),
        (
            OWNER,
            mollusk_svm::program::create_program_account_loader_v2(&[]),
        ),
        mollusk_svm::program::keyed_account_for_system_program(),
    ]
}

// System Program's pinned bincode wire tags: Transfer=2, Allocate=8, Assign=1.
// Host construction is independent of the SBF Pinocchio CPI implementation.
fn system(tag: u32, key: Pubkey, argument: &[u8]) -> Instruction {
    let mut data = tag.to_le_bytes().to_vec();
    data.extend_from_slice(argument);
    Instruction {
        program_id: SYSTEM,
        accounts: vec![AccountMeta::new(key, true)],
        data,
    }
}
fn transfer(lamports: u64) -> Instruction {
    let mut ix = system(2, PAYER, &lamports.to_le_bytes());
    ix.accounts.push(AccountMeta::new(STATE, false));
    ix
}
fn assert_bytes(accounts: &[(Pubkey, Account)], key: Pubkey, len: usize) {
    let data = &accounts.iter().find(|(k, _)| *k == key).unwrap().1.data;
    assert_eq!(data.len(), len);
    assert_eq!(&data[..8], b"LEASE001");
    assert_eq!(
        u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize,
        len
    );
    assert_eq!(data[12], 42);
    assert!(data[INITIAL.min(len)..].iter().all(|byte| *byte == 0));
}

pub(crate) fn fresh_top_level_windows_accumulate_in_one_transaction(emit: bool) {
    for profile in 0..3 {
        let vm = runtime(profile);
        let accounts = inputs(100_000_000_000);
        let result = vm.process_transaction_instructions(
            &[
                instruction(0, INITIAL + WINDOW),
                instruction(0, INITIAL + 2 * WINDOW),
            ],
            &accounts,
            Some(&PAYER),
        );
        assert!(
            result.program_result.is_ok(),
            "{profile}: {:?}",
            result.program_result
        );
        assert_bytes(&result.resulting_accounts, STATE, INITIAL + 2 * WINDOW);
        record(
            emit,
            ("fresh_windows", profile, "transaction"),
            result.compute_units_consumed,
            &accounts,
            &result.resulting_accounts,
        );
        let result = vm.process_transaction_instructions(
            &[
                instruction(0, INITIAL + WINDOW),
                instruction(0, INITIAL + 2 * WINDOW + 1),
            ],
            &accounts,
            Some(&PAYER),
        );
        let expected: Error = ErrorKind::GrowthLimit.into();
        assert_eq!(
            result.program_result,
            TransactionProgramResult::Failure(1, expected.into())
        );
        assert_eq!(
            result.resulting_accounts, accounts,
            "whole transaction rollback"
        );
        record(
            emit,
            ("fresh_windows_rollback", profile, "transaction"),
            result.compute_units_consumed,
            &accounts,
            &result.resulting_accounts,
        );
    }
}

pub(crate) fn cpi_return_obeys_the_callers_original_window(emit: bool) {
    for profile in 0..3 {
        let vm = runtime(profile);
        let accounts = inputs(100_000_000_000);
        for action in [1, 2] {
            let result = vm.process_and_validate_instruction(
                &instruction(action, WINDOW),
                &accounts,
                &[Check::success(), Check::inner_instruction_count(1)],
            );
            assert_bytes(
                &result.resulting_accounts,
                STATE,
                if action == 2 {
                    INITIAL - 1
                } else {
                    INITIAL + WINDOW
                },
            );
            record(
                emit,
                (
                    if action == 2 {
                        "cpi_shrink"
                    } else {
                        "cpi_exact_window"
                    },
                    profile,
                    "instruction",
                ),
                result.compute_units_consumed,
                &accounts,
                &result.resulting_accounts,
            );
        }
        let result = vm.process_instruction(&instruction(1, WINDOW + 1), &accounts);
        assert_eq!(result.raw_result, Err(InstructionError::InvalidRealloc));
        assert_eq!(result.resulting_accounts, accounts);
        record(
            emit,
            ("cpi_window_rollback", profile, "instruction"),
            result.compute_units_consumed,
            &accounts,
            &result.resulting_accounts,
        );
    }
}

pub(crate) fn later_instruction_failure_reverts_commits_and_system_transfers(emit: bool) {
    for profile in 0..3 {
        let vm = runtime(profile);
        let accounts = inputs(100_000_000_000);
        let result = vm.process_transaction_instructions(
            &[
                transfer(123),
                instruction(0, INITIAL + 100),
                instruction(3, 7),
            ],
            &accounts,
            Some(&PAYER),
        );
        assert_eq!(
            result.program_result,
            TransactionProgramResult::Failure(2, pinocchio::error::ProgramError::Custom(0x4040))
        );
        assert_eq!(result.resulting_accounts, accounts);
        record(
            emit,
            ("commit_transfer_rollback", profile, "transaction"),
            result.compute_units_consumed,
            &accounts,
            &result.resulting_accounts,
        );
    }
}

pub(crate) fn prepaid_requires_funding_before_commit_even_when_bank_rent_is_deferred(emit: bool) {
    for profile in 0..3 {
        let vm = runtime(profile);
        let target = INITIAL + 100;
        let before = vm.sysvars.rent.minimum_balance(INITIAL);
        let after = vm.sysvars.rent.minimum_balance(target);
        let accounts = inputs(before);
        let failed = vm.process_transaction_instructions(
            &[instruction(0, target), transfer(after - before)],
            &accounts,
            Some(&PAYER),
        );
        let error: Error = ErrorKind::RentShortfall.into();
        assert_eq!(
            failed.program_result,
            TransactionProgramResult::Failure(0, error.into())
        );
        assert_eq!(failed.resulting_accounts, accounts);
        record(
            emit,
            ("prepaid_late_funding_rollback", profile, "transaction"),
            failed.compute_units_consumed,
            &accounts,
            &failed.resulting_accounts,
        );
        for instructions in [
            [transfer(after - before), instruction(0, target)],
            [instruction(4, target), transfer(after - before)],
        ] {
            let result =
                vm.process_transaction_instructions(&instructions, &accounts, Some(&PAYER));
            assert!(result.program_result.is_ok(), "{:?}", result.program_result);
            assert_bytes(&result.resulting_accounts, STATE, target);
            assert_eq!(
                result
                    .resulting_accounts
                    .iter()
                    .find(|(key, _)| *key == STATE)
                    .unwrap()
                    .1
                    .lamports,
                after
            );
            record(
                emit,
                (
                    if instructions[0].program_id == SYSTEM {
                        "fund_then_commit"
                    } else {
                        "raw_grow_then_fund"
                    },
                    profile,
                    "transaction",
                ),
                result.compute_units_consumed,
                &accounts,
                &result.resulting_accounts,
            );
        }
        let mut vm = runtime(profile);
        vm.config.panic = false;
        let result =
            vm.process_transaction_instructions(&[instruction(4, target)], &accounts, Some(&PAYER));
        assert!(matches!(
            result.program_result,
            TransactionProgramResult::TransactionError(_)
        ));
        assert!(vm.config.rent_exempt_checks);
        record(
            emit,
            ("bank_rent_rejection", profile, "transaction"),
            result.compute_units_consumed,
            &accounts,
            &result.resulting_accounts,
        );
    }
}

pub(crate) fn signed_aggregate_growth_is_twenty_mib_and_shrink_restores_budget(emit: bool) {
    for profile in 0..3 {
        let vm = runtime(profile);
        let second = Pubkey::new_from_array([10; 32]);
        let third = Pubkey::new_from_array([11; 32]);
        let mut accounts = inputs(100_000_000_000);
        accounts[0] = account(STATE, SYSTEM, 0, 100_000_000_000);
        accounts.push(account(second, SYSTEM, 0, 100_000_000_000));
        accounts.push(account(third, SYSTEM, 0, 100_000_000_000));
        let allocate = |key, len: usize| system(8, key, &(len as u64).to_le_bytes());
        let mut instructions = vec![allocate(STATE, MAX), allocate(second, MAX)];
        let result = vm.process_transaction_instructions(&instructions, &accounts, Some(&PAYER));
        assert!(result.program_result.is_ok(), "{:?}", result.program_result);
        record(
            emit,
            ("aggregate_exact_20mib", profile, "transaction"),
            result.compute_units_consumed,
            &accounts,
            &result.resulting_accounts,
        );
        instructions.push(allocate(third, 1));
        let result = vm.process_transaction_instructions(&instructions, &accounts, Some(&PAYER));
        assert_eq!(
            result.program_result,
            TransactionProgramResult::Failure(
                2,
                pinocchio::error::ProgramError::MaxAccountsDataAllocationsExceeded
            )
        );
        assert_eq!(result.resulting_accounts, accounts);
        record(
            emit,
            ("aggregate_rollback", profile, "transaction"),
            result.compute_units_consumed,
            &accounts,
            &result.resulting_accounts,
        );
        instructions.pop();
        instructions.push(system(1, STATE, OWNER.as_ref()));
        instructions.push(instruction(0, MAX - 1));
        instructions.push(allocate(third, 1));
        let result = vm.process_transaction_instructions(&instructions, &accounts, Some(&PAYER));
        assert!(result.program_result.is_ok(), "{:?}", result.program_result);
        assert_eq!(
            result
                .resulting_accounts
                .iter()
                .find(|(key, _)| *key == third)
                .unwrap()
                .1
                .data
                .len(),
            1
        );
        record(
            emit,
            ("aggregate_shrink_reuse", profile, "transaction"),
            result.compute_units_consumed,
            &accounts,
            &result.resulting_accounts,
        );
    }
}

pub(crate) fn duplicate_metas_merge_privileges_and_share_genuine_guard_exclusion(emit: bool) {
    for profile in 0..3 {
        let vm = runtime(profile);
        let accounts = inputs(100_000_000_000);
        for flags in [[false, true], [true, false], [true, true]] {
            let ix = Instruction {
                program_id: OWNER,
                accounts: flags
                    .into_iter()
                    .map(|writable| {
                        if writable {
                            AccountMeta::new(STATE, false)
                        } else {
                            AccountMeta::new_readonly(STATE, false)
                        }
                    })
                    .collect(),
                data: vec![16],
            };
            let result = vm.process_and_validate_instruction(&ix, &accounts, &[Check::success()]);
            assert_eq!(result.resulting_accounts, accounts);
            let name = match flags {
                [false, true] => "duplicate_ro_rw",
                [true, false] => "duplicate_rw_ro",
                _ => "duplicate_rw_rw",
            };
            record(
                emit,
                (name, profile, "instruction"),
                result.compute_units_consumed,
                &accounts,
                &result.resulting_accounts,
            );
        }
    }
}

/// Equal-width editing still needs no transaction growth when reservation is zero.
#[allow(dead_code)]
pub(crate) fn exhausted_transaction_budget_rejects_max_but_allows_zero(emit: bool) {
    // Exercise both gated mapping profiles.
    for profile in 1..3 {
        let vm = runtime(profile);
        let second = Pubkey::new_from_array([10; 32]);
        let third = Pubkey::new_from_array([11; 32]);
        let mut accounts = inputs(100_000_000_000);
        accounts[0].1.data[8..12].copy_from_slice(&16u32.to_le_bytes());
        accounts.push(account(second, SYSTEM, 0, 100_000_000_000));
        accounts.push(account(third, SYSTEM, 0, 100_000_000_000));
        for action in [5, 6, 7] {
            let instructions = [
                system(8, second, &(MAX as u64).to_le_bytes()),
                system(8, third, &(MAX as u64).to_le_bytes()),
                instruction(action, 0),
            ];
            let result =
                vm.process_transaction_instructions(&instructions, &accounts, Some(&PAYER));
            if action == 5 {
                assert_eq!(
                    result.program_result,
                    TransactionProgramResult::Failure(
                        2,
                        pinocchio::error::ProgramError::InvalidRealloc
                    ),
                    "profile={profile}"
                );
                assert_eq!(result.resulting_accounts, accounts);
            } else {
                assert!(
                    result.program_result.is_ok(),
                    "profile={profile}: {:?}",
                    result.program_result
                );
                let state = &result
                    .resulting_accounts
                    .iter()
                    .find(|(key, _)| *key == STATE)
                    .unwrap()
                    .1
                    .data;
                assert_eq!(&state[8..12], &16u32.to_le_bytes());
                assert_eq!(&state[12..16], &42u32.to_le_bytes());
            }
            record(
                emit,
                (
                    "exhausted_reservation",
                    profile,
                    match action {
                        5 => "max",
                        6 => "zero",
                        _ => "default",
                    },
                ),
                result.compute_units_consumed,
                &accounts,
                &result.resulting_accounts,
            );
        }
    }
}
