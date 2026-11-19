//! Bounded API histories compared with model-owned returns and side effects.

use super::{api_oracle, bridge::Bridge, oracle, ID};
use blubber::raw::{Edit, Resize};
use blubber::{SliceStore, View};
use blubber_model::{ApiEdit, Callbacks, EntryMode, Model, PathSegment as Segment, Schema};

pub(super) fn action(model: &Model, command: &oracle::Command) -> Option<(Vec<Segment>, ApiEdit)> {
    if api_oracle::paths(model.schema(), model.value()).is_empty() {
        return None;
    }
    let (path, mut edit) = api_oracle::action(model.schema(), model.value(), command);
    // Packed is the fixed native aggregate in this fuzz inventory. The owned
    // tree codec deliberately does not encode the Rust Removed type. Matched
    // entry returns already have separate native fixed tests; use a combinator
    // here, for which both aggregate representations have the same contract.
    if let ApiEdit::Entry { mode, .. } = &mut edit {
        let (schema, _) = oracle::at(model.schema(), model.value(), &path);
        if let Schema::Map(_, child) = schema {
            if super::edits::copied_aggregate(child)
                && matches!(mode, EntryMode::MatchInsert | EntryMode::MatchRemove)
            {
                *mode = EntryMode::InsertWith;
            }
        }
    }
    Some((path, edit))
}

pub(super) fn apply<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    path: &[Segment],
    edit: &ApiEdit,
    calls: &mut Callbacks,
    destination: &mut Option<Vec<u8>>,
) -> blubber::Result<blubber_model::ApiReturn> {
    S::api(
        blubber::raw::project::<S>(Edit::load_trusted::<S>(store, ID)?),
        path,
        edit,
        calls,
        destination,
    )
}

fn one<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    model: &mut Model,
    path: &[Segment],
    edit: &ApiEdit,
) {
    let before = store.bytes().to_vec();
    let expected = model.apply_api(path, edit, store.growth_limit() - 12, store.is_poisoned());
    let mut calls = Callbacks::default();
    let mut destination = match edit {
        ApiEdit::RemoveInto { capacity, .. } => Some(vec![0xa5; *capacity]),
        _ => None,
    };
    let actual = apply::<S>(store, path, edit, &mut calls, &mut destination);
    match (expected.result, actual) {
        (Ok(expected), Ok(actual)) => assert_eq!(actual, expected),
        (Err(expected), Err(actual)) => assert!(super::edits::error_matches(expected, actual)),
        (expected, actual) => {
            panic!("API return mismatch {path:?} {edit:?}: {expected:?} {actual:?}")
        }
    }
    assert_eq!(calls, expected.callbacks);
    assert_eq!(destination, expected.destination);
    assert_eq!(store.is_poisoned(), expected.poisoned);
    if expected.poisoned {
        assert_eq!(store.bytes(), before);
        return;
    }
    let view = View::load::<S>(store.bytes(), ID).unwrap();
    assert_eq!(view.bytes(), model.encode().unwrap());
    assert_eq!(S::value(S::decode_owned(view).unwrap()), *model.value());
    if matches!(
        edit,
        ApiEdit::Fill(_) | ApiEdit::Rewrite { .. } | ApiEdit::RemoveInto { .. }
    ) {
        super::edits::check_physical_frame(&before, store.bytes());
    }
}

pub(super) fn execute<S: Bridge + blubber::RootSchema>(
    store: &mut dyn Resize,
    model: &mut Model,
    path: &[Segment],
    edit: &ApiEdit,
) {
    let mut unlimited = model.clone();
    if unlimited
        .apply_api(path, edit, u32::MAX as usize, false)
        .result
        .is_ok()
    {
        let required = 12 + unlimited.encode().unwrap().len();
        if required > 12 + model.encode().unwrap().len() {
            for capacity in [required - 1, required] {
                let mut bytes = store.bytes().to_vec();
                bytes.resize(capacity, 0xa5);
                one::<S>(
                    &mut SliceStore::new(&mut bytes),
                    &mut model.clone(),
                    path,
                    edit,
                );
            }
        }
    }
    one::<S>(store, model, path, edit);
}
