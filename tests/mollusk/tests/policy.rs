//! Fresh point-access costs with independent wire and checksum oracles.
#![forbid(unsafe_code)]
use blubber_cu_fixture::schemas::{KeyWords, Numbers, PublicKeyMap};
use mollusk_svm::{result::ProgramResult, Mollusk};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::fmt::Write;

fn encoded(workload: u8, n: usize) -> (Vec<u8>, u64) {
    let identity = match workload {
        1 => Numbers::DISCRIMINATOR,
        2 => PublicKeyMap::DISCRIMINATOR,
        4 => KeyWords::DISCRIMINATOR,
        _ => unreachable!(),
    };
    let mut bytes = identity.to_vec();
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&(n as u32).to_le_bytes());
    for i in 0..n {
        if workload != 1 {
            let mut key = [42; 32];
            key[..4].copy_from_slice(&(2 * (i + 1) as u32).to_be_bytes());
            bytes.extend_from_slice(&key);
        }
        if workload == 4 {
            bytes.extend_from_slice(&((8 * (i + 1)) as u32).to_le_bytes());
        } else {
            bytes.extend_from_slice(&(i as u64 + 1).to_le_bytes());
        }
    }
    if workload == 4 {
        for i in 0..n {
            bytes.extend_from_slice(format!("{i:08x}").as_bytes());
        }
    }
    let len = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&len.to_le_bytes());
    let checksum = if n == 0 {
        0
    } else if workload == 4 {
        format!("{:08x}", n / 2).bytes().map(u64::from).sum()
    } else {
        n as u64 / 2 + 1
    };
    // Strict host ingress stays a control, separate from the SBF policy under test.
    match workload {
        1 => Numbers::load(&bytes).unwrap().verify().unwrap(),
        2 => PublicKeyMap::load(&bytes).unwrap().verify().unwrap(),
        4 => KeyWords::load(&bytes).unwrap().verify().unwrap(),
        _ => unreachable!(),
    }
    (bytes, checksum)
}

#[test]
fn point_access_and_repeated_loads_match_full_byte_oracles() {
    let Some(directory) = std::env::var_os("BLUBBER_D21_POINT_DIR") else {
        return;
    };
    let eager = std::env::var("BLUBBER_D21_POINT_EAGER").unwrap() == "1";
    let artifact = std::path::PathBuf::from(directory).join("blubber_policy");
    assert!(artifact.with_extension("so").is_file());
    let owner = Pubkey::new_from_array([7; 32]);
    let key = Pubkey::new_from_array([8; 32]);
    let mut csv = String::from("profile,workload,n,policy,repeats,operation,total_cu\n");
    let mut rows = 0;
    for profile in 0..3 {
        let mut vm = Mollusk::default();
        vm.feature_set.virtual_address_space_adjustments = profile != 0;
        vm.feature_set.account_data_direct_mapping = profile == 2;
        vm.program_cache =
            mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
        vm.add_program(&owner, artifact.to_str().unwrap());
        for workload in [1, 2, 4] {
            for n in [0, 1, 10, 100, 1000] {
                let (bytes, checksum) = encoded(workload, n);
                let accounts = [(
                    key,
                    Account {
                        lamports: 1_000_000_000,
                        data: bytes,
                        owner,
                        ..Account::default()
                    },
                )];
                for (policy, name) in [(1, "lazy"), (2, "checked"), (13, "trusted")] {
                    for (offset, repeat) in [(0, 1), (20, 2), (40, 8)] {
                        for op in [0, if workload == 1 { 2 } else { 1 }] {
                            let instruction = Instruction {
                                program_id: owner,
                                accounts: vec![AccountMeta::new(key, false)],
                                data: vec![policy + offset, workload, op, n as u8, (n >> 8) as u8],
                            };
                            let result = vm.process_instruction(&instruction, &accounts);
                            assert_eq!(result.program_result, ProgramResult::Success);
                            assert_eq!(result.resulting_accounts, accounts);
                            let expected = if op == 0 { 0 } else { checksum * repeat };
                            assert_eq!(result.return_data, expected.to_le_bytes());
                            let cu = result.compute_units_consumed;
                            writeln!(csv, "{profile},{workload},{n},{name},{repeat},{op},{cu}")
                                .unwrap();
                            rows += 1;
                            if profile == 0 && n == 1000 && op != 0 && repeat == 1 {
                                eprintln!("T4 W{workload} {name} eager={eager} CU={cu}");
                            }
                            if !eager && policy == 1 && n == 1000 && op != 0 && repeat == 1 {
                                let ceiling = match workload {
                                    1 => 942,
                                    2 => 1682,
                                    4 => 1941,
                                    _ => unreachable!(),
                                };
                                assert!(
                                    cu <= ceiling,
                                    "profile={profile} W{workload}: {cu}>{ceiling}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(rows, 810);
    std::fs::write(std::env::var_os("BLUBBER_D21_POINT_OUT").unwrap(), csv).unwrap();
}
