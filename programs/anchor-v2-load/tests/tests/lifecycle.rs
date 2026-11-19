//! Exact allocation, full-rent settlement and successful-exit close controls.
#![forbid(unsafe_code)]
#[allow(dead_code)]
mod support;
use solana_account::Account;
use solana_instruction::{error::InstructionError, AccountMeta};
use solana_pubkey::Pubkey;
use support::*;

const RECIPIENT: Pubkey = Pubkey::new_from_array([10; 32]);
const SYSTEM: Pubkey = Pubkey::new_from_array([0; 32]);
fn fresh(profile: u8) -> Case {
    let mut c = Case::new(profile, false);
    c.accounts
        .push((RECIPIENT, Account::new(1_000_000, 0, &Pubkey::default())));
    c.accounts
        .push(mollusk_svm::program::keyed_account_for_system_program());
    c
}
fn resize_metas() -> Vec<AccountMeta> {
    vec![
        writable(),
        AccountMeta::new(PAYER, true),
        AccountMeta::new_readonly(SYSTEM, false),
    ]
}
fn close_metas() -> Vec<AccountMeta> {
    let mut m = resize_metas();
    m.insert(2, AccountMeta::new(RECIPIENT, false));
    m
}
fn resize_args(space: u32, mode: u8) -> Vec<u8> {
    let mut args = space.to_le_bytes().to_vec();
    args.push(mode);
    args
}
fn raw_args(mode: u8, space: u32) -> Vec<u8> {
    let mut args = vec![mode];
    args.extend(space.to_le_bytes());
    args
}
fn balance(c: &Case, key: Pubkey) -> u64 {
    c.accounts
        .iter()
        .find(|(k, _)| *k == key)
        .unwrap()
        .1
        .lamports
}
fn payer_mut(c: &mut Case) -> &mut Account {
    &mut c.accounts.iter_mut().find(|(k, _)| *k == PAYER).unwrap().1
}
fn resize(c: &mut Case, space: u32, mode: u8) {
    c.run(31, &resize_args(space, mode), resize_metas(), None);
    c.assert_counter(19);
    assert_eq!(c.state().data.len(), space as usize);
    assert_eq!(
        c.state().lamports,
        c.svm.sysvars.rent.minimum_balance(space as usize)
    );
}

#[test]
fn exact_same_grow_shrink_and_regrow_preserve_allocation_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        let initial = balance(&c, PAYER);
        resize(&mut c, 256, 0);
        assert_eq!(c.last_cpis, 0);
        resize(&mut c, 512, 0);
        assert_eq!(c.last_cpis, 1);
        assert!(c.state().data[256..].iter().all(|byte| *byte == 0));
        resize(&mut c, 128, 0);
        assert_eq!(c.last_cpis, 0);
        resize(&mut c, 512, 5);
        assert_eq!(c.last_cpis, 2);
        assert_eq!(
            balance(&c, PAYER),
            initial
                - (c.svm.sysvars.rent.minimum_balance(512)
                    - c.svm.sysvars.rent.minimum_balance(256))
        );
        for mode in [5, 6] {
            let mut c = fresh(profile);
            c.run(33, &raw_args(mode, 512), close_metas(), None);
            c.assert_counter(7);
            assert_eq!(c.state().data.len(), 512);
            assert!(c.state().data[256..].iter().all(|byte| *byte == 0));
        }
    }
}

#[test]
fn exact_edits_cannot_select_managed_growth_or_trim_all_profiles() {
    for profile in 0..3 {
        for mode in [1, 2, 7] {
            let mut c = fresh(profile);
            c.run(
                31,
                &resize_args(384, mode),
                resize_metas(),
                Some(InstructionError::Custom(83)),
            );
        }
        let mut c = fresh(profile);
        c.run(
            33,
            &raw_args(4, 384),
            close_metas(),
            Some(InstructionError::InvalidRealloc),
        );
    }
}

#[test]
fn below_used_cap_and_original_invocation_window_fail_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        let used = c.used() as u32;
        for space in [0, 11, used - 1, 10_485_761, u32::MAX] {
            c.reject(31, &resize_args(space, 0), resize_metas());
            assert_eq!(c.last_cpis, 0);
        }
        c.run(
            31,
            &resize_args(256 + 10_241, 0),
            resize_metas(),
            Some(InstructionError::InvalidRealloc),
        );
        c.run(
            31,
            &resize_args(256, 6),
            resize_metas(),
            Some(InstructionError::InvalidRealloc),
        );
        resize(&mut c, 256 + 10_240, 0);
    }
}

#[test]
fn exact_rent_shortfall_aliases_and_payer_validation_roll_back_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        payer_mut(&mut c).lamports = 1;
        c.reject(31, &resize_args(512, 0), resize_metas());
        assert_eq!(c.last_cpis, 0);
        for bad in 0..5 {
            let mut c = fresh(profile);
            let mut m = close_metas();
            match bad {
                0 => m[1] = AccountMeta::new(STATE, true),
                1 => m[1].is_signer = false,
                2 => m[1].is_writable = false,
                3 => payer_mut(&mut c).owner = owner(),
                4 => payer_mut(&mut c).data = vec![1],
                _ => unreachable!(),
            }
            c.reject(33, &raw_args(5, 512), m);
        }
    }
}

#[test]
#[allow(deprecated)]
fn exact_full_runtime_rent_refunds_all_excess_once_all_profiles() {
    for profile in 0..3 {
        for threshold in [1f64, 2f64] {
            let mut c = fresh(profile);
            c.svm.sysvars.rent.lamports_per_byte = 19;
            c.svm.sysvars.rent.exemption_threshold = threshold.to_le_bytes();
            c.state_mut().lamports = c.svm.sysvars.rent.minimum_balance(256) + 17_000;
            let total = balance(&c, PAYER) + c.state().lamports;
            resize(&mut c, 128, 4);
            assert_eq!(balance(&c, PAYER) + c.state().lamports, total);
            assert_eq!(c.last_cpis, 0);
            resize(&mut c, 512, 0);
            assert_eq!(c.last_cpis, 1);
            assert_eq!(balance(&c, PAYER) + c.state().lamports, total);
        }
        let mut c = fresh(profile);
        c.svm.sysvars.rent.exemption_threshold = 1.5f64.to_le_bytes();
        c.run(
            31,
            &resize_args(256, 0),
            resize_metas(),
            Some(InstructionError::UnsupportedSysvar),
        );
    }
}

#[test]
fn authorized_close_transfers_full_balance_and_becomes_terminal_all_profiles() {
    for profile in 0..3 {
        for opcode in [32, 33] {
            let mut c = fresh(profile);
            c.state_mut().lamports += 123_456;
            let total = c.state().lamports + balance(&c, RECIPIENT);
            let args = if opcode == 32 {
                vec![0]
            } else {
                raw_args(0, 0)
            };
            c.run(opcode, &args, close_metas(), None);
            assert_eq!(c.state().lamports, 0);
            assert_eq!(c.state().data.len(), 12);
            assert_eq!(&c.state().data[8..12], &[0; 4]);
            assert_eq!(c.state().owner, owner());
            assert_eq!(balance(&c, RECIPIENT), total);
            assert_eq!(c.last_cpis, 0);
        }
    }
}

#[test]
fn close_authority_recipient_self_alias_borrow_and_overflow_rejections_all_profiles() {
    for profile in 0..3 {
        for bad in 0..6 {
            let mut c = fresh(profile);
            let mut m = close_metas();
            match bad {
                0 => m[1] = AccountMeta::new(RECIPIENT, true),
                1 => m[2] = AccountMeta::new(PAYER, false),
                2 => m[2].is_writable = false,
                3 => {
                    c.accounts
                        .iter_mut()
                        .find(|(k, _)| *k == RECIPIENT)
                        .unwrap()
                        .1
                        .lamports = u64::MAX
                }
                4 => m[2] = AccountMeta::new(STATE, false),
                5 => m[1].is_signer = false,
                _ => unreachable!(),
            }
            c.reject(32, &[0], m);
        }
        for mode in [1, 3] {
            let mut c = fresh(profile);
            c.reject(33, &raw_args(mode, 0), close_metas());
        }
        let mut c = fresh(profile);
        let mut m = close_metas();
        m[2] = writable();
        c.reject(33, &raw_args(0, 0), m);
        let mut c = fresh(profile);
        c.state_mut().owner = Pubkey::default();
        c.run(
            33,
            &raw_args(0, 0),
            close_metas(),
            Some(InstructionError::IllegalOwner),
        );
    }
}

#[test]
fn resize_refund_then_close_does_not_refund_twice_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        c.state_mut().lamports += 20_000;
        let before_payer = balance(&c, PAYER);
        let before_recipient = balance(&c, RECIPIENT);
        let state_balance = c.state().lamports;
        let used = c.used();
        let rent = c.svm.sysvars.rent.minimum_balance(used);
        c.run(32, &[2], close_metas(), None);
        assert_eq!(balance(&c, PAYER), before_payer + state_balance - rent);
        assert_eq!(balance(&c, RECIPIENT), before_recipient + rent);
        assert_eq!(c.state().lamports, 0);
        assert_eq!(c.state().data.len(), 12);
        assert_eq!(&c.state().data[8..12], &[0; 4]);
        assert_eq!(c.last_cpis, 0);
    }
}

#[test]
fn handler_errors_skip_successful_exit_close_and_roll_back_realloc_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        c.run(32, &[1], close_metas(), Some(InstructionError::Custom(85)));
        c.assert_counter(7);
        c.run(
            31,
            &resize_args(512, 3),
            resize_metas(),
            Some(InstructionError::Custom(84)),
        );
        c.assert_counter(7);
        assert_eq!(c.state().data.len(), 256);
    }
}

#[test]
fn pinned_anchor_close_reinit_all_profiles() {
    use solana_instruction::Instruction;
    let pda = Pubkey::find_program_address(&[b"native-init"], &owner()).0;
    let other = Pubkey::new_from_array([11; 32]);
    for profile in 0..3 {
        for slab in [false, true] {
            for payer in [PAYER, other] {
                let mut c = fresh(profile);
                c.target = pda;
                c.accounts[0].0 = pda;
                *c.state_mut() = Account::default();
                c.accounts
                    .push((other, Account::new(100_000_000, 0, &SYSTEM)));
                let init = Instruction {
                    program_id: owner(),
                    accounts: vec![
                        AccountMeta::new(PAYER, true),
                        AccountMeta::new(pda, false),
                        AccountMeta::new_readonly(SYSTEM, false),
                    ],
                    data: vec![if slab { 54 } else { 52 }],
                };
                let result = c.svm.process_instruction(&init, &c.accounts);
                assert_eq!(result.raw_result, Ok(()));
                c.accounts = result.resulting_accounts;
                // Native Anchor's fixed header is [disc][authority][counter].
                c.state_mut().data[40..48].copy_from_slice(&19u64.to_le_bytes());
                let close = Instruction {
                    program_id: owner(),
                    accounts: vec![
                        AccountMeta::new(pda, false),
                        AccountMeta::new(PAYER, true),
                        AccountMeta::new(RECIPIENT, false),
                    ],
                    data: vec![if slab { 55 } else { 53 }],
                };
                let mut next = init;
                next.accounts[0] = AccountMeta::new(payer, true);
                let result = c.svm.process_transaction_instructions(
                    &[close, next],
                    &c.accounts,
                    Some(&PAYER),
                );
                println!(
                    "Anchor baseline profile={profile} slab={slab} different_payer={} result={:?}",
                    payer != PAYER,
                    result.program_result
                );
                assert!(result.program_result.is_ok());
                let data = &result
                    .resulting_accounts
                    .iter()
                    .find(|(k, _)| *k == pda)
                    .unwrap()
                    .1
                    .data;
                assert_eq!(u64::from_le_bytes(data[40..48].try_into().unwrap()), 7);
                assert_eq!(&data[8..40], payer.as_ref());
            }
        }
    }
}

#[test]
fn closed_native_account_reuse_in_shared_transaction_all_profiles() {
    use solana_instruction::Instruction;
    let other = Pubkey::new_from_array([11; 32]);
    for profile in 0..3 {
        for branch in 0..4 {
            for top_up in [false, true] {
                let mut c = fresh(profile);
                let pda = Pubkey::find_program_address(&[b"native-init"], &owner()).0;
                c.target = pda;
                c.accounts[0].0 = pda;
                c.state_mut().data[44..52].copy_from_slice(&19u64.to_le_bytes());
                c.accounts
                    .push((other, Account::new(100_000_000, 0, &SYSTEM)));
                let close = Instruction {
                    program_id: owner(),
                    accounts: vec![
                        AccountMeta::new(pda, false),
                        AccountMeta::new(PAYER, true),
                        AccountMeta::new(RECIPIENT, false),
                        AccountMeta::new_readonly(SYSTEM, false),
                    ],
                    data: vec![32, 0],
                };
                let payer = if branch == 1 { other } else { PAYER };
                let next = match branch {
                    0 | 1 => {
                        let mut data = vec![25];
                        data.extend(resize_args(256, 0));
                        Instruction {
                            program_id: owner(),
                            accounts: vec![
                                AccountMeta::new(payer, true),
                                AccountMeta::new(pda, false),
                                AccountMeta::new_readonly(SYSTEM, false),
                            ],
                            data,
                        }
                    }
                    2 => {
                        let mut data = vec![0];
                        data.extend(19u64.to_le_bytes());
                        Instruction {
                            program_id: owner(),
                            accounts: vec![AccountMeta::new_readonly(pda, false)],
                            data,
                        }
                    }
                    3 => {
                        let mut data = vec![56];
                        data.extend(256u32.to_le_bytes());
                        Instruction {
                            program_id: owner(),
                            accounts: vec![
                                AccountMeta::new(pda, false),
                                AccountMeta::new(PAYER, true),
                                AccountMeta::new(RECIPIENT, false),
                                AccountMeta::new_readonly(SYSTEM, false),
                            ],
                            data,
                        }
                    }
                    _ => unreachable!(),
                };
                let mut instructions = vec![close];
                if top_up {
                    // System transfer preserves the closed envelope and owner.
                    let mut data = 2u32.to_le_bytes().to_vec();
                    data.extend(c.svm.sysvars.rent.minimum_balance(256).to_le_bytes());
                    instructions.push(Instruction {
                        program_id: SYSTEM,
                        accounts: vec![AccountMeta::new(PAYER, true), AccountMeta::new(pda, false)],
                        data,
                    });
                }
                instructions.push(next);
                let result = c.svm.process_transaction_instructions(
                    &instructions,
                    &c.accounts,
                    Some(&PAYER),
                );
                if branch == 3 {
                    assert!(
                        result.program_result.is_ok(),
                        "raw regrowth: {:?}",
                        result.program_result
                    );
                    let state = &result
                        .resulting_accounts
                        .iter()
                        .find(|(k, _)| *k == pda)
                        .unwrap()
                        .1;
                    assert_eq!(state.owner, owner());
                    assert_eq!(state.data.len(), 256);
                    assert!(state.data[8..].iter().all(|b| *b == 0));
                    assert_eq!(
                        state.lamports,
                        if top_up {
                            c.svm.sysvars.rent.minimum_balance(256)
                        } else {
                            0
                        }
                    );
                } else {
                    assert!(
                        result.program_result.is_err(),
                        "profile={profile} branch={branch} top_up={top_up}"
                    );
                    assert_eq!(result.resulting_accounts, c.accounts);
                    c.assert_counter(19);
                    assert_eq!(&c.state().data[12..44], PAYER.as_ref());
                }
            }
        }
    }
}
