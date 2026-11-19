//! Bounded operation streams against native trees and arbitrary initial bytes.

use super::{
    account,
    bridge::{Bridge, Observed},
    oracle, ID,
};
use blubber::raw::{Edit, Resize};
use blubber::Resize as _;
use blubber::{Error, ErrorKind, ReadStorage, Schema, SliceStore, VecStore, View};
use blubber_core::raw::{InlinePath, Step};
use blubber_model::{Edit as ModelEdit, Error as ModelError, Model, PathSegment as Segment, Value};

const MAX_STORAGE: usize = 8192;

fn apply<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    path: &[Segment],
    edit: &ModelEdit,
) -> blubber::Result<Observed> {
    S::apply(
        blubber::raw::project::<S>(Edit::load_trusted::<S>(store, ID)?),
        path,
        edit,
    )
}

pub(super) fn copied_aggregate(schema: &blubber_model::Schema) -> bool {
    use blubber_model::{IntWidth, Schema};
    // Independent fixture contract for Fixed Packed, including its native return.
    schema
        == &Schema::Struct(vec![
            Schema::Bool,
            Schema::Unsigned(IntWidth::W2),
            Schema::Array(3),
        ])
}

fn expected(
    schema: &blubber_model::Schema,
    path: &[Segment],
    edit: &ModelEdit,
    old: Option<Value>,
) -> Observed {
    match edit {
        ModelEdit::ListRemove(_) | ModelEdit::ListPop | ModelEdit::MapRemove(_) => {
            Observed::Old(old)
        }
        ModelEdit::MapInsert { .. } => {
            use blubber_model::Schema;
            let mut selected = schema;
            for segment in path {
                selected = match (selected, segment) {
                    (Schema::Struct(fields), Segment::Field(i)) => &fields[*i],
                    (Schema::List(child) | Schema::Option(child) | Schema::Map(_, child), _) => {
                        child
                    }
                    _ => panic!("map schema path"),
                };
            }
            let Schema::Map(_, child) = selected else {
                panic!("map insertion schema")
            };
            if matches!(
                child.as_ref(),
                Schema::Unsigned(_)
                    | Schema::Signed(_)
                    | Schema::Bool
                    | Schema::Pubkey
                    | Schema::Array(_)
            ) || copied_aggregate(child)
            {
                Observed::Old(old)
            } else {
                Observed::Present(old.is_some())
            }
        }
        _ => Observed::Unit,
    }
}

pub(super) fn error_matches(expected: ModelError, actual: Error) -> bool {
    let Error::Fault { kind, .. } = actual else {
        return false;
    };
    match expected {
        ModelError::Index => kind == ErrorKind::IndexOutOfBounds,
        ModelError::Missing => matches!(kind, ErrorKind::MissingKey | ErrorKind::MissingValue),
        ModelError::Capacity => kind == ErrorKind::Capacity,
        ModelError::InvalidBoundary => kind == ErrorKind::InvalidBoundary,
        ModelError::Callback => kind == ErrorKind::InvalidPlan,
        ModelError::Poisoned => kind == ErrorKind::Poisoned,
        _ => panic!("unexpected oracle error {expected:?}"),
    }
}

fn region<S: Schema>(bytes: &[u8], model: &Model, path: &[Segment]) -> std::ops::Range<usize> {
    let mut inline = InlinePath::new();
    let (mut shape, mut value) = (model.schema(), model.value());
    for segment in path {
        let step = match segment {
            Segment::Field(i) => Step::Field(*i as u16),
            Segment::Index(i) => Step::Index(*i as u32),
            Segment::Some => Step::Some,
            Segment::Key(key) => {
                let Value::Map(values) = value else {
                    unreachable!()
                };
                Step::Index(values.keys().position(|k| k == key).unwrap() as u32)
            }
        };
        inline.push(step).unwrap();
        (shape, value) = oracle::at(shape, value, std::slice::from_ref(segment));
    }
    // Supplemental production-span assertion. Model encoding and decoded values
    // are the independent oracle; this helper does not establish independence.
    let span = blubber_core::raw::resolve(bytes, S::LAYOUT, &inline).unwrap();
    span.start as usize..span.end as usize
}

fn check_state<S: Bridge + blubber::RootSchema>(store: &dyn Resize, model: &Model) {
    assert!(!store.is_poisoned());
    let view = View::load::<S>(store.bytes(), ID).unwrap();
    assert_eq!(view.bytes(), model.encode().unwrap());
    assert_eq!(S::value(S::decode_owned(view).unwrap()), *model.value());
}

pub(super) fn check_physical_frame(before: &[u8], after: &[u8]) {
    let used = |bytes: &[u8]| u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let old_used = used(before);
    let new_used = used(after);
    let spare = old_used.max(new_used);
    if spare < before.len() {
        assert_eq!(
            &after[spare..before.len()],
            &before[spare..],
            "preserved spare"
        );
    }
    if new_used < old_used {
        assert!(after[new_used..old_used].iter().all(|b| *b == 0));
    }
}

fn execute<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    model: &mut Model,
    path: &[Segment],
    edit: &ModelEdit,
) {
    let before = store.bytes().to_vec();
    let old = model.clone();
    let mut unlimited = old.clone();
    if unlimited.apply(path, edit.clone()).is_ok() {
        let required = 12 + unlimited.encode().unwrap().len();
        if required > 12 + old.encode().unwrap().len() {
            let mut limited = before.clone();
            limited.resize(required - 1, 0xa5);
            let saved = limited.clone();
            let mut limited = SliceStore::new(&mut limited);
            assert!(error_matches(
                ModelError::Capacity,
                apply::<S>(&mut limited, path, edit).unwrap_err()
            ));
            assert_eq!(limited.bytes(), saved);
            check_state::<S>(&limited, &old);
            let mut exact = before.clone();
            exact.resize(required, 0xa5);
            let mut exact = SliceStore::new(&mut exact);
            let actual = apply::<S>(&mut exact, path, edit).unwrap();
            let mut expected = old.clone();
            let previous = expected.apply(path, edit.clone()).unwrap();
            assert_eq!(actual, self::expected(model.schema(), path, edit, previous));
            check_state::<S>(&exact, &expected);
        }
    }
    let result = model.apply_with_limit(path, edit.clone(), store.growth_limit() - 12);
    let actual = apply::<S>(store, path, edit);
    match result {
        Err(error) => {
            assert!(error_matches(error, actual.unwrap_err()));
            assert_eq!(store.bytes(), before, "ordinary error wrote bytes");
            assert_eq!(*model, old);
        }
        Ok(previous) => {
            let actual = actual.unwrap();
            assert_eq!(actual, expected(model.schema(), path, edit, previous));
            for (from, to) in oracle::frames(old.schema(), old.value(), path, edit) {
                assert_eq!(
                    store.bytes()[region::<S>(store.bytes(), model, &to)],
                    before[region::<S>(&before, &old, &from)]
                );
            }
            let new_used = 12 + model.encode().unwrap().len();
            assert_eq!(
                u32::from_le_bytes(store.bytes()[8..12].try_into().unwrap()) as usize,
                new_used
            );
            check_physical_frame(&before, store.bytes());
        }
    }
    check_state::<S>(store, model);
}

fn commands(bytes: &[u8]) -> impl Iterator<Item = oracle::Command> + '_ {
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .take(32)
        .map(|c| oracle::Command {
            selector: u16::from_le_bytes([c[0], c[1]]),
            kind: c[2],
            ordinal: u16::from_le_bytes([c[3], c[4]]),
            data: c[5..8].to_vec(),
        })
}

fn history<S: Bridge + blubber::RootSchema>(store: &mut dyn Resize, initial: Value, stream: &[u8]) {
    let mut model = Model::new(S::schema(), initial).unwrap();
    check_state::<S>(store, &model);
    for command in commands(stream) {
        if command.kind & 0x80 != 0 {
            if let Some((path, edit)) = super::api::action(&model, &command) {
                super::api::execute::<S>(store, &mut model, &path, &edit);
                continue;
            }
        }
        let (path, edit) = oracle::action(model.schema(), model.value(), &command);
        execute::<S>(store, &mut model, &path, &edit);
    }
}

// Struct paths use declaration ordinals. Collection selection is attempted
// through real typed mutators, including invalid ordinals and absent map keys.
fn corrupted<S: Bridge + blubber::RootSchema>(bytes: &mut [u8], stream: &[u8]) {
    let mut store = SliceStore::new(bytes);
    for command in commands(stream) {
        let before = store.bytes().to_vec();
        let shape = S::schema();
        let mut value = oracle::sample(
            &shape,
            &mut oracle::Entropy(u64::from(command.ordinal)),
            false,
        );
        // Paths/edits are schema-valid even when the account's structure is not.
        if let Ok(account) = shape.decode_account(ID, store.bytes()) {
            value = account.value;
        }
        let (path, edit) = oracle::action(&shape, &value, &command);
        if let Ok(account) = shape.decode_account(ID, store.bytes()) {
            let mut model = Model::new(shape, account.value).unwrap();
            execute::<S>(&mut store, &mut model, &path, &edit);
            continue;
        }
        let result = apply::<S>(&mut store, &path, &edit);
        if store.is_poisoned() {
            // A lazy patch pass that detects malformed source poisons, returns
            // its error and never publishes a new used length. Poison ends the
            // session: every later typed access fails.
            assert!(result.is_err(), "corrupted poison without an error");
            assert_eq!(
                store.bytes().get(..12),
                before.get(..12),
                "corrupted poisoned edit published"
            );
            assert!(matches!(
                apply::<S>(&mut store, &path, &edit),
                Err(error) if error == blubber::ErrorKind::Poisoned.into()
            ));
            return;
        }
        if result.is_err() {
            assert_eq!(
                store.bytes(),
                before,
                "corrupted ordinary error wrote bytes"
            );
        } else {
            // Success admitted both envelopes even if unrelated children remain
            // corrupt, so their physical used/spare frames are still specified.
            check_physical_frame(&before, store.bytes());
        }
        // A successful edit can repair the last fault; switch to the full oracle
        // for every subsequent operation once decoding succeeds.
        if let Ok(account) = shape.decode_account(ID, store.bytes()) {
            history::<S>(&mut store, account.value, &[]);
        }
    }
}

pub(crate) fn check<S: Bridge + blubber::RootSchema>(bytes: &[u8]) {
    let [mode, cap_lo, cap_hi, len_lo, len_hi, rest @ ..] = bytes else {
        return;
    };
    let initial_len = usize::from(u16::from_le_bytes([*len_lo, *len_hi]))
        .min(rest.len())
        .min(MAX_STORAGE);
    let (initial, stream) = rest.split_at(initial_len);
    let requested = usize::from(u16::from_le_bytes([*cap_lo, *cap_hi])) % (MAX_STORAGE + 1);
    if mode % 4 == 0 || mode % 4 == 3 {
        let mut bytes = if mode % 4 == 3 {
            account(initial)
        } else {
            initial.to_vec()
        };
        // Retain the arbitrary envelope, used length and discriminator exactly.
        bytes.resize(requested.max(bytes.len()), 0xa5);
        if let Ok(account) = S::schema().decode_account(ID, &bytes) {
            let mut store = SliceStore::new(&mut bytes);
            history::<S>(&mut store, account.value, stream);
        } else {
            corrupted::<S>(&mut bytes, stream);
        }
        return;
    }
    let seed = initial.iter().fold(34u64, |a, b| {
        a.wrapping_mul(257).wrapping_add(u64::from(*b))
    });
    let value = oracle::sample(&S::schema(), &mut oracle::Entropy(seed), mode & 4 == 0);
    let root = S::schema().encode(&value).unwrap();
    let mut bytes = account(&root);
    let limit = requested.max(bytes.len());
    bytes.resize(limit, 0xa5);
    if mode % 4 == 1 {
        let mut store = SliceStore::new(&mut bytes);
        history::<S>(&mut store, value, stream);
    } else {
        let mut store = VecStore::new(&mut bytes);
        history::<S>(&mut store, value, stream);
    }
}
