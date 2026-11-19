//! Managed edits against the independent codec, rent oracle and runtime rollback.
#![forbid(unsafe_code)]
#[allow(dead_code)]
mod support;
use blubber_model::{
    IntWidth::{W4, W8},
    Key, KeySchema, Schema, Value,
};
use solana_account::Account;
use solana_instruction::{error::InstructionError, AccountMeta};
use solana_pubkey::Pubkey;
use support::*;

const REFUND: Pubkey = Pubkey::new_from_array([10; 32]);
const SYSTEM: Pubkey = Pubkey::new_from_array([0; 32]);
const DISC: [u8; 8] = [148, 173, 82, 162, 49, 78, 173, 254];
fn schema() -> Schema {
    Schema::Struct(vec![
        Schema::Pubkey,
        Schema::Unsigned(W8),
        Schema::List(Box::new(Schema::Str)),
        Schema::Map(KeySchema::Unsigned(W4), Box::new(Schema::Str)),
    ])
}
fn value(count: u32, width: usize, counter: u64) -> Value {
    Value::Struct(vec![
        Value::Pubkey(PAYER.to_bytes()),
        Value::Unsigned(counter.into()),
        Value::List((0..count).map(|_| Value::Str("x".repeat(width))).collect()),
        Value::Map(
            (0..count)
                .map(|i| (Key::Unsigned(i.into()), Value::Str("x".repeat(width))))
                .collect(),
        ),
    ])
}
fn fresh(profile: u8, len: usize) -> Case {
    let mut c = Case::new(profile, false);
    c.state_mut().data = schema().encode_account(DISC, &value(0, 0, 7), len).unwrap();
    c.state_mut().lamports = c.svm.sysvars.rent.minimum_balance(len);
    c.accounts
        .push(mollusk_svm::program::keyed_account_for_system_program());
    c.accounts
        .push((REFUND, Account::new(1_000_000, 0, &Pubkey::default())));
    c
}
fn metas() -> Vec<AccountMeta> {
    vec![
        writable(),
        AccountMeta::new(PAYER, true),
        AccountMeta::new_readonly(SYSTEM, false),
        AccountMeta::new(REFUND, false),
    ]
}
fn args(count: u32, width: u32, mode: u8, headroom: u8, prepaid: bool, refund: bool) -> Vec<u8> {
    let mut args = count.to_le_bytes().to_vec();
    args.extend_from_slice(&width.to_le_bytes());
    args.extend([mode, headroom, u8::from(prepaid), u8::from(refund)]);
    args
}
fn run(c: &mut Case, count: u32, width: u32, mode: u8, headroom: u8, prepaid: bool, refund: bool) {
    c.run(
        27,
        &args(count, width, mode, headroom, prepaid, refund),
        metas(),
        None,
    );
}
fn balance(c: &Case, key: Pubkey) -> u64 {
    c.accounts
        .iter()
        .find(|(k, _)| *k == key)
        .unwrap()
        .1
        .lamports
}
fn oracle(c: &Case, count: u32, width: usize) {
    let expected = value(count, width, 19);
    assert_eq!(
        schema()
            .decode_account(DISC, &c.state().data)
            .unwrap()
            .value,
        expected
    );
    let bytes = schema()
        .encode_account(DISC, &expected, c.state().data.len())
        .unwrap();
    assert_eq!(&bytes[..c.used()], &c.state().data[..c.used()]);
}

#[test]
fn constraint_bound_growth_funds_once_and_matches_oracle_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile, 128);
        let before = balance(&c, PAYER);
        c.run(26, &args(10, 40, 0, 0, false, false)[..9], metas(), None);
        oracle(&c, 10, 40);
        assert_eq!(c.state().data.len(), c.used());
        let rent = c.svm.sysvars.rent.minimum_balance(c.used());
        assert_eq!(c.state().lamports, rent);
        assert_eq!(
            balance(&c, PAYER),
            before - (rent - c.svm.sysvars.rent.minimum_balance(128))
        );
        assert_eq!(c.last_cpis, 1);
    }
}

#[test]
fn all_headroom_modes_retain_floor_and_net_transient_growth_all_profiles() {
    for profile in 0..3 {
        for headroom in 0..4 {
            let mut c = fresh(profile, 256);
            let before = balance(&c, PAYER);
            run(&mut c, 0, 0, 0, headroom, false, false);
            oracle(&c, 0, 0);
            assert_eq!(c.state().data.len(), 256);
            assert_eq!(balance(&c, PAYER), before);
            assert_eq!(c.last_cpis, 0);
            if headroom != 1 {
                run(&mut c, 5, 60, 1, headroom, false, false);
                oracle(&c, 0, 0);
                assert_eq!(c.state().data.len(), 256);
                assert_eq!(balance(&c, PAYER), before);
                assert_eq!(c.last_cpis, 0);
            }
        }
    }
}

#[test]
fn reserve_trim_refund_and_reopen_use_actual_lengths_all_profiles() {
    for profile in 0..3 {
        for headroom in [0, 2, 3] {
            let mut c = fresh(profile, 256);
            run(&mut c, 0, 0, 2, headroom, false, false);
            oracle(&c, 0, 0);
            assert_eq!(c.state().data.len(), c.used() + 700);
            assert_eq!(c.last_cpis, 1);
            let before = balance(&c, REFUND);
            let total = c.state().lamports;
            run(&mut c, 0, 0, 3, 1, true, true);
            let used = c.used();
            assert_eq!(c.state().data.len(), used);
            assert_eq!(c.last_cpis, 0);
            let rent = c.svm.sysvars.rent.minimum_balance(used);
            assert_eq!(c.state().lamports, rent);
            assert_eq!(balance(&c, REFUND), before + total - rent);
            run(&mut c, 4, 30, 0, headroom, false, false);
            oracle(&c, 4, 30);
            assert_eq!(c.last_cpis, 1);
        }
    }
}

#[test]
fn prepaid_capacity_window_and_settlement_failures_roll_back_all_profiles() {
    for profile in 0..3 {
        for (count, width, mode, headroom, prepaid) in [
            (4, 50, 0, 1, false),
            (20, 400, 0, 0, false),
            (20, 400, 0, 3, false),
            (4, 50, 0, 0, true),
            (0, 0, 2, 0, true),
        ] {
            let mut c = fresh(profile, 128);
            c.reject(
                27,
                &args(count, width, mode, headroom, prepaid, false),
                metas(),
            );
            assert_eq!(c.last_cpis, 0);
        }
        let mut c = fresh(profile, 128);
        c.accounts
            .iter_mut()
            .find(|(k, _)| *k == PAYER)
            .unwrap()
            .1
            .lamports = 1;
        c.reject(27, &args(4, 50, 0, 0, false, false), metas());
        assert_eq!(c.last_cpis, 0);
    }
}

#[test]
fn funding_and_refund_aliases_and_invalid_views_fail_before_callback_all_profiles() {
    for profile in 0..3 {
        for prepaid in [true, false] {
            for headroom in [0, 2, 3] {
                let mut c = fresh(profile, 64);
                if !prepaid {
                    c.accounts
                        .iter_mut()
                        .find(|(key, _)| *key == PAYER)
                        .unwrap()
                        .1
                        .lamports = 1;
                }
                let before = c.accounts.clone();
                run(&mut c, 1, 50, 6, headroom, prepaid, false);
                assert_eq!(c.accounts, before, "caught native rent frame");
                assert_eq!(c.last_cpis, 0);
            }
        }
    }

    for profile in 0..3 {
        for field in [1, 2, 3] {
            let mut c = fresh(profile, 256);
            let mut m = metas();
            m[field] = writable();
            c.reject(27, &args(0, 0, 0, 0, false, true), m);
            assert_eq!(c.last_cpis, 0);
        }
        for bad in 0..4 {
            let mut c = fresh(profile, 256);
            let mut m = metas();
            match bad {
                0 => m[1].is_signer = false,
                1 => m[1].is_writable = false,
                2 => {
                    c.accounts
                        .iter_mut()
                        .find(|(k, _)| *k == PAYER)
                        .unwrap()
                        .1
                        .owner = owner()
                }
                3 => {
                    c.accounts
                        .iter_mut()
                        .find(|(k, _)| *k == PAYER)
                        .unwrap()
                        .1
                        .data = vec![0]
                }
                _ => unreachable!(),
            }
            c.reject(27, &args(0, 0, 0, 0, false, false), m);
            assert_eq!(c.last_cpis, 0);
        }
        let mut c = fresh(profile, 256);
        let mut m = metas();
        m[3] = AccountMeta::new(PAYER, true);
        c.reject(27, &args(0, 0, 0, 0, false, true), m);
    }
}

#[test]
fn callback_and_post_commit_errors_reacquire_then_roll_back_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile, 128);
        c.run(
            27,
            &args(4, 50, 4, 3, false, false),
            metas(),
            Some(InstructionError::Custom(81)),
        );
        assert_eq!(c.last_cpis, 0);
        c.run(
            27,
            &args(4, 50, 5, 3, false, false),
            metas(),
            Some(InstructionError::Custom(82)),
        );
        assert_eq!(c.last_cpis, 1);
    }
}

#[test]
fn repeated_managed_handoffs_keep_exclusion_and_bounded_slots_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile, 256);
        c.run(28, &24u32.to_le_bytes(), metas(), None);
        oracle(&c, 0, 0);
        assert_eq!(c.last_cpis, 0);
        assert_eq!(c.state().data.len(), 256);
    }
}

#[test]
fn widest_native_prefix_edits_and_refreshes_all_profiles() {
    let schema = Schema::Struct(vec![
        Schema::Array(1024),
        Schema::List(Box::new(Schema::Str)),
    ]);
    let prefix: Vec<u8> = (0..1024).map(|i| (i % 251) as u8).collect();
    let disc = [11, 138, 208, 111, 251, 146, 84, 243];
    for profile in 0..3 {
        let mut c = Case::new(profile, false);
        let initial = Value::Struct(vec![Value::Array(prefix.clone()), Value::List(vec![])]);
        c.state_mut().data = schema.encode_account(disc, &initial, 2048).unwrap();
        c.state_mut().lamports = c.svm.sysvars.rent.minimum_balance(2048);
        c.run(29, &[], vec![writable()], None);
        let expected = Value::Struct(vec![
            Value::Array(prefix.clone()),
            Value::List(vec![Value::Str("wide".into())]),
        ]);
        assert_eq!(
            schema.decode_account(disc, &c.state().data).unwrap().value,
            expected
        );
        let encoded = schema.encode_account(disc, &expected, 2048).unwrap();
        assert_eq!(&c.state().data[..c.used()], &encoded[..c.used()]);
        assert_eq!(c.state().data.len(), 2048);
        assert_eq!(c.last_cpis, 0);
    }
}

#[test]
fn init_hook_funding_bindings_survive_initialization_all_profiles() {
    for profile in 0..3 {
        let mut c = fresh(profile, 256);
        *c.state_mut() = Account::default();
        let before = balance(&c, PAYER);
        c.run(
            30,
            &256u32.to_le_bytes(),
            vec![
                AccountMeta::new(PAYER, true),
                AccountMeta::new(STATE, true),
                AccountMeta::new_readonly(SYSTEM, false),
            ],
            None,
        );
        oracle(&c, 1, 16);
        let rent = c.svm.sysvars.rent.minimum_balance(256);
        assert_eq!(c.state().lamports, rent);
        assert_eq!(balance(&c, PAYER), before - rent);
        assert_eq!(c.state().data.len(), 256);
        assert_eq!(c.last_cpis, 1);
    }
}

#[test]
fn bound_and_plain_funding_defaults_reject_growth_without_writes_all_profiles() {
    for profile in 0..3 {
        for mode in [7, 8] {
            let mut c = fresh(profile, 128);
            let before = c.state().clone();
            let payer = balance(&c, PAYER);
            c.run(26, &args(1, 256, mode, 0, false, false)[..9], metas(), None);
            assert_eq!(c.state(), &before);
            assert_eq!(balance(&c, PAYER), payer);
            assert_eq!(c.last_cpis, 0);
        }
    }
}
