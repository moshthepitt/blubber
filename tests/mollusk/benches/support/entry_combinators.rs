//! Permanent combinator observations with independent full-byte results.
use super::{account, OWNER, STATE};
use blubber_cu_fixture::entry_combinators::{key, Fixed, Indexed};
use blubber_model::{IntWidth, Key, KeySchema, Schema, Value};
use mollusk_svm::{result::Check, Mollusk};
use solana_instruction::{AccountMeta, Instruction};

fn image(
    layout: u8,
    n: u16,
    common: bool,
    op: u8,
    present: bool,
    grow: bool,
    after: bool,
) -> Vec<u8> {
    let selected = key(2 * (n / 2) + if present { 2 } else { 1 }, common).to_bytes();
    let old = if layout == 0 {
        Value::Unsigned(17)
    } else {
        Value::Str("original".into())
    };
    let new = if layout == 0 {
        Value::Unsigned(123)
    } else {
        Value::Str(
            if grow {
                "replacement grows"
            } else {
                "replaced"
            }
            .into(),
        )
    };
    let default = if layout == 0 {
        Value::Unsigned(0)
    } else {
        Value::Str(String::new())
    };
    let mut pairs = (0..n)
        .map(|i| (Key::Pubkey(key(2 * i + 2, common).to_bytes()), old.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    if after {
        if present {
            if matches!(op, 0 | 3 | 4) {
                *pairs.get_mut(&Key::Pubkey(selected)).unwrap() = new;
            }
        } else {
            pairs.insert(
                Key::Pubkey(selected),
                match op {
                    0 | 1 | 4 => new,
                    2 => default,
                    3 => old,
                    _ => unreachable!(),
                },
            );
        }
    }
    let schema = Schema::Struct(vec![
        Schema::Map(
            KeySchema::Pubkey,
            Box::new(if layout == 0 {
                Schema::Unsigned(IntWidth::W8)
            } else {
                Schema::Str
            }),
        ),
        Schema::Str,
    ]);
    let value = Value::Struct(vec![
        Value::Map(pairs),
        Value::Str("retained suffix".into()),
    ]);
    let disc = if layout == 0 {
        Fixed::DISCRIMINATOR
    } else {
        Indexed::DISCRIMINATOR
    };
    schema
        .encode_account(disc, &value, 12 + schema.encode(&value).unwrap().len())
        .unwrap()
}

pub(super) fn run() {
    let vm = Mollusk::new(&OWNER, "blubber_cu_entry_combinators");
    let filter = std::env::var_os("BLUBBER_ENTRY_SCREEN").is_some();
    let mut rows = 0;
    println!("entry,layout,n,present,common31,grow,operation,explicit,combinator,delta");
    for layout in [0, 1] {
        for n in [0, 1, 10, 100] {
            for present in [false, true] {
                if n == 0 && present {
                    continue;
                }
                for common in [false, true] {
                    for grow in [false, true] {
                        if layout == 0 && grow {
                            continue;
                        }
                        if filter && (common || ![0, 10].contains(&n)) {
                            continue;
                        }
                        for op in 0..5 {
                            let before = image(layout, n, common, op, present, grow, false);
                            let expected = image(layout, n, common, op, present, grow, true);
                            let mut costs = [0; 2];
                            for control in [1, 0] {
                                let ix = Instruction {
                                    program_id: OWNER,
                                    accounts: vec![AccountMeta::new(STATE, false)],
                                    data: vec![
                                        90,
                                        layout,
                                        op,
                                        u8::from(present),
                                        n as u8,
                                        (n >> 8) as u8,
                                        u8::from(common),
                                        u8::from(grow),
                                        control,
                                    ],
                                };
                                let result = vm.process_and_validate_instruction(
                                    &ix,
                                    &[(STATE, account(before.clone()))],
                                    &[Check::success(), Check::inner_instruction_count(0)],
                                );
                                assert_eq!(
                                    result.get_account(&STATE).unwrap().data,
                                    expected,
                                    "entry/{layout}/{n}/{present}/{common}/{grow}/{op}/{control}"
                                );
                                costs[usize::from(control == 0)] = result.compute_units_consumed;
                            }
                            println!(
                                "entry,{layout},{n},{present},{common},{grow},{op},{},{},{}",
                                costs[0],
                                costs[1],
                                costs[1] as i64 - costs[0] as i64
                            );
                            rows += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(rows, if filter { 45 } else { 210 });
    println!("entry_oracles,pass,{rows}");
}
