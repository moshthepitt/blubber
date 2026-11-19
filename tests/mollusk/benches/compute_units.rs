//! Fresh uninstrumented instruction totals; every result is checked for correctness.
#![forbid(unsafe_code)]

use blubber::{Pubkey as WireKey, Schema};
use blubber_cu_fixture::{key, schemas::*, text};
use mollusk_svm::{result::Check, Mollusk};
use mollusk_svm_bencher::MolluskComputeUnitBencher;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

#[path = "../fixtures/invocation_boundaries.rs"]
mod invocation_boundaries;
#[path = "support/lease.rs"]
mod lease;
#[path = "support/matrix.rs"]
mod matrix;
#[path = "../fixtures/runtime_report.rs"]
mod runtime_report;
#[path = "../fixtures/runtime_sequences.rs"]
mod runtime_sequences;

#[path = "support/entry_combinators.rs"]
mod entry_combinators;

#[path = "support/traversal.rs"]
mod traversal;

const OWNER: Pubkey = Pubkey::new_from_array([7; 32]);
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);
const PAYER: Pubkey = Pubkey::new_from_array([9; 32]);

fn encode<S: Schema>(init: S::Init<'_>, discriminator: [u8; 8]) -> Vec<u8> {
    let used = 12 + S::measure(&init).unwrap() as usize;
    let mut bytes = vec![0; used];
    bytes[..8].copy_from_slice(&discriminator);
    bytes[8..12].copy_from_slice(&(used as u32).to_le_bytes());
    S::encode(&init, &mut bytes[12..]);
    bytes
}

fn fixture(op: u8) -> Vec<u8> {
    match op {
        0 => encode::<Number>(NumberInit { value: 0 }, Number::DISCRIMINATOR),
        1..=3 => encode::<Text>(TextInit { value: "original" }, Text::DISCRIMINATOR),
        4 => encode::<Numbers>(
            NumbersInit {
                value: &(0..100).collect::<Vec<_>>(),
            },
            Numbers::DISCRIMINATOR,
        ),
        _ => {
            let items: Vec<_> = (1..=100)
                .map(|i| {
                    let mut bytes = [i * 2; 32];
                    if op == 8 || op == 12 {
                        bytes = [42; 32];
                        bytes[31] = i * 2;
                    }
                    (WireKey::from_bytes(bytes), u64::from(i))
                })
                .collect();
            if op < 9 {
                encode::<PublicKeyMap>(
                    PublicKeyMapInit { value: &items },
                    PublicKeyMap::DISCRIMINATOR,
                )
            } else {
                let items: Vec<_> = items
                    .into_iter()
                    .map(|(k, v)| (pinocchio::Address::new_from_array(k.to_bytes()), v))
                    .collect();
                encode::<AddressMap>(AddressMapInit { value: &items }, AddressMap::DISCRIMINATOR)
            }
        }
    }
}

fn check(op: u8, bytes: &[u8]) {
    match op {
        0 => {
            let root = Number::load(bytes).unwrap();
            assert_eq!(root.value(), 123);
        }
        1..=3 => {
            let root = Text::load(bytes).unwrap();
            assert_eq!(root.value(), text(op));
        }
        4 => {
            let root = Numbers::load(bytes).unwrap();
            assert_eq!(root.value().len(), 101);
            for i in 0..=100 {
                assert_eq!(root.value().get(i), Some(i as u64));
            }
        }
        5..=8 => {
            let root = PublicKeyMap::load(bytes).unwrap();
            assert_eq!(root.value().len(), 101);
            for i in 1..=100u8 {
                let mut bytes = [i * 2; 32];
                if op == 8 {
                    bytes = [42; 32];
                    bytes[31] = i * 2;
                }
                assert_eq!(
                    root.value().get(&WireKey::from_bytes(bytes)),
                    Some(u64::from(i))
                );
            }
            assert_eq!(
                root.value().get(&WireKey::from_bytes(key(op - 5))),
                Some(123)
            );
        }
        _ => {
            let root = AddressMap::load(bytes).unwrap();
            assert_eq!(root.value().len(), 101);
            for i in 1..=100u8 {
                let mut bytes = [i * 2; 32];
                if op == 12 {
                    bytes = [42; 32];
                    bytes[31] = i * 2;
                }
                assert_eq!(
                    root.value().get(&pinocchio::Address::new_from_array(bytes)),
                    Some(u64::from(i))
                );
            }
            assert_eq!(
                root.value()
                    .get(&pinocchio::Address::new_from_array(key(op - 9))),
                Some(123)
            );
        }
    }
}

fn account(data: Vec<u8>) -> Account {
    Account {
        lamports: 100_000_000_000,
        data,
        owner: OWNER,
        executable: false,
        rent_epoch: 0,
    }
}

fn measured_stages(messages: &[String]) -> [u64; 5] {
    let mut marker = None;
    let mut readings = std::collections::BTreeMap::new();
    for line in messages {
        if line.contains("0x4355,") {
            let value = line.split(',').nth(1).unwrap().trim();
            marker = Some(u8::from_str_radix(value.trim_start_matches("0x"), 16).unwrap());
        }
        if line.contains("Program consumption:") {
            if let Some(id) = marker.take() {
                let remaining = line
                    .split_whitespace()
                    .nth(2)
                    .unwrap()
                    .parse::<u64>()
                    .unwrap();
                assert!(readings.insert(id, remaining).is_none());
            }
        }
    }
    assert_eq!(readings.len(), 7, "complete marker inventory");
    let calibration = readings[&10] - readings[&11];
    let stage = |a, b| readings[&a] - readings[&b] - calibration;
    [
        stage(0, 1),
        stage(1, 2),
        stage(2, 3),
        stage(3, 4),
        calibration,
    ]
}

fn ceiling_store() -> Option<&'static str> {
    static STORE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    STORE
        .get_or_init(|| {
            std::env::var_os("BLUBBER_CEILINGS_PATH").map(|path| {
                std::fs::read_to_string(&path).unwrap_or_else(|error| {
                    panic!("Cannot read benchmark ceilings {path:?}: {error}")
                })
            })
        })
        .as_deref()
}

fn ceiling(name: &str) -> u64 {
    let Some(store) = ceiling_store() else {
        return u64::MAX;
    };
    let row = store.lines().skip(1).find_map(|line| {
        let mut cells = line.split('\t');
        let id = cells.next()?;
        let arch = cells.next()?;
        let value = cells.next()?;
        if id == name && arch == "all" {
            value.parse().ok()
        } else {
            None
        }
    });
    row.unwrap_or_else(|| panic!("Missing benchmark ceiling: {name}"))
}

fn main() {
    let _ = ceiling_store();
    if std::env::var_os("BLUBBER_ENTRY_COMBINATORS").is_some() {
        entry_combinators::run();
        return;
    }
    if std::env::var_os("BLUBBER_TRAVERSAL_ONLY").is_some() {
        traversal::run();
        return;
    }
    if std::env::var_os("BLUBBER_VALIDATION_ONLY").is_some() {
        matrix::run(std::env::var_os("BLUBBER_CU_METRICS").is_some());
        return;
    }
    let metrics = std::env::var_os("BLUBBER_CU_METRICS").is_some();
    let mut vm = Mollusk::default();
    vm.program_cache =
        mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
    vm.add_program(
        &OWNER,
        if metrics {
            "blubber_cu_fixture_metrics"
        } else {
            "blubber_cu_fixture"
        },
    );
    let cases = [
        "scalar",
        "string_same8",
        "string_grow32",
        "string_shrink1",
        "list100_push",
        "pubkey100_append",
        "pubkey100_middle",
        "pubkey100_front",
        "pubkey100_common_prefix",
        "address100_append",
        "address100_middle",
        "address100_front",
        "address100_common_prefix",
    ];
    if metrics {
        println!("case,open_cu,load_cu,typed_operation_cu,commit_cu,marker_calibration,instrumented_instruction_cu");
    } else {
        println!("case,whole_instruction_cu,before_len,after_len");
    }
    // Current whole-instruction caps, including E6 default headroom.
    // Exact typed maxima; rent-attributed map caps retain their existing limits.
    let mut scheduled = Vec::new();
    for (op, name) in cases.iter().enumerate() {
        let data = fixture(op as u8);
        let before = data.len();
        let ix = Instruction {
            program_id: OWNER,
            accounts: vec![AccountMeta::new(STATE, false)],
            data: vec![op as u8],
        };
        if metrics {
            vm.logger = Some(Default::default());
        }
        let accounts = vec![(STATE, account(data))];
        let result = vm.process_and_validate_instruction(&ix, &accounts, &[Check::success()]);
        let after = &result.get_account(&STATE).unwrap().data;
        check(op as u8, after);
        if metrics {
            let stages =
                measured_stages(vm.logger.as_ref().unwrap().borrow().get_recorded_content());
            assert!(
                stages[2] <= ceiling(&format!("mollusk/{name}/typed")),
                "{name} typed-operation ceiling: {} > {}",
                stages[2],
                ceiling(&format!("mollusk/{name}/typed"))
            );
            println!(
                "{name},{},{},{},{},{},{}",
                stages[0],
                stages[1],
                stages[2],
                stages[3],
                stages[4],
                result.compute_units_consumed
            );
        } else {
            println!(
                "{name},{},{before},{}",
                result.compute_units_consumed,
                after.len()
            );
            assert!(
                result.compute_units_consumed <= ceiling(&format!("mollusk/{name}/whole")),
                "{name} non-regression ceiling"
            );
        }
        scheduled.push((*name, ix, accounts));
    }
    if metrics {
        matrix::run(true);
        lease::run(true, false);
        lease::run(true, true);
        return;
    }
    let mut data = Vec::new();
    for i in 0..100u32 {
        data.extend_from_slice(&(1000 + i).to_le_bytes());
    }
    let mut instruction = vec![22];
    for word in [0u32, 4, 100] {
        instruction.extend_from_slice(&word.to_le_bytes());
    }
    instruction.extend_from_slice(&1i64.to_le_bytes());
    instruction.extend_from_slice(&u32::MAX.to_le_bytes());
    let ix = Instruction {
        program_id: OWNER,
        accounts: vec![AccountMeta::new(STATE, false)],
        data: instruction,
    };
    let accounts = vec![(STATE, account(data))];
    let result = vm.process_and_validate_instruction(&ix, &accounts, &[Check::success()]);
    let after = &result.get_account(&STATE).unwrap().data;
    assert_eq!(after.len(), 400);
    for i in 0..100usize {
        assert_eq!(
            u32::from_le_bytes(after[i * 4..i * 4 + 4].try_into().unwrap()),
            1001 + i as u32
        );
    }
    assert!(
        result.compute_units_consumed <= ceiling("mollusk/patch100/whole"),
        "Patch100 both-pass kernel ceiling: {}",
        result.compute_units_consumed
    );
    println!("patch100,{},400,400", result.compute_units_consumed);
    scheduled.push(("patch100", ix, accounts));
    // Bencher success checks accompany the byte/value assertions above.
    let output = std::env::var("BLUBBER_BENCH_OUT")
        .unwrap_or_else(|_| format!("{}/../../.tmp/cu-bencher", env!("CARGO_MANIFEST_DIR")));
    for (op, name, ceiling) in [
        (
            0,
            "validated_scalar",
            ceiling("mollusk/validated_scalar/whole"),
        ),
        (
            1,
            "validated_string_same8",
            ceiling("mollusk/validated_string_same8/whole"),
        ),
    ] {
        let data = fixture(op);
        let before = data.len();
        let ix = Instruction {
            program_id: OWNER,
            accounts: vec![AccountMeta::new(STATE, false)],
            data: vec![30 + op],
        };
        let result = vm.process_and_validate_instruction(
            &ix,
            &[(STATE, account(data))],
            &[Check::success()],
        );
        let after = &result.get_account(&STATE).unwrap().data;
        check(op, after);
        assert!(
            result.compute_units_consumed <= ceiling,
            "{name} validated ingress ceiling"
        );
        println!(
            "{name},{},{before},{}",
            result.compute_units_consumed,
            after.len()
        );
    }
    let mut bencher = MolluskComputeUnitBencher::new(vm)
        .must_pass(true)
        .out_dir(&format!("{output}/fixtures"));
    for (name, ix, accounts) in &scheduled {
        bencher = bencher.bench((name, ix, accounts));
    }
    bencher.execute();

    lifecycle();

    let mut tour = Mollusk::default();
    tour.program_cache =
        mollusk_svm::program::ProgramCache::new(&tour.feature_set, &tour.compute_budget, false);
    tour.add_program(&OWNER, "blubber_demo");
    let init = blubber_demo::state::seed(WireKey::from_bytes(PAYER.to_bytes()));
    let data = encode::<blubber_demo::state::MyAccount>(
        init,
        blubber_demo::state::MyAccount::DISCRIMINATOR,
    );
    let before = data.len();
    let accounts = [
        (STATE, account(data)),
        (PAYER, Account::new(100_000_000_000, 0, &Pubkey::default())),
        mollusk_svm::program::keyed_account_for_system_program(),
    ];
    let ix = Instruction {
        program_id: OWNER,
        accounts: vec![
            AccountMeta::new(STATE, false),
            AccountMeta::new(PAYER, true),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
        data: vec![2],
    };
    let result = tour.process_and_validate_instruction(&ix, &accounts, &[Check::success()]);
    let after = &result.get_account(&STATE).unwrap().data;
    let root = blubber_demo::state::MyAccount::load(after).unwrap();
    assert_eq!(root.counter(), 1);
    println!(
        "tour,{},{before},{}",
        result.compute_units_consumed,
        after.len()
    );
    assert!(
        result.compute_units_consumed <= ceiling("mollusk/tour/whole"),
        "tour instruction ceiling"
    );
    matrix::run(false);
    matrix::run_extended();
    for profile in 0..3 {
        let mut vm = Mollusk::default();
        vm.feature_set.virtual_address_space_adjustments = profile != 0;
        vm.feature_set.account_data_direct_mapping = profile == 2;
        vm.program_cache =
            mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
        vm.add_program(&OWNER, "blubber_cu_matrix");
        runtime_sequences::depth_eight_physical_grow_shrink_regrow_preserves_untouched_branches(
            &vm, profile, true,
        );
        runtime_sequences::all_scalar_codecs_and_fixed_options_match_independent_encoding(
            &vm, profile, true,
        );
    }
    invocation_boundaries::fresh_top_level_windows_accumulate_in_one_transaction(true);
    invocation_boundaries::cpi_return_obeys_the_callers_original_window(true);
    invocation_boundaries::later_instruction_failure_reverts_commits_and_system_transfers(true);
    invocation_boundaries::prepaid_requires_funding_before_commit_even_when_bank_rent_is_deferred(
        true,
    );
    invocation_boundaries::signed_aggregate_growth_is_twenty_mib_and_shrink_restores_budget(true);
    invocation_boundaries::duplicate_metas_merge_privileges_and_share_genuine_guard_exclusion(true);
    lease::run(false, false);
    lease::run(false, true);
    MolluskComputeUnitBencher::new(tour)
        .bench(("tour", &ix, &accounts))
        .must_pass(true)
        .out_dir(&format!("{output}/tour"))
        .execute();
}

// Raw lease instructions isolate resize/funding from typed operation costs.
// Rent-only measurements use the same result checks.
// Their SBF dispatcher asserts close/release, receipts and actual physical size.
fn lifecycle() {
    const INITIAL: usize = 128;
    const RECIPIENT: Pubkey = Pubkey::new_from_array([10; 32]);
    let mut vm = Mollusk::default();
    vm.add_program(&OWNER, "blubber_borrow_resize");
    for (name, used, reserve, top_up, refund, ceiling) in [
        (
            "lease_prepaid_same",
            INITIAL,
            0u32,
            false,
            false,
            ceiling("mollusk/lease_prepaid_same/whole"),
        ),
        (
            "lease_topup_grow200",
            INITIAL + 200,
            0,
            true,
            false,
            ceiling("mollusk/lease_topup_grow200/whole"),
        ),
        (
            "lease_refund_shrink64",
            64,
            0,
            false,
            true,
            ceiling("mollusk/lease_refund_shrink64/whole"),
        ),
        (
            "lease_retained32",
            INITIAL,
            32,
            true,
            false,
            ceiling("mollusk/lease_retained32/whole"),
        ),
    ] {
        let mut data = vec![0; INITIAL];
        data[..8].copy_from_slice(b"LEASE001");
        data[8..12].copy_from_slice(&(INITIAL as u32).to_le_bytes());
        let mut state = account(data);
        state.lamports = vm.sysvars.rent.minimum_balance(INITIAL);
        let recipient = Account::new(vm.sysvars.rent.minimum_balance(0), 0, &Pubkey::default());
        let mut ix_data = vec![40, 0, u8::from(top_up), u8::from(refund), 0];
        ix_data.extend_from_slice(&(used as u32).to_le_bytes());
        ix_data.extend_from_slice(&reserve.to_le_bytes());
        ix_data.push(0);
        let ix = Instruction {
            program_id: OWNER,
            accounts: vec![
                AccountMeta::new(STATE, false),
                AccountMeta::new(PAYER, true),
                AccountMeta::new_readonly(Pubkey::default(), false),
                AccountMeta::new(RECIPIENT, false),
            ],
            data: ix_data,
        };
        let final_len = used + reserve as usize;
        let before_balance = state.lamports;
        let final_minimum = vm.sysvars.rent.minimum_balance(final_len);
        let topped_up = final_minimum.saturating_sub(before_balance);
        let refunded = if refund {
            before_balance.saturating_sub(final_minimum)
        } else {
            0
        };
        let result = vm.process_and_validate_instruction(
            &ix,
            &[
                (STATE, state),
                (PAYER, Account::new(100_000_000_000, 0, &Pubkey::default())),
                mollusk_svm::program::keyed_account_for_system_program(),
                (RECIPIENT, recipient.clone()),
            ],
            &[
                Check::success(),
                Check::inner_instruction_count(usize::from(topped_up != 0)),
            ],
        );
        let state = result.get_account(&STATE).unwrap();
        assert_eq!(state.data.len(), final_len);
        assert_eq!(state.data[12], 42);
        assert_eq!(state.lamports, before_balance + topped_up - refunded);
        assert_eq!(
            result.get_account(&PAYER).unwrap().lamports,
            100_000_000_000 - topped_up
        );
        assert_eq!(
            result.get_account(&RECIPIENT).unwrap().lamports,
            recipient.lamports + refunded
        );
        assert!(
            result.compute_units_consumed <= ceiling,
            "{name} lifecycle ceiling"
        );
        println!(
            "{name},{},{INITIAL},{final_len}",
            result.compute_units_consumed
        );
    }
}
