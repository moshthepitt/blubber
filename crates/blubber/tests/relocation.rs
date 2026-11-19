//! Host borrows across physical moves, callback unwinds and forgotten handles.

#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

#[path = "support/record.rs"]
mod records;

blubber::__root_schema!(__ConcreteRoot0, Option<Interrupt>);

use blubber::raw::{Edit, Resize};
use blubber::{Error, ErrorKind, List, Map, Schema, SliceStore, Str, View};
use records::{Record, RecordInit};
use std::panic::{catch_unwind, AssertUnwindSafe};

#[blubber::schema(account)]
struct Root {
    records: List<Record>,
    mapped: Map<i16, Record>,
    tail: Str,
}

fn init() -> RootInit<'static> {
    RootInit {
        records: &[
            RecordInit {
                name: "one",
                tags: &[1],
            },
            RecordInit {
                name: "two",
                tags: &[2],
            },
        ],
        mapped: &[
            (
                -1,
                RecordInit {
                    name: "first",
                    tags: &[3],
                },
            ),
            (
                2,
                RecordInit {
                    name: "last",
                    tags: &[4],
                },
            ),
        ],
        tail: "retained suffix",
    }
}

macro_rules! backends {
    ($slice:ident, $vec:ident, $exercise:ident) => {
        #[test]
        fn $slice() {
            let mut room = [0; 1024];
            $exercise(&mut SliceStore::new(&mut room));
        }

        #[cfg(feature = "alloc")]
        #[test]
        fn $vec() {
            let mut bytes = vec![0; 1024];
            $exercise(&mut blubber::VecStore::new(&mut bytes));
        }
    };
}

fn readers_then_resize(store: &mut dyn Resize) {
    let mut root = Root::initialize(store, init()).unwrap();
    for round in 0..3 {
        // These references survive the temporary Ref, iterator and projection.
        let first = root.records().iter().next().unwrap().name();
        let last = root.mapped().try_iter().last().unwrap().unwrap().1.name();
        assert_eq!((first, last), ("one", "last"));
        let mut list = root.records_mut();
        let from_mut = (&list).into_iter().next().unwrap().name();
        let mut iter = list.try_iter().unwrap();
        let mut cloned = iter.clone();
        let from_next = iter.next().unwrap().unwrap().name();
        assert_eq!(cloned.next().unwrap().unwrap().name(), from_mut);
        assert_eq!(iter.next().unwrap().unwrap().name(), "two");
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
        assert_eq!((from_mut, from_next), ("one", "one"));
        list.reborrow()
            .at_mut(0)
            .unwrap()
            .tags_mut()
            .push(round)
            .unwrap();

        let map = root.mapped_mut();
        let mut iter = (&map).into_iter();
        let (key, record) = iter.next().unwrap();
        assert_eq!((key, record.name()), (-1, "first"));
        let mut cloned = iter.clone();
        assert_eq!(iter.next().unwrap().1.name(), "last");
        assert_eq!(cloned.next().unwrap().0, 2);
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());

        root.tail_mut().set("").unwrap();
        assert!(root.shrink_to_fit().unwrap());
        assert_eq!(root.storage_len(), root.used_len());
        root.reserve(64).unwrap();
        root.tail_mut().set("retained suffix").unwrap();
        assert_eq!(root.records().get(1).unwrap().name(), "two");
        assert_eq!(root.mapped().get(&2).unwrap().name(), "last");
        root.validate().unwrap();
    }
    let receipt = root.commit().unwrap();
    assert!(receipt.storage_len >= receipt.used_len);
    assert!(store.is_closed());
}

backends!(
    slice_readers_then_resize,
    vec_readers_then_resize,
    readers_then_resize
);

#[cfg(feature = "alloc")]
#[test]
fn vec_physical_shrink_relocates_before_fresh_reads_and_regrowth() {
    let mut bytes = vec![0; 1024];
    let mut store = blubber::VecStore::new(&mut bytes);
    let mut root = Root::initialize(&mut store, init()).unwrap();
    for _ in 0..3 {
        let address = root.records().get(0).unwrap().name().as_ptr();
        assert!(root.shrink_to_fit().unwrap());
        // The new allocation was reserved while the original was still live.
        assert!(!core::ptr::eq(
            address,
            root.records().get(0).unwrap().name().as_ptr()
        ));
        root.reserve(64).unwrap();
        root.records_mut()
            .at_mut(0)
            .unwrap()
            .name_mut()
            .push_str("é")
            .unwrap();
        assert_eq!(root.mapped().get(&2).unwrap().name(), "last");
        root.validate().unwrap();
    }
    assert_eq!(root.records().get(0).unwrap().name(), "oneééé");
}

fn callback_unwind(store: &mut dyn Resize) {
    {
        let mut root = Root::initialize(store, init()).unwrap();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            root.records_mut()
                .try_for_each_mut(|record| {
                    record.name_mut().push_str(" edited")?;
                    assert_ne!(record.tags().get(0), Some(2), "callback interrupted");
                    Ok(())
                })
                .unwrap();
        }));
        assert!(panic.is_err());
        assert_eq!(root.records().get(0).unwrap().name(), "one edited");
        assert_eq!(root.records().get(1).unwrap().name(), "two edited");
        let panic = catch_unwind(AssertUnwindSafe(|| {
            root.mapped_mut()
                .try_for_each_mut(|key, record| {
                    record.name_mut().set("committed edit")?;
                    assert_ne!(key, -1, "map callback interrupted");
                    Ok(())
                })
                .unwrap();
        }));
        assert!(panic.is_err());
        assert_eq!(root.mapped().get(&-1).unwrap().name(), "committed edit");
        assert_eq!(root.mapped().get(&2).unwrap().name(), "last");
        assert_eq!(root.tail(), "retained suffix");
        // A callback panic between complete operations does not poison bytes.
        root.validate().unwrap();
        root.records_mut()
            .at_mut(1)
            .unwrap()
            .name_mut()
            .set("retry")
            .unwrap();
    }
    assert!(!store.is_poisoned());
    Root::load_mut(store).unwrap().validate().unwrap();
}

backends!(slice_callback_unwind, vec_callback_unwind, callback_unwind);

#[allow(
    clippy::forget_non_drop,
    reason = "Exercise forgotten lending handles and iterators"
)]
fn swap_forget(left: &mut dyn Resize, right: &mut dyn Resize) {
    let mut a = Root::initialize(left, init()).unwrap();
    let mut b = Root::initialize(right, init()).unwrap();
    let mut first = a.records_mut().cursor_mut().unwrap();
    let mut second = b.records_mut().cursor_mut().unwrap();
    first
        .next()
        .unwrap()
        .unwrap()
        .name_mut()
        .set("left first")
        .unwrap();
    core::mem::swap(&mut first, &mut second);
    let mut child = first.next().unwrap().unwrap();
    child.name_mut().set("right first").unwrap();
    core::mem::forget(child);
    first.next().unwrap().unwrap().tags_mut().push(77).unwrap();
    let mut child = second.next().unwrap().unwrap();
    child.name_mut().set("left second").unwrap();
    core::mem::forget(child);
    assert!(second.next().unwrap().is_none());
    core::mem::forget(first);
    core::mem::forget(second);
    assert_eq!(a.records().get(0).unwrap().name(), "left first");
    assert_eq!(a.records().get(1).unwrap().name(), "left second");
    assert_eq!(b.records().get(0).unwrap().name(), "right first");
    assert_eq!(b.records().get(1).unwrap().tags().get(1), Some(77));

    let mut first = a.mapped_mut().cursor_mut().unwrap();
    let mut second = b.mapped_mut().cursor_mut().unwrap();
    assert_eq!(first.next().unwrap().unwrap().0, -1);
    core::mem::swap(&mut first, &mut second);
    let (key, mut child) = first.next().unwrap().unwrap();
    assert_eq!(key, -1);
    child.name_mut().set("right map").unwrap();
    core::mem::forget(child);
    let (key, mut child) = second.next().unwrap().unwrap();
    assert_eq!(key, 2);
    child.name_mut().set("left map").unwrap();
    core::mem::forget(child);
    core::mem::forget(first);
    core::mem::forget(second);
    assert_eq!(a.mapped().get(&2).unwrap().name(), "left map");
    assert_eq!(b.mapped().get(&-1).unwrap().name(), "right map");

    let mut value = a
        .mapped_mut()
        .into_entry(9)
        .unwrap()
        .or_insert(RecordInit {
            name: "new",
            tags: &[],
        })
        .unwrap();
    value.name_mut().push_str(" value").unwrap();
    core::mem::forget(value);
    let mut records = a.records_mut();
    core::mem::forget(records.iter());
    records
        .push(RecordInit {
            name: "after forget",
            tags: &[],
        })
        .unwrap();
    core::mem::swap(&mut a, &mut b);
    a.tail_mut().set("right root").unwrap();
    b.tail_mut().set("left root").unwrap();
    a.validate().unwrap();
    b.validate().unwrap();
    core::mem::forget(a);
    core::mem::forget(b);
    assert!(!left.is_poisoned());
    assert!(!right.is_poisoned());
    assert_eq!(Root::load(left.bytes()).unwrap().tail(), "left root");
    assert_eq!(Root::load(right.bytes()).unwrap().tail(), "right root");
}

#[test]
fn slice_swapped_and_forgotten_handles_release_borrows() {
    let mut left = [0; 1024];
    let mut right = [0; 1024];
    swap_forget(
        &mut SliceStore::new(&mut left),
        &mut SliceStore::new(&mut right),
    );
    Root::load(&left).unwrap();
    Root::load(&right).unwrap();
}

#[cfg(feature = "alloc")]
#[test]
fn vec_swapped_and_forgotten_handles_release_borrows() {
    let mut left = vec![0; 1024];
    let mut right = vec![0; 1024];
    swap_forget(
        &mut blubber::VecStore::new(&mut left),
        &mut blubber::VecStore::new(&mut right),
    );
    Root::load(&left).unwrap();
    Root::load(&right).unwrap();
}

// Deliberately unsupported encoder: interrupt after writing an invalid bool.
struct Interrupt;

impl Schema for Interrupt {
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        &blubber::identity::WireDescriptor::Custom {
            codec: blubber::identity::CodecIdentity {
                namespace: "test:relocation:Interrupt",
                epoch: 0,
            },
            wire: <bool as blubber::Schema>::WIRE_DESCRIPTOR,
        };
    const KIND: blubber::Kind = blubber::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <bool as Schema>::LAYOUT;
    const __LEN: usize = <bool as blubber::Schema>::__LEN;
    const __HEADER: &'static [u32] = <bool as blubber::Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = bool;
    type Mut<'a> = blubber::ScalarMut<'a, bool>;
    type Init<'a> = u8;
    type Removed = bool;

    fn read(view: View<'_>) -> blubber::Result<bool> {
        view.read::<bool>()
    }
    fn project(edit: blubber::__private::Projection<'_, Self>) -> Self::Mut<'_> {
        blubber::raw::scalar(blubber::raw::into_edit(edit))
    }
    fn read_removed(view: View<'_>) -> blubber::Result<bool> {
        Self::read(view)
    }
    fn measure(_: &u8) -> blubber::Result<u32> {
        Ok(1)
    }
    fn encode(value: &u8, destination: &mut [u8]) {
        destination[0] = *value;
        assert_ne!(*value, 99, "encoder interrupted");
    }
    #[cfg(feature = "alloc")]
    type Owned = bool;
    #[cfg(feature = "alloc")]
    fn measure_owned(_: &bool) -> blubber::Result<u32> {
        Ok(1)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &bool, destination: &mut [u8]) {
        destination[0] = u8::from(*value);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: View<'_>) -> blubber::Result<bool> {
        Self::read(view)
    }
}

fn interrupt(store: &mut dyn Resize) {
    let edit = Edit::initialize::<__ConcreteRoot0>(store, *b"unwind35", Some(1)).unwrap();
    let mut value = blubber::raw::project::<Option<Interrupt>>(edit);
    assert!(catch_unwind(AssertUnwindSafe(|| value.set(Some(99)))).is_err());
    assert_eq!(value.try_get(), Err(ErrorKind::Poisoned.into()));
    assert_eq!(value.set(None), Err(ErrorKind::Poisoned.into()));
}

fn poison_recovery(store: &mut dyn Resize) {
    interrupt(store);
    let before = store.bytes().to_vec();
    assert!(store.is_poisoned());
    assert!(matches!(
        Edit::load::<__ConcreteRoot0>(store, *b"unwind35"),
        Err(Error::Fault {
            kind: ErrorKind::InvalidBool,
            ..
        })
    ));
    assert!(store.is_poisoned());
    assert_eq!(store.bytes(), before);
    // Repair the known fixture's bool, then require full validation to recover.
    store.bytes_mut()[13] = 1;
    let mut root = Edit::load::<__ConcreteRoot0>(store, *b"unwind35").unwrap();
    assert_eq!(
        root.view().unwrap().read::<Option<Interrupt>>(),
        Ok(Some(true))
    );
    root.reserve(64).unwrap();
    assert!(root.shrink_to_fit().unwrap());
    root.commit().unwrap();
    assert!(!store.is_poisoned());
    assert!(store.is_closed());
}

backends!(slice_poison_recovery, vec_poison_recovery, poison_recovery);

fn poison_commit(store: &mut dyn Resize) {
    interrupt(store);
    assert_eq!(store.commit(14), Err(ErrorKind::Poisoned.into()));
    assert!(store.is_closed());
    assert!(store.is_poisoned());
    assert_eq!(store.commit(14), Err(ErrorKind::Closed.into()));
    assert!(
        matches!(Edit::load::<__ConcreteRoot0>(store, *b"unwind35"), Err(e) if e == ErrorKind::Closed.into())
    );
}

backends!(slice_poison_commit, vec_poison_commit, poison_commit);

#[blubber::schema(account)]
struct InterruptedRoot {
    value: Option<Interrupt>,
}

fn typed_poison_commit(store: &mut dyn Resize) {
    let mut root =
        InterruptedRoot::initialize(store, InterruptedRootInit { value: Some(1) }).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| root.value_mut().set(Some(99)))).is_err());
    assert_eq!(root.commit(), Err(ErrorKind::Poisoned.into()));
    assert!(store.is_closed());
    assert!(store.is_poisoned());
    assert_eq!(store.commit(14), Err(ErrorKind::Closed.into()));
    assert!(matches!(InterruptedRoot::load_mut(store), Err(e) if e == ErrorKind::Closed.into()));
}

backends!(
    slice_typed_poison_commit,
    vec_typed_poison_commit,
    typed_poison_commit
);
