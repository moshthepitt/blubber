//! Whole mixed-ELF observations with independent complete-byte/read oracles.
#![forbid(unsafe_code)]

use blubber_mollusk::traversal::TraversalCase;
use mollusk_svm::{result::Check, Mollusk};
use mollusk_svm_bencher::MolluskComputeUnitBencher;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

fn indexed_result(value: &blubber_model::Value) -> u64 {
    use blubber_model::{Key, Value};
    let mut result = 0u64;
    let mut consume = |bytes: &[u8]| {
        for &byte in bytes {
            result = (result ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    };
    let Value::Struct(fields) = value else {
        panic!("iteration root");
    };
    match &fields[0] {
        Value::List(values) => {
            for value in values {
                let Value::Str(text) = value else {
                    panic!("iteration value");
                };
                consume(text.as_bytes());
            }
        }
        Value::Map(entries) => {
            for (key, value) in entries {
                let Key::Pubkey(key) = key else {
                    panic!("iteration key");
                };
                let Value::Str(text) = value else {
                    panic!("iteration value");
                };
                consume(key);
                consume(text.as_bytes());
            }
        }
        _ => panic!("iteration collection"),
    }
    result
}

fn main() {
    let owner = Pubkey::new_from_array([7; 32]);
    let state = Pubkey::new_from_array([8; 32]);
    let trace = std::env::var("BLUBBER_MIXED_TRACE_DIR").ok();
    let filter = std::env::var("BLUBBER_MIXED_FILTER").ok();
    let mut vm = Mollusk::new_debuggable(&owner, "blubber_cu_mixed", trace.is_some());
    vm.compute_budget.compute_unit_limit = 40_000_000;
    vm.compute_budget.heap_size = 256 * 1024;
    let mut scheduled = Vec::new();
    let mut count = 0;
    for layout in 0..4 {
        for common in [false, true] {
            if layout < 2 && common {
                continue;
            }
            let ops: &[u8] = match layout {
                0 => &[0, 6, 7, 20, 26, 27, 30, 31, 32, 33, 34, 70],
                2 => &[0, 6, 7, 20, 26, 27, 30, 31, 32, 33, 34],
                3 => &[0, 6, 7, 20, 26, 27, 30, 31, 33, 34, 63, 64, 66, 71, 72, 73],
                _ => &[0, 6, 7, 20, 26, 27, 30, 31, 33, 34, 63, 64],
            };
            for n in [0, 1, 10, 100, 1000] {
                for &op in ops {
                    // Manual selection writes the same logical replacements as
                    // cursor_set. The model never calls production encoding.
                    let case = TraversalCase {
                        layout,
                        n,
                        op: match op {
                            70 => 31,
                            71..=73 => 71,
                            _ => op,
                        },
                        common,
                    };
                    let name = if op == 70 {
                        case.name().replace("cursor_set", "manual_get_mut_set")
                    } else if op == 72 {
                        case.name()
                            .replace("map_grow_from_old_values", "map_grow_from_old_values_keyed")
                    } else if op == 73 {
                        case.name().replace(
                            "map_grow_from_old_values",
                            "map_grow_from_old_values_fixed_width",
                        )
                    } else {
                        case.name()
                    };
                    if filter.as_ref().is_some_and(|filter| !name.contains(filter)) {
                        continue;
                    }
                    let (schema, mut value, disc) = case.model_case().model();
                    if matches!(op, 71..=73) {
                        use blubber_model::Value;
                        let Value::Struct(fields) = &mut value else {
                            panic!("recipe root");
                        };
                        let Value::Map(entries) = &mut fields[0] else {
                            panic!("recipe map");
                        };
                        for (i, old) in entries.values_mut().enumerate() {
                            *old = Value::Str(format!("{i:08x}"));
                        }
                    }
                    let indexed_iteration =
                        matches!(layout, 1 | 3) && matches!(op, 0 | 6 | 7 | 26 | 27);
                    if indexed_iteration {
                        use blubber_model::Value;
                        let Value::Struct(fields) = &mut value else {
                            panic!("iteration root");
                        };
                        let values: &mut dyn Iterator<Item = &mut Value> = match &mut fields[0] {
                            Value::List(values) => &mut values.iter_mut(),
                            Value::Map(values) => &mut values.values_mut(),
                            _ => panic!("iteration collection"),
                        };
                        for (i, old) in values.enumerate() {
                            *old = Value::Str(format!("{i:08x}"));
                        }
                    }
                    let expected_read = if indexed_iteration && op != 0 {
                        indexed_result(&value)
                    } else {
                        case.read_result(case.op)
                    };
                    let before = schema
                        .encode_account(disc, &value, 12 + schema.encode(&value).unwrap().len())
                        .unwrap();
                    let expected_value = case.expected(value);
                    let expected = schema
                        .encode_account(
                            disc,
                            &expected_value,
                            12 + schema.encode(&expected_value).unwrap().len(),
                        )
                        .unwrap();
                    let instruction = |op| Instruction {
                        program_id: owner,
                        accounts: vec![AccountMeta::new(state, false)],
                        data: case.instruction_data(op),
                    };
                    let account = Account {
                        lamports: 100_000_000_000,
                        data: before.clone(),
                        owner,
                        executable: false,
                        rent_epoch: 0,
                    };
                    let idle = if matches!(op, 0 | 6 | 7) { 0 } else { 20 };
                    let mut totals = [0; 2];
                    for (i, operation) in [idle, op].into_iter().enumerate() {
                        if let Some(base) = &trace {
                            vm.invocation_inspect_callback = Box::new(
                                mollusk_svm::register_tracing::DefaultRegisterTracingCallback {
                                    sbf_trace_dir: format!("{base}/{name}/op{operation}"),
                                    ..Default::default()
                                },
                            );
                        }
                        let result = vm.process_and_validate_instruction(
                            &instruction(operation),
                            &[(state, account.clone())],
                            &[Check::success(), Check::inner_instruction_count(0)],
                        );
                        assert_eq!(
                            result.get_account(&state).unwrap().data,
                            if i == 0 { &before } else { &expected }.as_slice(),
                            "{name} complete bytes op{operation}"
                        );
                        let read = if i == 0 { 0 } else { expected_read };
                        assert_eq!(
                            result.return_data,
                            read.to_le_bytes(),
                            "{name} read result op{operation}"
                        );
                        totals[i] = result.compute_units_consumed;
                    }
                    println!(
                        "mixed,{name},{layout},{n},{op},{},{},{},{}",
                        u8::from(common),
                        totals[0],
                        totals[1],
                        expected_read
                    );
                    scheduled.push((
                        name.clone() + "_common_idle",
                        instruction(idle),
                        vec![(state, account.clone())],
                    ));
                    scheduled.push((name, instruction(op), vec![(state, account)]));
                    count += 1;
                }
            }
        }
    }
    assert!(count > 0, "empty mixed diagnostic selection");
    println!(
        "{},pass,{count}",
        if filter.is_some() {
            "mixed_oracles_partial"
        } else {
            "mixed_oracles"
        }
    );
    if trace.is_none() {
        let output =
            std::env::var("BLUBBER_BENCH_OUT").unwrap_or_else(|_| ".tmp/mixed-bencher".into());
        let mut bencher = MolluskComputeUnitBencher::new(vm)
            .must_pass(true)
            .out_dir(&output);
        for (name, instruction, accounts) in &scheduled {
            bencher = bencher.bench((name, instruction, accounts));
        }
        bencher.execute();
    }
}
