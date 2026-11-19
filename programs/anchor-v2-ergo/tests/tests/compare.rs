#![forbid(unsafe_code)]
use blubber_model::{IntWidth::W4, IntWidth::W8, Key, KeySchema, Schema, Value};
use mollusk_svm::Mollusk;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::str::FromStr;
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);
const PAYER: Pubkey = Pubkey::new_from_array([9; 32]);
fn owner() -> Pubkey {
    Pubkey::from_str("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u").unwrap()
}
fn expected(account: &Account, kind: &str, list: &[u64], entries: &[(u32, u32)]) {
    if account.data.is_empty() {
        return;
    }
    if kind.starts_with("native_") {
        let schema = Schema::Struct(vec![
            Schema::Pubkey,
            Schema::List(Box::new(Schema::Unsigned(W8))),
            Schema::Map(KeySchema::Unsigned(W4), Box::new(Schema::Unsigned(W4))),
        ]);
        let disc = [83, 246, 84, 154, 133, 213, 17, 211];
        assert_eq!(
            schema.decode_account(disc, &account.data).unwrap().value,
            Value::Struct(vec![
                Value::Pubkey(PAYER.to_bytes()),
                Value::List(list.iter().map(|v| Value::Unsigned((*v).into())).collect()),
                Value::Map(
                    entries
                        .iter()
                        .map(|(k, v)| (Key::Unsigned((*k).into()), Value::Unsigned((*v).into())))
                        .collect()
                )
            ])
        );
    } else if kind == "slab" {
        assert_eq!(&account.data[8..40], PAYER.as_ref());
        assert_eq!(
            u64::from_le_bytes(account.data[40..48].try_into().unwrap()),
            list.len() as u64
        );
        assert_eq!(
            u32::from_le_bytes(account.data[48..52].try_into().unwrap()),
            (list.len() + entries.len()) as u32
        );
        let payload: Vec<u8> = list
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .chain(
                entries
                    .iter()
                    .flat_map(|(k, v)| k.to_le_bytes().into_iter().chain(v.to_le_bytes())),
            )
            .collect();
        assert_eq!(&account.data[52..52 + payload.len()], &payload);
    } else {
        let mut bytes = account.data[..8].to_vec();
        bytes.extend(PAYER.to_bytes());
        bytes.extend((list.len() as u32).to_le_bytes());
        for value in list {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend((entries.len() as u32).to_le_bytes());
        for (key, value) in entries {
            bytes.extend(key.to_le_bytes());
            bytes.extend(value.to_le_bytes());
        }
        assert_eq!(&account.data[..bytes.len()], &bytes);
    }
}
#[test]
fn equivalent_list_and_map_lifecycle() {
    let base = std::env::var("CARGO_TARGET_DIR").unwrap();
    let arch = std::env::var("BLUBBER_ERGO_ARCH").unwrap();
    let mut rows = vec!["arch,profile,n,implementation,operation,cu".to_string()];
    let selected_profile: u8 = std::env::var("BLUBBER_ERGO_PROFILE")
        .unwrap()
        .parse()
        .unwrap();
    assert!(selected_profile < 3);
    for profile in 0..3 {
        if profile != selected_profile {
            continue;
        }
        for n in [10u32, 100] {
            let kinds = [
                "native_lazy_off",
                "native_checked_off",
                "native_lazy_on",
                "native_checked_on",
                "slab",
                "borsh",
            ];
            let selected = std::env::var("BLUBBER_ERGO_KIND").unwrap();
            assert!(kinds.contains(&selected.as_str()));
            for kind in kinds.into_iter().filter(|kind| *kind == selected) {
                let mut svm = Mollusk::default();
                svm.feature_set.virtual_address_space_adjustments = profile != 0;
                svm.feature_set.account_data_direct_mapping = profile == 2;
                svm.program_cache = mollusk_svm::program::ProgramCache::new(
                    &svm.feature_set,
                    &svm.compute_budget,
                    true,
                );
                svm.add_program(
                    &owner(),
                    &format!(
                        "{base}/ergo-{kind}-{arch}/blubber_ergo_{}",
                        if kind.starts_with("native_") {
                            "native"
                        } else {
                            kind
                        }
                    ),
                );
                let mut accounts = vec![
                    (STATE, Account::default()),
                    (PAYER, Account::new(100_000_000, 0, &Pubkey::default())),
                    mollusk_svm::program::keyed_account_for_system_program(),
                ];
                let mut list: Vec<u64> = (0..n).map(u64::from).collect();
                let mut entries: Vec<(u32, u32)> = (0..n).map(|v| (v, v)).collect();
                for opcode in 0..6 {
                    let (op, args) = match opcode {
                        0 => ("init", n.to_le_bytes().to_vec()),
                        1 => ("push", 777u64.to_le_bytes().to_vec()),
                        2 => (
                            "insert",
                            n.to_le_bytes()
                                .into_iter()
                                .chain(888u32.to_le_bytes())
                                .collect(),
                        ),
                        3 => ("remove_list", vec![0, 0, 0, 0, 0]),
                        4 => ("remove_map", vec![1, 0, 0, 0, 0]),
                        5 => ("close", vec![]),
                        _ => unreachable!(),
                    };
                    let disc = if opcode >= 4 { opcode - 1 } else { opcode };
                    let mut data = vec![disc];
                    data.extend(args);
                    let metas = if opcode == 0 {
                        vec![
                            AccountMeta::new(PAYER, true),
                            AccountMeta::new(STATE, true),
                            AccountMeta::new_readonly(Pubkey::default(), false),
                        ]
                    } else {
                        vec![
                            AccountMeta::new(STATE, false),
                            AccountMeta::new(PAYER, true),
                            AccountMeta::new_readonly(Pubkey::default(), false),
                        ]
                    };
                    let ix = Instruction {
                        program_id: owner(),
                        accounts: metas,
                        data,
                    };
                    let result = svm.process_instruction(&ix, &accounts);
                    assert_eq!(
                        result.raw_result,
                        Ok(()),
                        "{arch}/{profile}/{n}/{kind}/{op}"
                    );
                    rows.push(format!(
                        "{arch},{profile},{n},{kind},{op},{}",
                        result.compute_units_consumed
                    ));
                    accounts = result.resulting_accounts;
                    let account = &accounts.iter().find(|(k, _)| *k == STATE).unwrap().1;
                    match opcode {
                        1 => list.push(777),
                        2 => entries.push((n, 888)),
                        3 => {
                            list.remove(0);
                        }
                        4 => {
                            entries.remove(0);
                        }
                        _ => (),
                    }
                    if opcode == 5 {
                        assert_eq!(account.lamports, 0);
                        if kind.starts_with("native_") {
                            // Native close prevents recreation until runtime purge.
                            assert_eq!(account.data.len(), 12);
                            assert_eq!(&account.data[8..12], &[0; 4]);
                            assert_ne!(&account.data[..8], &[83, 246, 84, 154, 133, 213, 17, 211]);
                            assert_eq!(account.owner, owner());
                        } else {
                            assert!(account.data.is_empty());
                            assert_eq!(account.owner, Pubkey::default());
                        }
                    } else {
                        expected(account, kind, &list, &entries);
                        assert_eq!(account.data.len(), 128 + 16 * n as usize);
                        assert_eq!(
                            account.lamports,
                            svm.sysvars.rent.minimum_balance(account.data.len())
                        );
                    }
                }
            }
        }
    }
    std::fs::write(
        std::env::var("BLUBBER_ERGO_ROWS").unwrap(),
        rows.join("\n") + "\n",
    )
    .unwrap();
}
