use blubber_model::{IntWidth::W8, Schema, Value};
use mollusk_svm::Mollusk;
use solana_account::Account;
use solana_instruction::{error::InstructionError, AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::{path::PathBuf, str::FromStr};
pub const STATE: Pubkey = Pubkey::new_from_array([8; 32]);
pub const PAYER: Pubkey = Pubkey::new_from_array([9; 32]);
pub fn owner() -> Pubkey {
    Pubkey::from_str("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u").unwrap()
}
pub fn readonly() -> AccountMeta {
    AccountMeta::new_readonly(STATE, false)
}
pub fn writable() -> AccountMeta {
    AccountMeta::new(STATE, false)
}
pub fn signer() -> AccountMeta {
    AccountMeta::new_readonly(PAYER, true)
}
pub fn read_metas() -> Vec<AccountMeta> {
    vec![readonly()]
}
pub fn edit_metas() -> Vec<AccountMeta> {
    vec![writable(), signer()]
}
pub fn duplicates() -> Vec<AccountMeta> {
    vec![writable(), writable()]
}
fn schema(scalar: bool) -> Schema {
    if scalar {
        Schema::Struct(vec![Schema::Pubkey, Schema::Unsigned(W8)])
    } else {
        Schema::Struct(vec![
            Schema::Pubkey,
            Schema::Unsigned(W8),
            Schema::Bool,
            Schema::Str,
            Schema::List(Box::new(Schema::Str)),
        ])
    }
}
fn value(scalar: bool, counter: u64) -> Value {
    let mut fields = vec![
        Value::Pubkey(PAYER.to_bytes()),
        Value::Unsigned(counter.into()),
    ];
    if !scalar {
        fields.extend([
            Value::Bool(true),
            Value::Str("hello".into()),
            Value::List(vec![Value::Str("a".into()), Value::Str("bc".into())]),
        ]);
    }
    Value::Struct(fields)
}
pub struct Case {
    pub svm: Mollusk,
    pub accounts: Vec<(Pubkey, Account)>,
    pub target: Pubkey,
    pub last_cpis: usize,
    scalar: bool,
}
impl Case {
    pub fn new(profile: u8, scalar: bool) -> Self {
        let mut svm = Mollusk::default();
        svm.feature_set.virtual_address_space_adjustments = profile != 0;
        svm.feature_set.account_data_direct_mapping = profile == 2;
        assert!(std::env::var_os("SBF_TRACE_DIR").is_some());
        svm.program_cache = mollusk_svm::program::ProgramCache::new(
            &svm.feature_set,
            &svm.compute_budget,
            std::env::var("BLUBBER_NATIVE_TRACE").as_deref() != Ok("0"),
        );
        let directory =
            PathBuf::from(std::env::var("BLUBBER_ANCHOR_LOAD_DIR").expect("run load/bench lane"));
        let elf = directory.join("blubber_anchor_v2_load.so");
        assert!(elf.is_file());
        svm.add_program(&owner(), elf.with_extension("").to_str().unwrap());
        let disc = if scalar {
            [183, 96, 142, 15, 26, 195, 25, 235]
        } else {
            [251, 27, 27, 26, 249, 14, 179, 236]
        };
        let mut data = schema(scalar)
            .encode_account(disc, &value(scalar, 7), 256)
            .unwrap();
        let used = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
        data[used..].fill(0xA5);
        let account = Account {
            data,
            owner: owner(),
            lamports: svm.sysvars.rent.minimum_balance(256),
            ..Account::default()
        };
        Self {
            svm,
            scalar,
            target: STATE,
            last_cpis: 0,
            accounts: vec![
                (STATE, account),
                (PAYER, Account::new(100_000_000, 0, &Pubkey::default())),
            ],
        }
    }
    pub fn state(&self) -> &Account {
        &self
            .accounts
            .iter()
            .find(|(k, _)| *k == self.target)
            .unwrap()
            .1
    }
    pub fn state_mut(&mut self) -> &mut Account {
        &mut self
            .accounts
            .iter_mut()
            .find(|(k, _)| *k == self.target)
            .unwrap()
            .1
    }
    pub fn used(&self) -> usize {
        u32::from_le_bytes(self.state().data[8..12].try_into().unwrap()) as usize
    }
    pub fn assert_counter(&self, counter: u64) {
        self.assert_value(counter, "hello");
    }
    pub fn assert_value(&self, counter: u64, text: &str) {
        let disc: [u8; 8] = self.state().data[..8].try_into().unwrap();
        let mut expected = value(self.scalar, counter);
        if !self.scalar {
            let Value::Struct(fields) = &mut expected else {
                unreachable!()
            };
            fields[3] = Value::Str(text.into());
        }
        assert_eq!(
            schema(self.scalar)
                .decode_account(disc, &self.state().data)
                .unwrap()
                .value,
            expected
        );
    }
    pub fn execute(
        &mut self,
        opcode: u8,
        args: &[u8],
        metas: Vec<AccountMeta>,
        reject: bool,
        exact: Option<InstructionError>,
    ) -> u64 {
        let mut data = vec![opcode];
        data.extend_from_slice(args);
        let ix = Instruction {
            program_id: owner(),
            accounts: metas,
            data,
        };
        let before = self.accounts.clone();
        let result = self.svm.process_instruction(&ix, &self.accounts);
        self.last_cpis = result.inner_instructions.len();
        if reject {
            assert!(
                result.raw_result.is_err(),
                "opcode {opcode} unexpectedly succeeded"
            );
            if let Some(error) = exact {
                assert_eq!(result.raw_result, Err(error));
            }
            assert_eq!(result.resulting_accounts, before);
        } else {
            assert_eq!(result.raw_result, Ok(()), "opcode {opcode}");
            if opcode < 20 {
                assert!(result.inner_instructions.is_empty());
            }
            self.accounts = result.resulting_accounts;
        }
        result.compute_units_consumed
    }
    pub fn run(
        &mut self,
        opcode: u8,
        args: &[u8],
        metas: Vec<AccountMeta>,
        error: Option<InstructionError>,
    ) -> u64 {
        self.execute(opcode, args, metas, error.is_some(), error)
    }
    pub fn reject(&mut self, opcode: u8, args: &[u8], metas: Vec<AccountMeta>) {
        self.execute(opcode, args, metas, true, None);
    }
}
