//! Run the quickstart instructions against the deployed program.

#![forbid(unsafe_code)]

use blubber::{Error, ErrorKind};
use blubber_quickstart::{Queue, QueueRef};
use mollusk_svm::{result::Check, Mollusk};
use pinocchio::{account::MAX_PERMITTED_DATA_INCREASE, error::ProgramError};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program::ID as SYSTEM;

const PROGRAM: Pubkey = Pubkey::new_from_array([21; 32]);
const STATE: Pubkey = Pubkey::new_from_array([22; 32]);
const AUTHORITY: Pubkey = Pubkey::new_from_array([23; 32]);
const PAYER: Pubkey = Pubkey::new_from_array([24; 32]);
const OTHER_SIGNER: Pubkey = Pubkey::new_from_array([25; 32]);

fn instruction(authority: Pubkey, value: Option<u64>) -> Instruction {
    let mut data = vec![0];
    if let Some(value) = value {
        data[0] = 1;
        data.extend(value.to_le_bytes());
    }
    Instruction {
        program_id: PROGRAM,
        accounts: vec![
            AccountMeta::new(STATE, value.is_none()),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(PAYER, true),
            AccountMeta::new_readonly(SYSTEM, false),
        ],
        data,
    }
}

// docs:start queue-client-read
fn read_queue(address: Pubkey, account: &Account) -> Option<QueueRef<'_>> {
    if address != STATE || account.owner != PROGRAM {
        return None;
    }
    let queue = Queue::load(&account.data).ok()?;
    (queue.authority().to_bytes() == AUTHORITY.to_bytes()).then_some(queue)
}
// docs:end queue-client-read

#[test]
fn creates_and_pushes_with_exact_lengths_and_rent() {
    let mut mollusk = Mollusk::new(&PROGRAM, "blubber_quickstart");
    mollusk.sysvars.rent.lamports_per_byte = 19;
    let payer_balance = 10_000_000;
    let mut accounts = vec![
        (STATE, Account::default()),
        (AUTHORITY, Account::new(1, 0, &SYSTEM)),
        (PAYER, Account::new(payer_balance, 0, &SYSTEM)),
        mollusk_svm::program::keyed_account_for_system_program(),
    ];
    let values = [4, 9];
    let exact_lengths = [48, 56, 64];
    // 12-byte envelope, 32-byte authority, 4-byte count, then u64 values.
    for (count, value) in [None, Some(values[0]), Some(values[1])]
        .into_iter()
        .enumerate()
    {
        let length = 48 + count * 8;
        assert_eq!(length, exact_lengths[count]);
        let rent = mollusk.sysvars.rent.minimum_balance(length);
        let result = mollusk.process_and_validate_instruction(
            &instruction(AUTHORITY, value),
            &accounts,
            &[
                Check::success(),
                Check::inner_instruction_count(1),
                Check::account(&STATE)
                    .owner(&PROGRAM)
                    .lamports(rent)
                    .build(),
                Check::account(&PAYER)
                    .lamports(payer_balance - rent)
                    .build(),
            ],
        );
        let state = result.get_account(&STATE).unwrap();
        let queue = read_queue(STATE, state).unwrap();
        assert_eq!(queue.authority().to_bytes(), AUTHORITY.to_bytes());
        assert_eq!(queue.used_len() as usize, length);
        assert_eq!(queue.storage_len() as usize, length);
        assert_eq!(
            queue
                .values()
                .try_iter()
                .collect::<blubber::Result<Vec<_>>>()
                .unwrap(),
            values[..count]
        );
        accounts = result.resulting_accounts;
    }

    let state = accounts.iter().find(|(key, _)| *key == STATE).unwrap();
    assert!(read_queue(OTHER_SIGNER, &state.1).is_none());
    let mut wrong_owner = state.1.clone();
    wrong_owner.owner = SYSTEM;
    assert!(read_queue(STATE, &wrong_owner).is_none());

    accounts.push((OTHER_SIGNER, Account::new(1, 0, &SYSTEM)));
    let rejected = mollusk.process_and_validate_instruction(
        &instruction(OTHER_SIGNER, Some(11)),
        &accounts,
        &[
            Check::err(Error::from(ErrorKind::Unauthorized).into()),
            Check::inner_instruction_count(0),
        ],
    );
    assert_eq!(rejected.resulting_accounts, accounts);

    accounts
        .iter_mut()
        .find(|(key, _)| *key == PAYER)
        .unwrap()
        .1
        .lamports = 0;
    let unfunded = mollusk.process_and_validate_instruction(
        &instruction(AUTHORITY, Some(11)),
        &accounts,
        &[
            Check::err(Error::from(ErrorKind::RentShortfall).into()),
            Check::inner_instruction_count(0),
        ],
    );
    assert_eq!(unfunded.resulting_accounts, accounts);
}

fn migrate_instruction() -> Instruction {
    let mut instruction = instruction(AUTHORITY, None);
    instruction.accounts[0].is_signer = false;
    instruction.data = vec![2];
    instruction
}

fn borsh_queue(values: &[u64]) -> Vec<u8> {
    const IDENTITY: [u8; 8] = [0xcc, 0xa7, 0x06, 0xf7, 0x14, 0x21, 0x02, 0xbc];
    borsh::to_vec(&(IDENTITY, AUTHORITY.to_bytes(), values)).unwrap()
}

fn migration_accounts(mollusk: &Mollusk, values: &[u64]) -> Vec<(Pubkey, Account)> {
    let data = borsh_queue(values);
    let rent = mollusk.sysvars.rent.minimum_balance(data.len());
    vec![
        (STATE, Account::new_with_data(rent, data, &PROGRAM)),
        (AUTHORITY, Account::new(1, 0, &SYSTEM)),
        (PAYER, Account::new(10_000_000, 0, &SYSTEM)),
        mollusk_svm::program::keyed_account_for_system_program(),
    ]
}

#[test]
fn migrates_borsh_queues_in_place() {
    let mut mollusk = Mollusk::new(&PROGRAM, "blubber_quickstart");
    mollusk.sysvars.rent.lamports_per_byte = 19;
    for count in [0, 3, 1280] {
        let values: Vec<u64> = (0..count).map(|index| u64::MAX - index * 257).collect();
        let accounts = migration_accounts(&mollusk, &values);
        let old_state = &accounts[0].1;
        let new_len = 48 + values.len() * 8;
        let rent = mollusk.sysvars.rent.minimum_balance(new_len);
        let top_up = rent - old_state.lamports;
        assert_eq!(new_len - old_state.data.len(), 4);
        assert!(new_len - old_state.data.len() <= MAX_PERMITTED_DATA_INCREASE);
        // This case was already larger than the growth window before migration.
        if count == 1280 {
            assert!(old_state.data.len() > MAX_PERMITTED_DATA_INCREASE);
        }
        let result = mollusk.process_and_validate_instruction(
            &migrate_instruction(),
            &accounts,
            &[
                Check::success(),
                Check::inner_instruction_count(1),
                Check::account(&STATE)
                    .owner(&PROGRAM)
                    .lamports(rent)
                    .build(),
                Check::account(&PAYER)
                    .lamports(accounts[2].1.lamports - top_up)
                    .build(),
            ],
        );
        eprintln!(
            "migrate {count} values ({new_len} bytes): {} CU",
            result.compute_units_consumed
        );
        let state = result.get_account(&STATE).unwrap();
        assert_eq!(&state.data[..8], &Queue::DISCRIMINATOR);
        assert_eq!(state.data.len(), new_len);
        let queue = Queue::load(&state.data).unwrap();
        assert_eq!(queue.authority().to_bytes(), AUTHORITY.to_bytes());
        assert_eq!(queue.used_len() as usize, new_len);
        assert_eq!(
            queue
                .values()
                .try_iter()
                .collect::<blubber::Result<Vec<_>>>()
                .unwrap(),
            values
        );
    }
}

fn rejected_migration(
    mollusk: &Mollusk,
    instruction: &Instruction,
    accounts: &[(Pubkey, Account)],
    error: impl Into<ProgramError>,
) {
    let result = mollusk.process_and_validate_instruction(
        instruction,
        accounts,
        &[Check::err(error.into()), Check::inner_instruction_count(0)],
    );
    assert_eq!(result.resulting_accounts, accounts);
}

#[test]
fn migration_rejects_wrong_authority_and_owner() {
    let mollusk = Mollusk::new(&PROGRAM, "blubber_quickstart");
    let mut accounts = migration_accounts(&mollusk, &[9]);
    let mut instruction = migrate_instruction();
    instruction.accounts[1].is_signer = false;
    rejected_migration(
        &mollusk,
        &instruction,
        &accounts,
        ProgramError::MissingRequiredSignature,
    );

    instruction.accounts[1] = AccountMeta::new_readonly(OTHER_SIGNER, true);
    accounts.push((OTHER_SIGNER, Account::new(1, 0, &SYSTEM)));
    rejected_migration(
        &mollusk,
        &instruction,
        &accounts,
        Error::from(ErrorKind::Unauthorized),
    );

    accounts[0].1.owner = OTHER_SIGNER;
    rejected_migration(
        &mollusk,
        &migrate_instruction(),
        &accounts,
        Error::from(ErrorKind::WrongOwner),
    );
}

#[test]
fn migration_rejects_invalid_borsh() {
    let mollusk = Mollusk::new(&PROGRAM, "blubber_quickstart");
    let instruction = migrate_instruction();
    let mut accounts = migration_accounts(&mollusk, &[9]);
    let valid_body = accounts[0].1.data.clone();
    accounts[0].1.data.truncate(43);
    rejected_migration(
        &mollusk,
        &instruction,
        &accounts,
        Error::from(ErrorKind::Truncated),
    );

    accounts[0].1.data = valid_body.clone();
    accounts[0].1.data[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
    rejected_migration(
        &mollusk,
        &instruction,
        &accounts,
        Error::from(ErrorKind::ArithmeticOverflow),
    );

    accounts[0].1.data = valid_body;
    accounts[0].1.data.pop();
    rejected_migration(
        &mollusk,
        &instruction,
        &accounts,
        ProgramError::InvalidAccountData,
    );
    accounts[0].1.data.extend([0, 0]);
    rejected_migration(
        &mollusk,
        &instruction,
        &accounts,
        ProgramError::InvalidAccountData,
    );
}

#[test]
fn migration_rejects_a_shortfall_and_a_second_migration() {
    let mollusk = Mollusk::new(&PROGRAM, "blubber_quickstart");
    let instruction = migrate_instruction();
    let mut accounts = migration_accounts(&mollusk, &[4, 9]);
    let state = &accounts[0].1;
    let top_up = mollusk.sysvars.rent.minimum_balance(state.data.len() + 4) - state.lamports;
    accounts[2].1.lamports = top_up - 1;
    rejected_migration(
        &mollusk,
        &instruction,
        &accounts,
        Error::from(ErrorKind::RentShortfall),
    );

    accounts[2].1.lamports = 10_000_000;
    let result =
        mollusk.process_and_validate_instruction(&instruction, &accounts, &[Check::success()]);
    rejected_migration(
        &mollusk,
        &instruction,
        &result.resulting_accounts,
        Error::from(ErrorKind::Discriminator),
    );
}
