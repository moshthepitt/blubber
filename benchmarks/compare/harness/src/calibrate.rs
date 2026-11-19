//! Replay published blubber cases in this same Mollusk executable.
use super::*;
use blubber_mollusk::traversal::TraversalCase;
pub fn run() {
    let arch = std::env::var("COMPARE_ARCH").unwrap();
    let root = std::env::var("BLUBBER_ROOT").unwrap();
    let published =
        std::fs::read_to_string(format!("{root}/benchmarks/traversal-mixed-{arch}.csv")).unwrap();
    let owner = Pubkey::new_from_array([7; 32]);
    let state = Pubkey::new_from_array([8; 32]);
    let mut vm = Mollusk::new(&owner, "blubber_cu_fixture");
    vm.compute_budget.compute_unit_limit = 40_000_000;
    vm.compute_budget.heap_size = 256 * 1024;
    let mut rows = Vec::new();
    for (layout, op) in [(0, 6), (0, 34), (2, 6), (2, 34), (1, 63), (3, 63), (3, 71)] {
        let case = TraversalCase {
            layout,
            n: 1000,
            op,
            common: false,
        };
        let (schema, mut value, disc) = case.model_case().model();
        if op == 71 {
            let Value::Struct(fields) = &mut value else {
                unreachable!()
            };
            let Value::Map(map) = &mut fields[0] else {
                unreachable!()
            };
            for (i, s) in map.values_mut().enumerate() {
                *s = Value::Str(format!("{i:08x}"));
            }
        }
        let before = schema
            .encode_account(disc, &value, 12 + schema.encode(&value).unwrap().len())
            .unwrap();
        let expected = case.expected(value);
        let wanted = schema
            .encode_account(
                disc,
                &expected,
                12 + schema.encode(&expected).unwrap().len(),
            )
            .unwrap();
        let ix = Instruction {
            program_id: owner,
            accounts: vec![AccountMeta::new(state, false)],
            data: case.instruction_data(op),
        };
        let account = Account {
            lamports: 100_000_000_000,
            data: before,
            owner,
            executable: false,
            rent_epoch: 0,
        };
        let result = vm.process_and_validate_instruction(
            &ix,
            &[(state, account)],
            &[Check::success(), Check::inner_instruction_count(0)],
        );
        assert_eq!(
            result.get_account(&state).unwrap().data,
            wanted,
            "calibration full bytes"
        );
        assert_eq!(result.return_data, case.read_result(op).to_le_bytes());
        let name = case.name();
        let cells: Vec<&str> = published
            .lines()
            .find(|line| line.split(',').next() == Some(&name))
            .unwrap()
            .split(',')
            .collect();
        let old = cells[6].parse::<u64>().unwrap();
        let observed = result.compute_units_consumed;
        let difference = old.abs_diff(observed) as f64 / old as f64;
        assert!(
            difference <= 0.01,
            "published CU mismatch {name}: {old} -> {observed}"
        );
        rows.push(json!({"arch":arch,"case":name,"published_cu":old,"observed_cu":observed,"relative_error":difference}));
    }
    std::fs::write(
        std::env::var("COMPARE_OUT").unwrap(),
        serde_json::to_string_pretty(&rows).unwrap() + "\n",
    )
    .unwrap();
}
