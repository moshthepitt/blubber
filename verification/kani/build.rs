//! Generate bounded edit geometries with explicit capacity boundary cases.

#![forbid(unsafe_code)]

use std::{env, fs, path::PathBuf};

fn capacities(old_used: usize, new_used: usize) -> Vec<usize> {
    let required = old_used.max(new_used);
    let mut cases = vec![old_used, required.saturating_sub(1), required, 48];
    cases.retain(|&capacity| (old_used..=48).contains(&capacity));
    cases.sort_unstable();
    cases.dedup();
    cases
}

fn emit(code: &mut String, name: &str, call: &str, unwind: usize) {
    code.push_str(&format!(
        "#[kani::proof]\n#[kani::unwind({unwind})]\n#[forbid(unsafe_code)]\nfn {name}() {{ {call} }}\n"
    ));
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let mut code = String::new();
    for fixed in [true, false] {
        let kind = if fixed { "fixed" } else { "record" };
        for count in 0..=2 {
            emit(
                &mut code,
                &format!("record_admit_{kind}_n{count}"),
                &format!("admission::<{fixed}, {count}>();"),
                49,
            );
        }
    }
    for count in 0..=2 {
        for insert in [true, false] {
            let group = if insert {
                "fixed_insert"
            } else {
                "fixed_remove"
            };
            for index in 0..count + usize::from(insert) {
                let new_count = if insert { count + 1 } else { count - 1 };
                for capacity in capacities(16 + 4 * count, 16 + 4 * new_count) {
                    emit(
                        &mut code,
                        &format!("record_{group}_n{count}_i{index}_c{capacity}"),
                        &format!("fixed_case::<{count}, {index}, {capacity}, {insert}>();"),
                        49,
                    );
                }
            }
        }
    }
    fs::write(out.join("record_cases.rs"), code).expect("write generated harnesses");
    let mut typed = String::new();
    for cap in [0, 1, 2, 8, 32, 64] {
        emit(
            &mut typed,
            &format!("typed_plan_admit_c{cap}"),
            &format!("admission::<{cap}>();"),
            65,
        );
    }
    for (n, key, op) in [
        (0, 2, 0),
        (1, 1, 0),
        (1, 2, 0),
        (1, 3, 0),
        (2, 3, 0),
        (1, 2, 1),
        (2, 2, 1),
        (2, 3, 1),
        (2, 1, 1),
        (2, 5, 1),
        (2, 4, 1),
        (2, 0, 2),
    ] {
        emit(
            &mut typed,
            &format!("typed_plan_fixed_n{n}_k{key}_o{op}"),
            &format!("fixed::<{n}, {key}, {op}>();"),
            9,
        );
    }
    fs::write(out.join("typed_plan_cases.rs"), typed).expect("write typed plan harnesses");
}
