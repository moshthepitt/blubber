//! Seeded typed edit histories against the independent owned codec.

#![forbid(unsafe_code)]
#![cfg(all(feature = "alloc", feature = "derive"))]

#[path = "support/differential/api_oracle.rs"]
mod api_oracle;
#[path = "support/differential/api_run.rs"]
mod api_run;
#[path = "support/differential/bridge.rs"]
mod bridge;
#[path = "support/differential/generate.rs"]
mod generate;
#[path = "support/differential/oracle.rs"]
mod oracle;
#[path = "support/differential/persistence.rs"]
mod persistence;
#[path = "support/differential/phases.rs"]
mod phases;

use blubber::raw::{Edit, Resize};
use blubber::Resize as _;
use blubber::{
    Error, ErrorKind, List, Map, Pubkey, ReadStorage, Schema, SliceStore, Str, VecStore, View,
};
use blubber_core::raw::{InlinePath, Step};
use blubber_model::{
    Edit as ModelEdit, Error as ModelError, Model, PathSegment as Segment, Schema as ModelSchema,
    Value,
};
use bridge::{Bridge, Observed};
use oracle::{Command, Entropy, Path};
use proptest::{
    prelude::*,
    test_runner::{Config, FileFailurePersistence, RngAlgorithm, TestRng, TestRunner},
};

// Each u64 is repeated four times in little-endian order to form the 32-byte
// ChaCha seed. Concrete tapes also replay without the command generator.
const SEEDS: [u64; 4] = [0, 1, 0x626c756262657233, u64::MAX];
const CASES: u32 = 4;

#[derive(Debug, Default)]
struct Stats {
    edits: usize,
    api_edits: usize,
    callback_errors: usize,
    errors: usize,
    capacity_controls: usize,
    poison_controls: usize,
    frames: usize,
    moved_frames: usize,
    relocations: usize,
    depth: usize,
    successful_depth: usize,
}

fn region<S: Schema>(bytes: &[u8], model: &Model, path: &[Segment]) -> std::ops::Range<usize> {
    let mut inline = InlinePath::new();
    let (mut schema, mut value) = (model.schema(), model.value());
    for segment in path {
        let step = match segment {
            Segment::Field(i) => Step::Field((*i).try_into().unwrap()),
            Segment::Index(i) => Step::Index((*i).try_into().unwrap()),
            Segment::Some => Step::Some,
            Segment::Key(key) => {
                let Value::Map(values) = value else {
                    panic!("native map selection")
                };
                Step::Index(
                    values
                        .keys()
                        .position(|k| k == key)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                )
            }
        };
        inline.push(step).unwrap();
        (schema, value) = oracle::at(schema, value, std::slice::from_ref(segment));
    }
    // Supplemental production-span check. Full root encodings, decoding and
    // values in check() are the independent oracle; this helper is not one.
    let span = blubber_core::raw::resolve(bytes, S::LAYOUT, &inline).unwrap();
    span.start as usize..span.end as usize
}

fn check<S: Bridge + blubber::RootSchema>(
    store: &dyn Resize,
    discriminator: [u8; 8],
    model: &Model,
) {
    assert!(!store.is_poisoned());
    assert!(!store.is_closed());
    let view = View::load::<S>(store.bytes(), discriminator).unwrap();
    let expected = model.encode().unwrap();
    assert_eq!(view.bytes(), expected);
    let decoded = model
        .schema()
        .decode_account(discriminator, store.bytes())
        .unwrap();
    assert_eq!(&decoded.value, model.value());
    assert_eq!(decoded.used_len, 12 + expected.len());
    assert_eq!(S::value(S::decode_owned(view).unwrap()), *model.value());
    let owned = S::owned(model.value());
    let mut encoded = vec![0xa5; S::measure_owned(&owned).unwrap() as usize];
    S::encode_owned(&owned, &mut encoded);
    assert_eq!(encoded, expected);
}

fn apply<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    discriminator: [u8; 8],
    path: &[Segment],
    edit: &ModelEdit,
) -> Result<Observed, Error> {
    let root = Edit::load_trusted::<S>(store, discriminator)?;
    S::apply(blubber::raw::project::<S>(root), path, edit)
}

fn expected_observation(
    schema: &ModelSchema,
    path: &[Segment],
    edit: &ModelEdit,
    old: Option<Value>,
) -> Observed {
    match edit {
        ModelEdit::ListRemove(_) | ModelEdit::ListPop | ModelEdit::MapRemove(_) => {
            Observed::Old(old)
        }
        ModelEdit::MapInsert { .. } => {
            let mut selected = schema;
            for segment in path {
                selected = match (selected, segment) {
                    (ModelSchema::Struct(fields), Segment::Field(i)) => &fields[*i],
                    (
                        ModelSchema::List(child)
                        | ModelSchema::Option(child)
                        | ModelSchema::Map(_, child),
                        _,
                    ) => child,
                    _ => panic!("map schema path"),
                };
            }
            let ModelSchema::Map(_, child) = selected else {
                panic!("map insertion schema")
            };
            if matches!(
                child.as_ref(),
                ModelSchema::Unsigned(_)
                    | ModelSchema::Signed(_)
                    | ModelSchema::Bool
                    | ModelSchema::Pubkey
                    | ModelSchema::Array(_)
            ) {
                Observed::Old(old)
            } else {
                Observed::Present(old.is_some())
            }
        }
        _ => Observed::Unit,
    }
}

fn matches_error(expected: ModelError, actual: Error) -> bool {
    let Error::Fault { kind, .. } = actual else {
        return false;
    };
    match expected {
        ModelError::Index => kind == ErrorKind::IndexOutOfBounds,
        ModelError::Capacity => kind == ErrorKind::Capacity,
        ModelError::InvalidBoundary => kind == ErrorKind::InvalidBoundary,
        ModelError::Callback => kind == ErrorKind::InvalidPlan,
        ModelError::Poisoned => kind == ErrorKind::Poisoned,
        ModelError::Missing => matches!(kind, ErrorKind::MissingKey | ErrorKind::MissingValue),
        other => panic!("unexpected reference rejection: {other:?}"),
    }
}

fn check_frames<S: Bridge + blubber::RootSchema>(
    before: &[u8],
    old: &Model,
    after: &[u8],
    model: &Model,
    path: &[Segment],
    edit: &ModelEdit,
    stats: &mut Stats,
) {
    for (from, to) in oracle::frames(old.schema(), old.value(), path, edit) {
        let old_region = region::<S>(before, old, &from);
        let new_region = region::<S>(after, model, &to);
        let (schema, value) = oracle::at(old.schema(), old.value(), &from);
        assert_eq!(before[old_region.clone()], schema.encode(value).unwrap());
        assert_eq!(
            after[new_region.clone()],
            before[old_region.clone()],
            "untouched subtree {from:?} -> {to:?}"
        );
        stats.frames += 1;
        stats.moved_frames += usize::from(old_region.start != new_region.start);
    }
}

fn execute<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    discriminator: [u8; 8],
    model: &mut Model,
    path: Path,
    edit: ModelEdit,
    stats: &mut Stats,
) {
    stats.edits += 1;
    stats.depth = stats.depth.max(path.len() + 1);
    let before = store.bytes().to_vec();
    let old = model.clone();
    let mut unlimited = old.clone();
    // Re-run every otherwise valid growth at exactly one byte below its required
    // extent, including growth selected through seven ancestors at depth eight.
    if unlimited.apply(&path, edit.clone()).is_ok() {
        let required = 12 + unlimited.encode().unwrap().len();
        let old_used = 12 + old.encode().unwrap().len();
        if required > old_used {
            let mut bytes = before.clone();
            bytes.resize(required - 1, 0xa5);
            let saved = bytes.clone();
            let mut limited = SliceStore::new(&mut bytes);
            let failure = apply::<S>(&mut limited, discriminator, &path, &edit).unwrap_err();
            assert!(
                matches_error(ModelError::Capacity, failure),
                "{path:?} {edit:?}: {failure:?}"
            );
            assert_eq!(limited.bytes(), saved);
            assert!(!limited.is_poisoned());
            check::<S>(&limited, discriminator, &old);
            stats.capacity_controls += 1;
            let mut exact = before.clone();
            exact.resize(required, 0xa5);
            let mut exact = SliceStore::new(&mut exact);
            let actual = apply::<S>(&mut exact, discriminator, &path, &edit).unwrap();
            let mut expected = old.clone();
            let previous = expected.apply(&path, edit.clone()).unwrap();
            assert_eq!(
                actual,
                expected_observation(model.schema(), &path, &edit, previous)
            );
            check::<S>(&exact, discriminator, &expected);
        }
    }
    let expected = model.apply_with_limit(&path, edit.clone(), store.growth_limit() - 12);
    let actual = apply::<S>(store, discriminator, &path, &edit);
    match expected {
        Err(error) => {
            let failure = actual.unwrap_err();
            assert!(
                matches_error(error, failure),
                "{path:?} {edit:?}: expected {error:?}, got {failure:?}"
            );
            assert_eq!(
                store.bytes(),
                before,
                "ordinary error wrote bytes: {path:?} {edit:?}"
            );
            assert_eq!(*model, old);
            stats.errors += 1;
        }
        Ok(previous) => {
            stats.successful_depth = stats.successful_depth.max(path.len() + 1);
            let actual = actual.unwrap();
            assert_eq!(
                actual,
                expected_observation(model.schema(), &path, &edit, previous),
                "{path:?} {edit:?}"
            );
            phases::compare::<S>(&before, &old, &path, &edit, store.bytes(), model);
            check_frames::<S>(&before, &old, store.bytes(), model, &path, &edit, stats);
            let old_used = 12 + old.encode().unwrap().len();
            let new_used = 12 + model.encode().unwrap().len();
            let spare = old_used.max(new_used);
            if spare < before.len() {
                assert_eq!(
                    &store.bytes()[spare..before.len()],
                    &before[spare..],
                    "preserved spare"
                );
            }
            assert_eq!(
                u32::from_le_bytes(store.bytes()[8..12].try_into().unwrap()) as usize,
                new_used
            );
            if new_used < old_used {
                assert!(store.bytes()[new_used..old_used].iter().all(|b| *b == 0));
            }
        }
    }
    check::<S>(store, discriminator, model);
}

fn directed<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    discriminator: [u8; 8],
    model: &mut Model,
    stats: &mut Stats,
) {
    // Postorder keeps every initial child path alive until its own checks finish.
    let nodes = oracle::nodes(model.schema(), model.value());
    let mut entropy = Entropy(0x6469726563746564);
    for path in nodes {
        let (schema, value) = oracle::at(model.schema(), model.value(), &path);
        let (schema, value) = (schema.clone(), value.clone());
        let mut edits = vec![];
        match (&schema, &value) {
            (ModelSchema::List(child), Value::List(values)) => {
                for index in 0..=values.len() {
                    // Isolate each run so short live capacity cannot remove an
                    // original element or invalidate the next insertion index.
                    for count in [0, 1, 3] {
                        let edit = ModelEdit::ListInsert {
                            index,
                            values: (0..count)
                                .map(|_| oracle::sample(child, &mut entropy, false))
                                .collect(),
                        };
                        let mut probe = model.clone();
                        let mut expected = probe.clone();
                        expected.apply(&path, edit.clone()).unwrap();
                        let mut bytes = store.bytes().to_vec();
                        bytes.resize(bytes.len().max(12 + expected.encode().unwrap().len()), 0xa5);
                        execute::<S>(
                            &mut SliceStore::new(&mut bytes),
                            discriminator,
                            &mut probe,
                            path.clone(),
                            edit,
                            stats,
                        );
                    }
                    edits.push((
                        path.clone(),
                        ModelEdit::ListInsert {
                            index,
                            values: vec![oracle::sample(child, &mut entropy, false)],
                        },
                    ));
                    edits.push((path.clone(), ModelEdit::ListRemove(index)));
                }
                for index in 0..values.len() {
                    let mut child_path = path.clone();
                    child_path.push(Segment::Index(index));
                    edits.push((
                        child_path,
                        ModelEdit::Set(oracle::sample(child, &mut entropy, false)),
                    ));
                }
                edits.push((path.clone(), ModelEdit::Clear));
                edits.push((path.clone(), ModelEdit::ListPop));
                for index in 0..3 {
                    edits.push((
                        path.clone(),
                        ModelEdit::ListInsert {
                            index,
                            values: vec![oracle::sample(child, &mut entropy, false)],
                        },
                    ));
                }
                edits.push((path.clone(), ModelEdit::ListRemove(1)));
                edits.push((path.clone(), ModelEdit::ListPop));
                edits.push((path.clone(), ModelEdit::ListTruncate(0)));
                edits.push((path.clone(), ModelEdit::ListTruncate(usize::MAX)));
                edits.push((path.clone(), ModelEdit::ListRemove(0)));
                edits.push((
                    path.clone(),
                    ModelEdit::ListInsert {
                        index: 1,
                        values: vec![oracle::sample(child, &mut entropy, false)],
                    },
                ));
            }
            (ModelSchema::Map(key_schema, child), Value::Map(values)) => {
                let mut absent = oracle::key(key_schema, &mut entropy);
                while values.contains_key(&absent) {
                    absent = oracle::key(key_schema, &mut entropy);
                }
                edits.push((path.clone(), ModelEdit::MapRemove(absent.clone())));
                for key in values.keys().chain(std::iter::once(&absent)) {
                    for _ in 0..2 {
                        edits.push((
                            path.clone(),
                            ModelEdit::MapInsert {
                                key: key.clone(),
                                value: oracle::sample(child, &mut entropy, false),
                            },
                        ));
                    }
                }
                edits.push((path.clone(), ModelEdit::MapRemove(absent.clone())));
                edits.push((path.clone(), ModelEdit::Clear));
                // Force 0 -> 1 -> 2 -> 1 -> 0 even when random keys collide.
                for key in values.keys() {
                    edits.push((
                        path.clone(),
                        ModelEdit::MapInsert {
                            key: key.clone(),
                            value: oracle::sample(child, &mut entropy, false),
                        },
                    ));
                }
                for key in values.keys() {
                    edits.push((path.clone(), ModelEdit::MapRemove(key.clone())));
                }
                edits.push((path.clone(), ModelEdit::MapRemove(absent)));
            }
            (ModelSchema::Option(child), _) => {
                edits.push((path.clone(), ModelEdit::Clear));
                let mut missing = path.clone();
                missing.push(Segment::Some);
                edits.push((
                    missing,
                    ModelEdit::Set(oracle::sample(child, &mut entropy, false)),
                ));
                edits.push((
                    path.clone(),
                    ModelEdit::Set(Value::Option(Some(Box::new(oracle::sample(
                        child,
                        &mut entropy,
                        true,
                    ))))),
                ));
                edits.push((path.clone(), ModelEdit::Set(Value::Option(None))));
            }
            (ModelSchema::Str, _) => {
                for text in ["", "a", "é🦀", "xxxxxx", "x", ""] {
                    edits.push((path.clone(), ModelEdit::Set(Value::Str(text.into()))));
                }
                edits.push((path.clone(), ModelEdit::StringPush("\0é🦀".into())));
            }
            _ => edits.push((
                path.clone(),
                ModelEdit::Set(oracle::sample(&schema, &mut entropy, false)),
            )),
        }
        for (path, edit) in edits {
            execute::<S>(store, discriminator, model, path, edit, stats);
        }
    }
}

fn poison_control<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    discriminator: [u8; 8],
    model: &Model,
    stats: &mut Stats,
) {
    let before = store.bytes().to_vec();
    store.set_poisoned(true);
    let error = apply::<S>(
        store,
        discriminator,
        &[Segment::Field(0)],
        &ModelEdit::Clear,
    )
    .unwrap_err();
    assert_eq!(error, ErrorKind::Poisoned.into());
    assert_eq!(store.bytes(), before);
    assert!(store.is_poisoned());
    Edit::load::<S>(store, discriminator).unwrap();
    check::<S>(store, discriminator, model);
    stats.poison_controls += 1;
}

fn history<S: Bridge + blubber::RootSchema>(
    id: usize,
    discriminator: [u8; 8],
    initial: u64,
    commands: &[Command],
    prelude: bool,
    stats: &mut Stats,
) {
    let schema = S::schema();
    let value = oracle::sample(&schema, &mut Entropy(initial), true);
    let model = Model::new(schema, value).unwrap();
    let used = 12 + model.encode().unwrap().len();
    let bytes = model
        .schema()
        .encode_account(discriminator, model.value(), used + 64)
        .unwrap();
    for relocating in [false, true] {
        let mut bytes = bytes.clone();
        bytes[used..].fill(0xa5);
        let mut model = model.clone();
        let mut slice;
        let mut vector;
        let store: &mut dyn Resize = if relocating {
            vector = VecStore::new(&mut bytes);
            &mut vector
        } else {
            slice = SliceStore::new(&mut bytes);
            &mut slice
        };
        check::<S>(store, discriminator, &model);
        poison_control::<S>(store, discriminator, &model, stats);
        if prelude {
            api_run::directed::<S>(store, discriminator, &model, stats);
            directed::<S>(store, discriminator, &mut model, stats);
        }
        let initial_bytes = store.bytes().to_vec();
        let mut actions = vec![];
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for (i, command) in commands.iter().enumerate() {
                if command.kind & 0x80 != 0 {
                    let (path, edit) = api_oracle::action(model.schema(), model.value(), command);
                    actions.push(serde_json::json!({"api_command": [command.selector, command.kind, command.ordinal, command.data]}));
                    api_run::execute::<S>(store, discriminator, &mut model, &path, &edit, stats);
                    continue;
                }
                let (path, edit) = oracle::action(model.schema(), model.value(), command);
                actions.push(persistence::encode(model.schema(), &path, &edit));
                execute::<S>(store, discriminator, &mut model, path, edit, stats);
                if relocating && i % 16 == 0 {
                    let used = (12 + model.encode().unwrap().len()) as u32;
                    store.reserve(used, 1).unwrap();
                    let old_address = store.bytes().as_ptr() as usize;
                    assert!(store.shrink_to_fit(used).unwrap());
                    assert_ne!(store.bytes().as_ptr() as usize, old_address);
                    stats.relocations += 1;
                    check::<S>(store, discriminator, &model);
                }
            }
        }));
        if let Err(failure) = outcome {
            persistence::save(id, model.schema(), &initial_bytes, relocating, &actions);
            std::panic::resume_unwind(failure);
        }
    }
}

fn replay<S: Bridge + blubber::RootSchema>(id: usize, discriminator: [u8; 8], stats: &mut Stats) {
    let mut tapes: Vec<_> = std::fs::read_dir(persistence::directory())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(&format!("fixture-{id:02}"))
        })
        .collect();
    tapes.sort();
    for path in tapes {
        let tape: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(tape["version"], 1);
        assert_eq!(tape["schema"], format!("{:?}", S::schema()));
        let mut bytes: Vec<u8> = serde_json::from_value(tape["initial"].clone()).unwrap();
        let initial = S::schema().decode_account(discriminator, &bytes).unwrap();
        let mut model = Model::new(S::schema(), initial.value).unwrap();
        let mut vector;
        let mut slice;
        let store: &mut dyn Resize = if tape["relocating"].as_bool().unwrap() {
            vector = VecStore::new(&mut bytes);
            &mut vector
        } else {
            slice = SliceStore::new(&mut bytes);
            &mut slice
        };
        check::<S>(store, discriminator, &model);
        for (i, action) in tape["actions"].as_array().unwrap().iter().enumerate() {
            if let Some(command) = action.get("api_command") {
                let command = Command {
                    selector: command[0].as_u64().unwrap() as u16,
                    kind: command[1].as_u64().unwrap() as u8,
                    ordinal: command[2].as_u64().unwrap() as u16,
                    data: serde_json::from_value(command[3].clone()).unwrap(),
                };
                let (path, edit) = api_oracle::action(model.schema(), model.value(), &command);
                api_run::execute::<S>(store, discriminator, &mut model, &path, &edit, stats);
                continue;
            }
            let (path, edit) = persistence::decode(model.schema(), action);
            assert_eq!(persistence::encode(model.schema(), &path, &edit), *action);
            execute::<S>(store, discriminator, &mut model, path, edit, stats);
            if tape["relocating"].as_bool().unwrap() && i % 16 == 0 {
                let used = (12 + model.encode().unwrap().len()) as u32;
                store.reserve(used, 1).unwrap();
                store.shrink_to_fit(used).unwrap();
                check::<S>(store, discriminator, &model);
            }
        }
    }
}

fn run_fixture<S: Bridge + blubber::RootSchema>(id: usize, discriminator: [u8; 8]) {
    let stats = std::cell::RefCell::new(Stats::default());
    replay::<S>(id, discriminator, &mut stats.borrow_mut());
    history::<S>(
        id,
        discriminator,
        id as u64,
        &[],
        true,
        &mut stats.borrow_mut(),
    );
    let commands = prop::collection::vec(
        (
            any::<u16>(),
            any::<u8>(),
            any::<u16>(),
            prop::collection::vec(any::<u8>(), 0..24),
        )
            .prop_map(|(selector, kind, ordinal, data)| Command {
                selector,
                kind,
                ordinal,
                data,
            }),
        0..97,
    );
    let strategy = (any::<u64>(), commands);
    let seeds = std::env::var("BLUBBER_DIFFERENTIAL_SEEDS")
        .map(|value| {
            value
                .split(',')
                .map(|seed| {
                    seed.trim()
                        .parse::<u64>()
                        .expect("decimal u64 differential seed")
                })
                .collect()
        })
        .unwrap_or_else(|_| SEEDS.to_vec());
    let cases = std::env::var("BLUBBER_DIFFERENTIAL_CASES")
        .map(|value| {
            value
                .parse::<u32>()
                .expect("positive u32 differential cases")
        })
        .unwrap_or(CASES);
    assert!(cases > 0 && !seeds.is_empty());
    for seed in seeds {
        let mut expanded = [0; 32];
        for chunk in expanded.as_chunks_mut::<8>().0 {
            chunk.copy_from_slice(&seed.to_le_bytes());
        }
        let config = Config {
            cases,
            source_file: Some(file!()),
            failure_persistence: Some(Box::new(FileFailurePersistence::Direct(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/differential/proptest-regressions.txt"
            )))),
            max_shrink_iters: 512,
            ..Config::default()
        };
        let mut runner =
            TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &expanded));
        runner
            .run(&strategy, |(initial, commands)| {
                history::<S>(
                    id,
                    discriminator,
                    initial,
                    &commands,
                    false,
                    &mut stats.borrow_mut(),
                );
                Ok(())
            })
            .unwrap_or_else(|error| panic!("fixture {id}, seed {seed:#018x}: {error}"));
    }
    let stats = stats.into_inner();
    assert!(stats.capacity_controls > 0 && stats.errors > 0 && stats.poison_controls > 0);
    assert!(stats.api_edits > 0 && stats.callback_errors > 0);
    assert!(stats.moved_frames > 0 && stats.relocations > 0);
    if id % 8 == 7 {
        assert_eq!(stats.depth, 8);
        assert_eq!(stats.successful_depth, 8);
    }
    eprintln!("fixture {id:02}: {stats:?}");
}

fn detection_controls_for<S: Bridge + blubber::RootSchema>(discriminator: [u8; 8]) {
    let schema = S::schema();
    let value = oracle::sample(&schema, &mut Entropy(0), true);
    let old = Model::new(schema, value).unwrap();
    let before = old
        .schema()
        .encode_account(discriminator, old.value(), 4096)
        .unwrap();
    let path = vec![Segment::Field(0)];
    let edit = ModelEdit::StringPush("moves the suffix".into());
    let mut next = old.clone();
    next.apply(&path, edit.clone()).unwrap();
    let mut after = next
        .schema()
        .encode_account(discriminator, next.value(), 4096)
        .unwrap();
    let moved = region::<S>(&after, &next, &[Segment::Field(3)]);
    assert_ne!(
        moved.start,
        region::<S>(&before, &old, &[Segment::Field(3)]).start
    );
    after[moved.start + 4] ^= 1; // A changed u8 element remains structurally valid.
    View::load::<S>(&after, discriminator).unwrap();
    assert!(std::panic::catch_unwind(|| {
        check_frames::<S>(
            &before,
            &old,
            &after,
            &next,
            &path,
            &edit,
            &mut Stats::default(),
        );
    })
    .is_err());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check::<S>(&SliceStore::new(&mut after), discriminator, &next);
    }))
    .is_err());
}

include!(concat!(env!("OUT_DIR"), "/differential.rs"));

#[test]
fn generated_source_is_reproducible() {
    let source = generate::source();
    assert_eq!(
        source,
        include_str!(concat!(env!("OUT_DIR"), "/differential.rs"))
    );
    assert_eq!(source.matches("fn histories()").count(), 32);
    assert_eq!(source.matches("#![forbid(unsafe_code)]").count(), 32);
}

#[test]
fn frame_oracle_tracks_shifted_ordinals() {
    let schema = ModelSchema::List(Box::new(ModelSchema::Str));
    let value = Value::List(vec![Value::Str("a".into()), Value::Str("b".into())]);
    let edit = ModelEdit::ListInsert {
        index: 0,
        values: vec![Value::Str(String::new())],
    };
    assert_eq!(
        oracle::frames(&schema, &value, &[], &edit),
        vec![
            (vec![Segment::Index(0)], vec![Segment::Index(1)]),
            (vec![Segment::Index(1)], vec![Segment::Index(2)]),
        ]
    );
    let edit = ModelEdit::ListRemove(0);
    assert_eq!(
        oracle::frames(&schema, &value, &[], &edit),
        vec![(vec![Segment::Index(1)], vec![Segment::Index(0)]),]
    );
}

#[path = "support/keyless_differential.rs"]
mod keyless_differential;

proptest! {
    #[test]
    fn keyless_rewrite_matches_keyed_current_keys(data in proptest::collection::vec(any::<u8>(), 0..128)) {
        keyless_differential::check(&data);
    }
}
