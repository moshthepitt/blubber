//! Matching typed writers and explicit raw re-admission.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::{ReadStorage, Resize, Schema, SliceStore};

#[blubber::schema]
pub struct Generic<T> {
    pub value: T,
}

#[blubber::schema(account)]
pub struct State {
    pub flag: bool,
    pub child: Generic<u16>,
    pub flags: blubber::List<bool>,
}

#[test]
fn generated_and_associated_mut_writers_preserve_matching_schema() {
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    let mut root = State::initialize(
        &mut store,
        StateInit {
            flag: false,
            child: GenericInit { value: 7 },
            flags: &[true, false],
        },
    )
    .unwrap();
    root.flag_mut().set(true).unwrap();
    root.child_mut().value_mut().set(9).unwrap();
    root.flags_mut().rewrite_from(&[false, true]).unwrap();
    root.flags_mut()
        .reborrow()
        .at_mut(0)
        .unwrap()
        .set(true)
        .unwrap();
    root.validate().unwrap();
    root.commit().unwrap();
    let read = State::load(&bytes).unwrap();
    assert!(read.flag());
    assert_eq!(read.child().value(), 9);
    assert_eq!(read.flags().iter().collect::<Vec<_>>(), [true, true]);
}

#[test]
fn raw_writes_require_checked_recovery_and_cannot_reinterpret_typed_authority() {
    let mut bytes = [0; 128];
    let mut store = SliceStore::new(&mut bytes);
    State::initialize(
        &mut store,
        StateInit {
            flag: true,
            child: GenericInit { value: 3 },
            flags: &[],
        },
    )
    .unwrap();
    blubber::raw::backend(&mut store).bytes_mut()[12] = 2;
    assert!(matches!(
        State::load_mut(&mut store),
        Err(blubber::Error::Fault {
            kind: blubber::ErrorKind::InvalidBool,
            ..
        })
    ));
    blubber::raw::backend(&mut store).set_poisoned(true);
    blubber::raw::backend(&mut store).bytes_mut()[12] = 0;
    let mut root: <State as Schema>::Mut<'_> = State::load_checked_mut(&mut store).unwrap();
    root.flag_mut().set(true).unwrap();
    root.commit().unwrap();
    assert!(store.is_closed());
    assert!(!store.is_poisoned());
    assert!(State::load(&bytes).unwrap().flag());
}
