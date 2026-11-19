//! Complete mutation cohort, linked with the library unit tests.
#![forbid(unsafe_code)]
crate::__root_schema!(__IndexedCallerInput, crate::List<crate::Str>);

#[test]
#[cfg(not(feature = "verify-on-load"))]
fn indexed_caller_measurement_fault_with_offset_preserves_frame() {
    use crate::{ReadStorage as _, Resize as _};
    let mut bytes = [0; 64];
    let mut store = crate::SliceStore::new(&mut bytes);
    crate::edit::AccountEdit::initialize::<__IndexedCallerInput>(
        &mut store,
        *b"badinput",
        &["first", "second"],
    )
    .unwrap();
    let before = store.bytes().to_vec();
    let fault = crate::Error::new(crate::ErrorKind::InvalidBoundary, 17);
    let result = {
        let edit = crate::edit::AccountEdit::load_trusted::<__IndexedCallerInput>(
            &mut store,
            *b"badinput",
        )
        .unwrap();
        let mut walk = crate::edit::indexed_walk::IndexedWalk::<false>::new(edit, None)
            .unwrap()
            .unwrap();
        let mut loan = walk.next().unwrap().unwrap();
        let result = loan.replace_with(
            |_| Err::<(u32, ()), _>(fault),
            |(), _| panic!("caller measurement failed before encoding"),
        );
        assert_eq!(loan.check(), Ok(()));
        result
    };
    assert_eq!(result, Err(fault));
    assert!(!store.is_poisoned());
    assert_eq!(store.bytes(), before);
}

#[path = "../ancestor_pass.rs"]
mod ancestor_pass;
#[path = "../cursor.rs"]
mod cursor;
#[path = "../differential.rs"]
mod differential;
#[path = "../entry.rs"]
mod entry;
#[path = "../fast_edits.rs"]
mod fast_edits;
#[path = "../list_edits.rs"]
mod list_edits;
#[path = "../map_edits.rs"]
mod map_edits;
#[path = "../map_patch.rs"]
mod map_patch;
#[path = "../owned.rs"]
mod owned;
#[path = "../prepared.rs"]
mod prepared;
#[path = "../projections.rs"]
mod projections;
#[path = "../readers.rs"]
mod readers;
#[path = "../relocation.rs"]
mod relocation;
#[path = "../remove_into.rs"]
mod remove_into;
#[path = "../rewrite_kernel.rs"]
mod rewrite_kernel;
#[path = "../source_poison.rs"]
mod source_poison;
#[path = "../storage.rs"]
mod storage;
#[path = "../verification_policy.rs"]
mod verification_policy;
