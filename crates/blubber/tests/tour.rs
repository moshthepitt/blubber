//! Execute the architecture's unchanged state and exercise on host stores.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "support/tour/exercise.rs"]
mod exercise;
#[path = "support/tour/state.rs"]
mod state;

use blubber::{Pubkey, Result, SliceStore};
use state::{Limits, MyAccount};

fn assert_finished(account: state::MyAccountRef<'_>, actor: Pubkey) {
    account.validate().unwrap();
    assert_eq!(account.authority(), actor);
    assert_eq!(account.counter(), 1);
    assert_eq!(account.records().len(), 0);
    assert_eq!(account.balances().len(), 0);
    assert_eq!(account.labels().len(), 0);
    assert_eq!(account.note(), "grew after physical shrink");
    assert_eq!(account.attachment().len(), 0);
    assert_eq!(account.memo(), None);
    assert_eq!(
        account.limits(),
        Limits {
            enabled: true,
            limit: 200
        }
    );
}

#[test]
fn tour_runs_on_a_fixed_slice_and_reopens_after_commit() -> Result<()> {
    let actor = Pubkey::from_bytes([1; 32]);
    let mut room = [0; 4096];
    let mut store = SliceStore::new(&mut room);
    let mut account = MyAccount::initialize(&mut store, state::seed(actor))?;
    exercise::exercise(&mut account, actor)?;
    let receipt = account.commit()?;
    assert!(receipt.used_len <= receipt.storage_len);
    assert!(receipt.storage_len < 4096);
    assert_eq!(
        (receipt.topped_up, receipt.refunded, receipt.shrink_error),
        (0, 0, None)
    );
    assert_finished(
        MyAccount::load(&room[..receipt.storage_len as usize])?,
        actor,
    );
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn tour_runs_on_a_vec_that_grows_from_exact_seed_capacity() -> Result<()> {
    let actor = Pubkey::from_bytes([2; 32]);
    let mut bytes = Vec::new();
    let mut store = blubber::VecStore::new(&mut bytes);
    let mut account = MyAccount::initialize(&mut store, state::seed(actor))?;
    assert_eq!(account.used_len(), account.storage_len());
    exercise::exercise(&mut account, actor)?;
    let receipt = account.commit()?;
    assert!(receipt.used_len <= receipt.storage_len);
    assert_eq!(receipt.storage_len as usize, bytes.len());
    assert_finished(MyAccount::load(&bytes)?, actor);
    Ok(())
}

#[test]
fn unauthorized_exercise_preserves_the_complete_storage() -> Result<()> {
    let mut room = [0; 4096];
    let actor = Pubkey::from_bytes([1; 32]);
    MyAccount::initialize(&mut SliceStore::new(&mut room), state::seed(actor))?;
    let before = room;
    let mut store = SliceStore::new(&mut room);
    let mut account = MyAccount::load_mut(&mut store)?;
    assert_eq!(
        exercise::exercise(&mut account, Pubkey::from_bytes([2; 32])),
        Err(blubber::ErrorKind::Unauthorized.into())
    );
    account.validate()?;
    assert_eq!(room, before);
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn owned_tour_inputs_do_not_borrow_the_inserted_mirror() -> Result<()> {
    let mut room = [0; 4096];
    let mut store = SliceStore::new(&mut room);
    let mut account = MyAccount::initialize(&mut store, state::seed(Pubkey::from_bytes([1; 32])))?;
    let mut owned = state::RecordOwned {
        name: "x".into(),
        tags: vec![1, 2],
    };
    account.records_mut().push_owned(&owned)?;
    account.records_mut().insert_owned(0, &owned)?;
    owned.name.clear();
    owned.tags.clear();
    assert_eq!(account.records().get(0).unwrap().name(), "x");
    assert_eq!(
        account
            .records()
            .get(3)
            .unwrap()
            .tags()
            .iter()
            .collect::<Vec<_>>(),
        [1, 2]
    );
    account.validate()
}

#[test]
fn compiled_tour_matches_the_specification_acceptance_source() {
    let specification = include_str!("../../../SPEC.md");
    // Preserve literal contents; ignore docs, indentation and blank lines only.
    fn normalize(source: &str) -> String {
        source
            .lines()
            .filter(|line| {
                !line.trim_start().starts_with("///")
                    && !line.trim_start().starts_with("//!")
                    && line.trim() != "#![forbid(unsafe_code)]"
            })
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
    for (name, source, demo, lifecycle) in [
        (
            "state",
            include_str!("support/tour/state.rs"),
            include_str!("../../../programs/blubber-demo/src/state.rs"),
            include_str!("../../../programs/borrow-resize/src/tour_state.rs"),
        ),
        (
            "exercise",
            include_str!("support/tour/exercise.rs"),
            include_str!("../../../programs/blubber-demo/src/exercise.rs"),
            include_str!("../../../programs/borrow-resize/src/tour_exercise.rs"),
        ),
    ] {
        let heading = format!("`programs/blubber-demo/src/{name}.rs`:");
        let spec = specification
            .split_once(&heading)
            .unwrap()
            .1
            .split_once("```rust\n")
            .unwrap()
            .1
            .split_once("```")
            .unwrap()
            .0;
        assert_eq!(
            normalize(source),
            normalize(spec),
            "{name} drifted from the tour"
        );
        assert_eq!(source, demo, "{name} demo drift");
        assert_eq!(source, lifecycle, "{name} lifecycle fixture drift");
    }
    let dispatcher = specification
        .split_once("`programs/blubber-demo/src/lib.rs`:")
        .unwrap()
        .1
        .split_once("```rust\n")
        .unwrap()
        .1
        .split_once("```")
        .unwrap()
        .0;
    assert_eq!(
        normalize(include_str!("../../../programs/blubber-demo/src/lib.rs")),
        normalize(dispatcher),
        "dispatcher drifted from the tour"
    );
}
