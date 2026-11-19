//! Public storage contracts exercised without schema or generated types.

#![forbid(unsafe_code)]

use blubber::raw::Resize;
use blubber::Resize as _;
#[cfg(feature = "alloc")]
use blubber::VecStore;
use blubber::{CommitInfo, ErrorKind, ReadStorage, SliceStore};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn check_backend(store: &mut dyn Resize) {
    assert!(!store.is_closed());
    assert!(!store.is_poisoned());
    assert_eq!(store.storage_len(), 24);
    assert_eq!(store.bytes(), &[0xa5; 24]);
    store.resize(12).unwrap();
    store.reserve(12, 5).unwrap();
    assert_eq!(store.storage_len(), 17);
    assert_eq!(&store.bytes()[..12], &[0xa5; 12]);
    assert_eq!(&store.bytes()[12..], &[0; 5]);
    store.bytes_mut()[12..].fill(0x55);
    // Satisfied reservations never trim existing spare.
    store.reserve(12, 2).unwrap();
    assert_eq!(store.storage_len(), 17);
    assert_eq!(&store.bytes()[12..], &[0x55; 5]);
    assert!(store.shrink_to_fit(12).unwrap());
    assert_eq!(
        store.commit(12).unwrap(),
        CommitInfo {
            used_len: 12,
            storage_len: 12,
            topped_up: 0,
            refunded: 0,
            shrink_error: None,
        }
    );
    assert!(store.is_closed());
    assert_eq!(store.storage_len(), 12);
    assert_eq!(store.commit(12), Err(ErrorKind::Closed.into()));
    assert_eq!(store.resize(usize::MAX), Err(ErrorKind::Closed.into()));
    assert_eq!(
        store.reserve(u32::MAX, usize::MAX),
        Err(ErrorKind::Closed.into())
    );
    assert_eq!(store.shrink_to_fit(0), Err(ErrorKind::Closed.into()));
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = store.bytes();
    }))
    .is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = store.bytes_mut();
    }))
    .is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| store.set_poisoned(false))).is_err());
}

#[test]
fn slice_backend_is_object_safe_and_closes_byte_access() {
    let mut room = [0xa5; 24];
    let mut store = SliceStore::new(&mut room);
    check_backend(&mut store);
    assert_eq!(&room[..12], &[0xa5; 12]);
}

#[test]
fn slice_retains_original_room_and_zeroes_every_regrowth() {
    let mut room = [0xa5; 24];
    let mut store = SliceStore::new(&mut room);
    assert_eq!(store.growth_limit(), 24);
    store.resize(4).unwrap();
    assert_eq!(store.growth_limit(), 24);
    assert_eq!(store.check_resize(25), Err(ErrorKind::Capacity.into()));
    assert_eq!(store.resize(25), Err(ErrorKind::Capacity.into()));
    assert_eq!(store.bytes(), &[0xa5; 4]);
    store.resize(24).unwrap();
    assert_eq!(&store.bytes()[..4], &[0xa5; 4]);
    assert_eq!(&store.bytes()[4..], &[0; 20]);
    store.bytes_mut().fill(0xff);
    store.resize(0).unwrap();
    store.resize(24).unwrap();
    assert_eq!(store.bytes(), &[0; 24]);
}

fn check_poison(store: &mut dyn Resize) {
    store.set_poisoned(true);
    assert!(store.is_poisoned());
    let before = store.bytes().to_vec();
    assert_eq!(
        store.check_resize(usize::MAX),
        Err(ErrorKind::Poisoned.into())
    );
    assert_eq!(store.resize(0), Err(ErrorKind::Poisoned.into()));
    assert_eq!(
        store.reserve(u32::MAX, usize::MAX),
        Err(ErrorKind::Poisoned.into())
    );
    assert_eq!(
        store.shrink_to_fit(u32::MAX),
        Err(ErrorKind::Poisoned.into())
    );
    assert_eq!(store.bytes(), before);
    // A kernel call can still finish and then explicitly clear poison.
    store.bytes_mut()[0] = 7;
    store.set_poisoned(false);
    store.resize(12).unwrap();
    store.set_poisoned(true);
    assert_eq!(store.commit(u32::MAX), Err(ErrorKind::Poisoned.into()));
    assert!(store.is_closed());
    assert!(store.is_poisoned());
    assert_eq!(store.commit(12), Err(ErrorKind::Closed.into()));
}

#[test]
fn slice_poison_precedence_and_interrupted_commit() {
    let mut room = [0xa5; 24];
    check_poison(&mut SliceStore::new(&mut room));
}

fn check_bad_lengths(store: &mut dyn Resize) {
    let before = store.bytes().to_vec();
    assert_eq!(store.reserve(25, 0), Err(ErrorKind::UsedLength.into()));
    assert_eq!(store.shrink_to_fit(25), Err(ErrorKind::UsedLength.into()));
    assert_eq!(
        store.reserve(12, usize::MAX),
        Err(ErrorKind::ArithmeticOverflow.into())
    );
    assert_eq!(
        store.reserve(12, u32::MAX as usize),
        Err(ErrorKind::ArithmeticOverflow.into())
    );
    assert_eq!(
        store.check_resize(usize::MAX),
        Err(ErrorKind::Capacity.into())
    );
    assert_eq!(store.resize(usize::MAX), Err(ErrorKind::Capacity.into()));
    assert_eq!(store.bytes(), before);
    assert!(!store.is_closed());
    assert_eq!(store.commit(25), Err(ErrorKind::UsedLength.into()));
    assert!(store.is_closed());
}

#[test]
fn slice_length_faults_are_atomic_and_bad_commit_closes() {
    let mut room = [0xa5; 24];
    check_bad_lengths(&mut SliceStore::new(&mut room));
}

#[test]
#[allow(
    clippy::drop_non_drop,
    clippy::forget_non_drop,
    reason = "Exercise explicit drop/forget of safe leases, including after swap"
)]
fn slice_drop_forget_and_swap_preserve_completed_edits() {
    let mut left = [1; 8];
    let mut right = [2; 8];
    let mut a = SliceStore::new(&mut left);
    let mut b = SliceStore::new(&mut right);
    a.resize(4).unwrap();
    a.bytes_mut()[0] = 3;
    core::mem::swap(&mut a, &mut b);
    assert_eq!(b.storage_len(), 4);
    assert_eq!(a.bytes(), &[2; 8]);
    a.bytes_mut()[0] = 4;
    b.resize(8).unwrap();
    assert_eq!(b.bytes(), &[3, 1, 1, 1, 0, 0, 0, 0]);
    core::mem::forget(b);
    drop(a);
    left[1] = 9;
    right[1] = 8;
    assert_eq!(left, [3, 9, 1, 1, 0, 0, 0, 0]);
    assert_eq!(right, [4, 8, 2, 2, 2, 2, 2, 2]);
}

#[test]
fn empty_slice_can_close_without_interpreting_an_envelope() {
    let mut room = [];
    let mut store = SliceStore::new(&mut room);
    store.reserve(0, 0).unwrap();
    assert!(store.shrink_to_fit(0).unwrap());
    assert_eq!(store.resize(1), Err(ErrorKind::Capacity.into()));
    assert_eq!(store.commit(0).unwrap().storage_len, 0);
}

#[cfg(feature = "alloc")]
#[test]
fn vec_backend_is_object_safe_and_closes_byte_access() {
    let mut buffer = vec![0xa5; 24];
    check_backend(&mut VecStore::new(&mut buffer));
    assert_eq!(buffer.len(), 12);
    assert_eq!(buffer, [0xa5; 12]);
}

#[cfg(feature = "alloc")]
#[test]
fn vec_poison_precedence_and_interrupted_commit() {
    let mut buffer = vec![0xa5; 24];
    check_poison(&mut VecStore::new(&mut buffer));
}

#[cfg(feature = "alloc")]
#[test]
fn vec_length_faults_are_atomic_and_bad_commit_closes() {
    let mut buffer = vec![0xa5; 24];
    check_bad_lengths(&mut VecStore::new(&mut buffer));
}

#[cfg(feature = "alloc")]
#[test]
fn vec_length_is_initialized_storage_and_commit_retains_reserve() {
    let mut buffer = Vec::with_capacity(128);
    buffer.extend_from_slice(&[0xa5; 12]);
    let mut store = VecStore::new(&mut buffer);
    assert_eq!(store.storage_len(), 12);
    assert_eq!(
        store.growth_limit(),
        (u32::MAX as usize).min(isize::MAX as usize)
    );
    store.reserve(12, 7).unwrap();
    assert_eq!(store.storage_len(), 19);
    assert_eq!(&store.bytes()[..12], &[0xa5; 12]);
    assert_eq!(&store.bytes()[12..], &[0; 7]);
    assert_eq!(store.commit(12).unwrap().storage_len, 19);
    assert_eq!(buffer.len(), 19);
}

#[cfg(feature = "alloc")]
#[test]
fn vec_shrink_to_zero_and_regrow_initializes_all_exposed_bytes() {
    let mut buffer = vec![0xa5; 24];
    let mut store = VecStore::new(&mut buffer);
    assert!(store.shrink_to_fit(0).unwrap());
    assert_eq!(store.storage_len(), 0);
    store.resize(24).unwrap();
    assert_eq!(store.bytes(), &[0; 24]);
    assert_eq!(store.commit(0).unwrap().storage_len, 24);
}

#[cfg(feature = "alloc")]
#[test]
#[allow(
    clippy::drop_non_drop,
    clippy::forget_non_drop,
    reason = "Exercise explicit drop/forget of safe leases, including after swap"
)]
fn vec_drop_forget_and_swap_end_safe_borrows() {
    let mut left = vec![1; 8];
    let mut right = vec![2; 8];
    let mut a = VecStore::new(&mut left);
    let mut b = VecStore::new(&mut right);
    a.resize(4).unwrap();
    core::mem::swap(&mut a, &mut b);
    assert_eq!(a.bytes(), &[2; 8]);
    a.bytes_mut()[0] = 4;
    b.resize(16).unwrap();
    b.bytes_mut()[0] = 3;
    core::mem::forget(b);
    drop(a);
    left.push(9);
    right.push(8);
    assert_eq!(&left[..4], &[3, 1, 1, 1]);
    assert_eq!(&left[4..16], &[0; 12]);
    assert_eq!(left[16], 9);
    assert_eq!(right, [4, 2, 2, 2, 2, 2, 2, 2, 8]);
}
