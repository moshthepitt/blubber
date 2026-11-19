//! Heap-free SBF metadata copies, padding cap, refresh and wide-prefix opt-out.

#![forbid(unsafe_code)]

use blubber_model::{IntWidth::W8, Schema, Value};
use mollusk_svm::{result::Check, Mollusk};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::path::PathBuf;

const OWNER: Pubkey = Pubkey::new_from_array([7; 32]);
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);

fn run(opcode: u8, bytes: Vec<u8>, expected: Vec<u8>) {
    let directory = PathBuf::from(
        std::env::var("BLUBBER_ANCHOR_PREFIX_DIR").expect("run the Anchor compile lane"),
    );
    let elf = directory.join("blubber_stack_matrix.so");
    assert!(elf.is_file(), "missing prefix stack ELF");
    assert!(
        std::env::var_os("SBF_TRACE_DIR").is_some(),
        "compile lane requires traces"
    );
    for profile in 0..3 {
        let mut svm = Mollusk::default();
        svm.feature_set.virtual_address_space_adjustments = profile != 0;
        svm.feature_set.account_data_direct_mapping = profile == 2;
        svm.program_cache =
            mollusk_svm::program::ProgramCache::new(&svm.feature_set, &svm.compute_budget, true);
        svm.add_program(&OWNER, elf.with_extension("").to_str().unwrap());
        let accounts = [(
            STATE,
            Account {
                lamports: svm.sysvars.rent.minimum_balance(bytes.len()),
                data: bytes.clone(),
                owner: OWNER,
                ..Account::default()
            },
        )];
        let result = svm.process_and_validate_instruction(
            &Instruction {
                program_id: OWNER,
                accounts: vec![AccountMeta::new(STATE, false)],
                data: vec![opcode],
            },
            &accounts,
            &[Check::success(), Check::inner_instruction_count(0)],
        );
        assert_eq!(
            result.get_account(&STATE).unwrap().data,
            expected,
            "profile {profile}"
        );
    }
}

#[test]
fn at_cap_prefix_copies_and_refreshes_all_profiles() {
    let schema = Schema::Struct(vec![
        Schema::Bool,
        Schema::Unsigned(W8),
        Schema::Array(1001),
        Schema::Str,
    ]);
    let value = |code| {
        Value::Struct(vec![
            Value::Bool(true),
            Value::Unsigned(code),
            Value::Array((0..1001).map(|index| (index % 251) as u8).collect()),
            Value::Str("wide".into()),
        ])
    };
    // D21 layout-bound identity, computed independently.
    let discriminator = [221, 163, 101, 98, 23, 191, 49, 137];
    let space = 12 + schema.encode(&value(19)).unwrap().len();
    let bytes = schema
        .encode_account(discriminator, &value(19), space)
        .unwrap();
    let expected = schema
        .encode_account(discriminator, &value(42), space)
        .unwrap();
    run(0, bytes, expected);
}

#[test]
fn opted_out_wide_prefix_keeps_checked_readers_all_profiles() {
    let schema = Schema::Struct(vec![Schema::Array(4096), Schema::Str]);
    let value = Value::Struct(vec![Value::Array(vec![7; 4096]), Value::Str("wide".into())]);
    // D21 layout-bound identity, computed independently.
    let discriminator = [106, 49, 73, 78, 22, 23, 206, 31];
    let space = 12 + schema.encode(&value).unwrap().len();
    let bytes = schema.encode_account(discriminator, &value, space).unwrap();
    run(1, bytes.clone(), bytes);
}
