//! Scoped generated/SDK CPI, corrupted callees and saved invocation geometry.
#![forbid(unsafe_code)]
#[allow(dead_code)]
mod support;
use solana_account::Account;
use solana_instruction::{error::InstructionError, AccountMeta};
use solana_pubkey::Pubkey;
use support::*;

fn fresh(profile: u8) -> Case {
    let mut c = Case::new(profile, false);
    c.accounts.push((
        owner(),
        mollusk_svm::program::create_program_account_loader_v3(&owner()),
    ));
    c.state_mut().lamports = c.svm.sysvars.rent.minimum_balance(10_496);
    c
}
fn metas() -> Vec<AccountMeta> {
    vec![
        writable(),
        signer(),
        AccountMeta::new_readonly(owner(), false),
    ]
}
fn args(scenario: u8, arg: u32) -> Vec<u8> {
    let mut data = vec![scenario];
    data.extend_from_slice(&arg.to_le_bytes());
    data
}

#[test]
fn generated_readonly_cpi_restores_shared_and_exclusive_guards_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        c.run(
            39,
            &[],
            vec![readonly(), AccountMeta::new_readonly(owner(), false)],
            None,
        );
        c.assert_counter(7);
        for scenario in [0, 1] {
            let mut c = fresh(profile);
            c.run(34, &args(scenario, 0), metas(), None);
            c.assert_counter(7);
            assert_eq!(c.last_cpis, 1);
        }
    }
}

#[test]
fn generated_writable_cpi_observes_commit_and_refreshes_prefix_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        c.run(34, &args(2, 0), metas(), None);
        c.assert_counter(29);
        assert_eq!(c.last_cpis, 1);
        c.run(34, &args(14, 0), metas(), None);
        c.assert_counter(13);
    }
}

#[test]
fn generated_signed_cpi_checks_pda_and_transaction_authority_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        let (pda, _) = Pubkey::find_program_address(&[b"cpi-authority"], &owner());
        c.state_mut().data[12..44].copy_from_slice(&pda.to_bytes());
        c.accounts
            .push((pda, Account::new(1_000_000, 0, &Pubkey::default())));
        let mut m = metas();
        m[1] = AccountMeta::new_readonly(pda, false);
        c.run(34, &args(3, 0), m.clone(), None);
        assert_eq!(
            u64::from_le_bytes(c.state().data[44..52].try_into().unwrap()),
            29
        );
        c.reject(34, &args(3, 1), m.clone());
        c.reject(34, &args(2, 0), m);
        c.reject(34, &args(3, 0), metas());
        let mut c = fresh(profile);
        let mut m = metas();
        m[1].is_signer = false;
        c.reject(34, &args(2, 0), m);
    }
}

#[test]
fn checked_sdk_cpi_grows_and_shrinks_with_full_reload_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        for size in [512, 128, 256] {
            c.run(34, &args(4, size), metas(), None);
            c.assert_counter(7);
            assert_eq!(c.state().data.len(), size as usize);
        }
    }
}

#[test]
fn duplicate_cpi_handles_and_direct_guard_conflicts_all_profiles() {
    for profile in 0..3 {
        for scenario in [5, 13, 16] {
            let mut c = fresh(profile);
            c.run(34, &args(scenario, 256), metas(), None);
            c.assert_counter(7);
            if scenario == 16 {
                assert_eq!(c.last_cpis, 0);
            }
        }
        let mut c = fresh(profile);
        c.run(
            34,
            &args(6, 0),
            metas(),
            Some(InstructionError::AccountBorrowFailed),
        );
        c.assert_counter(7);
    }
}

#[test]
fn returned_callback_errors_reacquire_and_callee_errors_roll_back_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        c.run(34, &args(7, 0), metas(), None);
        c.assert_counter(7);
        assert_eq!(c.last_cpis, 1);
        c.run(
            34,
            &args(17, 0),
            metas(),
            Some(InstructionError::Custom(91)),
        );
        c.assert_counter(7);
    }
}

#[test]
fn callee_corruption_owner_changes_and_error_precedence_roll_back_all_profiles() {
    for profile in 0..3 {
        for damage in 1..=5 {
            for scenario in [8, 15] {
                let mut c = fresh(profile);
                c.execute(
                    34,
                    &args(scenario, damage),
                    metas(),
                    true,
                    (scenario == 15).then_some(InstructionError::Custom(92)),
                );
                assert_eq!(c.last_cpis, 1);
                c.assert_counter(7);
            }
        }
    }
}

#[test]
fn failed_reacquisition_is_terminal_and_keeps_callback_error_all_profiles() {
    for profile in 0..3 {
        for (scenario, error) in [
            (9, InstructionError::AccountBorrowFailed),
            (10, InstructionError::Custom(92)),
        ] {
            let mut c = fresh(profile);
            c.run(34, &args(scenario, 0), metas(), Some(error));
            c.assert_counter(7);
        }
    }
}

#[test]
fn same_invocation_cpi_and_reopen_preserve_saved_original_window_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile);
        c.run(34, &args(11, 10_496), metas(), None);
        c.assert_counter(7);
        assert_eq!(c.state().data.len(), 10_496);
        // A new top-level instruction gets a new original length.
        c.state_mut().lamports = c.svm.sysvars.rent.minimum_balance(10_497);
        c.run(34, &args(4, 10_497), metas(), None);
        assert_eq!(c.state().data.len(), 10_497);
    }
}

#[test]
fn nested_cpi_keeps_ancestor_growth_ceiling_all_profiles() {
    for profile in 0..3 {
        for size in [10_496, 10_497] {
            let mut c = fresh(profile);
            c.run(
                34,
                &args(12, size),
                metas(),
                (size > 10_496).then_some(InstructionError::InvalidRealloc),
            );
            c.assert_counter(7);
            assert_eq!(
                c.state().data.len(),
                if size == 10_496 { 10_496 } else { 256 }
            );
            assert_eq!(c.last_cpis, 3);
        }
    }
}

#[test]
fn widest_prefix_cpi_reload_preserves_bytes_and_stack_bounds_all_profiles() {
    use blubber_model::{Schema, Value};
    let schema = Schema::Struct(vec![
        Schema::Array(1024),
        Schema::List(Box::new(Schema::Str)),
    ]);
    let prefix: Vec<u8> = (0..1024).map(|i| (i % 251) as u8).collect();
    let disc = [11, 138, 208, 111, 251, 146, 84, 243];
    let expected = Value::Struct(vec![Value::Array(prefix), Value::List(vec![])]);
    for profile in 0..3 {
        let mut c = fresh(profile);
        c.state_mut().data = schema.encode_account(disc, &expected, 2048).unwrap();
        c.run(34, &args(19, 2500), metas(), None);
        assert_eq!(c.state().data.len(), 2500);
        assert_eq!(
            schema.decode_account(disc, &c.state().data).unwrap().value,
            expected
        );
        assert_eq!(c.last_cpis, 1);
    }
}
