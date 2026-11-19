//! Native creation/reuse under real Anchor constraints and three SBF mappings.
#![forbid(unsafe_code)]
#[allow(dead_code)]
mod support;
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;
use support::*;

const SYSTEM: Pubkey = Pubkey::new_from_array([0; 32]);

fn fresh(profile: u8, prefund: u64, pda: bool) -> Case {
    let mut c = Case::new(profile, false);
    if pda {
        c.target = Pubkey::find_program_address(&[b"native-init"], &owner()).0;
        c.accounts[0].0 = c.target;
    }
    *c.state_mut() = Account {
        lamports: prefund,
        ..Account::default()
    };
    c.accounts
        .push(mollusk_svm::program::keyed_account_for_system_program());
    c
}
fn metas(c: &Case, signer_target: bool) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(PAYER, true),
        AccountMeta::new(c.target, signer_target),
        AccountMeta::new_readonly(SYSTEM, false),
    ]
}
fn args(space: u32, mode: u8) -> Vec<u8> {
    let mut args = space.to_le_bytes().to_vec();
    args.push(mode);
    args
}
fn initialize(c: &mut Case, pda: bool, space: u32, mode: u8) {
    c.run(
        if pda { 21 } else { 20 },
        &args(space, mode),
        metas(c, !pda),
        None,
    );
}
fn payer(c: &mut Case) -> &mut Account {
    &mut c.accounts.iter_mut().find(|(k, _)| *k == PAYER).unwrap().1
}

#[test]
fn signer_create_and_prefund_all_profiles() {
    for profile in 0..3 {
        for prefund in [0, 1, 1_000_000, 10_000_000] {
            let mut c = fresh(profile, prefund, false);
            let before = payer(&mut c).lamports;
            initialize(&mut c, false, 256, 4);
            c.assert_counter(7);
            let rent = c.svm.sysvars.rent.minimum_balance(256);
            assert_eq!(c.state().lamports, prefund.max(rent));
            assert_eq!(
                payer(&mut c).lamports,
                before - rent.saturating_sub(prefund)
            );
            assert_eq!(c.state().owner, owner());
            assert_eq!(c.state().data.len(), 256);
            assert!(c.state().data[c.used()..].iter().all(|b| *b == 0));
            assert_eq!(c.last_cpis, 1);
        }
    }
}

#[test]
fn pda_create_and_prefund_all_profiles() {
    for profile in 0..3 {
        for prefund in [0, 1, 10_000_000] {
            let mut c = fresh(profile, prefund, true);
            initialize(&mut c, true, 256, 0);
            c.assert_counter(7);
            assert_eq!(c.state().data.len(), 256);
            assert_eq!(c.last_cpis, 1);
        }
    }
}

#[test]
fn new_exit_and_capacity_rollback_all_profiles() {
    for profile in 0..3 {
        for pda in [false, true] {
            for (space, mode) in [(256, 1), (64, 0), (256, 2)] {
                let mut c = fresh(profile, 0, pda);
                c.reject(
                    if pda { 21 } else { 20 },
                    &args(space, mode),
                    metas(&c, !pda),
                );
                // Omitted initializer and borrowed input overflow happen after
                // the allocation CPI; all owner/data/balance changes roll back.
                assert_eq!(c.last_cpis, 1);
            }
        }
    }
}

#[test]
fn creation_space_and_initialized_rejections_all_profiles() {
    for profile in 0..3 {
        for space in [0, 11, 60, 10_241, 10_485_761, u32::MAX] {
            let mut c = fresh(profile, 0, false);
            c.reject(20, &args(space, 0), metas(&c, true));
            assert_eq!(c.last_cpis, 0);
        }
        let mut c = fresh(profile, 0, false);
        initialize(&mut c, false, 10_240, 0);
        c.assert_counter(7);
        c.reject(20, &args(10_240, 0), metas(&c, true));
        assert_eq!(c.last_cpis, 0);
    }
}

#[test]
fn seed_bump_owner_and_payer_seed_rejections_all_profiles() {
    for profile in 0..3 {
        // Incorrect PDA address is rejected by Anchor before allocation.
        let mut c = fresh(profile, 0, false);
        c.reject(21, &args(256, 0), metas(&c, false));
        assert_eq!(c.last_cpis, 0);
        // The direct hook independently checks seeds including their bump,
        // owner identity and unsupported payer seeds before any CPI.
        assert_ne!(
            Pubkey::find_program_address(&[b"native-init"], &owner()).1,
            0
        );
        for mode in 1..=4 {
            let mut c = fresh(profile, 0, true);
            c.reject(23, &args(256, mode), metas(&c, false));
            assert_eq!(c.last_cpis, 0);
        }
        let mut c = fresh(profile, 0, false);
        let pda_payer = Pubkey::find_program_address(&[b"native-payer"], &owner()).0;
        c.accounts[1].0 = pda_payer;
        let mut accounts = metas(&c, true);
        accounts[0] = AccountMeta::new(pda_payer, false);
        c.run(
            24,
            &256u32.to_le_bytes(),
            accounts,
            Some(solana_instruction::error::InstructionError::InvalidSeeds),
        );
        assert_eq!(c.last_cpis, 0);
    }
}

#[test]
fn payer_and_alias_rejections_all_profiles() {
    for profile in 0..3 {
        for fault in 0..9 {
            let mut c = fresh(profile, 0, false);
            let mut accounts = metas(&c, true);
            match fault {
                0 => payer(&mut c).lamports = 1,
                1 => accounts[0].is_signer = false,
                2 => accounts[0].is_writable = false,
                3 => payer(&mut c).owner = owner(),
                4 => payer(&mut c).data = vec![0],
                5 => accounts[1] = accounts[0].clone(),
                6 => accounts[1] = accounts[2].clone(),
                7 => accounts[0] = accounts[2].clone(),
                8 => accounts[1].is_signer = false,
                _ => unreachable!(),
            }
            c.reject(23, &args(256, 0), accounts);
            assert_eq!(c.last_cpis, 0);
        }
        for fault in 0..3 {
            let mut c = fresh(profile, 0, false);
            match fault {
                0 => c.state_mut().owner = owner(),
                1 => c.state_mut().data = vec![0],
                2 => c.state_mut().executable = true,
                _ => unreachable!(),
            }
            c.reject(23, &args(256, 0), metas(&c, true));
            assert_eq!(c.last_cpis, 0);
        }
    }
}

#[test]
fn reuse_preserves_state_and_allocation_all_profiles() {
    for profile in 0..3 {
        for pda in [false, true] {
            let mut c = fresh(profile, 0, pda);
            let opcode = if pda { 25 } else { 22 };
            c.run(opcode, &args(256, 0), metas(&c, !pda), None);
            c.run(
                1,
                &11u64.to_le_bytes(),
                vec![AccountMeta::new(c.target, false), signer()],
                None,
            );
            let before = c.accounts.clone();
            // This input would not fit a New account; reused input is ignored.
            c.run(opcode, &args(256, 2), metas(&c, !pda), None);
            assert_eq!(c.accounts, before);
            assert_eq!(c.last_cpis, 0);
            c.assert_counter(11);
        }
    }
}

#[test]
fn reuse_authority_space_rent_and_root_rejections_all_profiles() {
    for profile in 0..3 {
        for fault in 0..5 {
            let mut c = fresh(profile, 0, false);
            initialize(&mut c, false, 256, 0);
            let mut space = 256;
            match fault {
                0 => c.state_mut().data[12..44].fill(4),
                1 => space = 257,
                2 => c.state_mut().lamports = 1,
                3 => c.state_mut().owner = PAYER,
                4 => {
                    let used = c.used();
                    c.state_mut().data[used - 1] = 255;
                }
                _ => unreachable!(),
            }
            if fault == 4 && std::env::var("BLUBBER_NATIVE_LAZY").as_deref() == Ok("1") {
                c.run(22, &args(space, 0), metas(&c, true), None);
            } else {
                c.reject(22, &args(space, 0), metas(&c, true));
            }
            assert_eq!(c.last_cpis, 0);
        }
    }
}

#[test]
fn opening_reserve_remains_usable_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile, 0, false);
        initialize(&mut c, false, 256, 3);
        assert_eq!(c.state().data.len(), 256);
        let used = c.used();
        c.assert_value(7, "longer text");
        assert!(used < 256);
        // Exact physical space remains the reservation floor on reuse.
        c.run(22, &args(256, 2), metas(&c, true), None);
        assert_eq!(c.state().data.len(), 256);
    }
}

#[test]
#[allow(deprecated)]
fn full_runtime_rent_creation_and_reuse_all_profiles() {
    for profile in 0..3 {
        for threshold in [1f64, 2f64] {
            let mut c = fresh(profile, 0, false);
            c.svm.sysvars.rent.lamports_per_byte = 3480;
            c.svm.sysvars.rent.exemption_threshold = threshold.to_le_bytes();
            initialize(&mut c, false, 256, 0);
            let rent = c.svm.sysvars.rent.minimum_balance(256);
            assert_eq!(c.state().lamports, rent);
            c.run(22, &args(256, 2), metas(&c, true), None);
            c.state_mut().lamports = rent - 1;
            c.reject(22, &args(256, 2), metas(&c, true));
            assert_eq!(c.last_cpis, 0);
        }
        let mut c = fresh(profile, 0, false);
        c.svm.sysvars.rent.exemption_threshold = 1.5f64.to_le_bytes();
        c.reject(20, &args(256, 0), metas(&c, true));
        assert_eq!(c.last_cpis, 0);
    }
}
