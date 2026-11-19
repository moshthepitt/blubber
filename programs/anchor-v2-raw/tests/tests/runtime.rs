//! Fresh raw ELFs, three memory profiles, and an independent account codec.
#![forbid(unsafe_code)]

use blubber_model::{IntWidth::W8, Schema, Value};
use mollusk_svm::{result::Check, Mollusk};
use solana_account::Account;
use solana_instruction::{error::InstructionError, AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::{path::PathBuf, str::FromStr};

const STATE: Pubkey = Pubkey::new_from_array([8; 32]);
const PAYER: Pubkey = Pubkey::new_from_array([9; 32]);
const SYSTEM: Pubkey = Pubkey::new_from_array([0; 32]);
const DISC: [u8; 8] = [204, 73, 17, 192, 123, 130, 171, 124];

fn owner() -> Pubkey {
    Pubkey::from_str("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u").unwrap()
}

fn schema() -> Schema {
    Schema::Struct(vec![
        Schema::Pubkey,
        Schema::Unsigned(W8),
        Schema::List(Box::new(Schema::Unsigned(W8))),
    ])
}

fn value(counter: u64, values: &[u64]) -> Value {
    Value::Struct(vec![
        Value::Pubkey(PAYER.to_bytes()),
        Value::Unsigned(counter.into()),
        Value::List(
            values
                .iter()
                .map(|n| Value::Unsigned((*n).into()))
                .collect(),
        ),
    ])
}

fn edit_metas() -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(STATE, false),
        AccountMeta::new_readonly(PAYER, true),
    ]
}

fn grow_metas() -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(STATE, false),
        AccountMeta::new(PAYER, true),
        AccountMeta::new_readonly(SYSTEM, false),
    ]
}

fn read_metas() -> Vec<AccountMeta> {
    vec![AccountMeta::new_readonly(STATE, false)]
}

struct Case {
    svm: Mollusk,
    accounts: Vec<(Pubkey, Account)>,
}

impl Case {
    fn new(profile: u8) -> Self {
        let mut svm = Mollusk::default();
        svm.feature_set.virtual_address_space_adjustments = profile != 0;
        svm.feature_set.account_data_direct_mapping = profile == 2;
        // The runner checks executed load/store addresses for heap access.
        assert!(
            std::env::var_os("SBF_TRACE_DIR").is_some(),
            "run the traced compile lane"
        );
        svm.program_cache =
            mollusk_svm::program::ProgramCache::new(&svm.feature_set, &svm.compute_budget, true);
        let directory =
            PathBuf::from(std::env::var("BLUBBER_ANCHOR_RAW_DIR").expect("run the compile lane"));
        let elf = directory.join("blubber_anchor_v2_raw.so");
        assert!(elf.is_file(), "missing fresh raw ELF: {}", elf.display());
        svm.add_program(&owner(), elf.with_extension("").to_str().unwrap());
        Self {
            svm,
            accounts: vec![
                (STATE, Account::default()),
                (PAYER, Account::new(100_000_000_000, 0, &SYSTEM)),
                mollusk_svm::program::keyed_account_for_system_program(),
            ],
        }
    }

    fn run(
        &mut self,
        opcode: u8,
        args: &[u8],
        metas: Vec<AccountMeta>,
        error: Option<InstructionError>,
    ) -> usize {
        let mut data = vec![opcode];
        data.extend_from_slice(args);
        let ix = Instruction {
            program_id: owner(),
            accounts: metas,
            data,
        };
        let result = self.svm.process_and_validate_instruction(
            &ix,
            &self.accounts,
            &[Check::program_result(
                error.clone().map_or(Ok(()), Err).into(),
            )],
        );
        let cpis = result.inner_instructions.len();
        if error.is_some() {
            assert_eq!(
                result.resulting_accounts, self.accounts,
                "failed instruction changed accounts"
            );
        } else {
            self.accounts = result.resulting_accounts;
        }
        cpis
    }

    fn initialize(&mut self) {
        let cpis = self.run(
            0,
            &[],
            vec![
                AccountMeta::new(PAYER, true),
                AccountMeta::new(STATE, true),
                AccountMeta::new_readonly(SYSTEM, false),
            ],
            None,
        );
        assert_eq!(cpis, 1, "creation must invoke System");
        self.assert_value(0, &[]);
        assert_eq!(
            self.state().data.len(),
            schema()
                .decode_account(DISC, &self.state().data)
                .unwrap()
                .used_len,
            "creation measures the exact initial reservation"
        );
    }

    fn state(&self) -> &Account {
        &self
            .accounts
            .iter()
            .find(|(key, _)| key == &STATE)
            .unwrap()
            .1
    }

    fn account_mut(&mut self, address: Pubkey) -> &mut Account {
        &mut self
            .accounts
            .iter_mut()
            .find(|(key, _)| *key == address)
            .unwrap()
            .1
    }

    fn assert_value(&self, counter: u64, values: &[u64]) {
        let state = self.state();
        assert_eq!(state.owner, owner());
        assert!(!state.executable);
        let decoded = schema().decode_account(DISC, &state.data).unwrap();
        assert_eq!(decoded.value, value(counter, values));
        assert!(state.data.len() >= decoded.used_len);
        assert!(state.lamports >= self.svm.sysvars.rent.minimum_balance(state.data.len()));
    }
}

#[test]
fn init_edit_read_all_profiles_without_heap() {
    for profile in 0..3 {
        let mut case = Case::new(profile);
        case.initialize();
        let initial_len = case.state().data.len();
        let state_balance = case.state().lamports;
        let payer_balance = case.account_mut(PAYER).lamports;
        assert_eq!(case.run(3, &7u64.to_le_bytes(), edit_metas(), None), 0);
        case.assert_value(7, &[]);
        assert_eq!(case.state().data.len(), initial_len);
        assert_eq!(case.state().lamports, state_balance);
        assert_eq!(case.account_mut(PAYER).lamports, payer_balance);

        let mut values = Vec::new();
        for value in [42u64, 43, 44] {
            assert_eq!(case.run(1, &value.to_le_bytes(), grow_metas(), None), 1);
            values.push(value);
            case.assert_value(7, &values);
            assert!(case.state().data.len() > initial_len);
            let minimum = case
                .svm
                .sysvars
                .rent
                .minimum_balance(case.state().data.len());
            assert_eq!(case.state().lamports, minimum);
            assert_eq!(
                case.account_mut(PAYER).lamports,
                payer_balance - (minimum - state_balance)
            );
        }
        let before = case.accounts.clone();
        assert_eq!(case.run(2, &42u64.to_le_bytes(), read_metas(), None), 0);
        assert_eq!(case.accounts, before, "readonly read changed accounts");
    }
}

#[test]
fn exact_capacity_and_committed_write_rollback_all_profiles() {
    for profile in 0..3 {
        let mut case = Case::new(profile);
        case.initialize();
        let initial_len = case.state().data.len();
        assert_eq!(case.run(6, &0u32.to_le_bytes(), edit_metas(), None), 0);
        case.run(
            6,
            &1u32.to_le_bytes(),
            edit_metas(),
            Some(InstructionError::Custom(0xB116)),
        );
        // The initial reservation is exactly full. Prepaid cannot fund even
        // one retained byte; the absolute cap and SDK window fail before rent.
        for (additional, code) in [
            (10_240 + 1, 0xB115),
            (10_485_760 + 1 - initial_len as u32, 0xB10F),
        ] {
            case.run(
                6,
                &additional.to_le_bytes(),
                edit_metas(),
                Some(InstructionError::Custom(code)),
            );
        }
        assert_eq!(case.state().data.len(), initial_len);
        for values in [vec![], vec![42]] {
            if !values.is_empty() {
                case.run(1, &42u64.to_le_bytes(), grow_metas(), None);
            }
            assert_eq!(
                case.run(4, &[], edit_metas(), Some(InstructionError::Custom(77))),
                0
            );
            case.assert_value(0, &values);
            assert_eq!(
                case.run(
                    5,
                    &99u64.to_le_bytes(),
                    grow_metas(),
                    Some(InstructionError::Custom(77))
                ),
                1
            );
            case.assert_value(0, &values);
        }

        // Retain the former 256-byte boundary check with an independently
        // prepaid rent budget and an empty payer. TopUp can only spend zero.
        let mut prepaid = Case::new(profile);
        prepaid.initialize();
        prepaid.account_mut(STATE).lamports = prepaid.svm.sysvars.rent.minimum_balance(256);
        prepaid.account_mut(PAYER).lamports = 0;
        let additional = 256 - prepaid.state().data.len() as u32;
        prepaid.run(6, &additional.to_le_bytes(), edit_metas(), None);
        assert_eq!(prepaid.state().data.len(), 256);
        let mut values = Vec::new();
        while schema()
            .encode_account(DISC, &value(0, &[values.as_slice(), &[1]].concat()), 256)
            .is_ok()
        {
            assert_eq!(prepaid.run(1, &1u64.to_le_bytes(), grow_metas(), None), 0);
            values.push(1);
            prepaid.assert_value(0, &values);
        }
        assert!(!values.is_empty());
        prepaid.run(
            1,
            &1u64.to_le_bytes(),
            grow_metas(),
            Some(InstructionError::Custom(0xB116)),
        );
        prepaid.assert_value(0, &values);
    }
}

#[test]
fn authentication_and_envelope_reject_bad_inputs_all_profiles() {
    for profile in 0..3 {
        let mut case = Case::new(profile);
        let mut unsigned_state = vec![
            AccountMeta::new(PAYER, true),
            AccountMeta::new(STATE, false),
            AccountMeta::new_readonly(SYSTEM, false),
        ];
        case.run(
            0,
            &[],
            unsigned_state.clone(),
            Some(InstructionError::MissingRequiredSignature),
        );
        unsigned_state[1].is_signer = true;
        case.account_mut(STATE).lamports = 1;
        case.run(
            0,
            &[],
            unsigned_state,
            Some(InstructionError::Custom(0xB11A)),
        );
        case.account_mut(STATE).lamports = 0;
        case.initialize();
        let mut readonly = grow_metas();
        readonly[0] = AccountMeta::new_readonly(STATE, false);
        case.run(
            1,
            &1u64.to_le_bytes(),
            readonly,
            Some(InstructionError::Custom(2000)),
        );
        let mut readonly_payer = grow_metas();
        readonly_payer[1] = AccountMeta::new_readonly(PAYER, true);
        case.run(
            1,
            &1u64.to_le_bytes(),
            readonly_payer,
            // rc.2's mutable Signer fuses both flag checks into this error.
            Some(InstructionError::MissingRequiredSignature),
        );
        let impostor = Pubkey::new_from_array([7; 32]);
        case.accounts
            .push((impostor, Account::new(1_000_000, 0, &SYSTEM)));
        for (opcode, mut metas) in [(1, grow_metas()), (3, edit_metas())] {
            metas[1] = AccountMeta::new(impostor, true);
            case.run(
                opcode,
                &1u64.to_le_bytes(),
                metas,
                Some(InstructionError::Custom(0xB117)),
            );
        }
        case.account_mut(PAYER).owner = owner();
        case.run(
            1,
            &1u64.to_le_bytes(),
            grow_metas(),
            Some(InstructionError::Custom(0xB118)),
        );
        case.account_mut(PAYER).owner = SYSTEM;
        let payer_balance = case.account_mut(PAYER).lamports;
        case.account_mut(PAYER).lamports = 0;
        case.run(
            1,
            &1u64.to_le_bytes(),
            grow_metas(),
            Some(InstructionError::Custom(0xB116)),
        );
        case.account_mut(PAYER).lamports = payer_balance;
        let mut unsigned = grow_metas();
        unsigned[1].is_signer = false;
        case.run(
            1,
            &1u64.to_le_bytes(),
            unsigned,
            Some(InstructionError::MissingRequiredSignature),
        );
        case.assert_value(0, &[]);
        case.account_mut(STATE).owner = SYSTEM;
        case.run(
            2,
            &0u64.to_le_bytes(),
            read_metas(),
            Some(InstructionError::IllegalOwner),
        );
        case.account_mut(STATE).owner = owner();
        case.account_mut(STATE).executable = true;
        for (opcode, metas) in [(1, grow_metas()), (2, read_metas()), (3, edit_metas())] {
            case.run(
                opcode,
                &0u64.to_le_bytes(),
                metas,
                Some(InstructionError::Custom(0xB117)),
            );
        }
        case.account_mut(STATE).executable = false;
        case.account_mut(STATE).data[8..12].copy_from_slice(&1u32.to_le_bytes());
        for (opcode, metas) in [(1, grow_metas()), (2, read_metas()), (3, edit_metas())] {
            case.run(
                opcode,
                &0u64.to_le_bytes(),
                metas,
                Some(InstructionError::Custom(0xB103)),
            );
        }
    }
}
