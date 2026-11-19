//! Reuse the production Mollusk bencher with independent traversal oracles.
use super::{account, measured_stages, OWNER, STATE};
use blubber_mollusk::traversal::{inventory, TraversalCase};
use mollusk_svm::{result::Check, Mollusk};
use mollusk_svm_bencher::MolluskComputeUnitBencher;
use solana_instruction::{AccountMeta, Instruction};

fn instruction(case: &TraversalCase, op: u8) -> Instruction {
    Instruction {
        program_id: OWNER,
        accounts: vec![AccountMeta::new(STATE, false)],
        data: case.instruction_data(op),
    }
}

pub(super) fn run() {
    let rewrite = std::env::var_os("BLUBBER_REWRITE_KERNEL").is_some();
    let rewrite_api = std::env::var_os("BLUBBER_REWRITE_API").is_some();
    let rewrite_contract = std::env::var_os("BLUBBER_REWRITE_CONTRACT").is_some();
    let metrics = std::env::var_os("BLUBBER_CU_METRICS").is_some();
    let artifact = if metrics {
        "blubber_cu_traversal_metrics"
    } else {
        "blubber_cu_traversal"
    };
    let trace = std::env::var("BLUBBER_TRAVERSAL_TRACE_DIR").ok();
    let filter = std::env::var("BLUBBER_TRAVERSAL_FILTER").ok();
    let mut vm = Mollusk::new_debuggable(&OWNER, artifact, trace.is_some());
    vm.compute_budget.compute_unit_limit = 40_000_000;
    // Owned n=1000 is explicitly a requested 256 KiB heap scaling observation.
    vm.compute_budget.heap_size = 256 * 1024;
    let mut scheduled = Vec::new();
    let cases: Vec<_> = if rewrite_contract {
        blubber_mollusk::traversal::rewrite_contract_inventory()
    } else if rewrite_api {
        blubber_mollusk::traversal::rewrite_api_inventory()
    } else if rewrite {
        blubber_mollusk::traversal::rewrite_inventory()
    } else if std::env::var_os("BLUBBER_WALK_CONTRACT").is_some() {
        blubber_mollusk::traversal::source_inventory()
    } else {
        inventory()
    }
    .into_iter()
    .filter(|case| {
        filter
            .as_ref()
            .is_none_or(|filter| case.name().contains(filter))
    })
    .collect();
    assert!(!cases.is_empty(), "empty traversal diagnostic selection");
    for case in &cases {
        if rewrite || rewrite_api || rewrite_contract {
            assert_eq!(
                (case.op, case.common_idle(), case.admitted_idle()),
                (
                    case.op,
                    20,
                    if case.op == 66 {
                        23
                    } else if matches!(case.op, 64 | 65) {
                        22
                    } else {
                        21
                    }
                ),
                "rewrite ingress opcodes"
            );
        }
        let model = case.model_case();
        let (schema, value, disc) = model.model();
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
        let mut totals = [0; 3];
        let mut phases = [0; 5];
        let mut observed = 0u64;
        for (i, op) in [case.common_idle(), case.admitted_idle(), case.op]
            .into_iter()
            .enumerate()
        {
            if let Some(base) = &trace {
                vm.invocation_inspect_callback = Box::new(
                    mollusk_svm::register_tracing::DefaultRegisterTracingCallback {
                        sbf_trace_dir: format!("{base}/{}/op{op}", case.name()),
                        ..Default::default()
                    },
                );
            }
            if metrics {
                vm.logger = Some(Default::default());
            }
            let ix = instruction(case, op);
            let result = vm.process_and_validate_instruction(
                &ix,
                &[(STATE, account(before.clone()))],
                &[Check::success(), Check::inner_instruction_count(0)],
            );
            assert_eq!(
                result.get_account(&STATE).unwrap().data,
                if i == 2 { &expected } else { &before }.as_slice(),
                "{} independent complete-byte oracle op{op}",
                case.name()
            );
            assert_eq!(
                result.return_data,
                case.read_result(op).to_le_bytes(),
                "{} independent read-result oracle op{op}",
                case.name()
            );
            if i == 2 {
                observed = u64::from_le_bytes(result.return_data.as_slice().try_into().unwrap());
            }
            totals[i] = result.compute_units_consumed;
            if metrics && i == 2 {
                phases =
                    measured_stages(vm.logger.as_ref().unwrap().borrow().get_recorded_content());
            }
        }
        println!(
            "traversal,{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            case.name(),
            case.layout,
            case.n,
            case.op,
            u8::from(case.common),
            totals[0],
            totals[1],
            totals[2],
            observed,
            phases[0],
            phases[1],
            phases[2],
            phases[3],
            phases[4]
        );
        if rewrite || rewrite_api {
            for (suffix, op) in [
                ("common_idle", case.common_idle()),
                ("admitted_idle", case.admitted_idle()),
            ] {
                scheduled.push((
                    format!("{}_{}", case.name(), suffix),
                    instruction(case, op),
                    vec![(STATE, account(before.clone()))],
                ));
            }
        }
        scheduled.push((
            case.name(),
            instruction(case, case.op),
            vec![(STATE, account(before))],
        ));
    }
    println!(
        "{},pass,{}",
        if filter.is_some() {
            "traversal_oracles_partial"
        } else {
            "traversal_oracles"
        },
        cases.len()
    );
    if !metrics && trace.is_none() {
        let output =
            std::env::var("BLUBBER_BENCH_OUT").unwrap_or_else(|_| ".tmp/traversal-bencher".into());
        let mut bencher = MolluskComputeUnitBencher::new(vm)
            .must_pass(true)
            .out_dir(&output);
        for (name, ix, accounts) in &scheduled {
            bencher = bencher.bench((name, ix, accounts));
        }
        bencher.execute();
    }
}
