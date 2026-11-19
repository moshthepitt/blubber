//! Correctness-checked provisional scalar gates, whole instructions.
#![forbid(unsafe_code)]
#[allow(dead_code)]
mod support;
use support::*;

#[test]
fn matched_scalar_budgets_all_profiles() {
    let mut rows = Vec::new();
    for profile in 0..3 {
        let mut native = Case::new(profile, true);
        let read = native.run(16, &7u64.to_le_bytes(), read_metas(), None);
        let update = native.run(17, &11u64.to_le_bytes(), edit_metas(), None);
        native.assert_counter(11);
        let mut baseline = Case::new(profile, true);
        // Account<T> is the native fixed layout: discriminator + 40-byte header.
        let data = &mut baseline.state_mut().data;
        data[..8].copy_from_slice(&[253, 82, 53, 107, 104, 82, 37, 77]);
        data[8..40].copy_from_slice(&PAYER.to_bytes());
        data[40..48].copy_from_slice(&7u64.to_le_bytes());
        let baseline_read = baseline.run(18, &7u64.to_le_bytes(), read_metas(), None);
        let baseline_update = baseline.run(19, &11u64.to_le_bytes(), edit_metas(), None);
        assert_eq!(
            u64::from_le_bytes(baseline.state().data[40..48].try_into().unwrap()),
            11
        );
        rows.push(format!(
            "{profile},{read},{baseline_read},{update},{baseline_update}"
        ));
        // Retain every measurement before enforcing ceilings.
    }
    let path = std::env::var("BLUBBER_ANCHOR_BENCH_ROWS").expect("run bench lane");
    std::fs::write(path, rows.join("\n") + "\n").unwrap();
    for row in rows {
        let n: Vec<u64> = row.split(',').map(|n| n.parse().unwrap()).collect();
        assert!(n[1] <= n[2] + 1200, "read {row}");
        assert!(n[3] <= n[4] + 1500, "update {row}");
    }
}
