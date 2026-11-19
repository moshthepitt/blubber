//! Allocation failures injected at production reservation boundaries.

use crate::Resize as _;
extern crate std;

use super::*;
use crate::{ReadStorage, SliceStore, Str};
use core::cell::Cell;

std::thread_local! {
    static REMAINING: Cell<Option<usize>> = const { Cell::new(None) };
}

pub(super) fn allocation() -> Result<()> {
    REMAINING.with(|remaining| match remaining.get() {
        None => Ok(()),
        Some(0) => Err(ErrorKind::Allocation.into()),
        Some(count) => {
            remaining.set(Some(count - 1));
            Ok(())
        }
    })
}

struct FailAfter;
impl FailAfter {
    fn new(count: usize) -> Self {
        REMAINING.with(|remaining| {
            assert!(remaining.replace(Some(count)).is_none());
        });
        Self
    }
}
impl Drop for FailAfter {
    fn drop(&mut self) {
        REMAINING.with(|remaining| remaining.set(None));
    }
}

#[test]
fn every_nested_list_allocation_failure_preserves_storage() {
    type Tree = List<List<Str>>;
    let mut room = [0; 160];
    let mut store = SliceStore::new(&mut room);
    let first: &[&str] = &["a", "é"];
    let second: &[&str] = &["last"];
    {
        crate::__root_schema!(__ConcreteRoot, Tree);
        crate::Edit::initialize::<__ConcreteRoot>(&mut store, *b"ownalloc", &[first, second])
    }
    .unwrap();
    let before = store.bytes().to_vec();
    // Removing a List<Str> allocates its vector and its two nonempty strings.
    for count in 0..3 {
        let mut edit = {
            crate::__root_schema!(__ConcreteRoot, Tree);
            crate::Edit::load::<__ConcreteRoot>(&mut store, *b"ownalloc")
        }
        .unwrap();
        let mut list = crate::raw::project::<Tree>(edit.reborrow());
        let failure = FailAfter::new(count);
        assert_eq!(list.remove_owned(0), Err(ErrorKind::Allocation.into()));
        drop(failure);
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
    }
    let mut edit = {
        crate::__root_schema!(__ConcreteRoot, Tree);
        crate::Edit::load::<__ConcreteRoot>(&mut store, *b"ownalloc")
    }
    .unwrap();
    let failure = FailAfter::new(3);
    assert_eq!(
        crate::raw::project::<Tree>(edit.reborrow())
            .remove_owned(0)
            .unwrap(),
        ["a", "é"]
    );
    drop(failure);
    edit.validate().unwrap();
}

#[test]
fn every_nested_map_allocation_failure_preserves_storage() {
    type Values = Map<u8, Map<u16, Option<Str>>>;
    let mut room = [0; 160];
    let mut store = SliceStore::new(&mut room);
    let child: &[(u16, Option<&str>)] = &[(1, Some("a")), (256, Some("é"))];
    {
        crate::__root_schema!(__ConcreteRoot, Values);
        crate::Edit::initialize::<__ConcreteRoot>(&mut store, *b"mapalloc", &[(1, child)])
    }
    .unwrap();
    let before = store.bytes().to_vec();
    for count in 0..3 {
        let mut edit = {
            crate::__root_schema!(__ConcreteRoot, Values);
            crate::Edit::load::<__ConcreteRoot>(&mut store, *b"mapalloc")
        }
        .unwrap();
        let mut map = crate::raw::project::<Values>(edit.reborrow());
        let failure = FailAfter::new(count);
        assert_eq!(map.remove_owned(&1), Err(ErrorKind::Allocation.into()));
        drop(failure);
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
    }
}

#[test]
fn borrowed_owned_insertion_never_allocates_an_init_array() {
    type Values = List<Map<u16, List<Str>>>;
    let value = alloc::vec![(1, alloc::vec![String::from("one"), String::from("two")])];
    let mut room = [0; 256];
    let mut store = SliceStore::new(&mut room);
    let mut edit = {
        crate::__root_schema!(__ConcreteRoot, Values);
        crate::Edit::initialize::<__ConcreteRoot>(&mut store, *b"borrowed", &[])
    }
    .unwrap();
    let failure = FailAfter::new(0);
    let mut list = crate::raw::project::<Values>(edit.reborrow());
    list.push_owned(&value).unwrap();
    list.insert_owned(0, &value).unwrap();
    list.replace_owned(1, &value).unwrap();
    drop(failure);
    edit.validate().unwrap();
}

#[test]
fn real_reservation_overflow_maps_to_allocation() {
    assert_eq!(vector::<u8>(usize::MAX), Err(ErrorKind::Allocation.into()));
}

#[cfg(feature = "derive")]
#[test]
fn generated_owned_conversion_and_pop_fail_before_compaction() {
    #[blubber::schema]
    struct Record {
        text: Str,
        tags: List<u16>,
    }
    #[blubber::schema(account)]
    struct Records {
        records: List<Record>,
    }
    let mut room = [0; 128];
    let mut store = SliceStore::new(&mut room);
    Records::initialize(
        &mut store,
        RecordsInit {
            records: &[RecordInit {
                text: "é",
                tags: &[1, 2],
            }],
        },
    )
    .unwrap();
    let before = store.bytes().to_vec();
    for count in 0..3 {
        let root = Records::load_mut(&mut store).unwrap();
        let failure = FailAfter::new(count);
        assert!(matches!(root.to_owned(), Err(e) if e == ErrorKind::Allocation.into()));
        drop(failure);
        assert_eq!(store.bytes(), before);
    }
    for count in 0..2 {
        let mut root = Records::load_mut(&mut store).unwrap();
        let failure = FailAfter::new(count);
        assert!(
            matches!(root.records_mut().pop_owned(), Err(e) if e == ErrorKind::Allocation.into())
        );
        drop(failure);
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
    }
    let root = Records::load_mut(&mut store).unwrap();
    let failure = FailAfter::new(3);
    let owned = root.to_owned().unwrap();
    assert_eq!(owned.records[0].text, "é");
    assert_eq!(owned.records[0].tags, [1, 2]);
    drop(failure);
}
