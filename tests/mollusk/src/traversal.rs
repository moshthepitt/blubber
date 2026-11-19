//! fixture inventory and independent logical/observable result oracles.
use crate::matrix::Case;
use blubber_model::Value;

/// One isolated method measurement and both matching idle operations.
#[derive(Clone, Debug)]
pub struct TraversalCase {
    /// Fixed/indexed list/map wire layout.
    pub layout: u8,
    /// Exact collection count, including setup cases zero and one.
    pub n: usize,
    /// One public method or callback operation.
    pub op: u8,
    /// Keys share a long prefix (adjacent keys share 31 bytes).
    pub common: bool,
}
/// Stable, separately dispatched observations of current APIs.
pub const OPS: &[(u8, &str)] = &[
    (0, "ref_common_idle"),
    (1, "ref_admitted_idle"),
    (2, "ref_len"),
    (3, "ref_is_empty"),
    (4, "ref_get_lookup"),
    (5, "ref_try_get_lookup"),
    (6, "ref_iter"),
    (7, "ref_try_iter"),
    (8, "ref_validate"),
    (9, "selected_to_owned"),
    (10, "root_to_owned"),
    (11, "ref_iter_admitted_idle"),
    (12, "ref_try_iter_admitted_idle"),
    (17, "trusted_ref_admitted_idle"),
    (39, "cursor_admitted_idle"),
    (43, "mut_try_iter_admitted_idle"),
    (49, "mut_iter_admitted_idle"),
    (13, "ref_try_len"),
    (14, "ref_try_is_empty"),
    (20, "mut_common_idle"),
    (21, "mut_admitted_idle"),
    (22, "mut_len"),
    (23, "mut_is_empty"),
    (24, "mut_get_lookup"),
    (25, "mut_try_get_lookup"),
    (26, "mut_iter"),
    (27, "mut_try_iter"),
    (28, "mut_validate"),
    (29, "cursor_next"),
    (30, "cursor_get"),
    (31, "cursor_set"),
    (32, "empty_callback"),
    (33, "callback_get"),
    (34, "callback_equal_set"),
    (35, "callback_growing_set"),
    (38, "old_dependent_callback"),
    (40, "trusted_ref_idle"),
    (41, "trusted_ref_get"),
    (44, "mut_try_len"),
    (45, "mut_try_is_empty"),
    (50, "measure_str"),
    (51, "measure_bytes"),
    (52, "measure_derived"),
    (53, "source_contract"),
    (54, "source_contract_idle"),
    (55, "slots_common_idle"),
    (56, "slots_admitted_idle"),
    (57, "slots_cursor_get"),
    (58, "slots_cursor_set"),
    (59, "slots_empty_callback"),
    (60, "slots_callback_get"),
    (61, "slots_callback_set"),
    (62, "kernel_rewrite"),
    (63, "fill"),
    (64, "rewrite"),
    (66, "rewrite_values"),
    (71, "map_grow_from_old_values"),
    (65, "fill_then_rewrite"),
];

/// Builds every required layout/count/method and long-prefix map row.
pub fn inventory() -> Vec<TraversalCase> {
    let mut cases = Vec::new();
    for layout in 0..4 {
        for common in [false, true] {
            if layout < 2 && common {
                continue;
            }
            for n in [0, 1, 10, 100, 1000] {
                for &(op, _) in OPS {
                    if op >= 53 {
                        continue;
                    }
                    if op >= 50 && (layout != 0 || common) {
                        continue;
                    }
                    cases.push(TraversalCase {
                        layout,
                        n,
                        op,
                        common,
                    });
                }
            }
        }
    }
    cases
}
/// Shared-source representation cases, run against their own SBF artifact.
pub fn source_inventory() -> Vec<TraversalCase> {
    [0, 1, 10, 100, 1000]
        .into_iter()
        .flat_map(|n| {
            (53..=61).map(move |op| TraversalCase {
                layout: 0,
                n,
                op,
                common: false,
            })
        })
        .collect()
}
/// Low-level Q8 rows; public API rows keep their Q1 inventory and artifact.
pub fn rewrite_inventory() -> Vec<TraversalCase> {
    [1, 3]
        .into_iter()
        .flat_map(|layout| {
            [false, true]
                .into_iter()
                .filter(move |common| layout == 3 || !common)
                .flat_map(move |common| {
                    [0, 1, 10, 100, 1000]
                        .into_iter()
                        .map(move |n| TraversalCase {
                            layout,
                            n,
                            op: 62,
                            common,
                        })
                })
        })
        .collect()
}

/// Q9 public fill and rewrite rows, each with matching preflight and common idles.
pub fn rewrite_api_inventory() -> Vec<TraversalCase> {
    rewrite_inventory()
        .into_iter()
        .flat_map(|case| {
            let ops: &[u8] = if case.layout == 3 {
                &[63, 64, 66]
            } else {
                &[63, 64]
            };
            ops.iter()
                .map(move |&op| TraversalCase { op, ..case.clone() })
        })
        .collect()
}

/// All five methods share one compiled caller in the frame contract.
pub fn rewrite_contract_inventory() -> Vec<TraversalCase> {
    rewrite_inventory()
        .into_iter()
        .map(|case| TraversalCase { op: 65, ..case })
        .collect()
}
impl TraversalCase {
    /// Stable row key, independent of measured CU.
    pub fn name(&self) -> String {
        format!(
            "{}_n{}_{}_{}",
            ["list_fixed", "list_indexed", "map_fixed", "map_indexed"][self.layout as usize],
            self.n,
            OPS.iter().find(|x| x.0 == self.op).unwrap().1,
            if self.common {
                "common31_adjacent"
            } else {
                "plain"
            }
        )
    }
    /// Existing independent reference codec fixture, with no production encoding.
    pub fn model_case(&self) -> Case {
        Case {
            name: self.name(),
            layout: self.layout,
            n: self.n,
            op: 4,
            index: 0,
            prefix: self.common,
            opcode: 23,
            depth: 3,
        }
    }
    /// Exact instruction bytes for this case or one of its idles.
    pub fn instruction_data(&self, op: u8) -> Vec<u8> {
        let [lo, hi] = (self.n as u16).to_le_bytes();
        vec![self.layout, op, lo, hi, u8::from(self.common)]
    }
    /// Common-ingress idle with identical load/storage/commit/return publication.
    pub fn common_idle(&self) -> u8 {
        if self.op >= 62 {
            20
        } else if self.op >= 55 {
            55
        } else if matches!(self.op, 53 | 54) {
            54
        } else if self.op >= 50 {
            0
        } else if matches!(self.op, 17 | 40 | 41) {
            40
        } else if self.op >= 20 {
            20
        } else {
            0
        }
    }
    /// Same collection projection as the operation, retaining its admission cost.
    pub fn admitted_idle(&self) -> u8 {
        match self.op {
            62 => 21,
            63 => 21,
            64 => 22,
            66 => 23,
            65 => 22,
            56..=61 => 56,
            55 => 55,
            0 | 10 | 20 | 40 | 50..=54 => self.common_idle(),
            6 | 11 => 11,
            7 | 12 => 12,
            17 | 41 => 17,
            26 | 49 => 49,
            27 | 43 => 43,
            29..=35 | 38 | 39 => 39,
            _ if self.common_idle() == 20 => 21,
            _ => 1,
        }
    }

    /// Applies only the logical public edit, independently of production helpers.
    pub fn expected(&self, mut value: Value) -> Value {
        let Value::Struct(fields) = &mut value else {
            panic!("fixture struct");
        };
        let change = |i: usize, value: &mut Value| {
            if !matches!(self.op, 31 | 34 | 35 | 38 | 58 | 61 | 62..=66 | 71) {
                return;
            }
            if self.op == 71 {
                let Value::Str(old) = value else {
                    panic!("recipe string");
                };
                *value = Value::Str(format!("{old}{old}!"));
                return;
            }
            *value = if self.layout.is_multiple_of(2) {
                Value::Unsigned(if self.op == 38 { 18 } else { 123 })
            } else {
                Value::Str(if matches!(self.op, 64..=66) {
                    "abcdefghijklmnopq"[..i % 18].to_owned()
                } else if self.op == 31 || self.op == 34 {
                    "sameSIZE".to_owned()
                } else {
                    "replacement grows".to_owned()
                })
            };
        };
        match &mut fields[0] {
            Value::List(items) => {
                for (i, x) in items.iter_mut().enumerate() {
                    change(i, x)
                }
            }
            Value::Map(items) => {
                for (i, x) in items.values_mut().enumerate() {
                    change(i, x)
                }
            }
            _ => panic!("fixture collection"),
        }
        value
    }
    /// Observable scalar outside the VM; map iteration checks all copied key bytes.
    pub fn read_result(&self, op: u8) -> u64 {
        if self.op >= 62 && op != self.op {
            return 0;
        }
        let n = self.n as u64;
        let leaf = if self.layout.is_multiple_of(2) {
            17
        } else {
            "original".bytes().map(u64::from).sum()
        };
        let key_sum = |ordinal: usize| {
            let mut key = [42u8; 32];
            let at = if self.common { 28 } else { 0 };
            key[at..at + 4].copy_from_slice(&(2 * (ordinal + 1) as u32).to_be_bytes());
            key.into_iter().map(u64::from).sum::<u64>()
        };
        match op {
            53 | 57 | 60 => n * 17,
            2 | 13 | 22 | 44 => n,
            3 | 14 | 23 | 45 => u64::from(n == 0),
            6 | 7 | 9 | 10 | 26 | 27 | 30 | 33 => {
                n * leaf
                    + if self.layout >= 2 {
                        (0..self.n).map(key_sum).sum()
                    } else {
                        0
                    }
            }
            4 | 5 | 24 | 25 | 41 if n != 0 => leaf,
            50 => n * 8,
            51 => n * 12,
            52 => n * 68,
            _ => 0,
        }
    }
}
