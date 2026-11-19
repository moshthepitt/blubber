//! Remaining public methods use the bencher's independent full-byte oracle.
#![forbid(unsafe_code)]

use blubber_mollusk::matrix::Case;
use mollusk_svm::{result::Check, Mollusk};
use solana_account::Account;
use solana_pubkey::Pubkey;

const OWNER: Pubkey = Pubkey::new_from_array([7; 32]);
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);

fn runtime(profile: u8) -> Mollusk {
    let mut vm = Mollusk::default();
    vm.feature_set.virtual_address_space_adjustments = profile != 0;
    vm.feature_set.account_data_direct_mapping = profile == 2;
    vm.program_cache =
        mollusk_svm::program::ProgramCache::new(&vm.feature_set, &vm.compute_budget, false);
    vm.add_program(
        &OWNER,
        match (
            std::env::var_os("BLUBBER_RUNTIME_CU").is_some(),
            cfg!(feature = "alloc"),
        ) {
            (true, true) => "blubber_cu_matrix_alloc",
            (true, false) => "blubber_cu_matrix",
            (false, true) => "blubber_cu_runtime_alloc",
            (false, false) => "blubber_cu_runtime",
        },
    );
    vm
}

fn execute(vm: &Mollusk, case: &Case) {
    let (schema, value, disc) = case.model();
    let data = schema
        .encode_account(disc, &value, 12 + schema.encode(&value).unwrap().len())
        .unwrap();
    let value = case.expected(value);
    let expected = schema
        .encode_account(disc, &value, 12 + schema.encode(&value).unwrap().len())
        .unwrap();
    let accounts = [(
        STATE,
        Account {
            lamports: 100_000_000_000,
            data,
            owner: OWNER,
            ..Account::default()
        },
    )];
    let result = vm.process_and_validate_instruction(
        &case.instruction(case.op),
        &accounts,
        &[Check::success(), Check::inner_instruction_count(0)],
    );
    assert_eq!(
        result.get_account(&STATE).unwrap().data,
        expected,
        "{}",
        case.name
    );
    assert_eq!(schema.decode_account(disc, &expected).unwrap().value, value);
}

#[test]
fn remaining_collection_methods_match_the_shared_oracle() {
    for profile in 0..3 {
        let vm = runtime(profile);
        for layout in 0..6 {
            for n in [1, 2, 10] {
                for index in [0, n / 2, n - 1] {
                    let ops: &[u8] = if layout < 2 {
                        &[27, 28, 33, 34, 35, 36, 42]
                    } else {
                        &[27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 42]
                    };
                    for &op in ops {
                        for prefix in [false, true] {
                            if layout < 2 && prefix {
                                continue;
                            }
                            execute(&vm, &Case { name: format!("profile{profile}/layout{layout}/n{n}/i{index}/op{op}/prefix{prefix}"), layout, n, op, index: if layout < 2 && op == 35 { n } else { index }, prefix, opcode: 23, depth: 3 });
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn empty_list_transitions_and_errors_preserve_the_encoding() {
    for profile in 0..3 {
        let vm = runtime(profile);
        for layout in 0..2 {
            for op in [9, 13, 14, 15, 16, 17, 28, 33, 34, 35] {
                execute(
                    &vm,
                    &Case {
                        name: format!("empty/{profile}/{layout}/{op}"),
                        layout,
                        n: 0,
                        op,
                        index: 0,
                        prefix: false,
                        opcode: 23,
                        depth: 3,
                    },
                );
            }
        }
    }
}

#[cfg(feature = "alloc")]
#[test]
fn owned_list_and_map_methods_return_and_preserve_complete_values() {
    for profile in 0..3 {
        let vm = runtime(profile);
        for layout in 0..6 {
            for n in [1, 2, 10] {
                for &op in if layout < 2 {
                    &[37, 38, 39, 40, 41, 43][..]
                } else {
                    &[37, 38, 43, 47, 48][..]
                } {
                    execute(
                        &vm,
                        &Case {
                            name: format!("owned/{profile}/{layout}/{n}/{op}"),
                            layout,
                            n,
                            op,
                            index: n / 2,
                            prefix: true,
                            opcode: 23,
                            depth: 3,
                        },
                    );
                }
            }
        }
        for layout in 0..2 {
            execute(
                &vm,
                &Case {
                    name: "empty owned pop".into(),
                    layout,
                    n: 0,
                    op: 39,
                    index: 0,
                    prefix: false,
                    opcode: 23,
                    depth: 3,
                },
            );
        }
    }
}

#[test]
fn option_consuming_projections_handle_present_and_absent_values() {
    for profile in 0..3 {
        let vm = runtime(profile);
        for op in [25, 26, 44, 45, 46] {
            execute(
                &vm,
                &Case {
                    name: format!("option/{profile}/{op}"),
                    layout: 1,
                    n: 1,
                    op,
                    index: 0,
                    prefix: false,
                    opcode: 27,
                    depth: 3,
                },
            );
        }
    }
}

#[path = "../fixtures/runtime_report.rs"]
mod runtime_report;
#[path = "../fixtures/runtime_sequences.rs"]
mod sequences;

#[test]
fn depth_eight_physical_grow_shrink_regrow_preserves_untouched_branches() {
    for profile in 0..3 {
        sequences::depth_eight_physical_grow_shrink_regrow_preserves_untouched_branches(
            &runtime(profile),
            profile,
            false,
        );
    }
}

#[test]
fn all_scalar_codecs_and_fixed_options_match_independent_encoding() {
    for profile in 0..3 {
        sequences::all_scalar_codecs_and_fixed_options_match_independent_encoding(
            &runtime(profile),
            profile,
            false,
        );
    }
}
