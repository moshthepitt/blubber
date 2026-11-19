//! One runtime cohort; independent decoded-content and checksum oracles.
#![forbid(unsafe_code)]
use blubber_cu_fixture::schemas::{KeyWords, Numbers, PublicKeyMap, Words};
use blubber_model::{IntWidth, Key, KeySchema, Schema, Value};
use borsh::BorshDeserialize;
use compare_program as fixture;
use mollusk_svm::{result::Check, Mollusk};
use serde_json::json;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::collections::BTreeMap;
type K = [u8; 32];
#[derive(Clone, Debug, PartialEq, Eq)]
enum Data {
    List(Vec<u64>),
    Map(BTreeMap<K, u64>),
    Words(Vec<String>),
    WordMap(BTreeMap<K, String>),
}
fn key(i: usize) -> K {
    let mut k = [42; 32];
    k[..4].copy_from_slice(&((i as u32 + 1) * 2).to_be_bytes());
    k
}
fn insert(n: usize) -> K {
    let mut k = key(n / 2);
    let v = u32::from_be_bytes(k[..4].try_into().unwrap());
    k[..4].copy_from_slice(&(v - 1).to_be_bytes());
    k
}
fn initial(w: u8, n: usize) -> Data {
    match w {
        1 => Data::List((1..=n as u64).collect()),
        2 => Data::Map((0..n).map(|i| (key(i), i as u64 + 1)).collect()),
        3 => Data::Words((0..n).map(|i| format!("{i:08x}")).collect()),
        4 => Data::WordMap((0..n).map(|i| (key(i), format!("{i:08x}"))).collect()),
        _ => unreachable!(),
    }
}
fn sum(s: &str) -> u64 {
    s.as_bytes().iter().map(|b| u64::from(*b)).sum()
}
fn expected(mut d: Data, op: u8, n: usize) -> (Data, u64) {
    let mut out = 0;
    match &mut d {
        Data::List(v) => match op {
            0 => {}
            1 => out = v.iter().sum(),
            2 => out = v.get(n / 2).copied().unwrap_or(0),
            3 | 6 => v.fill(123),
            4 => v.push(123),
            5 => {
                if n != 0 {
                    v.remove(0);
                }
            }
            _ => unreachable!(),
        },
        Data::Map(v) => match op {
            0 => {}
            1 => out = v.get(&key(n / 2)).copied().unwrap_or(0),
            2 => {
                v.insert(insert(n), 123);
            }
            3 => {
                v.remove(&key(n / 2));
            }
            4 => v.values_mut().for_each(|x| *x = 123),
            5 => out = v.values().sum(),
            _ => unreachable!(),
        },
        Data::Words(v) => match op {
            0 => {}
            1 => out = v.iter().map(|s| sum(s)).sum(),
            2 | 3 => {
                if n > 0 {
                    v[n / 2] = if op == 2 {
                        "sameSIZE"
                    } else {
                        "replacement grows"
                    }
                    .into();
                }
            }
            4 => {
                for s in v {
                    *s = "replacement grows".into();
                }
            }
            5 => {
                for s in v {
                    *s = format!("{s}{s}!");
                }
            }
            _ => unreachable!(),
        },
        Data::WordMap(v) => match op {
            0 => {}
            1 => out = v.get(&key(n / 2)).map(|s| sum(s)).unwrap_or(0),
            2 => {
                v.insert(insert(n), "sameSIZE".into());
            }
            3 => {
                for s in v.values_mut() {
                    *s = format!("{s}{s}!");
                }
            }
            _ => unreachable!(),
        },
    }
    (d, out)
}
fn schema(d: &Data) -> (Schema, Value, [u8; 8]) {
    let (inner, v, disc) = match d {
        Data::List(v) => (
            Schema::List(Box::new(Schema::Unsigned(IntWidth::W8))),
            Value::List(v.iter().map(|x| Value::Unsigned((*x).into())).collect()),
            Numbers::DISCRIMINATOR,
        ),
        Data::Map(v) => (
            Schema::Map(KeySchema::Pubkey, Box::new(Schema::Unsigned(IntWidth::W8))),
            Value::Map(
                v.iter()
                    .map(|(k, x)| (Key::Pubkey(*k), Value::Unsigned((*x).into())))
                    .collect(),
            ),
            PublicKeyMap::DISCRIMINATOR,
        ),
        Data::Words(v) => (
            Schema::List(Box::new(Schema::Str)),
            Value::List(v.iter().cloned().map(Value::Str).collect()),
            Words::DISCRIMINATOR,
        ),
        Data::WordMap(v) => (
            Schema::Map(KeySchema::Pubkey, Box::new(Schema::Str)),
            Value::Map(
                v.iter()
                    .map(|(k, x)| (Key::Pubkey(*k), Value::Str(x.clone())))
                    .collect(),
            ),
            KeyWords::DISCRIMINATOR,
        ),
    };
    (Schema::Struct(vec![inner]), Value::Struct(vec![v]), disc)
}
fn star_encode(d: &Data) -> Vec<u8> {
    match d {
        Data::List(v) => borsh::to_vec(v).unwrap(),
        Data::Map(v) => borsh::to_vec(v).unwrap(),
        Data::Words(v) => {
            let payload: Vec<Vec<u8>> = v.iter().map(|s| borsh::to_vec(s).unwrap()).collect();
            let mut out = Vec::new();
            out.extend_from_slice(
                &(payload.iter().map(Vec::len).sum::<usize>() as u32).to_le_bytes(),
            );
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            let mut offset = 0u32;
            for x in &payload {
                out.extend_from_slice(&offset.to_le_bytes());
                offset += x.len() as u32;
            }
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            for x in payload {
                out.extend_from_slice(&x);
            }
            out
        }
        Data::WordMap(v) => {
            let mut out = Vec::new();
            let payload: Vec<Vec<u8>> = v.values().map(|s| borsh::to_vec(s).unwrap()).collect();
            out.extend_from_slice(
                &(payload.iter().map(Vec::len).sum::<usize>() as u32).to_le_bytes(),
            );
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            let mut offset = 0u32;
            for ((k, _), x) in v.iter().zip(&payload) {
                out.extend_from_slice(&offset.to_le_bytes());
                out.extend_from_slice(k);
                offset += x.len() as u32;
            }
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            for x in payload {
                out.extend_from_slice(&x);
            }
            out
        }
    }
}
fn wincode_encode(d: &Data) -> Vec<u8> {
    let mut out = Vec::new();
    let mut count = |n: usize| out.extend_from_slice(&(n as u64).to_le_bytes());
    match d {
        Data::List(v) => {
            count(v.len());
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        Data::Map(v) => {
            count(v.len());
            for (k, x) in v {
                out.extend_from_slice(k);
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        Data::Words(v) => {
            count(v.len());
            for s in v {
                out.extend_from_slice(&(s.len() as u64).to_le_bytes());
                out.extend_from_slice(s.as_bytes());
            }
        }
        Data::WordMap(v) => {
            count(v.len());
            for (k, s) in v {
                out.extend_from_slice(k);
                out.extend_from_slice(&(s.len() as u64).to_le_bytes());
                out.extend_from_slice(s.as_bytes());
            }
        }
    }
    out
}
fn encode(lib: u8, d: &Data) -> Vec<u8> {
    match lib {
        0 | 5 => match d {
            Data::List(v) => borsh::to_vec(v),
            Data::Map(v) => borsh::to_vec(v),
            Data::Words(v) => borsh::to_vec(v),
            Data::WordMap(v) => borsh::to_vec(v),
        }
        .unwrap(),
        1 | 13 | 14 => {
            let (s, v, disc) = schema(d);
            let len = 12 + s.encode(&v).unwrap().len();
            s.encode_account(disc, &v, len).unwrap()
        }
        2 => star_encode(d),
        3 => {
            let Data::List(v) = d else { unreachable!() };
            let mut b = vec![0; 8008];
            b[..8].copy_from_slice(&(v.len() as u64).to_le_bytes());
            for (i, x) in v.iter().enumerate() {
                b[8 + i * 8..16 + i * 8].copy_from_slice(&x.to_le_bytes());
            }
            b
        }
        #[cfg(feature = "competitors")]
        4 => {
            let Data::Map(v) = d else { unreachable!() };
            fixture::sokoban_adapter::encode(v.len())
        }
        9 => {
            let Data::List(v) = d else { unreachable!() };
            let mut bytes = vec![0; 16 + 8 * v.len()];
            bytes[..8].copy_from_slice(b"CMPSLAB!");
            bytes[8..12].copy_from_slice(&(v.len() as u32).to_le_bytes());
            for (i, value) in v.iter().enumerate() {
                bytes[16 + 8 * i..24 + 8 * i].copy_from_slice(&value.to_le_bytes());
            }
            bytes
        }
        #[cfg(feature = "competitors")]
        10 => {
            let Data::Map(v) = d else { unreachable!() };
            fixture::critbit_adapter::encode(v.len())
        }
        #[cfg(feature = "competitors")]
        11 | 12 => {
            let Data::Map(v) = d else { unreachable!() };
            fixture::manifest_adapter::encode(v.len())
        }
        6 => wincode_encode(d),
        #[cfg(feature = "competitors")]
        7 => match d {
            Data::List(v) => fixture::pinapod_adapter::encode(1, v.len()),
            Data::Words(v) => fixture::pinapod_adapter::encode(3, v.len()),
            _ => unreachable!(),
        },
        #[cfg(feature = "competitors")]
        8 => {
            let Data::List(v) = d else { unreachable!() };
            fixture::light_adapter::encode(v.len())
        }
        _ => unreachable!(),
    }
}
fn verify(lib: u8, bytes: &[u8], d: &Data) {
    if lib == 9 {
        let Data::List(wanted) = d else {
            unreachable!()
        };
        assert_eq!(&bytes[..8], b"CMPSLAB!");
        assert_eq!(&bytes[12..16], &[0; 4]);
        let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert_eq!(len, wanted.len());
        assert!(bytes.len() >= 16 + 8 * len && (bytes.len() - 16).is_multiple_of(8));
        let actual: Vec<u64> = bytes[16..16 + 8 * len]
            .chunks_exact(8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(&actual, wanted);
        return;
    }

    if matches!(lib, 1 | 2 | 6 | 13 | 14) {
        assert_eq!(bytes, encode(lib, d), "independent full-byte oracle");
        return;
    }
    #[cfg(feature = "competitors")]
    match lib {
        3 => {
            let Data::List(v) = d else { unreachable!() };
            let len = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
            assert_eq!(len, v.len());
            let got: Vec<u64> = bytes[8..8 + 8 * len]
                .chunks_exact(8)
                .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
                .collect();
            assert_eq!(&got, v);
            return;
        }
        4 => {
            let Data::Map(v) = d else { unreachable!() };
            assert_eq!(&fixture::sokoban_adapter::decode(bytes), v);
            return;
        }
        10 => {
            let Data::Map(v) = d else { unreachable!() };
            assert_eq!(&fixture::critbit_adapter::decode(bytes), v);
            return;
        }
        11 | 12 => {
            let Data::Map(v) = d else { unreachable!() };
            assert_eq!(&fixture::manifest_adapter::decode(bytes), v);
            return;
        }
        7 => {
            match d {
                Data::List(v) => assert_eq!(&fixture::pinapod_adapter::decode_numbers(bytes), v),
                Data::Words(v) => assert_eq!(&fixture::pinapod_adapter::decode_words(bytes), v),
                _ => unreachable!(),
            };
            return;
        }
        8 => {
            let Data::List(v) = d else { unreachable!() };
            assert_eq!(&fixture::light_adapter::decode(bytes), v);
            return;
        }
        _ => {}
    }
    let decoded = match d {
        Data::List(_) => Data::List(Vec::try_from_slice(bytes).unwrap()),
        Data::Map(_) => Data::Map(BTreeMap::try_from_slice(bytes).unwrap()),
        Data::Words(_) => Data::Words(Vec::try_from_slice(bytes).unwrap()),
        Data::WordMap(_) => Data::WordMap(BTreeMap::try_from_slice(bytes).unwrap()),
    };
    assert_eq!(&decoded, d, "independent logical oracle");
    assert_eq!(bytes, encode(lib, d), "canonical full bytes");
}
// Reuse checks are outside the CU rows: mutate one account across instructions.
fn check_slab_history(vm: &mut Mollusk, owner: Pubkey, state: Pubkey, lib: u8) {
    let n = 100;
    let w = if lib == 9 { 1 } else { 2 };
    let mut model = initial(w, n);
    let before = encode(lib, &model);
    let physical = before.len();
    let mut account = Account {
        lamports: 100_000_000_000,
        data: before,
        owner,
        executable: false,
        rent_epoch: 0,
    };
    let ops: &[u8] = if w == 1 { &[5, 4, 3, 1] } else { &[3, 2, 4, 5] };
    for &op in ops {
        let (wanted, checksum) = expected(model, op, n);
        let ix = Instruction {
            program_id: owner,
            accounts: vec![AccountMeta::new(state, false)],
            data: vec![lib, w, op, n as u8, 0],
        };
        vm.logger = Some(Default::default());
        let result = vm.process_and_validate_instruction(
            &ix,
            &[(state, account)],
            &[Check::success(), Check::inner_instruction_count(0)],
        );
        account = result.get_account(&state).unwrap().clone();
        assert_eq!(
            account.data.len(),
            physical,
            "retained capacity/free-slot reuse must avoid realloc"
        );
        assert_eq!(&result.return_data[..8], &checksum.to_le_bytes());
        verify(lib, &account.data, &wanted);
        model = wanted;
    }
    eprintln!("slab history {lib}: remove, reuse, bulk set and sum passed");
}

mod calibrate;
fn main() {
    if std::env::var_os("COMPARE_CALIBRATE").is_some() {
        calibrate::run();
        return;
    }
    let owner = Pubkey::new_from_array([7; 32]);
    let state = Pubkey::new_from_array([8; 32]);
    let arch = std::env::var("COMPARE_ARCH").unwrap_or("v0".into());
    let filter = std::env::var("COMPARE_LIBS").unwrap_or("0,1".into());
    let heap = std::env::var_os("COMPARE_HEAP").is_some();
    let mut rows = Vec::new();
    for lib in filter.split(',').map(|s| s.parse::<u8>().unwrap()) {
        eprintln!(
            "competitor {lib}: {}",
            String::from_utf8(
                std::process::Command::new("date")
                    .arg("-Is")
                    .output()
                    .unwrap()
                    .stdout
            )
            .unwrap()
            .trim()
        );
        let mut vm = Mollusk::new(
            &owner,
            if lib == 2 {
                "compare_star"
            } else {
                "compare_program"
            },
        );
        vm.compute_budget.compute_unit_limit = 40_000_000;
        vm.compute_budget.heap_size = 256 * 1024;
        let name = match lib {
            0 => "borsh",
            1 => "blubber",
            13 => "blubber_trusted",
            14 => "blubber_checked",
            2 => "star_frame",
            3 => "bytemuck",
            4 => "sokoban",
            5 => "zeropod",
            6 => "wincode",
            7 => "pinapod",
            8 => "light",
            9 => "anchor_slab",
            10 => "critbit",
            11 => "manifest",
            12 => "manifest_checked",
            _ => unreachable!(),
        };
        for w in 1..=4 {
            if matches!(lib, 3 | 8 | 9) && w != 1
                || matches!(lib, 4 | 10 | 11 | 12) && w != 2
                || lib == 7 && !matches!(w, 1 | 3)
            {
                continue;
            }
            for n in [0, 1, 10, 100, 1000] {
                let data = initial(w, n);
                let before = encode(lib, &data);
                let account = Account {
                    lamports: 100_000_000_000,
                    data: before.clone(),
                    owner,
                    executable: false,
                    rent_epoch: 0,
                };
                let ix = |op| Instruction {
                    program_id: owner,
                    accounts: vec![AccountMeta::new(state, false)],
                    data: vec![lib, w, op, n as u8, (n >> 8) as u8],
                };
                let mut operations = fixture::ops(w).to_vec();
                if matches!(lib, 1 | 13 | 14) && w == 1 {
                    operations.push(6);
                }
                for &op in &operations {
                    let idle_op = if lib == 6 && matches!((w, op), (1, 3) | (2, 4)) {
                        11
                    } else if matches!((w, op), (1, 1 | 2) | (2, 1 | 5) | (3, 1) | (4, 1)) {
                        0
                    } else {
                        10
                    };
                    vm.logger = Some(Default::default());
                    let idle = vm.process_and_validate_instruction(
                        &ix(idle_op),
                        &[(state, account.clone())],
                        &[Check::success(), Check::inner_instruction_count(0)],
                    );
                    assert_eq!(
                        &idle.get_account(&state).unwrap().data,
                        &before,
                        "idle must preserve all bytes"
                    );
                    verify(lib, &idle.get_account(&state).unwrap().data, &data);
                    assert_eq!(&idle.return_data[..8], &0u64.to_le_bytes());
                    let (wanted, checksum) = expected(data.clone(), op, n);
                    vm.logger = Some(Default::default());
                    let result = vm.process_instruction(&ix(op), &[(state, account.clone())]);
                    if result.program_result.is_err() {
                        let logs = vm
                            .logger
                            .as_ref()
                            .unwrap()
                            .borrow()
                            .get_recorded_content()
                            .to_vec();
                        let capacity = matches!(lib, 3 | 4 | 8 | 10)
                            && n == 1000
                            && matches!((w, op), (1, 4) | (2, 2));
                        if capacity {
                            assert_eq!(
                                format!("{:?}", result.program_result),
                                "Failure(AccountDataTooSmall)"
                            );
                        }
                        let oom = logs.iter().any(|s| {
                            s.contains("memory allocation") || s.contains("out of memory")
                        });
                        assert!(
                            capacity || oom,
                            "unexpected failure {name} W{w} n{n} op{op}: {:?} {:?}",
                            result.program_result,
                            logs
                        );
                        assert_eq!(
                            &result.get_account(&state).unwrap().data,
                            &before,
                            "failed instruction rollback"
                        );
                        let row = json!({"arch":arch,"lib":name,"workload":w,"n":n,"op":fixture::op_name(w,op),"idle_cu":idle.compute_units_consumed,"whole_cu":result.compute_units_consumed,"delta_cu":null,"before_bytes":before.len(),"after_bytes":before.len(),"heap_bytes":null,"checksum":null,"status":if capacity{"capacity_exhausted"}else{"heap_exhausted"},"error":format!("{:?}",result.program_result),"logs":logs});
                        println!("{row}");
                        rows.push(row);
                        continue;
                    }
                    result.run_checks(
                        &[Check::success(), Check::inner_instruction_count(0)],
                        &Default::default(),
                    );
                    let after = &result.get_account(&state).unwrap().data;
                    verify(lib, after, &wanted);
                    assert_eq!(
                        &result.return_data[..8],
                        &checksum.to_le_bytes(),
                        "{name} W{w} n{n} op{op} checksum"
                    );
                    let heap_bytes = if heap {
                        Some(u64::from_le_bytes(
                            result.return_data[8..16].try_into().unwrap(),
                        ))
                    } else {
                        None
                    };
                    let row = json!({"arch":arch,"lib":name,"workload":w,"n":n,"op":fixture::op_name(w,op),"idle_cu":idle.compute_units_consumed,"whole_cu":result.compute_units_consumed,"delta_cu":result.compute_units_consumed as i64-idle.compute_units_consumed as i64,"before_bytes":before.len(),"after_bytes":after.len(),"heap_bytes":heap_bytes,"idle_heap_bytes":if heap{Some(u64::from_le_bytes(idle.return_data[8..16].try_into().unwrap()))}else{None},"checksum":checksum,"status":"ok"});
                    println!("{row}");
                    rows.push(row);
                }
            }
        }
        if (9..=12).contains(&lib) {
            check_slab_history(&mut vm, owner, state, lib);
        }
    }
    let dest = std::env::var("COMPARE_OUT").unwrap_or_else(|_| format!(".tmp/compare-{arch}.json"));
    std::fs::write(dest, serde_json::to_string_pretty(&rows).unwrap() + "\n").unwrap();
}
