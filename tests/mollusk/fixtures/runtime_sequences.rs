//! Step-40 sequence and scalar fixtures shared by tests and measurements.
#![forbid(unsafe_code)]
use super::runtime_report::record;
use blubber_mollusk::matrix::Case;
use mollusk_svm::{result::Check, Mollusk};
use solana_account::Account;
use solana_pubkey::Pubkey;
const OWNER: Pubkey = Pubkey::new_from_array([7; 32]);
const STATE: Pubkey = Pubkey::new_from_array([8; 32]);

pub(crate) fn depth_eight_physical_grow_shrink_regrow_preserves_untouched_branches(
    vm: &Mollusk,
    profile: u8,
    emit: bool,
) {
    let mut case = Case {
        name: "depth8 sequence".into(),
        layout: 1,
        n: 1,
        op: 5,
        index: 0,
        prefix: false,
        opcode: 26,
        depth: 8,
    };
    let (schema, mut value, disc) = case.model();
    let data = schema
        .encode_account(disc, &value, 12 + schema.encode(&value).unwrap().len())
        .unwrap();
    let mut accounts = vec![(
        STATE,
        Account {
            lamports: 100_000_000_000,
            data,
            owner: OWNER,
            ..Account::default()
        },
    )];
    for (ordinal, op) in [5, 6, 22, 24, 5].into_iter().enumerate() {
        case.op = op;
        value = case.expected(value);
        let expected = schema
            .encode_account(disc, &value, 12 + schema.encode(&value).unwrap().len())
            .unwrap();
        let result = vm.process_and_validate_instruction(
            &case.instruction(op),
            &accounts,
            &[Check::success()],
        );
        assert_eq!(result.get_account(&STATE).unwrap().data, expected);
        record(
            emit,
            (
                &format!("depth8_sequence_{ordinal}_op{op}"),
                profile,
                "instruction",
            ),
            result.compute_units_consumed,
            &accounts,
            &result.resulting_accounts,
        );
        accounts = result.resulting_accounts;
    }
}

pub(crate) fn all_scalar_codecs_and_fixed_options_match_independent_encoding(
    vm: &Mollusk,
    profile: u8,
    emit: bool,
) {
    use blubber_cu_fixture::schemas::ScalarKinds;
    use blubber_model::{IntWidth, Schema, Value};
    use solana_instruction::{AccountMeta, Instruction};
    let widths = [
        IntWidth::W1,
        IntWidth::W2,
        IntWidth::W4,
        IntWidth::W8,
        IntWidth::W16,
    ];
    let mut fields: Vec<_> = widths.iter().copied().map(Schema::Unsigned).collect();
    fields.extend(widths.iter().copied().map(Schema::Signed));
    fields.extend([
        Schema::Bool,
        Schema::Array(17),
        Schema::Array(32),
        Schema::Array(32),
    ]);
    let schema = Schema::Struct(vec![
        Schema::Struct(fields),
        Schema::Option(Box::new(Schema::Unsigned(IntWidth::W4))),
    ]);
    let mut before = vec![Value::Unsigned(0); 5];
    before.extend(vec![Value::Signed(0); 5]);
    before.extend([
        Value::Bool(false),
        Value::Array(vec![0; 17]),
        Value::Array(vec![0; 32]),
        Value::Array(vec![0; 32]),
    ]);
    let mut after: Vec<_> = [
        u8::MAX as u128,
        u16::MAX as u128,
        u32::MAX as u128,
        u64::MAX as u128,
        u128::MAX,
    ]
    .into_iter()
    .map(Value::Unsigned)
    .collect();
    after.extend(
        [
            i8::MIN as i128,
            i16::MIN as i128,
            i32::MIN as i128,
            i64::MIN as i128,
            i128::MIN,
        ]
        .into_iter()
        .map(Value::Signed),
    );
    after.extend([
        Value::Bool(true),
        Value::Array(vec![0xa5; 17]),
        Value::Array(vec![0x31; 32]),
        Value::Array(vec![0x32; 32]),
    ]);
    let before = Value::Struct(vec![
        Value::Struct(before),
        Value::Option(Some(Box::new(Value::Unsigned(1)))),
    ]);
    let after = Value::Struct(vec![
        Value::Struct(after),
        Value::Option(Some(Box::new(Value::Unsigned(u32::MAX as u128)))),
    ]);
    let len = 12 + schema.encode(&before).unwrap().len();
    let data = schema
        .encode_account(ScalarKinds::DISCRIMINATOR, &before, len)
        .unwrap();
    let expected = schema
        .encode_account(ScalarKinds::DISCRIMINATOR, &after, len)
        .unwrap();
    let accounts = vec![(
        STATE,
        Account {
            lamports: 100_000_000_000,
            data: data.clone(),
            owner: OWNER,
            ..Account::default()
        },
    )];
    let result = vm.process_and_validate_instruction(
        &Instruction {
            program_id: OWNER,
            accounts: vec![AccountMeta::new(STATE, false)],
            data: vec![29],
        },
        &accounts,
        &[Check::success()],
    );
    assert_eq!(result.get_account(&STATE).unwrap().data, expected);
    record(
        emit,
        ("scalar_kinds", profile, "instruction"),
        result.compute_units_consumed,
        &accounts,
        &result.resulting_accounts,
    );
}
