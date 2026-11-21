//! One-call creation, scoped guards and callback completion through SBF.

#![forbid(unsafe_code)]

use blubber::{Error, ErrorKind};
use mollusk_svm::{
    result::{Check, InstructionResult},
    Mollusk,
};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program::ID as SYSTEM;

#[path = "support/rent.rs"]
mod rent_check;

const OWNER: Pubkey = Pubkey::new_from_array([7; 32]);
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);
const PAYER: Pubkey = Pubkey::new_from_array([9; 32]);

fn runtime(profile: u8) -> Mollusk {
    let mut mollusk = Mollusk::default();
    mollusk.feature_set.virtual_address_space_adjustments = profile != 0;
    mollusk.feature_set.account_data_direct_mapping = profile == 2;
    mollusk.program_cache = mollusk_svm::program::ProgramCache::new(
        &mollusk.feature_set,
        &mollusk.compute_budget,
        false,
    );
    mollusk.add_program(
        &OWNER,
        if cfg!(feature = "alloc") {
            "blubber_borrow_resize_alloc"
        } else {
            "blubber_borrow_resize"
        },
    );
    mollusk
}

fn run(
    mollusk: &Mollusk,
    action: u8,
    accounts: &[(Pubkey, Account)],
    cpis: usize,
    error: Option<Error>,
) -> InstructionResult {
    let ix = instruction(action, accounts[0].0);
    mollusk.process_and_validate_instruction(
        &ix,
        accounts,
        &[
            error.map_or_else(Check::success, |error| Check::err(error.into())),
            Check::inner_instruction_count(cpis),
        ],
    )
}

fn instruction(action: u8, state: Pubkey) -> Instruction {
    let (_, bump) = Pubkey::find_program_address(&[b"scoped"], &OWNER);
    Instruction {
        program_id: OWNER,
        accounts: vec![
            AccountMeta::new(state, matches!(action, 0 | 3 | 8 | 17)),
            AccountMeta::new(PAYER, true),
            AccountMeta::new_readonly(SYSTEM, false),
        ],
        data: vec![70, action, bump],
    }
}

fn accounts(state: Pubkey, balance: u64) -> Vec<(Pubkey, Account)> {
    vec![
        (
            state,
            Account {
                lamports: balance,
                owner: SYSTEM,
                ..Account::default()
            },
        ),
        (
            PAYER,
            Account {
                lamports: 100_000_000,
                owner: SYSTEM,
                ..Account::default()
            },
        ),
        mollusk_svm::program::keyed_account_for_system_program(),
    ]
}

#[test]
fn generated_creation_paths_commit_and_release_at_runtime_rent() {
    for profile in 0..3 {
        for action in [0, 2, 3, 4] {
            let mollusk = runtime(profile);
            let (pda, _) = Pubkey::find_program_address(&[b"scoped"], &OWNER);
            let state = if action == 2 || action == 4 {
                pda
            } else {
                STATE
            };
            for balance in if action >= 3 {
                &[1, 10_000_000][..]
            } else {
                &[0][..]
            } {
                let original = accounts(state, *balance);
                let result = run(&mollusk, action, &original, 1, None);
                let inner = &result.inner_instructions[0].instruction;
                let opcode: u32 = if action < 3 { 0 } else { 13 };
                assert_eq!(&inner.data[..4], &opcode.to_le_bytes());
                let keys = result.message.as_ref().unwrap().account_keys();
                assert_eq!(keys[inner.program_id_index as usize], SYSTEM);
                let account = result.get_account(&state).unwrap();
                assert_eq!(account.owner, OWNER);
                let minimum = mollusk.sysvars.rent.minimum_balance(account.data.len());
                if action >= 3 {
                    assert_eq!(keys[inner.accounts[0] as usize], state);
                    assert_eq!(inner.accounts.len(), 1 + usize::from(*balance < minimum));
                }
                assert_eq!(account.lamports, (*balance).max(minimum));
                assert_eq!(
                    result.get_account(&PAYER).unwrap().lamports,
                    original[1].1.lamports - minimum.saturating_sub(*balance)
                );
            }
        }
    }
}

#[test]
fn successful_edit_returns_value_and_receipt_and_commits_once() {
    let mollusk = runtime(1);
    let created = run(&mollusk, 0, &accounts(STATE, 0), 1, None);
    let edited = run(&mollusk, 5, &created.resulting_accounts, 1, None);
    let account = edited.get_account(&STATE).unwrap();
    assert_eq!(
        account.lamports,
        mollusk.sysvars.rent.minimum_balance(account.data.len())
    );
    let read = run(&mollusk, 9, &edited.resulting_accounts, 0, None);
    assert_eq!(read.resulting_accounts, edited.resulting_accounts);
}

#[test]
fn callback_error_is_rejected_by_rent_or_rolls_back_when_propagated() {
    let mut mollusk = runtime(1);
    let created = run(&mollusk, 0, &accounts(STATE, 0), 1, None);
    let caught = rent_check::rejected(
        &mut mollusk,
        &instruction(6, STATE),
        &created.resulting_accounts,
        0,
    );
    assert_eq!(
        caught.get_account(&STATE).unwrap().lamports,
        created.get_account(&STATE).unwrap().lamports
    );
    assert!(caught
        .get_account(&STATE)
        .unwrap()
        .data
        .ends_with(b"callback error leaves this complete edit"));
    let propagated = run(
        &mollusk,
        7,
        &created.resulting_accounts,
        0,
        Some(ErrorKind::Unauthorized.into()),
    );
    assert_eq!(propagated.resulting_accounts, created.resulting_accounts);
}

#[test]
fn creation_validates_input_before_cpi_and_read_authenticates_owner() {
    let mollusk = runtime(1);
    let original = accounts(STATE, 0);
    assert_eq!(
        run(&mollusk, 8, &original, 0, None).resulting_accounts,
        original
    );
    let created = run(&mollusk, 0, &original, 1, None);
    let mut wrong_owner = created.resulting_accounts;
    wrong_owner[0].1.owner = PAYER;
    assert_eq!(
        run(
            &mollusk,
            9,
            &wrong_owner,
            0,
            Some(ErrorKind::WrongOwner.into())
        )
        .resulting_accounts,
        wrong_owner
    );
}

#[test]
fn signer_allocation_and_pda_creation_check_authority_before_funding() {
    use pinocchio::error::ProgramError;
    let mollusk = runtime(1);
    for action in [0, 3] {
        let original = accounts(STATE, if action == 3 { 1 } else { 0 });
        let mut ix = instruction(action, STATE);
        ix.accounts[0].is_signer = false;
        let result = mollusk.process_and_validate_instruction(
            &ix,
            &original,
            &[
                Check::err(ProgramError::MissingRequiredSignature),
                Check::inner_instruction_count(0),
            ],
        );
        assert_eq!(result.resulting_accounts, original);
    }
    let (pda, _) = Pubkey::find_program_address(&[b"scoped"], &OWNER);
    for action in [2, 4] {
        let original = accounts(pda, if action == 4 { 1 } else { 0 });
        let mut ix = instruction(action, pda);
        ix.data[2] ^= 1;
        let result = mollusk.process_and_validate_instruction(
            &ix,
            &original,
            &[
                Check::err(ProgramError::InvalidSeeds),
                Check::inner_instruction_count(0),
            ],
        );
        assert_eq!(result.resulting_accounts, original);
    }
}

#[test]
fn scoped_ingress_rejects_before_callbacks_and_lazy_edit_repairs() {
    for profile in 0..3 {
        let mollusk = runtime(profile);
        let created = run(&mollusk, 0, &accounts(STATE, 0), 1, None);
        let mut input = created.resulting_accounts;
        let state = input.iter_mut().find(|(key, _)| *key == STATE).unwrap();
        *state.1.data.last_mut().unwrap() = 0xff;
        let used = state.1.data.len();
        let result = run(&mollusk, 10, &input, 0, None);
        let repaired = &result.get_account(&STATE).unwrap().data;
        assert_eq!(repaired.len(), used);
        assert_eq!(&repaired[used - 4..], b"okay");
    }
}

#[test]
fn explicit_budgets_preserve_equal_width_growth_and_rejection_bytes() {
    for profile in 0..3 {
        let mollusk = runtime(profile);
        let created = run(&mollusk, 0, &accounts(STATE, 0), 1, None);
        for action in 11..=15 {
            let cpis = usize::from(matches!(action, 12 | 14));
            let result = run(&mollusk, action, &created.resulting_accounts, cpis, None);
            let before = created.get_account(&STATE).unwrap();
            let after = result.get_account(&STATE).unwrap();
            if matches!(action, 13 | 15) {
                assert_eq!(after, before);
            } else {
                assert_eq!(
                    after.data.len(),
                    before.data.len()
                        + match action {
                            11 => 0,
                            12 => 8,
                            _ => 16,
                        }
                );
            }
        }
    }
}

#[test]
fn explicit_budget_preflights_capacity_before_rent() {
    for profile in 0..3 {
        let mollusk = runtime(profile);
        let mut created = run(&mollusk, 0, &accounts(STATE, 0), 1, None).resulting_accounts;
        created
            .iter_mut()
            .find(|(key, _)| *key == STATE)
            .unwrap()
            .1
            .lamports = 1;
        let result = run(&mollusk, 16, &created, 0, None);
        assert_eq!(result.resulting_accounts, created);
    }
}

#[test]
fn scoped_exact_numeric_push_and_equal_width_set() {
    for profile in 0..3 {
        let mollusk = runtime(profile);
        let created = run(&mollusk, 17, &accounts(STATE, 0), 1, None);
        for action in [18, 19] {
            let result = run(
                &mollusk,
                action,
                &created.resulting_accounts,
                usize::from(action == 19),
                None,
            );
            assert_eq!(
                result.get_account(&STATE).unwrap().data.len(),
                created.get_account(&STATE).unwrap().data.len() + usize::from(action == 19) * 8
            );
        }
    }
}

#[test]
fn default_capacity_and_checked_options_preserve_session_rules() {
    for profile in 0..3 {
        let mollusk = runtime(profile);
        let created = run(&mollusk, 17, &accounts(STATE, 0), 1, None);
        let original = &created.resulting_accounts;
        let rejected = run(&mollusk, 20, original, 0, None);
        assert_eq!(&rejected.resulting_accounts, original);
        let changed = run(&mollusk, 23, original, 0, None);
        assert_ne!(&changed.resulting_accounts, original);
        let mut spare = original.clone();
        let state = spare.iter_mut().find(|(key, _)| *key == STATE).unwrap();
        // Existing opening spare pays for one u64 push under the default policy.
        state.1.data.resize(state.1.data.len() + 8, 0);
        state.1.lamports = mollusk.sysvars.rent.minimum_balance(state.1.data.len());
        let grown = run(&mollusk, 21, &spare, 0, None);
        assert_eq!(
            grown.get_account(&STATE).unwrap().data.len(),
            state_len(original) + 8
        );
        let mut funded = original.clone();
        funded
            .iter_mut()
            .find(|(key, _)| *key == STATE)
            .unwrap()
            .1
            .lamports += 10_000_000;
        let checked = run(&mollusk, 22, &funded, 0, None);
        assert_eq!(
            checked.get_account(&STATE).unwrap().data.len(),
            state_len(original) + 8
        );
        assert!(
            checked.get_account(&PAYER).unwrap().lamports
                > funded
                    .iter()
                    .find(|(key, _)| *key == PAYER)
                    .unwrap()
                    .1
                    .lamports
        );
    }
}

fn state_len(accounts: &[(Pubkey, Account)]) -> usize {
    accounts
        .iter()
        .find(|(key, _)| *key == STATE)
        .unwrap()
        .1
        .data
        .len()
}
