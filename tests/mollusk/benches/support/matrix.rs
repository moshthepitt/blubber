//! Independent logical fixtures and full-byte oracles for scaling measurements.

use super::{account, measured_stages, OWNER, STATE};
use blubber_cu_fixture::matrix::*;
use mollusk_svm::{result::Check, Mollusk};
use mollusk_svm_bencher::MolluskComputeUnitBencher;
use std::collections::BTreeMap;

use blubber_mollusk::matrix::Case;

trait CaseGeometry {
    fn geometry(&self, before: usize) -> (usize, usize);
}

impl CaseGeometry for Case {
    // Sum the production relocation ranges, excluding identity moves, input
    // encoding, zeroing and directory stores. Bulk insertion moves payload once.
    // Words count retained/new end-word publications, excluding count/used words.
    fn geometry(&self, before: usize) -> (usize, usize) {
        if self.opcode != 23 {
            return if self.opcode != 24 && self.opcode != 27 && self.op != 4 {
                (
                    before - (12 + 4 + 8 * (self.depth - 2) + ORIGINAL.len()),
                    self.depth - 1,
                )
            } else {
                (0, 0)
            };
        }
        let (n, i) = (self.n, self.index);
        let indexed = !self.layout.is_multiple_of(2);
        let map = self.layout >= 2;
        let op = self.op;
        if !indexed {
            let stride = if map { 40 } else { 8 };
            return (
                match op {
                    5 | 17 | 21 | 32 | 37 => (n - i) * stride,
                    7 | 8 | 31 | 38 => (n - i - 1) * stride,
                    _ => 0,
                },
                0,
            );
        }
        if map {
            match op {
                5 | 21 | 32 | 37 => (8 * n + 36 * (n - i), n + 1),
                6 | 30 | 47 => (8 * (n - i - 1), n),
                10 | 20 | 27 => (8 * (n - i - 1), n - i),
                7 | 8 | 31 | 38 => (8 * (n - 1) + 36 * (n - i - 1), n - 1),
                11 => (8 * (n - 1), n),
                33 | 34 => (4 * n * (n - 1), n * (n + 1) / 2),
                _ => (0, 0),
            }
        } else {
            match op {
                5 | 37 => (8 * n + 4 * n.saturating_sub(1 + i), n),
                6 | 40 => (8 * (n - i - 1), n - 1),
                10 | 27 => (8 * (n - i - 1), (n - 1).saturating_sub(i)),
                7 | 8 | 38 => (8 * (n - 1), n.saturating_sub(2)),
                11 => (8 * (n - 1), n - 1),
                13 | 41 => (8 * n, n),
                14 | 39 if n != 0 => (8 * (n - 1), n.saturating_sub(2)),
                15 if i < n => (8 * i, i.saturating_sub(1)),
                16 | 17 => (8 * n, n + 1),
                33 | 34 => (4 * n * n.saturating_sub(1), n * n.saturating_sub(1) / 2),
                _ => (0, 0),
            }
        }
    }
}

fn execute(
    vm: &mut Mollusk,
    case: &Case,
    op: u8,
    data: &[u8],
    metrics: bool,
) -> (u64, Vec<u8>, [u64; 5]) {
    if metrics {
        vm.logger = Some(Default::default());
    }
    let result = vm.process_and_validate_instruction(
        &case.instruction(op),
        &[(STATE, account(data.to_vec()))],
        &[Check::success(), Check::inner_instruction_count(0)],
    );
    let stages = if metrics {
        measured_stages(vm.logger.as_ref().unwrap().borrow().get_recorded_content())
    } else {
        [0; 5]
    };
    (
        result.compute_units_consumed,
        result.get_account(&STATE).unwrap().data.clone(),
        stages,
    )
}

pub(super) fn run(metrics: bool) {
    let trace = std::env::var("BLUBBER_VALIDATION_TRACE_DIR").ok();
    let mut cases = Vec::new();
    for n in [10, 100, 1000] {
        for layout in 0..6u8 {
            for prefix in [false, true] {
                if layout < 2 && prefix {
                    continue;
                }
                let mut add = |op: u8, index: usize, position: &str| {
                    cases.push(Case {
                        name: format!(
                            "{}_n{n}_{}_{}_{}",
                            LAYOUTS[layout as usize],
                            OPERATIONS[op as usize],
                            position,
                            if prefix { "common" } else { "plain" }
                        ),
                        layout,
                        n,
                        op,
                        index,
                        prefix,
                        opcode: 23,
                        depth: 3,
                    })
                };
                for op in [0, 1, 2, 3, 4, 9, 11, 12] {
                    add(op, if op == 1 { n - 1 } else { 0 }, "all");
                }
                for (position, insert, existing) in
                    [("front", 0, 0), ("middle", n / 2, n / 2), ("end", n, n - 1)]
                {
                    for op in [5, 6, 7, 8, 10] {
                        add(op, if op == 5 { insert } else { existing }, position);
                    }
                    if layout < 2 {
                        add(17, insert, position);
                        add(15, insert, position);
                    } else {
                        for op in [18, 19, 20, 21] {
                            add(op, if op == 21 { insert } else { existing }, position);
                        }
                    }
                }
                if layout < 2 {
                    for op in [13, 14, 16] {
                        add(op, 0, "end");
                    }
                }
            }
        }
    }
    for (opcode, depth) in [(24, 1), (25, 4), (26, 8), (27, 3)] {
        let ops: &[u8] = if opcode == 27 {
            &[25, 26]
        } else {
            &[4, 5, 6, 22, 23, 24]
        };
        for &op in ops {
            let operation = match op {
                5 => "string_set_grow",
                6 => "string_set_shrink",
                _ => OPERATIONS[op as usize],
            };
            cases.push(Case {
                name: format!("depth{depth}_{operation}"),
                layout: 1,
                n: 1,
                op,
                index: 0,
                prefix: false,
                opcode,
                depth,
            });
        }
    }
    if std::env::var_os("BLUBBER_VALIDATION_ONLY").is_some() {
        cases.retain(|case| case.opcode == 23 && case.op == 2);
    }
    run_cases(metrics, false, false, cases, trace.as_deref());
}

// Step 40's additional methods use the same dispatcher, oracle and baselines.
// Allocating mirrors retain step 40's n<=10 cohort and the SDK's 32 KiB heap.
pub(super) fn run_extended() {
    for alloc in [false, true] {
        let mut cases = Vec::new();
        for n in if alloc {
            &[1, 2, 10][..]
        } else {
            &[1, 2, 10, 100, 1000][..]
        } {
            let n = *n;
            for layout in 0..6 {
                for prefix in [false, true] {
                    if layout < 2 && prefix {
                        continue;
                    }
                    let ops: &[u8] = match (alloc, layout < 2) {
                        (false, true) => &[27, 28, 33, 34, 35, 36, 42],
                        (false, false) => &[27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 42],
                        (true, true) => &[37, 38, 39, 40, 41, 43],
                        (true, false) => &[37, 38, 43, 47, 48],
                    };
                    for &op in ops {
                        for (position, index) in [("front", 0), ("middle", n / 2), ("end", n - 1)] {
                            cases.push(Case {
                                name: format!(
                                    "{}_n{n}_{}_{}_{}_{}",
                                    LAYOUTS[layout as usize],
                                    OPERATIONS[op as usize],
                                    position,
                                    if prefix { "common" } else { "plain" },
                                    if alloc { "alloc" } else { "no_alloc" }
                                ),
                                layout,
                                n,
                                op,
                                index: if layout < 2 && op == 35 { n } else { index },
                                prefix,
                                opcode: 23,
                                depth: 3,
                            });
                        }
                    }
                }
            }
        }
        for layout in 0..2 {
            for &op in if alloc {
                &[39][..]
            } else {
                &[9, 13, 14, 15, 16, 17, 28, 33, 34, 35][..]
            } {
                cases.push(Case {
                    name: format!(
                        "{}_n0_{}_empty_plain_{}",
                        LAYOUTS[layout as usize],
                        OPERATIONS[op as usize],
                        if alloc { "alloc" } else { "no_alloc" }
                    ),
                    layout,
                    n: 0,
                    op,
                    index: 0,
                    prefix: false,
                    opcode: 23,
                    depth: 3,
                });
            }
        }
        if !alloc {
            for op in [44, 45, 46] {
                cases.push(Case {
                    name: format!("depth3_{}", OPERATIONS[op as usize]),
                    layout: 1,
                    n: 1,
                    op,
                    index: 0,
                    prefix: false,
                    opcode: 27,
                    depth: 3,
                });
            }
        }
        run_cases(false, alloc, true, cases, None);
    }
}

fn run_cases(metrics: bool, alloc: bool, extended: bool, cases: Vec<Case>, trace: Option<&str>) {
    let artifact = match (metrics, alloc) {
        (false, false) => "blubber_cu_matrix",
        (true, false) => "blubber_cu_matrix_metrics",
        (false, true) => "blubber_cu_matrix_alloc",
        (true, true) => "blubber_cu_matrix_alloc_metrics",
    };
    let mut vm = Mollusk::new_debuggable(&OWNER, artifact, trace.is_some());
    // Scaling observations can exceed a transaction meter. Acceptance fixtures
    // and the complete tour retain the default 1,400,000-CU budget.
    vm.compute_budget.compute_unit_limit = 20_000_000;
    let mut scheduled = Vec::new();
    let mut baselines = BTreeMap::new();
    for case in cases {
        let (schema, value, disc) = case.model();
        let before = 12 + schema.encode(&value).unwrap().len();
        let data = schema.encode_account(disc, &value, before).unwrap();
        let expected_value = case.expected(value);
        let after_len = 12 + schema.encode(&expected_value).unwrap().len();
        let expected = schema
            .encode_account(disc, &expected_value, after_len)
            .unwrap();
        let baseline_op = if matches!(case.op, 42 | 43) {
            3
        } else if case.op < 4 || matches!(case.op, 18 | 19) {
            0
        } else {
            4
        };
        let cache_key = (case.opcode, case.layout, case.n, case.prefix, baseline_op);
        let baseline = *baselines.entry(cache_key).or_insert_with(|| {
            let (cu, after, _) = execute(&mut vm, &case, baseline_op, &data, metrics);
            assert_eq!(after, data, "{} baseline preserved full bytes", case.name);
            cu
        });
        let (total, after, stages) = if let Some(base) = trace.as_ref().filter(|_| case.n == 100) {
            let mut traced = Mollusk::new_debuggable(&OWNER, artifact, true);
            traced.compute_budget.compute_unit_limit = 20_000_000;
            traced.invocation_inspect_callback = Box::new(
                mollusk_svm::register_tracing::DefaultRegisterTracingCallback {
                    sbf_trace_dir: format!("{base}/{}", case.name),
                    ..Default::default()
                },
            );
            execute(&mut traced, &case, case.op, &data, metrics)
        } else {
            execute(&mut vm, &case, case.op, &data, metrics)
        };
        assert_eq!(
            after, expected,
            "{} complete independent encoding",
            case.name
        );
        assert_eq!(
            schema.decode_account(disc, &after).unwrap().value,
            expected_value
        );
        if !metrics
            && !extended
            && case.opcode == 23
            && matches!(case.layout, 0 | 1)
            && case.n == 1000
            && case.op == 12
        {
            let default_vm = Mollusk::new(&OWNER, artifact);
            let default_budget = default_vm.compute_budget.compute_unit_limit;
            let accounts = vec![(STATE, account(data.clone()))];
            // Fixed traversal retains its existing default-meter acceptance.
            // Indexed scaling reports the observed outcome without a Q7 cap.
            if case.layout == 0 || total <= default_budget {
                let result = default_vm.process_and_validate_instruction(
                    &case.instruction(case.op),
                    &accounts,
                    &[Check::success(), Check::inner_instruction_count(0)],
                );
                assert_eq!(result.get_account(&STATE).unwrap().data, expected);
                println!("meter_control,{},{default_budget},success_pass", case.name);
            } else {
                let result = default_vm.process_and_validate_instruction(
                    &case.instruction(case.op),
                    &accounts,
                    &[
                        Check::instruction_err(
                            solana_instruction::error::InstructionError::ProgramFailedToComplete,
                        ),
                        Check::compute_units(default_budget),
                    ],
                );
                assert_eq!(
                    result.resulting_accounts, accounts,
                    "default meter atomic rollback"
                );
                println!("meter_control,{},{default_budget},rollback_pass", case.name);
            }

            // Exhaustion remains tested as traversal gets faster, using the
            // same uninstrumented artifact as the measured whole instruction.
            let budget = total / 2;
            let mut limited_vm = Mollusk::new(&OWNER, artifact);
            limited_vm.compute_budget.compute_unit_limit = budget;
            let result = limited_vm.process_and_validate_instruction(
                &case.instruction(case.op),
                &accounts,
                &[
                    Check::instruction_err(
                        solana_instruction::error::InstructionError::ProgramFailedToComplete,
                    ),
                    Check::compute_units(budget),
                ],
            );
            assert_eq!(result.resulting_accounts, accounts, "meter atomic rollback");
            println!("meter_control,{},{budget},rollback_pass", case.name);
        }
        let (moved, patched) = case.geometry(before);
        println!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            if extended { "extended" } else { "matrix" },
            case.name,
            LAYOUTS[case.layout as usize],
            case.n,
            case.index,
            u8::from(case.prefix),
            case.depth,
            baseline,
            total,
            total as i64 - baseline as i64,
            before,
            after_len,
            before,
            after.len(),
            moved,
            patched,
            stages[0],
            stages[1],
            stages[2],
            stages[3],
            stages[4],
            u8::from(alloc)
        );
        let instruction = case.instruction(case.op);
        scheduled.push((case.name, instruction, vec![(STATE, account(data))]));
    }
    if !metrics {
        let output = std::env::var("BLUBBER_BENCH_OUT")
            .unwrap_or_else(|_| format!("{}/../../.tmp/cu-bencher", env!("CARGO_MANIFEST_DIR")));
        let mut bencher = MolluskComputeUnitBencher::new(vm)
            .must_pass(true)
            .out_dir(&format!(
                "{output}/{}",
                if extended {
                    if alloc {
                        "extended-alloc"
                    } else {
                        "extended"
                    }
                } else {
                    "matrix"
                }
            ));
        for (name, ix, accounts) in &scheduled {
            bencher = bencher.bench((name, ix, accounts));
        }
        bencher.execute();
    }
}
