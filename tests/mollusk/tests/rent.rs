//! Final-length rent accounting through SBF, including real System transfers.

#![forbid(unsafe_code)]

use blubber::{Error, ErrorKind};
use mollusk_svm::{
    result::{Check, InstructionResult},
    Mollusk,
};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

#[path = "support/rent.rs"]
mod rent_check;

const INITIAL: usize = 128;
const RATE: u64 = 19;
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);
const PAYER: Pubkey = Pubkey::new_from_array([9; 32]);
const RECIPIENT: Pubkey = Pubkey::new_from_array([10; 32]);
const SYSTEM: Pubkey = Pubkey::new_from_array([0; 32]);
const OWNER: Pubkey = Pubkey::new_from_array([7; 32]);

fn minimum(len: usize) -> u64 {
    (128 + len as u64) * RATE
}

fn modes() -> impl Iterator<Item = u8> {
    #[cfg(feature = "alloc")]
    let modes = [0, 1, 2];
    #[cfg(not(feature = "alloc"))]
    let modes = [0, 1];
    modes.into_iter()
}

struct Case {
    mollusk: Mollusk,
    ix: Instruction,
    accounts: Vec<(Pubkey, Account)>,
}

impl Case {
    fn new(mode: u8, profile: u8) -> Self {
        let mut mollusk = Mollusk::default();
        // An unusual rate proves settlement reads the current sysvar, rather
        // than using the default or charging speculative headroom.
        mollusk.sysvars.rent.lamports_per_byte = RATE;
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
        let mut data = vec![0x19; INITIAL];
        data[..8].copy_from_slice(b"LEASE001");
        data[8..12].copy_from_slice(&(INITIAL as u32).to_le_bytes());
        let account = |lamports, data, owner| Account {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        };
        let accounts = vec![
            (STATE, account(minimum(INITIAL), data, OWNER)),
            (PAYER, account(10_000_000, vec![], SYSTEM)),
            mollusk_svm::program::keyed_account_for_system_program(),
            (RECIPIENT, account(minimum(0), vec![], SYSTEM)),
        ];
        let mut ix = Instruction {
            program_id: OWNER,
            accounts: vec![
                AccountMeta::new(STATE, false),
                AccountMeta::new(PAYER, true),
                AccountMeta::new_readonly(SYSTEM, false),
                AccountMeta::new(RECIPIENT, false),
            ],
            // opcode, mode, TopUp, refund, action, used_len, reserve, expected fault
            data: vec![40, mode, 0, 0, 0],
        };
        ix.data.extend_from_slice(&(INITIAL as u32).to_le_bytes());
        ix.data.extend_from_slice(&0u32.to_le_bytes());
        ix.data.push(0);
        Self {
            mollusk,
            ix,
            accounts,
        }
    }

    fn used(&mut self, len: usize) {
        self.ix.data[5..9].copy_from_slice(&(len as u32).to_le_bytes());
    }

    fn reserve(&mut self, len: u32) {
        self.ix.data[9..13].copy_from_slice(&len.to_le_bytes());
    }

    fn reject(&mut self, action: u8, error: ErrorKind) {
        self.ix.data[4] = action;
        self.ix.data[13] = error as u8;
    }

    fn run(&self, error: Option<Error>, cpis: usize) -> InstructionResult {
        let mut checks = vec![Check::inner_instruction_count(cpis)];
        checks.push(match error {
            Some(error) => Check::err(error.into()),
            None => Check::success(),
        });
        let result =
            self.mollusk
                .process_and_validate_instruction(&self.ix, &self.accounts, &checks);
        if error.is_some() {
            assert_eq!(
                result.resulting_accounts, self.accounts,
                "failed instruction rollback"
            );
        }
        for inner in &result.inner_instructions {
            let message = result.message.as_ref().unwrap();
            assert_eq!(
                message.account_keys()[inner.instruction.program_id_index as usize],
                SYSTEM
            );
            assert_eq!(inner.instruction.data.len(), 12);
            assert_eq!(&inner.instruction.data[..4], &2u32.to_le_bytes());
            let keys = message.account_keys();
            assert_eq!(keys[inner.instruction.accounts[0] as usize], PAYER);
            assert_eq!(keys[inner.instruction.accounts[1] as usize], STATE);
        }
        result
    }

    fn successful(&self, cpis: usize, final_len: usize, top_up: u64, refund: u64) {
        let result = self.run(None, cpis);
        let state = result.get_account(&STATE).unwrap();
        assert_eq!(state.data.len(), final_len);
        assert_eq!(state.data[12], 42);
        assert_eq!(
            state.lamports,
            self.accounts[0].1.lamports + top_up - refund
        );
        assert_eq!(
            result.get_account(&PAYER).unwrap().lamports,
            self.accounts[1].1.lamports - top_up
        );
        assert_eq!(
            result.get_account(&RECIPIENT).unwrap().lamports,
            self.accounts[3].1.lamports + refund
        );
        if cpis == 1 {
            assert_eq!(
                &result.inner_instructions[0].instruction.data[4..],
                &top_up.to_le_bytes()
            );
        }
    }
}

#[test]
fn prepaid_charges_trimmed_physical_length_at_the_actual_rent_rate() {
    for mode in modes() {
        for profile in 0..3 {
            let mut case = Case::new(mode, profile);
            case.used(64);
            case.successful(0, 64, 0, 0);
        }
    }
}

#[test]
fn prepaid_shortfall_propagates_and_rolls_back_all_byte_edits() {
    for mode in modes() {
        let mut case = Case::new(mode, 1);
        case.used(INITIAL + 200);
        case.run(Some(ErrorKind::RentShortfall.into()), 0);
    }
}

#[test]
fn over_limit_resize_reports_rent_shortfall_first_only_with_on_demand_headroom() {
    let requested = INITIAL + 10_240 + 1;
    for profile in 0..3 {
        // The tour builds both fixture ELFs regardless of the host's features.
        for mode in [0, 1, 2] {
            for funded in [false, true] {
                let mut case = Case::new(mode, profile);
                case.used(requested);
                let expected = if funded {
                    case.accounts[0].1.lamports =
                        case.mollusk.sysvars.rent.minimum_balance(requested);
                    ErrorKind::GrowthLimit
                } else if mode == 2 {
                    ErrorKind::RentShortfall
                } else {
                    ErrorKind::GrowthLimit
                };
                case.run(Some(expected.into()), 0);
            }
        }
    }
}

#[test]
fn caught_growth_failure_preserves_bytes_before_commit() {
    for mode in modes() {
        let mut case = Case::new(mode, 1);
        case.used(INITIAL + 200);
        case.ix.data[4] = 4;
        let result = case.run(None, 0);
        assert_eq!(result.resulting_accounts, case.accounts);
    }
}

#[test]
fn top_up_nets_once_and_does_not_charge_opening_or_amortized_headroom() {
    for mode in modes() {
        for profile in 0..3 {
            for growth in [200, 10_240] {
                let mut case = Case::new(mode, profile);
                case.ix.data[2] = 1;
                case.used(INITIAL + growth);
                case.successful(
                    1,
                    INITIAL + growth,
                    minimum(INITIAL + growth) - minimum(INITIAL),
                    0,
                );
            }
        }
    }
}

#[test]
fn top_up_with_zero_deficit_never_invokes_system_even_with_empty_payer() {
    for mode in modes() {
        let mut case = Case::new(mode, 1);
        case.ix.data[2] = 1;
        case.accounts[1].1.lamports = 0;
        case.successful(0, INITIAL, 0, 0);
    }
}

#[test]
fn insufficient_payer_is_checked_before_any_transfer() {
    for mode in modes() {
        let mut case = Case::new(mode, 1);
        case.ix.data[2] = 1;
        case.used(INITIAL + 200);
        case.accounts[1].1.lamports = 1;
        case.run(Some(ErrorKind::RentShortfall.into()), 0);
    }
}

#[test]
fn reserve_retains_rent_bytes_and_shrink_to_fit_clears_retention() {
    for mode in modes() {
        for clear in [false, true] {
            let mut case = Case::new(mode, 1);
            case.ix.data[2] = 1;
            case.reserve(200);
            if clear {
                case.ix.data[4] = 10;
            }
            let len = INITIAL + if clear { 0 } else { 200 };
            case.successful(usize::from(!clear), len, minimum(len) - minimum(INITIAL), 0);
        }
    }
}

#[test]
fn direct_refund_uses_final_length_and_never_invokes_system() {
    for mode in modes() {
        for top_up in [false, true] {
            let mut case = Case::new(mode, 1);
            case.ix.data[2] = u8::from(top_up);
            case.ix.data[3] = 1;
            case.used(64);
            case.successful(0, 64, 0, minimum(INITIAL) - minimum(64));
        }
    }
}

#[test]
fn retained_capacity_reduces_refund_and_top_up_with_refund_does_not_double_pay() {
    for mode in modes() {
        let mut case = Case::new(mode, 1);
        case.ix.data[2] = 1;
        case.ix.data[3] = 1;
        case.used(64);
        case.reserve(10);
        case.successful(0, 74, 0, minimum(INITIAL) - minimum(74));
        case.used(INITIAL + 200);
        case.reserve(0);
        case.successful(
            1,
            INITIAL + 200,
            minimum(INITIAL + 200) - minimum(INITIAL),
            0,
        );
    }
}

#[test]
fn refund_overflow_preserves_both_balances_and_rolls_back_edits() {
    for mode in modes() {
        let mut case = Case::new(mode, 1);
        case.ix.data[3] = 1;
        case.accounts[3].1.lamports = u64::MAX;
        case.used(64);
        case.run(Some(ErrorKind::ArithmeticOverflow.into()), 0);
        case.ix.data[4] = 11;
        case.successful(0, 64, 0, 0);
        // Zero refund still succeeds at the same recipient balance.
        case.ix.data[4] = 0;
        case.used(INITIAL);
        case.successful(0, INITIAL, 0, 0);
    }
}

#[test]
fn propagated_error_after_successful_top_up_rolls_back_cpi_and_edits() {
    for mode in modes() {
        let mut case = Case::new(mode, 1);
        case.ix.data[2] = 1;
        case.ix.data[4] = 5;
        case.used(INITIAL + 200);
        case.run(Some(Error::Program { code: 99 }), 1);
    }
}

#[test]
fn drop_trims_without_funding_or_refunding() {
    for mode in modes() {
        let mut case = Case::new(mode, 1);
        case.ix.data[2] = 1;
        case.ix.data[3] = 1;
        case.ix.data[4] = 3;
        case.used(64);
        case.successful(0, 64, 0, 0);
    }
}

#[test]
fn grow_and_shrink_reopens_settle_net_rent_without_resetting_sdk_window() {
    for mode in modes() {
        for profile in 0..3 {
            for action in [1, 2] {
                let mut case = Case::new(mode, profile);
                case.ix.data[2] = 1;
                case.ix.data[4] = action;
                let first_len = if action == 1 { INITIAL + 501 } else { INITIAL };
                case.accounts[0].1.lamports = minimum(first_len);
                let final_len = INITIAL + 900;
                case.used(final_len);
                case.successful(1, final_len, minimum(final_len) - minimum(first_len), 0);
            }
        }
    }
}

#[test]
fn funding_signer_writable_owner_data_and_system_checks_precede_opening_growth() {
    for mode in modes() {
        for fault in 0..7 {
            let mut case = Case::new(mode, 1);
            case.ix.data[2] = 1;
            case.reject(6, ErrorKind::InvalidPayer);
            match fault {
                0 => case.ix.accounts[1].is_signer = false,
                1 => case.ix.accounts[1].is_writable = false,
                2 => case.accounts[1].1.owner = OWNER,
                3 => case.accounts[1].1.data.push(1),
                4 => case.accounts[1].1.executable = true,
                5 => case.accounts[2].1.executable = false,
                6 => case.ix.accounts[2].pubkey = RECIPIENT,
                _ => unreachable!(),
            }
            let result = case.run(None, 0);
            assert_eq!(result.resulting_accounts, case.accounts);
        }
        let mut case = Case::new(mode, 1);
        case.ix.data[2] = 1;
        case.reject(8, ErrorKind::BorrowConflict);
        assert_eq!(case.run(None, 0).resulting_accounts, case.accounts);
    }
}

#[test]
fn payer_state_and_refund_aliases_are_rejected_by_runtime_address() {
    for mode in modes() {
        for (payer, recipient, action) in
            [(STATE, RECIPIENT, 6), (PAYER, STATE, 7), (PAYER, PAYER, 7)]
        {
            let mut case = Case::new(mode, 1);
            case.ix.data[2] = 1;
            case.ix.data[3] = 1;
            case.ix.accounts[1].pubkey = payer;
            case.ix.accounts[3].pubkey = recipient;
            case.reject(action, ErrorKind::DuplicateAccount);
            assert_eq!(case.run(None, 0).resulting_accounts, case.accounts);
        }
    }
}

#[test]
fn refund_recipient_must_be_writable_non_executable_and_unborrowed() {
    for mode in modes() {
        for fault in 0..3 {
            let mut case = Case::new(mode, 1);
            case.ix.data[3] = 1;
            match fault {
                0 => {
                    case.ix.accounts[3].is_writable = false;
                    case.reject(7, ErrorKind::NotWritable);
                }
                1 => {
                    case.accounts[3].1.executable = true;
                    case.reject(7, ErrorKind::Unauthorized);
                }
                2 => case.reject(9, ErrorKind::BorrowConflict),
                _ => unreachable!(),
            }
            assert_eq!(case.run(None, 0).resulting_accounts, case.accounts);
        }
    }
}

#[test]
#[allow(deprecated)]
fn legacy_and_simd0194_rent_use_runtime_minima_for_funding_and_refunds() {
    for (rate, threshold) in [(3480, 2f64), (6960, 1f64), (19, 2f64), (19, 1f64)] {
        for mode in modes() {
            for profile in 0..3 {
                for final_len in [64, INITIAL, INITIAL + 200] {
                    let mut case = Case::new(mode, profile);
                    case.mollusk.sysvars.rent.lamports_per_byte = rate;
                    case.mollusk.sysvars.rent.exemption_threshold = threshold.to_le_bytes();
                    let before = case.mollusk.sysvars.rent.minimum_balance(INITIAL);
                    let after = case.mollusk.sysvars.rent.minimum_balance(final_len);
                    case.accounts[0].1.lamports = before;
                    case.accounts[1].1.lamports = 100_000_000;
                    case.accounts[3].1.lamports = case.mollusk.sysvars.rent.minimum_balance(0);
                    case.used(final_len);
                    case.ix.data[2] = 1;
                    case.ix.data[3] = 1;
                    let top_up = after.saturating_sub(before);
                    let refund = before.saturating_sub(after);
                    case.successful(usize::from(top_up != 0), final_len, top_up, refund);
                    let result = case.run(None, usize::from(top_up != 0));
                    assert_eq!(result.get_account(&STATE).unwrap().lamports, after);
                    case.ix.data[2] = 0;
                    if final_len > INITIAL {
                        case.run(Some(ErrorKind::RentShortfall.into()), 0);
                    } else {
                        case.successful(0, final_len, 0, refund);
                    }
                }
            }
        }
    }
}

#[test]
#[allow(deprecated)]
fn unsupported_thresholds_fail_before_any_transfer_or_refund() {
    for threshold in [0.5f64, 1.5, 2.5] {
        for mode in modes() {
            for final_len in [64, INITIAL + 200] {
                let mut case = Case::new(mode, 1);
                case.mollusk.sysvars.rent.exemption_threshold = threshold.to_le_bytes();
                case.accounts[0].1.lamports = case.mollusk.sysvars.rent.minimum_balance(INITIAL);
                case.ix.data[2] = 1;
                case.ix.data[3] = 1;
                case.used(final_len);
                case.run(
                    Some(pinocchio::error::ProgramError::UnsupportedSysvar.into()),
                    0,
                );
            }
        }
    }
}

#[test]
#[allow(deprecated)]
fn unchanged_settlement_skips_unsupported_rent_in_every_mapping_and_lease_mode() {
    for mode in modes() {
        for profile in 0..3 {
            for top_up in [0, 1] {
                let mut case = Case::new(mode, profile);
                // Full settlement rejects this threshold. Success proves
                // that unchanged sessions bypass that path altogether.
                case.mollusk.sysvars.rent.exemption_threshold = 1.5f64.to_le_bytes();
                case.ix.data[2] = top_up;
                case.successful(0, INITIAL, 0, 0);
            }
        }
    }
}

#[test]
#[allow(deprecated)]
fn growth_retention_refunds_and_either_balance_change_force_live_rent() {
    for mode in modes() {
        for profile in 0..3 {
            for reason in 0..7 {
                let mut case = Case::new(mode, profile);
                case.mollusk.sysvars.rent.exemption_threshold = 1.5f64.to_le_bytes();
                case.ix.data[2] = 1;
                match reason {
                    0 => case.used(INITIAL + 1),
                    1 => case.reserve(1),
                    2 => case.ix.data[3] = 1,
                    3 => case.ix.data[4] = 12,
                    4 => case.ix.data[4] = 13,
                    5 => case.used(64),
                    _ => case.used(INITIAL + 10_240),
                }
                case.run(
                    Some(pinocchio::error::ProgramError::UnsupportedSysvar.into()),
                    0,
                );
            }
        }
    }
}

#[test]
fn changed_balance_top_up_still_nets_only_the_final_deficit() {
    for mode in modes() {
        for profile in 0..3 {
            let mut case = Case::new(mode, profile);
            case.ix.data[2] = 1;
            case.ix.data[4] = 13;
            let result = case.run(None, 1);
            assert_eq!(
                result.get_account(&STATE).unwrap().lamports,
                minimum(INITIAL)
            );
            assert_eq!(
                result.get_account(&PAYER).unwrap().lamports,
                case.accounts[1].1.lamports
            );
            assert_eq!(
                result.get_account(&RECIPIENT).unwrap().lamports,
                case.accounts[3].1.lamports
            );
            assert_eq!(
                &result.inner_instructions[0].instruction.data[4..],
                &1u64.to_le_bytes()
            );

            case.ix.data[2] = 0;
            case.run(Some(ErrorKind::RentShortfall.into()), 0);
            case.ix.data[4] = 12;
            case.accounts[3].1.owner = OWNER;
            // The donor must remain rent-exempt after its one-lamport debit.
            case.accounts[3].1.lamports += 1;
            let result = case.run(None, 0);
            assert_eq!(
                result.get_account(&STATE).unwrap().lamports,
                minimum(INITIAL) + 1
            );
            assert_eq!(result.get_account(&RECIPIENT).unwrap().lamports, minimum(0));
        }
    }
}

#[test]
#[allow(deprecated)]
fn poisoned_and_invalid_completions_close_before_any_rent_lookup() {
    for mode in modes() {
        for profile in 0..3 {
            for action in [14, 15] {
                let mut case = Case::new(mode, profile);
                case.mollusk.sysvars.rent.exemption_threshold = 1.5f64.to_le_bytes();
                case.ix.data[4] = action;
                let physical = if mode == 2 { INITIAL } else { INITIAL + 10_240 };
                let result = if physical > INITIAL {
                    rent_check::rejected(&mut case.mollusk, &case.ix, &case.accounts, 0)
                } else {
                    case.run(None, 0)
                };
                assert_eq!(result.get_account(&STATE).unwrap().data.len(), physical);
                assert_eq!(
                    result.get_account(&STATE).unwrap().lamports,
                    minimum(INITIAL)
                );
            }
        }
    }
}

#[test]
#[allow(deprecated)]
fn a_shrunk_reopen_uses_its_own_opening_extent_for_the_rent_decision() {
    for mode in modes() {
        for profile in 0..3 {
            let mut case = Case::new(mode, profile);
            case.mollusk.sysvars.rent.exemption_threshold = 1.5f64.to_le_bytes();
            // The first session shrinks by 37 and must settle. An unsupported
            // Rent sysvar fails before a second session can reopen.
            case.ix.data[4] = 2;
            case.run(
                Some(pinocchio::error::ProgramError::UnsupportedSysvar.into()),
                0,
            );
        }
    }
}

#[test]
#[allow(deprecated)]
fn raised_rent_shrink_settles_and_passes_legacy_transaction_checks() {
    use solana_svm::rent_calculator::{
        get_post_exec_account_rent_state, get_pre_exec_account_rent_state, transition_allowed,
    };

    for (rate, threshold) in [(3480, 2f64), (6960, 1f64)] {
        for mode in modes() {
            for profile in 0..3 {
                for final_len in [64, INITIAL] {
                    let mut case = Case::new(mode, profile);
                    case.mollusk.sysvars.rent.lamports_per_byte = rate;
                    case.mollusk.sysvars.rent.exemption_threshold = threshold.to_le_bytes();
                    let old_minimum = case.mollusk.sysvars.rent.minimum_balance(INITIAL);
                    case.accounts[0].1.lamports = old_minimum;
                    case.mollusk.sysvars.rent.lamports_per_byte = rate * 2;
                    case.used(final_len);
                    let top_up = if final_len < INITIAL {
                        case.ix.data[2] = 1;
                        case.mollusk.sysvars.rent.minimum_balance(final_len) - old_minimum
                    } else {
                        0
                    };
                    if final_len < INITIAL {
                        case.ix.data[2] = 0;
                        case.run(Some(ErrorKind::RentShortfall.into()), 0);
                        case.ix.data[2] = 1;
                    }
                    let result = case.run(None, usize::from(top_up > 0));
                    let state = result.get_account(&STATE).unwrap();
                    assert_eq!(state.data.len(), final_len);
                    assert_eq!(state.lamports, old_minimum + top_up);
                    assert_eq!(
                        result.get_account(&PAYER).unwrap().lamports,
                        case.accounts[1].1.lamports - top_up
                    );
                    if final_len < INITIAL {
                        assert_eq!(top_up, 890_880);
                    }

                    // Mollusk checks its activated rent profile after execution.
                    // Also exercise both pinned runtime predicate modes:
                    // A settled shrink becomes exempt under legacy and SIMD-0392 rules.
                    for relaxed in [false, true] {
                        let rent = &case.mollusk.sysvars.rent;
                        let pre = get_pre_exec_account_rent_state(
                            old_minimum,
                            INITIAL,
                            rent.minimum_balance(INITIAL),
                            relaxed,
                        );
                        let post = get_post_exec_account_rent_state(
                            state.lamports,
                            final_len,
                            rent.minimum_balance(final_len),
                            &pre,
                            old_minimum,
                            relaxed,
                        );
                        assert!(transition_allowed(&pre, &post));
                    }
                }
            }
        }
    }
}
