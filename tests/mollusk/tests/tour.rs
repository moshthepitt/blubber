//! Execute the complete no-heap tour and compare every opcode with an owned oracle.

#![forbid(unsafe_code)]

use blubber::{Error, ErrorKind};
use blubber_model::{IntWidth::*, Key, KeySchema, Schema, Value};
use mollusk_svm::{
    result::{Check, InstructionResult},
    Mollusk,
};
use solana_account::Account;
use solana_instruction::{error::InstructionError, AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const OWNER: Pubkey = Pubkey::new_from_array([7; 32]);
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);
const PAYER: Pubkey = Pubkey::new_from_array([9; 32]);
const SYSTEM: Pubkey = Pubkey::new_from_array([0; 32]);
const DISCRIMINATOR: [u8; 8] = [47, 253, 130, 2, 67, 165, 139, 141];
const BALANCE: u64 = 100_000_000_000;
const WINDOW: usize = 10_240;

// Define the wire schema and values without importing production descriptors,
// seed inputs, codecs or the exercise's implementation.
fn schema() -> Schema {
    Schema::Struct(vec![
        Schema::Pubkey,
        Schema::Unsigned(W8),
        Schema::List(Box::new(Schema::Struct(vec![
            Schema::Str,
            Schema::List(Box::new(Schema::Unsigned(W4))),
        ]))),
        Schema::Map(KeySchema::Pubkey, Box::new(Schema::Unsigned(W8))),
        Schema::Map(KeySchema::Unsigned(W2), Box::new(Schema::Str)),
        Schema::Str,
        Schema::List(Box::new(Schema::Unsigned(W1))),
        Schema::Option(Box::new(Schema::Str)),
        Schema::Struct(vec![Schema::Bool, Schema::Unsigned(W2)]),
    ])
}

fn record(name: &str, tags: &[u32]) -> Value {
    Value::Struct(vec![
        Value::Str(name.into()),
        Value::List(
            tags.iter()
                .map(|tag| Value::Unsigned((*tag).into()))
                .collect(),
        ),
    ])
}

fn value(finished: bool) -> Value {
    Value::Struct(vec![
        Value::Pubkey(PAYER.to_bytes()),
        Value::Unsigned(u128::from(finished)),
        Value::List(if finished {
            vec![]
        } else {
            vec![record("one", &[1]), record("two", &[2, 3])]
        }),
        Value::Map(Default::default()),
        Value::Map(if finished {
            Default::default()
        } else {
            [(Key::Unsigned(1), Value::Str("first".into()))].into()
        }),
        Value::Str(if finished {
            "grew after physical shrink".into()
        } else {
            "ready".into()
        }),
        Value::List(vec![]),
        Value::Option(None),
        Value::Struct(vec![
            Value::Bool(true),
            Value::Unsigned(if finished { 200 } else { 100 }),
        ]),
    ])
}

fn bytes(finished: bool) -> Vec<u8> {
    let schema = schema();
    let value = value(finished);
    let len = 12 + schema.encode(&value).unwrap().len();
    schema.encode_account(DISCRIMINATOR, &value, len).unwrap()
}

struct Case {
    mollusk: Mollusk,
    accounts: Vec<(Pubkey, Account)>,
}

impl Case {
    fn new(profile: u8, artifact: &str) -> Self {
        let mut mollusk = Mollusk::default();
        mollusk.feature_set.virtual_address_space_adjustments = profile != 0;
        mollusk.feature_set.account_data_direct_mapping = profile == 2;
        mollusk.program_cache = mollusk_svm::program::ProgramCache::new(
            &mollusk.feature_set,
            &mollusk.compute_budget,
            false,
        );
        mollusk.add_program(&OWNER, artifact);
        // Distinct from the cluster default: rent must come from the sysvar.
        mollusk.sysvars.rent.lamports_per_byte = 19;
        let accounts = vec![
            (STATE, Account::default()),
            (PAYER, Account::new(BALANCE, 0, &SYSTEM)),
            mollusk_svm::program::keyed_account_for_system_program(),
        ];
        Self { mollusk, accounts }
    }

    fn seeded(profile: u8, artifact: &str) -> Self {
        let mut case = Self::new(profile, artifact);
        let data = bytes(false);
        case.accounts[0].1 = Account {
            lamports: case.mollusk.sysvars.rent.minimum_balance(data.len()),
            data,
            owner: OWNER,
            executable: false,
            rent_epoch: 0,
        };
        case
    }

    fn instruction(&self, opcode: u8) -> Instruction {
        Instruction {
            program_id: OWNER,
            accounts: vec![
                if opcode == 1 {
                    AccountMeta::new_readonly(STATE, false)
                } else {
                    AccountMeta::new(STATE, opcode == 0)
                },
                if opcode == 1 {
                    AccountMeta::new_readonly(PAYER, false)
                } else {
                    AccountMeta::new(PAYER, true)
                },
                AccountMeta::new_readonly(SYSTEM, false),
            ],
            data: vec![opcode],
        }
    }

    fn retaining_tour(&mut self) -> Instruction {
        self.accounts.push((
            OWNER,
            mollusk_svm::program::create_program_account_loader_v2(&[]),
        ));
        let mut ix = self.instruction(60);
        ix.data.push(0); // Max, then commit retained growth before the read CPI.
        ix.accounts.push(AccountMeta::new_readonly(OWNER, false));
        ix
    }

    fn run(&self, ix: &Instruction, error: Option<Error>, cpis: usize) -> InstructionResult {
        let result = self.mollusk.process_and_validate_instruction(
            ix,
            &self.accounts,
            &[
                match error {
                    Some(error) => Check::err(error.into()),
                    None => Check::success(),
                },
                Check::inner_instruction_count(cpis),
            ],
        );
        if error.is_some() {
            assert_eq!(result.resulting_accounts, self.accounts, "atomic rollback");
        }
        result
    }

    fn assert_model(&self, result: &InstructionResult, finished: bool) {
        let state = result.get_account(&STATE).unwrap();
        assert_eq!(state.owner, OWNER);
        assert_eq!(state.data, bytes(finished), "complete canonical allocation");
        assert_eq!(
            schema()
                .decode_account(DISCRIMINATOR, &state.data)
                .unwrap()
                .value,
            value(finished)
        );
    }
}

#[test]
fn each_tour_opcode_matches_the_model_in_all_memory_profiles() {
    for profile in 0..3 {
        let mut case = Case::new(profile, "blubber_demo");
        for opcode in [0, 1, 2, 1] {
            let before = case.accounts.clone();
            let result = case.run(&case.instruction(opcode), None, usize::from(opcode == 0));
            case.assert_model(&result, opcode == 2 || before[0].1.data == bytes(true));
            if opcode == 0 {
                let minimum = case
                    .mollusk
                    .sysvars
                    .rent
                    .minimum_balance(bytes(false).len());
                assert_eq!(result.get_account(&STATE).unwrap().lamports, minimum);
                assert_eq!(
                    result.get_account(&PAYER).unwrap().lamports,
                    BALANCE - minimum
                );
                let cpi = &result.inner_instructions[0].instruction;
                assert_eq!(&cpi.data[..4], &0u32.to_le_bytes());
                assert_eq!(&cpi.data[4..12], &minimum.to_le_bytes());
                assert_eq!(
                    &cpi.data[12..20],
                    &(bytes(false).len() as u64).to_le_bytes()
                );
                assert_eq!(&cpi.data[20..], OWNER.as_ref());
            } else {
                for (address, old) in &before {
                    assert_eq!(result.get_account(address).unwrap().lamports, old.lamports);
                }
                if opcode == 1 {
                    assert_eq!(result.resulting_accounts, before, "read-only opcode");
                }
            }
            case.accounts = result.resulting_accounts;
        }
    }
}

#[test]
fn create_read_edit_read_share_one_atomic_transaction() {
    for profile in 0..3 {
        let case = Case::new(profile, "blubber_demo");
        let instructions = [0, 1, 2, 1].map(|opcode| case.instruction(opcode));
        let result = case.mollusk.process_and_validate_transaction_instructions(
            &instructions,
            &case.accounts,
            &[
                Check::success(),
                Check::account(&STATE)
                    .data(&bytes(true))
                    .owner(&OWNER)
                    .build(),
            ],
            Some(&PAYER),
        );
        assert_eq!(
            result
                .inner_instructions
                .iter()
                .map(Vec::len)
                .collect::<Vec<_>>(),
            [1, 0, 0, 0]
        );
        let rent = case
            .mollusk
            .sysvars
            .rent
            .minimum_balance(bytes(false).len());
        let balance = |address| {
            result
                .resulting_accounts
                .iter()
                .find(|(key, _)| *key == address)
                .unwrap()
                .1
                .lamports
        };
        assert_eq!(balance(STATE), rent);
        assert_eq!(balance(PAYER), BALANCE - rent);
    }
}

#[test]
fn a_later_transaction_error_rolls_back_creation_funding_and_all_edits() {
    let case = Case::new(1, "blubber_demo");
    let mut instructions = [0, 1, 2, 1].map(|opcode| case.instruction(opcode));
    instructions[3].data.push(0);
    let result = case.mollusk.process_and_validate_transaction_instructions(
        &instructions,
        &case.accounts,
        &[Check::err(Error::Program { code: 3u64 << 32 }.into())],
        Some(&PAYER),
    );
    for (address, before) in &case.accounts {
        assert_eq!(
            result
                .resulting_accounts
                .iter()
                .find(|(key, _)| key == address)
                .unwrap()
                .1,
            *before
        );
    }
    assert_eq!(
        result
            .inner_instructions
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        [1, 0, 0, 0]
    );
}

#[test]
fn top_up_funds_retained_tour_growth_with_one_net_transfer() {
    let mut case = Case::seeded(1, "blubber_borrow_resize");
    case.accounts[0].1.lamports = 1;
    let ix = case.retaining_tour();
    let result = case.run(&ix, None, 2); // One System transfer and one read CPI.
    case.assert_model(&result, true);
    let minimum = case
        .mollusk
        .sysvars
        .rent
        .minimum_balance(bytes(false).len() + WINDOW);
    assert_eq!(result.get_account(&STATE).unwrap().lamports, minimum);
    assert_eq!(
        result.get_account(&PAYER).unwrap().lamports,
        BALANCE - (minimum - 1)
    );
    let cpi = &result.inner_instructions[0].instruction;
    assert_eq!(&cpi.data[..4], &2u32.to_le_bytes());
    assert_eq!(&cpi.data[4..], &(minimum - 1).to_le_bytes());
}

#[test]
fn failed_final_funding_rolls_back_the_entire_exercise() {
    let mut case = Case::seeded(1, "blubber_borrow_resize");
    case.accounts[0].1.lamports = 1;
    case.accounts[1].1.lamports = 0;
    let ix = case.retaining_tour();
    case.run(&ix, Some(ErrorKind::RentShortfall.into()), 0);
}

#[test]
#[allow(deprecated)]
fn shrinking_tour_uses_live_rent_and_rolls_back_on_settlement_failure() {
    assert!(bytes(true).len() < bytes(false).len());
    for profile in 0..3 {
        let mut case = Case::seeded(profile, "blubber_demo");
        case.accounts[0].1.lamports = 1;
        case.accounts[1].1.lamports = 0;
        // A shrink cannot rely on the opening balance to establish exemption.
        // Unsupported live rent fails commit, rolling back the complete tour.
        case.mollusk.sysvars.rent.exemption_threshold = 1.5f64.to_le_bytes();
        case.run(
            &case.instruction(2),
            Some(pinocchio::error::ProgramError::UnsupportedSysvar.into()),
            0,
        );
    }
}

#[test]
fn an_explicitly_insufficient_meter_rolls_back_the_complete_instruction() {
    let mut case = Case::seeded(1, "blubber_demo");
    let limit = 1_000;
    case.mollusk.compute_budget.compute_unit_limit = limit;
    let result = case.mollusk.process_and_validate_instruction(
        &case.instruction(2),
        &case.accounts,
        &[
            Check::instruction_err(InstructionError::ProgramFailedToComplete),
            Check::compute_units(limit),
        ],
    );
    assert_eq!(result.resulting_accounts, case.accounts);
}

#[test]
fn application_authority_is_required_before_any_tour_edit() {
    let mut case = Case::seeded(1, "blubber_demo");
    case.accounts[0].1.data[12..44].fill(0x11);
    case.run(
        &case.instruction(2),
        Some(ErrorKind::Unauthorized.into()),
        0,
    );
}

#[test]
fn creation_and_edit_require_their_signatures() {
    for opcode in [0, 2] {
        let case = if opcode == 0 {
            Case::new(1, "blubber_demo")
        } else {
            Case::seeded(1, "blubber_demo")
        };
        let mut ix = case.instruction(opcode);
        ix.accounts[1].is_signer = false;
        case.run(&ix, Some(Error::Program { code: 8u64 << 32 }), 0);
    }
    let case = Case::new(1, "blubber_demo");
    let mut ix = case.instruction(0);
    ix.accounts[0].is_signer = false;
    case.run(&ix, Some(Error::Program { code: 8u64 << 32 }), 0);
}

#[test]
fn readonly_state_wrong_owner_and_corrupt_identity_are_rejected() {
    let case = Case::seeded(1, "blubber_demo");
    let mut ix = case.instruction(2);
    ix.accounts[0].is_writable = false;
    case.run(&ix, Some(ErrorKind::NotWritable.into()), 0);
    for opcode in [1, 2] {
        let mut case = Case::seeded(1, "blubber_demo");
        case.accounts[0].1.owner = SYSTEM;
        case.run(
            &case.instruction(opcode),
            Some(ErrorKind::WrongOwner.into()),
            0,
        );
        case.accounts[0].1.owner = OWNER;
        case.accounts[0].1.data[0] ^= 1;
        case.run(
            &case.instruction(opcode),
            Some(ErrorKind::Discriminator.into()),
            0,
        );
    }
}

#[test]
fn readonly_opcode_fully_validates_even_unvisited_fixed_fields() {
    let mut case = Case::seeded(1, "blubber_demo");
    case.accounts[0].1.data[52] = 2; // Limits.enabled, after authority and counter.
    case.run(&case.instruction(1), Some(ErrorKind::InvalidBool.into()), 0);
}

#[test]
fn occupied_creation_and_insufficient_creation_funds_leave_accounts_unchanged() {
    let case = Case::seeded(1, "blubber_demo");
    case.run(
        &case.instruction(0),
        Some(ErrorKind::AlreadyInitialized.into()),
        0,
    );
    let mut case = Case::new(1, "blubber_demo");
    case.accounts[1].1.lamports = 0;
    case.run(
        &case.instruction(0),
        Some(ErrorKind::RentShortfall.into()),
        0,
    );
}

#[test]
fn malformed_and_unknown_opcodes_have_no_side_effects() {
    let case = Case::seeded(1, "blubber_demo");
    for data in [vec![], vec![1, 0], vec![3], vec![255]] {
        let mut ix = case.instruction(2);
        ix.data = data;
        case.run(&ix, Some(Error::Program { code: 3u64 << 32 }), 0);
    }
    let mut ix = case.instruction(1);
    ix.accounts.pop();
    case.run(&ix, Some(Error::Program { code: 11u64 << 32 }), 0);
}

#[test]
fn commit_cpi_and_reopen_preserve_the_invocation_window_in_every_mode() {
    let modes = if cfg!(feature = "alloc") { 3 } else { 2 };
    for mode in 0..modes {
        for profile in 0..3 {
            let mut case = Case::seeded(
                profile,
                if mode == 2 {
                    "blubber_borrow_resize_alloc"
                } else {
                    "blubber_borrow_resize"
                },
            );
            case.accounts.push((
                OWNER,
                mollusk_svm::program::create_program_account_loader_v2(&[]),
            ));
            let mut ix = case.instruction(60);
            ix.data.push(mode);
            ix.accounts.push(AccountMeta::new_readonly(OWNER, false));
            let result = case.run(&ix, None, 2); // One net System top-up, then the read CPI.
            case.assert_model(&result, true);
            let minimum = case
                .mollusk
                .sysvars
                .rent
                .minimum_balance(bytes(false).len() + WINDOW);
            assert_eq!(result.get_account(&STATE).unwrap().lamports, minimum);
            assert_eq!(
                result.get_account(&PAYER).unwrap().lamports,
                BALANCE - (minimum - case.accounts[0].1.lamports)
            );
            assert_eq!(
                &result.inner_instructions[0].instruction.data[..4],
                &2u32.to_le_bytes()
            );
            assert_eq!(result.inner_instructions[1].instruction.data, [61]);
        }
    }
}
