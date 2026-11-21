//! Signer/PDA creation and atomic borrowed initialization in genuine SBF memory.

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

#[path = "support/invalid_rent.rs"]
mod invalid_rent;
#[path = "support/rent.rs"]
mod rent_check;

const OWNER: Pubkey = Pubkey::new_from_array([7; 32]);
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);
const PAYER: Pubkey = Pubkey::new_from_array([9; 32]);
use pinocchio::account::MAX_PERMITTED_DATA_INCREASE as WINDOW;
const MAX: usize = 10_485_760;

fn modes() -> Vec<u8> {
    let mut modes = vec![0, 1];
    if cfg!(feature = "alloc") {
        modes.push(2);
    }
    modes
}

struct Case {
    mollusk: Mollusk,
    ix: Instruction,
    accounts: Vec<(Pubkey, Account)>,
}

impl Case {
    fn new(action: u8, mode: u8, profile: u8) -> Self {
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
            if mode == 2 {
                "blubber_borrow_resize_alloc"
            } else {
                "blubber_borrow_resize"
            },
        );
        // A nondefault rate proves creation reads the actual sysvar.
        mollusk.sysvars.rent.lamports_per_byte = 19;
        let (pda, bump) = Pubkey::find_program_address(&[b"init"], &OWNER);
        let state = if action == 0 || action == 3 {
            STATE
        } else {
            pda
        };
        let account = |lamports, data, owner| Account::new_with_data(lamports, data, &owner);
        let (data, balance, owner) = if action == 3 {
            let mut data = vec![0xa5; 64];
            data[..8].fill(0);
            (data, 100_000_000, OWNER)
        } else {
            (vec![], 0, SYSTEM)
        };
        let accounts = vec![
            (state, account(balance, data, owner)),
            (PAYER, account(100_000_000, vec![], SYSTEM)),
            mollusk_svm::program::keyed_account_for_system_program(),
        ];
        let mut data = vec![50, action, mode, 0];
        data.extend_from_slice(&64u32.to_le_bytes());
        data.extend_from_slice(&32u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&[bump, 0]);
        data.extend_from_slice(b"borrowed input");
        let ix = Instruction {
            program_id: OWNER,
            accounts: vec![
                AccountMeta::new(state, action == 0),
                AccountMeta::new(PAYER, true),
                AccountMeta::new_readonly(SYSTEM, false),
            ],
            data,
        };
        Self {
            mollusk,
            ix,
            accounts,
        }
    }

    fn word(&mut self, offset: usize, value: usize) {
        self.ix.data[offset..offset + 4].copy_from_slice(&(value as u32).to_le_bytes());
    }

    fn payload(&mut self, len: usize) {
        self.ix.data.truncate(18);
        self.ix
            .data
            .extend((0..len).map(|index| (index % 251) as u8));
    }

    fn init_error(&mut self, kind: ErrorKind, control: u8) {
        self.ix.data[3] = control;
        self.word(12, Error::from(kind).custom_code().unwrap() as usize);
    }

    fn run(&self, error: Option<Error>, cpis: &[u32]) -> InstructionResult {
        let result = self.mollusk.process_and_validate_instruction(
            &self.ix,
            &self.accounts,
            &[
                match error {
                    Some(error) => Check::err(error.into()),
                    None => Check::success(),
                },
                Check::inner_instruction_count(cpis.len()),
            ],
        );
        if error.is_some() {
            assert_eq!(
                result.resulting_accounts, self.accounts,
                "instruction rollback"
            );
        }
        self.check_cpis(&result, cpis);
        result
    }

    fn check_cpis(&self, result: &InstructionResult, cpis: &[u32]) {
        for (inner, opcode) in result.inner_instructions.iter().zip(cpis) {
            let keys = result.message.as_ref().unwrap().account_keys();
            assert_eq!(keys[inner.instruction.program_id_index as usize], SYSTEM);
            assert_eq!(&inner.instruction.data[..4], &opcode.to_le_bytes());
            let indices = &inner.instruction.accounts;
            if *opcode == 0 || *opcode == 2 {
                assert_eq!(keys[indices[0] as usize], PAYER);
                assert_eq!(keys[indices[1] as usize], self.accounts[0].0);
            } else {
                assert_eq!(keys[indices[0] as usize], self.accounts[0].0);
            }
        }
    }

    fn success(&self, cpis: &[u32]) {
        let result = self.run(None, cpis);
        let account = result.get_account(&self.accounts[0].0).unwrap();
        let payload = &self.ix.data[18..];
        // Independent wire oracle: envelope, packed u32 and counted bytes.
        let mut expected = b"INIT0001".to_vec();
        expected.extend_from_slice(&((20 + payload.len()) as u32).to_le_bytes());
        expected.extend_from_slice(&42u32.to_le_bytes());
        expected.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        expected.extend_from_slice(payload);
        assert_eq!(account.data, expected);
        assert_eq!(account.owner, OWNER);
        let space = u32::from_le_bytes(self.ix.data[4..8].try_into().unwrap()) as usize;
        let old_balance = self.accounts[0].1.lamports;
        let minimum = self.mollusk.sysvars.rent.minimum_balance(space);
        let paid = if self.ix.data[1] == 3 {
            0
        } else {
            minimum.saturating_sub(old_balance)
        };
        assert_eq!(account.lamports, old_balance + paid);
        assert_eq!(
            result.get_account(&PAYER).unwrap().lamports,
            self.accounts[1].1.lamports - paid
        );
        if paid != 0 {
            assert_eq!(
                &result.inner_instructions[0].instruction.data[4..12],
                &paid.to_le_bytes()
            );
        }
    }
}

#[test]
fn keypair_and_pda_creation_initialize_borrowed_values_in_every_mode_and_mapping() {
    for action in 0..=1 {
        for mode in modes() {
            for profile in 0..3 {
                Case::new(action, mode, profile).success(&[0]);
            }
        }
    }
}

#[test]
fn funded_system_pda_allocates_and_transfers_only_the_deficit() {
    for mode in modes() {
        for profile in 0..3 {
            for balance in [1, (128 + 64) * 19, 1_000_000] {
                let mut case = Case::new(2, mode, profile);
                case.accounts[0].1.lamports = balance;
                case.success(&[13]);
            }
        }
    }
}

#[test]
fn existing_program_owned_zero_identity_initializes_and_zeroes_all_spare_bytes() {
    for mode in modes() {
        Case::new(3, mode, 1).success(&[]);
    }
}

#[test]
fn wrong_overlong_and_excessive_seeds_fail_before_any_cpi() {
    for action in [1, 2] {
        for seed in [1, 2, 3] {
            let mut case = Case::new(action, 0, 1);
            case.ix.data[17] = seed;
            case.run(Some(Error::Program { code: 14u64 << 32 }), &[]);
        }
    }
}

#[test]
fn the_complete_sixteen_seed_group_is_preserved_for_signed_cpis() {
    for action in [1, 2] {
        let mut case = Case::new(action, 0, 1);
        let (address, bump) = Pubkey::find_program_address(&[&b"init"[..]; 15], &OWNER);
        case.accounts[0].0 = address;
        case.ix.accounts[0].pubkey = address;
        case.ix.data[16] = bump;
        case.ix.data[17] = 4;
        case.success(if action == 1 { &[0] } else { &[13] });
    }
}

#[test]
fn creation_rejects_occupied_data_owner_and_prefunded_addresses() {
    for action in 0..=2 {
        for occupied in 0..3 {
            if action == 2 && occupied == 2 {
                continue;
            }
            let mut case = Case::new(action, 0, 1);
            match occupied {
                0 => case.accounts[0].1.data = vec![0; 16],
                1 => case.accounts[0].1.owner = OWNER,
                _ => case.accounts[0].1.lamports = 1,
            }
            case.run(Some(ErrorKind::AlreadyInitialized.into()), &[]);
        }
    }
}

#[test]
fn insufficient_payer_funds_fail_before_creation_or_partial_allocation() {
    for action in 0..=2 {
        let mut case = Case::new(action, 0, 1);
        case.accounts[1].1.lamports = 1;
        case.run(Some(ErrorKind::RentShortfall.into()), &[]);
    }
}

#[test]
fn overflow_prone_rent_rates_are_rejected_before_any_cpi() {
    for action in 0..=2 {
        let mut case = Case::new(action, 0, 1);
        case.mollusk.sysvars.rent.lamports_per_byte = u64::MAX;
        // Pinned Rent::try_minimum_balance rejects an overflowing rate as
        // SDK InvalidArgument; preserve that error without any transfer.
        case.run(Some(Error::Program { code: 2u64 << 32 }), &[]);
    }
}

#[test]
fn signature_writability_and_executable_checks_precede_cpi() {
    let mut case = Case::new(0, 0, 1);
    case.ix.accounts[0].is_signer = false;
    case.run(Some(Error::Program { code: 8u64 << 32 }), &[]);
    for action in 0..=2 {
        let mut case = Case::new(action, 0, 1);
        case.ix.accounts[0].is_writable = false;
        case.run(Some(ErrorKind::NotWritable.into()), &[]);
        let mut case = Case::new(action, 0, 1);
        case.accounts[0].1.executable = true;
        case.run(Some(ErrorKind::Unauthorized.into()), &[]);
    }
}

#[test]
fn funding_metadata_and_account_aliases_fail_before_cpi() {
    for action in 0..=2 {
        for invalid in 0..7 {
            let mut case = Case::new(action, 0, 1);
            match invalid {
                0 => case.ix.accounts[1].is_signer = false,
                1 => case.ix.accounts[1].is_writable = false,
                2 => case.accounts[1].1.owner = OWNER,
                3 => case.accounts[1].1.data = vec![1],
                4 => case.accounts[1].1.executable = true,
                5 => case.accounts[2].1.executable = false,
                _ => {
                    let wrong = Pubkey::new_from_array([99; 32]);
                    case.accounts[2].0 = wrong;
                    case.ix.accounts[2].pubkey = wrong;
                }
            }
            case.run(Some(ErrorKind::InvalidPayer.into()), &[]);
        }
        for pair in [(0, 1), (0, 2), (1, 2)] {
            let mut case = Case::new(action, 0, 1);
            case.ix.accounts[pair.1].pubkey = case.ix.accounts[pair.0].pubkey;
            case.run(Some(ErrorKind::DuplicateAccount.into()), &[]);
        }
    }
}

#[test]
fn outstanding_state_or_payer_guards_prevent_creation() {
    for action in 0..=2 {
        for control in [2, 3] {
            let mut case = Case::new(action, 0, 1);
            case.ix.data[3] = control;
            case.run(Some(ErrorKind::BorrowConflict.into()), &[]);
        }
    }
}

#[test]
fn creation_rejects_both_limits_before_rent_or_cpi() {
    for action in 0..=2 {
        for (size, error) in [
            (WINDOW + 1, ErrorKind::GrowthLimit),
            (MAX, ErrorKind::GrowthLimit),
            (MAX + 1, ErrorKind::Capacity),
            (u32::MAX as usize, ErrorKind::Capacity),
        ] {
            let mut case = Case::new(action, 0, 1);
            case.word(4, size);
            case.run(Some(error.into()), &[]);
        }
    }
}

#[test]
fn up_to_zero_exact_bounds_and_partial_grants_preflight_before_writes() {
    for (headroom, payload, expected) in [
        (0, 45, Some(ErrorKind::Capacity)),
        (32, 77, Some(ErrorKind::Capacity)),
        (32, 76, None),
    ] {
        let mut case = Case::new(3, 1, 1);
        case.word(8, headroom);
        case.payload(payload);
        if let Some(error) = expected {
            case.init_error(error, 1);
            case.run(None, &[]);
        } else {
            case.success(&[]);
        }
    }
    for profile in 0..3 {
        let mut case = Case::new(0, 1, profile);
        case.word(4, WINDOW - 40);
        case.word(8, WINDOW);
        case.ix.data[3] = 8;
        case.success(&[0]);
        let mut case = Case::new(1, 1, profile);
        case.word(4, 64);
        case.word(8, WINDOW);
        case.payload(WINDOW + 1 - 20);
        case.init_error(ErrorKind::GrowthLimit, 1);
        let result = rent_check::rejected(&mut case.mollusk, &case.ix, &case.accounts, 1);
        case.check_cpis(&result, &[0]);
        let state = result.get_account(&case.accounts[0].0).unwrap();
        assert_eq!(state.owner, OWNER);
        assert_eq!(state.data, vec![0; WINDOW]);
        assert_eq!(state.lamports, (128 + 64) * 19);
    }
}

#[test]
fn full_cpi_window_can_initialize_and_reopen_without_a_new_allowance() {
    for mode in modes() {
        let mut case = Case::new(1, mode, 1);
        case.word(4, WINDOW);
        case.payload(WINDOW - 20);
        case.ix.data[3] = 10;
        if mode == 1 {
            case.word(8, WINDOW);
        }
        case.success(&[0]);
    }
}

#[test]
fn initialization_rejects_existing_identity_and_invalid_borrowed_map_atomically() {
    for mode in modes() {
        let mut case = Case::new(3, mode, 1);
        case.accounts[0].1.data[..8].fill(0xa5);
        case.init_error(ErrorKind::AlreadyInitialized, 6);
        case.run(None, &[]);
        let mut case = Case::new(3, mode, 1);
        case.init_error(ErrorKind::UnsortedKeys, 7);
        case.run(None, &[]);
        let mut case = Case::new(0, mode, 1);
        case.ix.data[3] = 5;
        case.success(&[0]);
    }
}

#[test]
fn create_success_then_init_failure_is_distinct_from_instruction_rollback() {
    for control in [1, 4] {
        let mut case = Case::new(0, 1, 1);
        case.word(8, 0);
        case.payload(45);
        case.init_error(ErrorKind::Capacity, control);
        let result = case.run((control == 4).then(|| ErrorKind::Capacity.into()), &[0]);
        if control == 1 {
            let account = result.get_account(&STATE).unwrap();
            assert_eq!(account.owner, OWNER);
            assert_eq!(account.data, vec![0; 64]);
            assert_eq!(account.lamports, (128 + 64) * 19);
        }
    }
}

#[test]
fn every_initialization_mode_enforces_the_independent_absolute_cap() {
    for mode in modes() {
        let mut case = Case::new(3, mode, 1);
        case.ix.data[3] = 9;
        case.success(&[]);
    }
}

#[cfg(feature = "alloc")]
#[test]
fn on_demand_exact_retry_succeeds_at_the_window_and_fails_before_init_writes() {
    for profile in 0..3 {
        let mut case = Case::new(3, 2, profile);
        case.payload(64 + WINDOW - 20);
        case.success(&[]);
        let mut case = Case::new(3, 2, profile);
        case.payload(64 + WINDOW + 1 - 20);
        case.init_error(ErrorKind::GrowthLimit, 1);
        case.run(None, &[]);
    }
}

#[test]
fn creation_and_prefunded_allocation_match_runtime_rent() {
    for rate in [6960, 19] {
        for mode in modes() {
            for profile in 0..3 {
                for action in 0..=2 {
                    let mut case = Case::new(action, mode, profile);
                    case.mollusk.sysvars.rent.lamports_per_byte = rate;
                    if action == 2 {
                        case.accounts[0].1.lamports = 1;
                    }
                    case.success(if action == 2 { &[13] } else { &[0] });
                    let result = case.run(None, if action == 2 { &[13] } else { &[0] });
                    let state = result.get_account(&case.accounts[0].0).unwrap();
                    assert!(
                        state.lamports
                            >= case.mollusk.sysvars.rent.minimum_balance(state.data.len())
                    );
                }
            }
        }
    }
}

#[test]
fn out_of_range_creation_rent_fails_before_cpi() {
    for action in 0..=2 {
        let mut case = Case::new(action, 0, 1);
        case.accounts.push(invalid_rent::sysvar(&case.mollusk));
        case.run(
            Some(pinocchio::error::ProgramError::InvalidArgument.into()),
            &[],
        );
    }
}
