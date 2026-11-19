//! Execute every documented native instruction against an independent codec.
#![forbid(unsafe_code)]

use blubber_model::{IntWidth::W8, Schema, Value};
use mollusk_svm::Mollusk;
use solana_account::Account;
use solana_instruction::{error::InstructionError, AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::{path::PathBuf, str::FromStr};

const PAYER: Pubkey = Pubkey::new_from_array([9; 32]);
const SYSTEM: Pubkey = Pubkey::new_from_array([0; 32]);
const MINIMUM: usize = 56; // Envelope + authority + counter + empty list.
const DISC: [u8; 8] = [51, 41, 160, 37, 136, 15, 218, 211];

fn owner() -> Pubkey {
    Pubkey::from_str("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u").unwrap()
}
fn schema() -> Schema {
    Schema::Struct(vec![
        Schema::Pubkey,
        Schema::Unsigned(W8),
        Schema::List(Box::new(Schema::Str)),
    ])
}
fn text_arg(text: &str) -> Vec<u8> {
    let mut data = (text.len() as u32).to_le_bytes().to_vec();
    data.extend_from_slice(text.as_bytes());
    data
}
struct Case {
    svm: Mollusk,
    accounts: Vec<(Pubkey, Account)>,
    cpis: usize,
    last_error: Option<InstructionError>,
}
impl Case {
    fn new(profile: u8, pda: bool, prefund: u64) -> Self {
        let mut svm = Mollusk::default();
        svm.feature_set.virtual_address_space_adjustments = profile != 0;
        svm.feature_set.account_data_direct_mapping = profile == 2;
        assert!(std::env::var_os("SBF_TRACE_DIR").is_some());
        svm.program_cache =
            mollusk_svm::program::ProgramCache::new(&svm.feature_set, &svm.compute_budget, true);
        let path = PathBuf::from(std::env::var("BLUBBER_ANCHOR_DEMO_DIR").unwrap())
            .join("blubber_anchor_v2_demo.so");
        assert!(path.is_file());
        svm.add_program(&owner(), path.with_extension("").to_str().unwrap());
        let state = if pda {
            Pubkey::find_program_address(&[b"state", PAYER.as_ref()], &owner()).0
        } else {
            Pubkey::new_from_array([8; 32])
        };
        Self {
            svm,
            cpis: 0,
            last_error: None,
            accounts: vec![
                (
                    state,
                    Account {
                        lamports: prefund,
                        ..Account::default()
                    },
                ),
                (PAYER, Account::new(100_000_000, 0, &SYSTEM)),
                mollusk_svm::program::keyed_account_for_system_program(),
            ],
        }
    }
    fn state(&self) -> &Account {
        &self.accounts[0].1
    }
    fn create_metas(&self, pda: bool) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(PAYER, true),
            AccountMeta::new(self.accounts[0].0, !pda),
            AccountMeta::new_readonly(SYSTEM, false),
        ]
    }
    fn edit_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.accounts[0].0, false),
            AccountMeta::new(PAYER, true),
            AccountMeta::new_readonly(SYSTEM, false),
        ]
    }
    fn run(&mut self, opcode: u8, args: &[u8], metas: Vec<AccountMeta>, reject: bool) {
        let mut data = vec![opcode];
        data.extend_from_slice(args);
        let before = self.accounts.clone();
        let result = self.svm.process_instruction(
            &Instruction {
                program_id: owner(),
                accounts: metas,
                data,
            },
            &self.accounts,
        );
        self.cpis = result.inner_instructions.len();
        self.last_error = result.raw_result.clone().err();
        if reject {
            assert!(result.raw_result.is_err(), "opcode {opcode}");
            assert_eq!(result.resulting_accounts, before);
        } else {
            assert_eq!(result.raw_result, Ok(()), "opcode {opcode}");
            self.accounts = result.resulting_accounts;
        }
    }
    fn initialize(&mut self, pda: bool, reserve: u32) {
        self.run(
            u8::from(pda),
            &reserve.to_le_bytes(),
            self.create_metas(pda),
            false,
        );
    }
    fn assert_value(&self, counter: u64, texts: &[&str]) {
        let expected = Value::Struct(vec![
            Value::Pubkey(PAYER.to_bytes()),
            Value::Unsigned(counter.into()),
            Value::List(texts.iter().map(|s| Value::Str((*s).into())).collect()),
        ]);
        let decoded = schema().decode_account(DISC, &self.state().data).unwrap();
        assert_eq!(decoded.value, expected);
        let encoded = schema()
            .encode_account(DISC, &expected, self.state().data.len())
            .unwrap();
        let used = u32::from_le_bytes(encoded[8..12].try_into().unwrap()) as usize;
        assert_eq!(&self.state().data[..used], &encoded[..used]);
    }
}

#[test]
fn signer_init_and_read_all_profiles() {
    for profile in 0..3 {
        let mut c = Case::new(profile, false, 0);
        c.initialize(false, 200);
        assert_eq!(c.cpis, 1);
        assert_eq!(c.state().data.len(), 256);
        assert_eq!(c.state().lamports, c.svm.sysvars.rent.minimum_balance(256));
        c.assert_value(0, &[]);
        c.run(
            4,
            &0u64.to_le_bytes(),
            vec![AccountMeta::new_readonly(c.accounts[0].0, false)],
            false,
        );
    }
}

#[test]
fn pda_prefund_reuse_keeps_state_all_profiles() {
    for profile in 0..3 {
        for prefund in [0, 1_000_000] {
            let mut c = Case::new(profile, true, prefund);
            c.initialize(true, 200);
            c.run(2, &text_arg("borrowed"), c.edit_metas(), false);
            let before = c.accounts.clone();
            c.initialize(true, 200);
            assert_eq!(c.cpis, 0);
            assert_eq!(c.accounts, before);
            c.assert_value(1, &["borrowed"]);
            c.run(1, &201u32.to_le_bytes(), c.create_metas(true), true);
        }
    }
}

#[test]
fn managed_growth_and_explicit_reservation_trim_all_profiles() {
    for profile in 0..3 {
        let mut c = Case::new(profile, false, 0);
        c.initialize(false, 0);
        let before = c.accounts[1].1.lamports;
        c.run(2, &text_arg("hello"), c.edit_metas(), false);
        assert_eq!(c.cpis, 1);
        c.assert_value(1, &["hello"]);
        assert_eq!(c.state().data.len(), MINIMUM + 5);
        assert_eq!(
            before - c.accounts[1].1.lamports,
            c.svm.sysvars.rent.minimum_balance(61) - c.svm.sysvars.rent.minimum_balance(56)
        );
        let mut args = 512u32.to_le_bytes().to_vec();
        args.push(0);
        c.run(3, &args, c.edit_metas(), false);
        assert!(c.state().data.len() >= 61 + 512);
        let reserved = c.state().data.len();
        c.run(2, &text_arg("world"), c.edit_metas(), false);
        assert_eq!(c.cpis, 0);
        assert_eq!(c.state().data.len(), reserved);
        args[4] = 1;
        c.run(3, &args, c.edit_metas(), false);
        assert_eq!(c.state().data.len(), MINIMUM + 4 + 10);
        assert_eq!(c.cpis, 0);
        c.assert_value(2, &["hello", "world"]);
        let balance = c.state().lamports;
        assert!(balance >= c.svm.sysvars.rent.minimum_balance(c.state().data.len()));
    }
}

#[test]
fn scoped_generated_cpi_commits_then_reloads_all_profiles() {
    for profile in 0..3 {
        let mut c = Case::new(profile, false, 0);
        c.initialize(false, 200);
        let metas = vec![
            AccountMeta::new(c.accounts[0].0, false),
            AccountMeta::new_readonly(PAYER, true),
            AccountMeta::new_readonly(owner(), false),
        ];
        c.run(5, &42u64.to_le_bytes(), metas, false);
        assert_eq!(c.cpis, 1);
        c.assert_value(42, &[]);
        c.run(
            4,
            &41u64.to_le_bytes(),
            vec![AccountMeta::new_readonly(c.accounts[0].0, false)],
            true,
        );
        assert_eq!(c.last_error, Some(InstructionError::Custom(6000)));
    }
}

#[test]
fn constraints_and_bad_ingress_roll_back_all_profiles() {
    for profile in 0..3 {
        let mut c = Case::new(profile, false, 0);
        c.initialize(false, 200);
        let mut metas = c.edit_metas();
        metas[1].is_signer = false;
        c.run(2, &text_arg("rejected"), metas, true);
        // A valid signer with a different stored address fails the copied-prefix constraint.
        c.accounts[0].1.data[12] ^= 1;
        c.run(2, &text_arg("rejected"), c.edit_metas(), true);
        c.accounts[0].1.data[12] ^= 1;
        // A directory count which has no matching end table fails full ingress.
        c.accounts[0].1.data[52..56].copy_from_slice(&u32::MAX.to_le_bytes());
        c.run(
            4,
            &0u64.to_le_bytes(),
            vec![AccountMeta::new_readonly(c.accounts[0].0, false)],
            true,
        );
    }
}

#[test]
fn wrong_pda_and_creation_limits_fail_before_cpi_all_profiles() {
    for profile in 0..3 {
        let mut c = Case::new(profile, false, 0);
        c.run(1, &200u32.to_le_bytes(), c.create_metas(true), true);
        assert_eq!(c.cpis, 0);
        for reserve in [10_241, u32::MAX] {
            c.run(0, &reserve.to_le_bytes(), c.create_metas(false), true);
            assert_eq!(c.cpis, 0);
        }
    }
}
