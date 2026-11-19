//! Checked SDK policy controls on fresh feature-off/on SBF artifacts.
#![forbid(unsafe_code)]
use blubber::{SliceStore, View};
use blubber_cu_fixture::policy::{PolicyState, PolicyStateInit};
use mollusk_svm::{result::ProgramResult, Mollusk};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

#[test]
fn checked_policies_and_root_verification_on_all_mappings() {
    let Some(directory) = std::env::var_os("BLUBBER_POLICY_DIR") else {
        return;
    };
    let ops: u8 = std::env::var("BLUBBER_POLICY_OPS")
        .unwrap()
        .parse()
        .unwrap();
    assert!(matches!(ops, 9 | 21));
    let eager = std::env::var("BLUBBER_POLICY_EAGER").unwrap() == "1";
    let owner = Pubkey::new_from_array([7; 32]);
    let key = Pubkey::new_from_array([8; 32]);
    let artifact = std::path::PathBuf::from(directory).join("blubber_cu_fixture");
    assert!(artifact.with_extension("so").is_file());
    for profile in 0..3 {
        let mut vm = Mollusk::default();
        vm.feature_set.virtual_address_space_adjustments = profile != 0;
        vm.feature_set.account_data_direct_mapping = profile == 2;
        vm.program_cache =
            mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
        vm.add_program(&owner, artifact.to_str().unwrap());
        for n in [0, 1, 10, 100, 1000] {
            let flags = vec![true; n];
            let init = PolicyStateInit {
                flag: true,
                text: "ok",
                entries: &[(1, true), (2, false)],
                flags: &flags,
            };
            let mut bytes = vec![0; PolicyState::encoded_len_of(&init).unwrap() as usize];
            PolicyState::initialize(&mut SliceStore::new(&mut bytes), init).unwrap();
            for damage in 0..5 {
                if damage == 1 && n == 0 {
                    continue;
                }
                let mut data = bytes.clone();
                let map = View::load::<PolicyState>(&data, PolicyState::DISCRIMINATOR)
                    .unwrap()
                    .field(2)
                    .unwrap()
                    .base() as usize;
                match damage {
                    1 => *data.last_mut().unwrap() = 2,
                    2 => data[map + 6] = 1,
                    3 => data[21] = 0xff, // First text byte after the fixed prefix and ends.
                    4 => data[12] = 2,
                    _ => {}
                }
                for op in 0..ops {
                    if ops == 21 && (6..9).contains(&op) {
                        continue;
                    }
                    let accounts = [(
                        key,
                        Account {
                            lamports: 1_000_000_000,
                            data: data.clone(),
                            owner,
                            ..Account::default()
                        },
                    )];
                    let instruction = Instruction {
                        program_id: owner,
                        accounts: vec![AccountMeta::new(key, false)],
                        data: vec![33, op],
                    };
                    let result = vm.process_instruction(&instruction, &accounts);
                    let lazy_untouched = matches!(op, 9..=13 | 17);
                    let lazy_touched =
                        (matches!(op, 4 | 14) && damage != 4) || (op == 15 && damage != 3);
                    let success = damage == 0 || (!eager && (lazy_untouched || lazy_touched));
                    let expected = if success {
                        ProgramResult::Success
                    } else {
                        let kind = match damage {
                            1 | 4 => blubber::ErrorKind::InvalidBool,
                            2 => blubber::ErrorKind::DuplicateKey,
                            3 => blubber::ErrorKind::InvalidUtf8,
                            _ => unreachable!("valid roots must succeed"),
                        };
                        let fault: blubber::Error = kind.into();
                        ProgramResult::Failure(fault.into())
                    };
                    assert_eq!(
                        result.program_result, expected,
                        "profile={profile} n={n} damage={damage} op={op}"
                    );
                    let mut expected_data = data.clone();
                    if op == 17 && success {
                        expected_data[12] = 1;
                    }
                    assert_eq!(result.get_account(&key).unwrap().data, expected_data);
                    if profile == 0 && damage == 0 {
                        eprintln!(
                            "policy-cU eager={eager} n={n} op={op} total={}",
                            result.compute_units_consumed
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn authenticated_ingress_rejects_privilege_identity_and_envelope_faults() {
    let Some(directory) = std::env::var_os("BLUBBER_POLICY_DIR") else {
        return;
    };
    if std::env::var("BLUBBER_POLICY_OPS").unwrap() != "21" {
        return;
    }
    let owner = Pubkey::new_from_array([7; 32]);
    let key = Pubkey::new_from_array([8; 32]);
    let artifact = std::path::PathBuf::from(directory).join("blubber_cu_fixture");
    let mut bytes = vec![0; 64];
    PolicyState::initialize(
        &mut SliceStore::new(&mut bytes),
        PolicyStateInit {
            flag: true,
            text: "ok",
            entries: &[(1, true), (2, false)],
            flags: &[true; 3],
        },
    )
    .unwrap();
    let used = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    bytes.truncate(used);
    for profile in 0..3 {
        let mut vm = Mollusk::default();
        vm.feature_set.virtual_address_space_adjustments = profile != 0;
        vm.feature_set.account_data_direct_mapping = profile == 2;
        vm.program_cache =
            mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
        vm.add_program(&owner, artifact.to_str().unwrap());
        for case in 0..7 {
            for op in [0, 1, 2, 3, 4, 5, 9, 10, 11, 12, 13, 14, 15, 16, 17] {
                let mut account = Account {
                    lamports: 1_000_000_000,
                    data: bytes.clone(),
                    owner,
                    ..Account::default()
                };
                let mut writable = true;
                let kind = match case {
                    0 => {
                        // Authentication precedes a malformed envelope.
                        account.owner = key;
                        account.data.clear();
                        blubber::ErrorKind::WrongOwner
                    }
                    1 => {
                        account.executable = true;
                        blubber::ErrorKind::Unauthorized
                    }
                    2 => {
                        writable = false;
                        blubber::ErrorKind::NotWritable
                    }
                    3 => {
                        account.data[0] ^= 1;
                        blubber::ErrorKind::Discriminator
                    }
                    4 => {
                        account.data[..8].fill(0);
                        blubber::ErrorKind::Discriminator
                    }
                    5 => {
                        account.data[8..12].copy_from_slice(&12u32.to_le_bytes());
                        blubber::ErrorKind::UsedLength
                    }
                    6 => {
                        account.data[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
                        blubber::ErrorKind::UsedLength
                    }
                    _ => unreachable!(),
                };
                let original = [(key, account)];
                let instruction = Instruction {
                    program_id: owner,
                    accounts: vec![if writable {
                        AccountMeta::new(key, false)
                    } else {
                        AccountMeta::new_readonly(key, false)
                    }],
                    data: vec![33, op],
                };
                let result = vm.process_instruction(&instruction, &original);
                let is_edit = matches!(op, 1 | 3 | 10 | 11 | 13 | 17);
                let expected = if case == 2 && !is_edit {
                    ProgramResult::Success
                } else {
                    let fault: blubber::Error = kind.into();
                    ProgramResult::Failure(fault.into())
                };
                assert_eq!(
                    result.program_result, expected,
                    "profile={profile} case={case} op={op}"
                );
                assert_eq!(result.resulting_accounts, original);
                assert!(result.inner_instructions.is_empty());
            }
        }
    }
}
