//! Private fault and relocation controls for the production host backend.

use super::*;
use crate::storage::Backend as Resize;
use crate::Resize as _;

use alloc::{vec, vec::Vec};
use blubber_core::raw::{begin_apply, plan_replace, InlinePath};
use blubber_core::Layout;

#[cfg(feature = "derive")]
#[test]
fn scoped_point_reacquires_forced_relocation_and_retries_growth() {
    #[crate::schema(account)]
    struct Words {
        words: crate::List<crate::Str>,
        tail: crate::Str,
    }
    let mut buffer = Vec::with_capacity(1024);
    let mut store = VecStore::new(&mut buffer);
    store.test_mode.relocate = true;
    Words::initialize(
        &mut store,
        WordsInit {
            words: &["a", "b"],
            tail: "tail",
        },
    )
    .unwrap();
    let address = store.bytes().as_ptr();
    store.test_mode.fail_next = Some(true);
    let mut root = Words::load_mut(&mut store).unwrap();
    root.words_mut()
        .with_mut(0, |word| {
            assert_eq!(word.set("longer"), Err(ErrorKind::Allocation.into()));
            assert_eq!(word.get(), "a");
            word.set("longer")?;
            assert_eq!(word.get(), "longer");
            word.set("equal!")?;
            assert_eq!(word.get(), "equal!");
            word.set("x")?;
            word.set("y")
        })
        .unwrap();
    assert_eq!(root.words().get(0), Some("y"));
    assert_eq!(root.words().get(1), Some("b"));
    assert_eq!(root.tail(), "tail");
    root.validate().unwrap();
    assert_ne!(address, store.bytes().as_ptr());
    assert_eq!(store.test_mode.resize_calls, 3);
    assert!(!store.is_poisoned());
}

#[test]
fn rewrite_kernel_reacquires_forced_relocation_after_atomic_allocation_failure() {
    let mut bytes = Vec::with_capacity(1024);
    bytes.extend_from_slice(b"rewrite8");
    bytes.extend_from_slice(&22u32.to_le_bytes());
    bytes.extend_from_slice(&[2, 0, 0, 0, 1, 0, 0, 0, b'a', b'b']);
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.relocate = true;
    let before = store.bytes().to_vec();
    let address = store.bytes().as_ptr();
    let layout = Layout::List {
        element: &Layout::Str,
    };
    let apply = |store: &mut dyn Resize| {
        crate::rewrite::rewrite_collection(
            store,
            &layout,
            &InlinePath::new(),
            (*b"rewrite8", 4),
            |view| {
                Ok((
                    blubber_core::raw::RewritePlan::check(view.bytes(), &view.layout(), 2, 6)?,
                    (),
                ))
            },
            |(), stream| {
                for _ in 0..2 {
                    {
                        stream.prepare_slot(3)?;
                        stream.destination()
                    }
                    .copy_from_slice(b"new");
                    stream.advance(3)?;
                }
                Ok(())
            },
        )
    };
    store.test_mode.fail_next = Some(true);
    assert_eq!(apply(&mut store), Err(ErrorKind::Allocation.into()));
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
    assert_eq!(store.test_mode.resize_calls, 1);
    apply(&mut store).unwrap();
    assert_eq!(store.test_mode.resize_calls, 2);
    assert!(!core::ptr::eq(address, store.bytes().as_ptr()));
    assert!(!store.is_poisoned());
    assert_eq!(&store.bytes()[8..12], &26u32.to_le_bytes());
    assert_eq!(&store.bytes()[20..26], b"newnew");
    blubber_core::validate(&store.bytes()[12..26], &layout, blubber_core::MAX_DEPTH).unwrap();
}

#[cfg(feature = "derive")]
#[path = "../../tests/support/record.rs"]
mod records;

#[cfg(feature = "derive")]
#[test]
fn typed_rewrite_reserves_once_and_retries_before_forced_relocation() {
    use crate::{List, Str};
    #[crate::schema(account)]
    struct Batch {
        values: List<Str>,
        tail: Str,
    }
    let mut bytes = Vec::with_capacity(1024);
    let mut store = VecStore::new(&mut bytes);
    Batch::initialize(
        &mut store,
        BatchInit {
            values: &["a", "b"],
            tail: "suffix",
        },
    )
    .unwrap();
    store.test_mode.relocate = true;
    store.test_mode.resize_calls = 0;
    store.test_mode.fail_next = Some(true);
    let before = store.bytes().to_vec();
    let address = store.bytes().as_ptr();
    {
        let mut root = Batch::load_mut(&mut store).unwrap();
        assert_eq!(
            root.values_mut().fill("replacement grows"),
            Err(ErrorKind::Allocation.into())
        );
        assert_eq!(root.values().get(0), Some("a"));
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
    assert_eq!(store.test_mode.resize_calls, 1);
    {
        let mut root = Batch::load_mut(&mut store).unwrap();
        root.values_mut()
            .rewrite_from(&["first grows", "second grows"])
            .unwrap();
        assert_eq!(root.values().get(1), Some("second grows"));
        assert_eq!(root.tail(), "suffix");
        root.validate().unwrap();
    }
    assert_eq!(store.test_mode.resize_calls, 2);
    assert!(!core::ptr::eq(address, store.bytes().as_ptr()));
    assert!(!store.is_poisoned());
}

#[cfg(feature = "derive")]
#[test]
fn typed_list_edits_reacquire_forced_relocations_and_retry_failed_growth() {
    use records::{RecordInit, Records, RecordsInit};

    let mut bytes = Vec::with_capacity(1024);
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.relocate = true;
    Records::initialize(&mut store, RecordsInit { records: &[] }).unwrap();
    let before = store.bytes().to_vec();
    store.test_mode.fail_next = Some(true);
    {
        let mut root = Records::load_mut(&mut store).unwrap();
        let mut list = root.records_mut();
        assert_eq!(
            list.push(RecordInit {
                name: "failed",
                tags: &[1]
            }),
            Err(ErrorKind::Allocation.into())
        );
        assert!(list.is_empty());
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
    let address = store.bytes().as_ptr();
    {
        let mut root = Records::load_mut(&mut store).unwrap();
        let mut list = root.records_mut();
        list.push(RecordInit {
            name: "first",
            tags: &[1],
        })
        .unwrap();
        let first_address = list.get(0).unwrap().name().as_ptr();
        list.insert(
            0,
            RecordInit {
                name: "",
                tags: &[],
            },
        )
        .unwrap();
        assert!(!core::ptr::eq(
            first_address,
            list.get(1).unwrap().name().as_ptr()
        ));
        list.replace(
            1,
            RecordInit {
                name: "a longer record",
                tags: &[2, 3],
            },
        )
        .unwrap();
        let mut record = list.reborrow().at_mut(1).unwrap();
        record.tags_mut().push(u32::MAX).unwrap();
        assert_eq!(record.tags().get(2), Some(u32::MAX));
        list.remove(0).unwrap();
        assert_eq!(list.get(0).unwrap().name(), "a longer record");
        root.validate().unwrap();
    }
    assert!(!core::ptr::eq(address, store.bytes().as_ptr()));
}

#[cfg(feature = "derive")]
#[test]
fn typed_bulk_retries_failed_growth_and_reacquires_relocated_storage() {
    use records::{RecordInit, Records, RecordsInit};
    let mut bytes = Vec::with_capacity(1024);
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.relocate = true;
    Records::initialize(&mut store, RecordsInit { records: &[] }).unwrap();
    let before = store.bytes().to_vec();
    let values = [
        RecordInit {
            name: "first",
            tags: &[1, 2],
        },
        RecordInit {
            name: "second",
            tags: &[],
        },
    ];
    store.test_mode.fail_next = Some(true);
    {
        let mut root = Records::load_mut(&mut store).unwrap();
        assert_eq!(
            root.records_mut().extend_from_slice(&values),
            Err(ErrorKind::Allocation.into())
        );
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
    let address = store.bytes().as_ptr();
    {
        let mut root = Records::load_mut(&mut store).unwrap();
        let mut records = root.records_mut();
        records.extend_from_slice(&values).unwrap();
        let name_address = records.get(0).unwrap().name().as_ptr();
        records.insert_from_slice(1, &values).unwrap();
        assert!(!core::ptr::eq(
            name_address,
            records.get(0).unwrap().name().as_ptr()
        ));
        assert_eq!(
            (&records)
                .into_iter()
                .map(|row| row.name())
                .collect::<Vec<_>>(),
            ["first", "first", "second", "second"]
        );
        root.validate().unwrap();
    }
    assert!(!core::ptr::eq(address, store.bytes().as_ptr()));
}

fn account(root: &[u8]) -> Vec<u8> {
    let mut bytes = b"blubber!".to_vec();
    bytes.extend_from_slice(&(12 + root.len() as u32).to_le_bytes());
    bytes.extend_from_slice(root);
    bytes
}

#[test]
fn relocation_is_forced_even_with_spare_vec_capacity() {
    let mut bytes = Vec::with_capacity(256);
    bytes.extend_from_slice(b"retained prefix");
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.relocate = true;
    for length in 16..24 {
        let before = store.bytes().to_vec();
        let address = store.bytes().as_ptr();
        store.resize(length).unwrap();
        assert!(!core::ptr::eq(address, store.bytes().as_ptr()));
        assert_eq!(&store.bytes()[..before.len()], before);
        assert!(store.bytes()[before.len()..].iter().all(|&byte| byte == 0));
    }
}

#[test]
fn failed_growth_preserves_every_byte_and_length() {
    let mut bytes = vec![0xa5; 20];
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.fail_next = Some(true);
    let before = store.bytes().to_vec();
    let address = store.bytes().as_ptr();
    // Preflight is read-only and does not consume the injected failure.
    store.check_resize(24).unwrap();
    assert_eq!(store.reserve(12, 12), Err(ErrorKind::Allocation.into()));
    assert_eq!(store.bytes(), before);
    assert!(core::ptr::eq(address, store.bytes().as_ptr()));
    assert_eq!(store.storage_len(), 20);
    assert!(!store.is_poisoned());
    store.reserve(12, 12).unwrap();
    assert_eq!(&store.bytes()[..20], before);
    assert_eq!(&store.bytes()[20..], &[0; 4]);
    assert_eq!(store.commit(12).unwrap().shrink_error, None);
}

#[test]
fn failed_shrink_retains_storage_and_reports_error_at_commit() {
    let mut bytes = vec![0xa5; 24];
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.fail_next = Some(false);
    assert!(!store.shrink_to_fit(12).unwrap());
    assert_eq!(store.bytes(), &[0xa5; 24]);
    assert_eq!(store.storage_len(), 24);
    // A subsequent growth does not erase evidence of the failed trim.
    store.reserve(12, 16).unwrap();
    let receipt = store.commit(12).unwrap();
    assert_eq!(receipt.storage_len, 28);
    assert_eq!(receipt.shrink_error, Some(ErrorKind::Allocation.into()));
}

#[test]
fn direct_shrink_failure_is_atomic_and_success_clears_receipt() {
    let mut bytes = vec![0xa5; 24];
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.fail_next = Some(false);
    let address = store.bytes().as_ptr();
    assert_eq!(store.resize(12), Err(ErrorKind::Allocation.into()));
    assert_eq!(store.bytes(), &[0xa5; 24]);
    assert!(core::ptr::eq(address, store.bytes().as_ptr()));
    assert!(store.shrink_to_fit(12).unwrap());
    assert_eq!(store.bytes(), &[0xa5; 12]);
    assert_eq!(store.buffer.capacity(), 12);
    assert_eq!(store.commit(12).unwrap().shrink_error, None);
}

#[test]
fn kernel_plan_survives_forced_relocation_and_reacquires_current_bytes() {
    let mut bytes = account(b"old");
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.relocate = true;
    for input in [b"a longer value".as_slice(), b"x", b"long again"] {
        let used = u32::from_le_bytes(store.bytes()[8..12].try_into().unwrap());
        let plan = plan_replace(
            store.bytes(),
            used,
            &Layout::Str,
            &InlinePath::new(),
            input.len() as u32,
            u32::MAX,
        )
        .unwrap();
        let needed = plan.required_storage_len() as usize;
        store.check_resize(needed).unwrap();
        if needed > store.storage_len() {
            store.resize(needed).unwrap();
        }
        plan.check_ready(store.bytes()).unwrap();
        store.set_poisoned(true);
        let mut apply = begin_apply(store.bytes_mut(), plan).unwrap();
        apply.move_regions();
        apply.destination().copy_from_slice(input);
        apply.finish();
        store.set_poisoned(false);
        let new_used = 12 + input.len();
        assert_eq!(&store.bytes()[12..new_used], input);
        assert_eq!(&store.bytes()[8..12], &(new_used as u32).to_le_bytes());
        assert!(store.bytes()[new_used..].iter().all(|&byte| byte == 0));
        assert!(store.shrink_to_fit(new_used as u32).unwrap());
    }
    assert_eq!(store.commit(22).unwrap().storage_len, 22);
}

#[test]
fn failed_growth_leaves_a_prepared_plan_retryable() {
    let mut bytes = account(b"old");
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.fail_next = Some(true);
    let before = store.bytes().to_vec();
    let plan = plan_replace(
        store.bytes(),
        15,
        &Layout::Str,
        &InlinePath::new(),
        10,
        u32::MAX,
    )
    .unwrap();
    assert_eq!(store.resize(22), Err(ErrorKind::Allocation.into()));
    assert_eq!(store.bytes(), before);
    store.test_mode.relocate = true;
    store.resize(22).unwrap();
    plan.check_ready(store.bytes()).unwrap();
    store.set_poisoned(true);
    let mut apply = begin_apply(store.bytes_mut(), plan).unwrap();
    apply.move_regions();
    apply.destination().copy_from_slice(b"0123456789");
    apply.finish();
    store.set_poisoned(false);
    assert_eq!(store.bytes(), account(b"0123456789"));
}

#[test]
#[allow(
    clippy::forget_non_drop,
    reason = "Forgetting an unfinished Apply must leave the owning store poisoned"
)]
fn forgotten_apply_keeps_poison_and_commit_closes_it() {
    let mut bytes = account(b"old");
    let mut store = VecStore::new(&mut bytes);
    let plan = plan_replace(store.bytes(), 15, &Layout::Str, &InlinePath::new(), 3, 15).unwrap();
    store.set_poisoned(true);
    let mut apply = begin_apply(store.bytes_mut(), plan).unwrap();
    apply.move_regions();
    core::mem::forget(apply);
    assert!(store.is_poisoned());
    assert_eq!(store.reserve(15, 0), Err(ErrorKind::Poisoned.into()));
    assert_eq!(store.commit(15), Err(ErrorKind::Poisoned.into()));
    assert!(store.is_closed());
}

#[cfg(feature = "derive")]
mod typed_projections {
    use super::*;
    use crate::Str;

    #[crate::schema]
    struct Child {
        text: Str,
        option: Option<Str>,
    }

    #[crate::schema(account)]
    struct Root {
        child: Child,
        tail: Str,
    }

    fn init() -> RootInit<'static> {
        RootInit {
            child: ChildInit {
                text: "a",
                option: None,
            },
            tail: "tail",
        }
    }

    #[test]
    fn typed_append_and_some_edits_reacquire_forced_relocations() {
        let mut bytes = Vec::with_capacity(1024);
        let mut store = VecStore::new(&mut bytes);
        store.test_mode.relocate = true;
        let mut root = Root::initialize(&mut store, init()).unwrap();
        let mut record = root.child_mut();
        let mut text = record.text_mut();
        let first_address = text.try_get().unwrap().as_ptr();
        text.push_str(" longer").unwrap();
        assert!(!core::ptr::eq(
            first_address,
            text.try_get().unwrap().as_ptr()
        ));
        text.set("a retained editor after relocation").unwrap();
        assert_eq!(text.try_get(), Ok("a retained editor after relocation"));
        let next_address = record.text().as_ptr();
        record.option_mut().set(Some("present")).unwrap();
        assert!(!core::ptr::eq(next_address, record.text().as_ptr()));
        record
            .option_mut()
            .some_mut()
            .unwrap()
            .push_str(" payload")
            .unwrap();
        record.text_mut().truncate(1).unwrap();
        assert_eq!(record.text(), "a");
        assert_eq!(record.option(), Some("present payload"));
        assert_eq!(root.tail(), "tail");
        root.validate().unwrap();
        let used = root.used_len();
        assert_eq!(root.commit().unwrap().used_len, used);
    }

    #[test]
    fn typed_allocation_failure_preserves_storage_and_a_retained_child() {
        let mut bytes = Vec::new();
        let mut store = VecStore::new(&mut bytes);
        Root::initialize(&mut store, init()).unwrap();
        let before = store.bytes().to_vec();
        store.test_mode.fail_next = Some(true);
        {
            let mut root = Root::load_mut(&mut store).unwrap();
            let mut record = root.child_mut();
            assert_eq!(
                record.text_mut().push_str("must grow"),
                Err(ErrorKind::Allocation.into())
            );
            assert_eq!(record.text(), "a");
            record.validate().unwrap();
        }
        assert_eq!(store.bytes(), before);
        assert!(!store.is_poisoned());
        let mut root = Root::load_mut(&mut store).unwrap();
        root.child_mut()
            .text_mut()
            .push_str("retry succeeds")
            .unwrap();
        root.validate().unwrap();
    }

    #[test]
    #[allow(clippy::forget_non_drop)] // Exercise forgotten handles deliberately.
    fn completed_typed_edits_survive_forget_and_handle_swaps() {
        let mut first = Vec::new();
        let mut second = Vec::new();
        {
            let mut left = VecStore::new(&mut first);
            let mut right = VecStore::new(&mut second);
            let mut a = Root::initialize(&mut left, init()).unwrap();
            let mut b = Root::initialize(&mut right, init()).unwrap();
            core::mem::swap(&mut a, &mut b);
            a.child_mut().text_mut().set("second account").unwrap();
            b.child_mut().text_mut().set("first account").unwrap();
            core::mem::forget(a);
            core::mem::forget(b);
            core::mem::forget(left);
            core::mem::forget(right);
        }
        assert_eq!(Root::load(&first).unwrap().child().text(), "first account");
        assert_eq!(
            Root::load(&second).unwrap().child().text(),
            "second account"
        );
    }
}

#[test]
fn typed_map_entries_reacquire_forced_relocations_and_retry_failed_growth() {
    use crate::{Entry, Map, Str};
    const ID: [u8; 8] = *b"mapstore";
    let mut bytes = Vec::with_capacity(1024);
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.relocate = true;
    {
        crate::__root_schema!(__ConcreteRoot, Map<u16, Str>);
        crate::Edit::initialize::<__ConcreteRoot>(&mut store, ID, &[])
    }
    .unwrap();
    let before = store.bytes().to_vec();
    store.test_mode.fail_next = Some(true);
    {
        let mut map = crate::raw::project::<Map<u16, Str>>(
            {
                crate::__root_schema!(__ConcreteRoot, Map<u16, Str>);
                crate::Edit::load::<__ConcreteRoot>(&mut store, ID)
            }
            .unwrap(),
        );
        assert_eq!(map.insert(2, "failed"), Err(ErrorKind::Allocation.into()));
        assert!(map.is_empty());
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
    {
        let mut edit = {
            crate::__root_schema!(__ConcreteRoot, Map<u16, Str>);
            crate::Edit::load::<__ConcreteRoot>(&mut store, ID)
        }
        .unwrap();
        let mut map = crate::raw::project::<Map<u16, Str>>(edit.reborrow());
        map.insert(2, "first").unwrap();
        let address = map.get(&2).unwrap().as_ptr();
        let mut value = map.entry(1).unwrap().or_insert("").unwrap();
        value.push_str("retained entry growth").unwrap();
        value.push_str(" after relocation").unwrap();
        assert!(!core::ptr::eq(address, map.get(&2).unwrap().as_ptr()));
        match map.entry(2).unwrap() {
            Entry::Occupied(entry) => assert_eq!(entry.insert("longer replacement"), Ok(())),
            Entry::Vacant(_) => panic!("existing key"),
        }
        assert_eq!(map.remove(&1), Ok(Some(())));
        assert_eq!(map.get(&2), Some("longer replacement"));
        edit.validate().unwrap();
    }
    let before = store.bytes().to_vec();
    let growth = "x".repeat(store.storage_len());
    store.test_mode.fail_next = Some(true);
    {
        let mut map = crate::raw::project::<Map<u16, Str>>(
            {
                crate::__root_schema!(__ConcreteRoot, Map<u16, Str>);
                crate::Edit::load::<__ConcreteRoot>(&mut store, ID)
            }
            .unwrap(),
        );
        assert!(
            matches!(map.entry(3).unwrap().or_insert(&growth), Err(e) if e == ErrorKind::Allocation.into())
        );
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(feature = "derive")]
#[test]
fn lending_cursors_and_callbacks_reacquire_forced_relocations() {
    use crate::{List, Map};
    use records::{Record, RecordInit};

    let mut bytes = Vec::with_capacity(1024);
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.relocate = true;
    {
        let edit = {
            crate::__root_schema!(__ConcreteRoot, List<Record>);
            crate::Edit::initialize::<__ConcreteRoot>(
                &mut store,
                *b"listlend",
                &[
                    RecordInit {
                        name: "first",
                        tags: &[],
                    },
                    RecordInit {
                        name: "last",
                        tags: &[1],
                    },
                ],
            )
        }
        .unwrap();
        let mut list = crate::raw::project::<List<Record>>(edit);
        let mut cursor = list.reborrow().cursor_mut().unwrap();
        for expected in ["first", "last"] {
            let mut record = cursor.next().unwrap().unwrap();
            assert_eq!(record.name(), expected);
            let address = record.name().as_ptr();
            record.name_mut().push_str(" forced growth").unwrap();
            assert!(!core::ptr::eq(address, record.name().as_ptr()));
            let address = record.name().as_ptr();
            record.tags_mut().push(42).unwrap();
            assert!(!core::ptr::eq(address, record.name().as_ptr()));
        }
        assert!(cursor.next().unwrap().is_none());
        list.try_for_each_mut(|record| {
            let address = record.name().as_ptr();
            record.name_mut().push_str(" callback")?;
            assert!(!core::ptr::eq(address, record.name().as_ptr()));
            Ok(())
        })
        .unwrap();
        assert_eq!(list.get(1).unwrap().name(), "last forced growth callback");
        list.validate().unwrap();
    }
    let mut bytes = Vec::with_capacity(1024);
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.relocate = true;
    let edit = {
        crate::__root_schema!(__ConcreteRoot, Map<u16, Record>);
        crate::Edit::initialize::<__ConcreteRoot>(
            &mut store,
            *b"maplend!",
            &[
                (
                    1,
                    RecordInit {
                        name: "first",
                        tags: &[],
                    },
                ),
                (
                    256,
                    RecordInit {
                        name: "last",
                        tags: &[1],
                    },
                ),
            ],
        )
    }
    .unwrap();
    let mut map = crate::raw::project::<Map<u16, Record>>(edit);
    let mut cursor = map.reborrow().cursor_mut().unwrap();
    for expected in [1, 256] {
        let (key, mut record) = cursor.next().unwrap().unwrap();
        assert_eq!(key, expected);
        let address = record.name().as_ptr();
        record.name_mut().push_str(" forced growth").unwrap();
        assert!(!core::ptr::eq(address, record.name().as_ptr()));
        record.tags_mut().push(u32::from(key)).unwrap();
    }
    assert!(cursor.next().unwrap().is_none());
    map.try_for_each_mut(|_, record| {
        let address = record.name().as_ptr();
        record.name_mut().push_str(" callback")?;
        assert!(!core::ptr::eq(address, record.name().as_ptr()));
        Ok(())
    })
    .unwrap();
    assert_eq!(map.get(&256).unwrap().name(), "last forced growth callback");
    map.validate().unwrap();
}

#[test]
fn exact_room_initialization_retains_a_prior_failed_shrink_receipt() {
    let mut bytes = vec![0; 16];
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.fail_next = Some(false);
    assert_eq!(store.shrink_to_fit(12), Ok(false));
    assert_eq!(store.storage_len(), 16);
    let edit = {
        crate::__root_schema!(__ConcreteRoot, u32);
        crate::Edit::initialize::<__ConcreteRoot>(&mut store, *b"initroom", 42)
    }
    .unwrap();
    let receipt = edit.commit().unwrap();
    assert_eq!(receipt.used_len, 16);
    assert_eq!(receipt.storage_len, 16);
    assert_eq!(receipt.shrink_error, Some(ErrorKind::Allocation.into()));
}

#[test]
fn exact_room_edit_retains_a_prior_failed_shrink_receipt() {
    let mut bytes = vec![0; 16];
    let mut store = VecStore::new(&mut bytes);
    {
        crate::__root_schema!(__ConcreteRoot, crate::Str);
        crate::Edit::initialize::<__ConcreteRoot>(&mut store, *b"editroom", "text")
    }
    .unwrap();
    store.test_mode.fail_next = Some(false);
    assert_eq!(store.shrink_to_fit(15), Ok(false));
    let edit = {
        crate::__root_schema!(__ConcreteRoot, crate::Str);
        crate::Edit::load::<__ConcreteRoot>(&mut store, *b"editroom")
    }
    .unwrap();
    let mut text = crate::raw::project::<crate::Str>(edit);
    text.set("next").unwrap();
    let receipt = {
        crate::__root_schema!(__ConcreteRoot, crate::Str);
        crate::Edit::load::<__ConcreteRoot>(&mut store, *b"editroom")
    }
    .unwrap()
    .commit()
    .unwrap();
    assert_eq!(receipt.shrink_error, Some(ErrorKind::Allocation.into()));
}

#[cfg(feature = "derive")]
#[test]
fn fixed_walks_reacquire_after_forced_reserve_and_failed_allocation() {
    crate::__root_schema!(__WalkNumbers, crate::List<u16>);
    let mut bytes = Vec::with_capacity(1024);
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.relocate = true;
    crate::Edit::initialize::<__WalkNumbers>(&mut store, *b"walkmove", &[1, 2, 3]).unwrap();
    let first = store.bytes().as_ptr();
    {
        let mut root = crate::Edit::load::<__WalkNumbers>(&mut store, *b"walkmove").unwrap();
        let mut list = crate::raw::project::<crate::List<u16>>(root.reborrow());
        list.try_for_each_mut(|value| value.set(value.get() + 1))
            .unwrap();
        root.reserve(256).unwrap();
        let mut cursor = crate::raw::project::<crate::List<u16>>(root.reborrow())
            .cursor_mut()
            .unwrap();
        assert_eq!(cursor.next().unwrap().unwrap().get(), 2);
        cursor.next().unwrap().unwrap().set(7).unwrap();
        root.validate().unwrap();
    }
    assert!(!core::ptr::eq(first, store.bytes().as_ptr()));
    let before = store.bytes().to_vec();
    store.test_mode.fail_next = Some(true);
    {
        let mut root = crate::Edit::load::<__WalkNumbers>(&mut store, *b"walkmove").unwrap();
        assert_eq!(root.reserve(4096), Err(ErrorKind::Allocation.into()));
        let mut list = crate::raw::project::<crate::List<u16>>(root.reborrow());
        let mut cursor = list.reborrow().cursor_mut().unwrap();
        assert_eq!(cursor.next().unwrap().unwrap().get(), 2);
        assert_eq!(cursor.next().unwrap().unwrap().get(), 7);
        assert_eq!(cursor.next().unwrap().unwrap().get(), 4);
    }
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(feature = "derive")]
#[test]
fn indexed_cursor_retries_allocation_and_refreshes_forced_relocations() {
    use crate::{List, Str};
    crate::__root_schema!(__IndexedRelocation, List<Str>);
    let mut bytes = Vec::with_capacity(1024);
    let mut store = VecStore::new(&mut bytes);
    store.test_mode.relocate = true;
    crate::Edit::initialize::<__IndexedRelocation>(&mut store, *b"idxreloc", &["a", "b", "c"])
        .unwrap();
    store.test_mode.fail_next = Some(true);
    let before = store.bytes().to_vec();
    let address = store.bytes().as_ptr();
    {
        let edit = crate::Edit::load::<__IndexedRelocation>(&mut store, *b"idxreloc").unwrap();
        let mut cursor = crate::raw::project::<List<Str>>(edit).cursor_mut().unwrap();
        let mut child = cursor.next().unwrap().unwrap();
        assert_eq!(
            child.set("longer failed allocation"),
            Err(ErrorKind::Allocation.into())
        );
        assert_eq!(child.get(), "a");
        child.set("longer successful allocation").unwrap();
        assert_eq!(child.get(), "longer successful allocation");
        child
            .reborrow()
            .set("again substantially longer successful allocation")
            .unwrap();
        assert_eq!(
            child.get(),
            "again substantially longer successful allocation"
        );
        let mut second = cursor.next().unwrap().unwrap();
        assert_eq!(second.get(), "b");
        second.clear().unwrap();
        second.set("B").unwrap();
        assert_eq!(cursor.next().unwrap().unwrap().get(), "c");
        assert!(cursor.next().unwrap().is_none());
    }
    assert!(!core::ptr::eq(address, store.bytes().as_ptr()));
    assert_eq!(&store.bytes()[..8], &before[..8]);
    let value = crate::View::load::<__IndexedRelocation>(store.bytes(), *b"idxreloc")
        .unwrap()
        .read::<List<Str>>()
        .unwrap();
    assert_eq!(
        value.iter().collect::<Vec<_>>(),
        ["again substantially longer successful allocation", "B", "c"]
    );
    assert!(!store.is_poisoned());
}
