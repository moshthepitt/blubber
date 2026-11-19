//! Release CU matrix with complete independent content and rent assertions.
#![forbid(unsafe_code)]
#[allow(dead_code)]
mod support;
use blubber_model::{
    IntWidth::{W4, W8},
    Key, KeySchema, Schema, Value,
};
use solana_account::Account;
use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;
use support::*;

fn schema() -> Schema {
    Schema::Struct(vec![
        Schema::Pubkey,
        Schema::Unsigned(W8),
        Schema::List(Box::new(Schema::Unsigned(W8))),
        Schema::Map(KeySchema::Unsigned(W4), Box::new(Schema::Unsigned(W4))),
    ])
}
fn value(kind: u8, n: u32, counter: u64) -> Value {
    Value::Struct(vec![
        Value::Pubkey(PAYER.to_bytes()),
        Value::Unsigned(counter.into()),
        Value::List(if kind == 0 {
            (0..n).map(|i| Value::Unsigned(i.into())).collect()
        } else {
            vec![]
        }),
        Value::Map(if kind == 1 {
            (0..n)
                .map(|i| (Key::Unsigned(i.into()), Value::Unsigned(i.into())))
                .collect()
        } else {
            Default::default()
        }),
    ])
}
fn disc(name: &str) -> [u8; 8] {
    // Series uses D21; stock Anchor baselines retain their own discriminators.
    // Each value was independently computed and pinned in this fixture.
    match name {
        "Series" => [32, 8, 246, 248, 39, 226, 61, 246],
        "Serialized" => [140, 237, 163, 207, 5, 129, 236, 9],
        "Baseline" => [253, 82, 53, 107, 104, 82, 37, 77],
        _ => unreachable!(),
    }
}
fn baseline(kind: u8, n: u32, counter: u64, len: usize) -> Vec<u8> {
    let mut out = disc(if kind == 0 { "Baseline" } else { "Serialized" }).to_vec();
    out.extend(PAYER.to_bytes());
    out.extend(counter.to_le_bytes());
    out.extend(n.to_le_bytes());
    for i in 0..n {
        if kind == 0 {
            out.extend(u64::from(i).to_le_bytes());
        } else {
            out.extend(i.to_le_bytes());
            out.extend(i.to_le_bytes());
        }
    }
    out.resize(len, 0);
    out
}
fn metas(write: bool) -> Vec<AccountMeta> {
    if write {
        vec![
            writable(),
            AccountMeta::new(PAYER, true),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ]
    } else {
        read_metas()
    }
}
fn args(kind: u8, n: u32, operation: Option<u8>) -> Vec<u8> {
    let mut bytes = vec![kind];
    bytes.extend(n.to_le_bytes());
    bytes.extend(operation);
    bytes
}
fn assert_contents(c: &Case, native: bool, kind: u8, n: u32, counter: u64) {
    if native {
        assert_eq!(
            schema()
                .decode_account(disc("Series"), &c.state().data)
                .unwrap()
                .value,
            value(kind, n, counter)
        );
        let encoded = schema()
            .encode_account(
                disc("Series"),
                &value(kind, n, counter),
                c.state().data.len(),
            )
            .unwrap();
        assert_eq!(encoded, c.state().data);
    } else {
        let expected = baseline(kind, n, counter, c.state().data.len());
        assert_eq!(c.state().data, expected);
    }
}

#[test]
fn release_collections_all_profiles() {
    let mut rows =
        vec!["profile,layout,n,operation,implementation,whole_cu,storage,cpis".to_owned()];
    for profile in 0..3 {
        for kind in 0..2 {
            for n in [0, 10, 100, 1000] {
                for native in [true, false] {
                    for operation in 0..8 {
                        if !native && (operation == 4 || operation == 5) {
                            continue;
                        }
                        let mut c = Case::new(profile, false);
                        c.accounts
                            .push(mollusk_svm::program::keyed_account_for_system_program());
                        let len = 128 + 8 * n as usize;
                        c.state_mut().data = if native {
                            schema()
                                .encode_account(disc("Series"), &value(kind, n, 7), len)
                                .unwrap()
                        } else {
                            baseline(kind, n, 7, len)
                        };
                        // Resize alone uses prepaid rent; funding starts at exact rent.
                        let rent_len = if operation == 4 { len + 64 } else { len };
                        c.state_mut().lamports = c.svm.sysvars.rent.minimum_balance(rent_len);
                        let before_rent = c.state().lamports;
                        let before_payer = c.accounts[1].1.lamports;
                        let (opcode, data, accounts) = if operation == 7 {
                            (
                                if native {
                                    49
                                } else if kind == 0 {
                                    50
                                } else {
                                    51
                                },
                                args(kind, n, None),
                                metas(false),
                            )
                        } else if operation == 6 {
                            *c.state_mut() = Account::default();
                            let mut data = (len as u32).to_le_bytes().to_vec();
                            data.extend(args(kind, n, None));
                            (
                                if native {
                                    44
                                } else if kind == 0 {
                                    48
                                } else {
                                    45
                                },
                                data,
                                vec![
                                    AccountMeta::new(PAYER, true),
                                    AccountMeta::new(STATE, true),
                                    AccountMeta::new_readonly(Pubkey::default(), false),
                                ],
                            )
                        } else if operation == 0 {
                            (
                                if native {
                                    40
                                } else if kind == 0 {
                                    46
                                } else {
                                    42
                                },
                                args(kind, n, None),
                                metas(false),
                            )
                        } else {
                            let op = match operation {
                                1 => 0,
                                2 => 1,
                                3 => 2,
                                4 => 3,
                                5 => 4,
                                _ => unreachable!(),
                            };
                            (
                                if native {
                                    41
                                } else if kind == 0 {
                                    47
                                } else {
                                    43
                                },
                                args(kind, n, Some(op)),
                                metas(true),
                            )
                        };
                        let cu = c.run(opcode, &data, accounts, None);
                        let new_n = n + u32::from(operation == 2);
                        assert_contents(
                            &c,
                            native,
                            kind,
                            new_n,
                            if operation == 1 { 11 } else { 7 },
                        );
                        let new_len = if operation == 4 || operation == 5 {
                            len + 64
                        } else {
                            len
                        };
                        assert_eq!(c.state().data.len(), new_len);
                        let rent = c.svm.sysvars.rent.minimum_balance(new_len);
                        let deficit = if operation == 6 {
                            rent
                        } else {
                            rent.saturating_sub(before_rent)
                        };
                        assert_eq!(
                            c.state().lamports,
                            if operation == 6 {
                                rent
                            } else {
                                before_rent + deficit
                            }
                        );
                        assert_eq!(
                            c.accounts
                                .iter()
                                .find(|(key, _)| *key == PAYER)
                                .unwrap()
                                .1
                                .lamports,
                            before_payer - deficit
                        );
                        assert_eq!(c.last_cpis, usize::from(operation == 6 || deficit > 0));
                        rows.push(format!(
                            "{profile},{},{n},{},{},{cu},{new_len},{}",
                            if kind == 0 { "list" } else { "map" },
                            [
                                "read", "scalar", "edit", "commit", "resize", "funding", "init",
                                "open"
                            ][operation],
                            if native {
                                "native"
                            } else if kind == 0 {
                                "slab"
                            } else {
                                "serialized_map"
                            },
                            c.last_cpis
                        ));
                    }
                }
            }
        }
    }
    let path = std::env::var("BLUBBER_ANCHOR_COMPAT_ROWS").expect("run bench lane");
    std::fs::write(path, rows.join("\n") + "\n").unwrap();
}
