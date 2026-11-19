//! Lifecycle tables with physical lengths, balances, telemetry and register traces.

use super::{account, measured_stages, OWNER, PAYER, STATE};
use mollusk_svm::{result::Check, Mollusk};
use mollusk_svm_bencher::MolluskComputeUnitBencher;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const RECIPIENT: Pubkey = Pubkey::new_from_array([10; 32]);
const MODES: [&str; 4] = ["Max", "UpTo0", "UpTo32", "OnDemand"];
const ACTIONS: [&str; 10] = [
    "same",
    "grow8",
    "fifty_pushes",
    "after_grow501",
    "after_shrink37",
    "two_cycles",
    "readonly",
    "shrink64",
    "exact_window",
    "failed_growth",
];

pub(super) fn run(metrics: bool, alloc: bool) {
    let artifact = match (metrics, alloc) {
        (false, false) => "blubber_cu_matrix",
        (true, false) => "blubber_cu_matrix_metrics",
        (false, true) => "blubber_cu_matrix_alloc",
        (true, true) => "blubber_cu_matrix_alloc_metrics",
    };
    let mut scheduled = Vec::new();
    let mut baselines = std::collections::BTreeMap::new();
    for n in [10, 100, 1000] {
        for (mode, mode_name) in MODES.iter().enumerate() {
            if (mode == 3) != alloc {
                continue;
            }
            for (action, action_name) in ACTIONS.iter().enumerate() {
                if (matches!(action, 2 | 8 | 9) && mode != 0 && mode != 3)
                    || (action == 1 && mode == 1)
                    || (action == 6 && mode != 0)
                {
                    continue;
                }
                for (funding, refund, reserve, suffix) in [
                    (false, false, 0, "prepaid"),
                    (true, false, 0, "topup"),
                    (false, true, 0, "refund"),
                    (true, false, 32, "retain32"),
                ] {
                    if (suffix != "prepaid" && !matches!(action, 0 | 1 | 7))
                        || (reserve != 0 && (mode == 1 || (mode == 2 && action == 1)))
                    {
                        continue;
                    }
                    let name = format!("lease_n{n}_{mode_name}_{action_name}_{suffix}");
                    let data_len = 12 + n * 8;
                    let mut data = vec![0; data_len];
                    data[..8].copy_from_slice(b"LEASE001");
                    data[8..12].copy_from_slice(&(data_len as u32).to_le_bytes());
                    let mut state = account(data.clone());
                    let trace = std::env::var("BLUBBER_LEASE_TRACE_DIR")
                        .ok()
                        .filter(|_| metrics);
                    let mut vm = Mollusk::new_debuggable(&OWNER, artifact, trace.is_some());
                    if let Some(base) = &trace {
                        vm.invocation_inspect_callback = Box::new(
                            mollusk_svm::register_tracing::DefaultRegisterTracingCallback {
                                sbf_trace_dir: format!("{base}/{name}"),
                                ..Default::default()
                            },
                        );
                    }
                    if metrics {
                        vm.logger = Some(Default::default());
                    }
                    if funding {
                        state.lamports = vm.sysvars.rent.minimum_balance(data_len);
                    }
                    let before_balance = state.lamports;
                    let recipient =
                        Account::new(vm.sysvars.rent.minimum_balance(0), 0, &Pubkey::default());
                    let accounts = vec![
                        (STATE, state),
                        (PAYER, Account::new(100_000_000_000, 0, &Pubkey::default())),
                        mollusk_svm::program::keyed_account_for_system_program(),
                        (RECIPIENT, recipient.clone()),
                    ];
                    let instruction = Instruction {
                        program_id: OWNER,
                        accounts: vec![
                            AccountMeta::new(STATE, false),
                            AccountMeta::new(PAYER, true),
                            AccountMeta::new_readonly(Pubkey::default(), false),
                            AccountMeta::new(RECIPIENT, false),
                        ],
                        data: vec![
                            28,
                            mode as u8,
                            action as u8,
                            u8::from(funding),
                            u8::from(refund),
                            reserve,
                        ],
                    };
                    // Baseline uses the same funding/refund policy and an unchanged lease.
                    // Avoid writing baseline trace files into a scenario's directory.
                    let baseline = *baselines
                        .entry((n, mode, funding, refund, reserve))
                        .or_insert_with(|| {
                            let mut baseline_vm = Mollusk::new(&OWNER, artifact);
                            if metrics {
                                baseline_vm.logger = Some(Default::default());
                            }
                            let mut baseline_ix = instruction.clone();
                            baseline_ix.data[2] = 0;
                            let result = baseline_vm.process_and_validate_instruction(
                                &baseline_ix,
                                &accounts,
                                &[Check::success()],
                            );
                            result.compute_units_consumed
                        });
                    let opening = match action {
                        3 => data_len + 501,
                        4 => data_len - 37,
                        _ => data_len,
                    };
                    let used = match action {
                        1 => opening + 8,
                        2 => opening + 400,
                        7 => 64,
                        8 => data_len + 10_240,
                        _ => opening,
                    };
                    let final_len = used + usize::from(reserve);
                    let final_minimum = vm.sysvars.rent.minimum_balance(final_len);
                    let topped_up = final_minimum.saturating_sub(before_balance);
                    let refunded = if refund {
                        before_balance.saturating_sub(final_minimum)
                    } else {
                        0
                    };
                    let result = vm.process_and_validate_instruction(
                        &instruction,
                        &accounts,
                        &[
                            Check::success(),
                            Check::inner_instruction_count(usize::from(topped_up != 0)),
                        ],
                    );
                    let after = result.get_account(&STATE).unwrap();
                    assert_eq!(after.data.len(), final_len, "{name} physical length");
                    let mut expected = data;
                    expected.resize(final_len, 0);
                    if action != 6 {
                        expected[8..12].copy_from_slice(&(used as u32).to_le_bytes());
                        expected[12] = 42;
                    }
                    assert_eq!(after.data, expected, "{name} full bytes");
                    assert_eq!(
                        after.lamports,
                        before_balance + topped_up - refunded,
                        "{name} state rent"
                    );
                    assert_eq!(
                        result.get_account(&PAYER).unwrap().lamports,
                        100_000_000_000 - topped_up
                    );
                    assert_eq!(
                        result.get_account(&RECIPIENT).unwrap().lamports,
                        recipient.lamports + refunded
                    );
                    let mut stages = [0; 5];
                    let mut telemetry = [0; 6];
                    if metrics {
                        let logger = vm.logger.as_ref().unwrap().borrow();
                        let messages = logger.get_recorded_content();
                        stages = measured_stages(messages);
                        let parse = |tag: &str| -> Vec<u64> {
                            let line = messages
                                .iter()
                                .find(|s| s.contains(tag))
                                .expect("lease telemetry");
                            line.split(',')
                                .skip(1)
                                .map(|s| {
                                    u64::from_str_radix(s.trim().trim_start_matches("0x"), 16)
                                        .unwrap()
                                })
                                .collect()
                        };
                        let lengths = parse("0x4c53,");
                        let counts = parse("0x4c54,")[0];
                        telemetry = [
                            lengths[0],
                            lengths[1],
                            lengths[2],
                            lengths[3],
                            counts >> 32,
                            counts & u64::from(u32::MAX),
                        ];
                        assert_eq!(telemetry[0] as usize, data_len);
                        assert_eq!(telemetry[1] as usize, opening);
                        assert!(telemetry[2] >= telemetry[1]);
                    }
                    println!("lease,{name},{n},{mode_name},{action_name},{baseline},{},{},{data_len},{used},{final_len},{},{},{},{},{},{},{},{},{},{},{},{topped_up},{refunded}",
                        result.compute_units_consumed, result.compute_units_consumed as i64 - baseline as i64,
                        stages[0], stages[1], stages[2], stages[3], stages[4], telemetry[1], telemetry[2], telemetry[3], telemetry[4], telemetry[5], usize::from(topped_up != 0));
                    scheduled.push((name, instruction, accounts));
                }
            }
        }
    }
    if !metrics {
        let vm = Mollusk::new(&OWNER, artifact);
        let output = std::env::var("BLUBBER_BENCH_OUT")
            .unwrap_or_else(|_| format!("{}/../../.tmp/cu-bencher", env!("CARGO_MANIFEST_DIR")));
        let mut bencher = MolluskComputeUnitBencher::new(vm)
            .must_pass(true)
            .out_dir(&format!("{output}/lease-{alloc}"));
        for (name, ix, accounts) in &scheduled {
            bencher = bencher.bench((name, ix, accounts));
        }
        bencher.execute();
    }
}
