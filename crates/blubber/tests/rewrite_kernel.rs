//! Run the byte kernel's independent rewrite controls under both Miri models.
#![forbid(unsafe_code)]
#[path = "../../blubber-core/tests/support/rewrite.rs"]
mod cases;

#[cfg(feature = "derive")]
#[path = "support/fixed_fill.rs"]
mod fixed_fill;

use blubber::raw::Resize;
use blubber::Resize as _;
use blubber::{ReadStorage, SliceStore};
use blubber_core::raw::{InlinePath, RewritePlan};
use blubber_core::{ErrorKind, Layout, VariableField};

#[cfg(feature = "derive")]
#[blubber::schema]
struct CheckedTail {
    values: blubber::List<blubber::Str>,
}

#[cfg(feature = "derive")]
#[blubber::schema]
struct CheckedTailMap {
    marker: u16,
    values: blubber::Map<u8, blubber::Str>,
}

static TEXTS: Layout = Layout::List {
    element: &Layout::Str,
};

static TAIL_ROOT: Layout = Layout::Struct {
    fixed_bytes: 0,
    fixed: &[],
    variable: &[VariableField {
        declaration: 0,
        layout: &TEXTS,
    }],
};

// Exercise the boundary around the byte kernel without adding Q9's typed API.
// All preflight completes before reserve; no account address survives it.
fn run(store: &mut dyn Resize, fault: u8) -> blubber::Result<()> {
    run_selected(store, fault, false)
}

fn run_selected(store: &mut dyn Resize, fault: u8, tail: bool) -> blubber::Result<()> {
    #[cfg(feature = "derive")]
    let tail_layout = Layout::Flat {
        checked: <CheckedTail as blubber::RootSchema>::__layout(),
    };
    #[cfg(not(feature = "derive"))]
    let tail_layout = TAIL_ROOT;
    let (layout, path) = if tail {
        (&tail_layout, InlinePath::root_field(0))
    } else {
        (&TEXTS, InlinePath::new())
    };
    blubber::raw::rewrite_collection(
        store,
        layout,
        &path,
        (*b"rewrite8", 4),
        |view| Ok((RewritePlan::check(view.bytes(), &view.layout(), 2, 6)?, ())),
        |(), stream| {
            for _ in 0..2 {
                if fault == 1 {
                    return Err(ErrorKind::InvalidBoundary.into());
                }
                {
                    stream.prepare_slot(3)?;
                    stream.destination()
                }
                .copy_from_slice(b"new");
                if fault == 2 {
                    return Err(ErrorKind::InvalidBoundary.into());
                }
                assert_ne!(fault, 3, "encoder unwind");
                if fault == 4 {
                    return Ok(());
                }
                stream.advance(if fault == 5 {
                    2
                } else if fault == 6 {
                    4
                } else {
                    3
                })?;
            }
            if fault == 7 {
                Err(ErrorKind::InvalidBoundary.into())
            } else {
                Ok(())
            }
        },
    )
}

fn original(spare: usize) -> Vec<u8> {
    let mut bytes = b"rewrite8".to_vec();
    bytes.extend_from_slice(&22u32.to_le_bytes());
    bytes.extend_from_slice(&[2, 0, 0, 0, 1, 0, 0, 0, b'a', b'b']);
    bytes.resize(bytes.len() + spare, 0xa5);
    bytes
}

#[test]
fn preflight_errors_and_empty_operations_preserve_every_byte_and_poison() {
    let mut bytes = original(4);
    let before = bytes.clone();
    let mut store = SliceStore::new(&mut bytes);
    let result = blubber::raw::rewrite_collection(
        &mut store,
        &TEXTS,
        &InlinePath::new(),
        (*b"rewrite8", 4),
        |_| Err::<(RewritePlan, ()), _>(ErrorKind::InvalidBoundary.into()),
        |(), _| panic!("encoder ran before preflight"),
    );
    assert_eq!(result, Err(ErrorKind::InvalidBoundary.into()));
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
    let mut empty = b"rewrite8".to_vec();
    empty.extend_from_slice(&16u32.to_le_bytes());
    empty.extend_from_slice(&0u32.to_le_bytes());
    empty.extend_from_slice(&[0xa5; 16]);
    let before = empty.clone();
    let mut store = SliceStore::new(&mut empty);
    blubber::raw::rewrite_collection(
        &mut store,
        &TEXTS,
        &InlinePath::new(),
        (*b"rewrite8", 4),
        |view| Ok((RewritePlan::check(view.bytes(), &view.layout(), 0, 0)?, ())),
        |(), _| panic!("empty encoder ran"),
    )
    .unwrap();
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[test]
#[allow(
    clippy::forget_non_drop,
    reason = "An unfinished raw stream must leave its backend poisoned"
)]
fn forgetting_a_raw_stream_cannot_clear_backend_poison() {
    let mut bytes = original(4);
    let mut store = SliceStore::new(&mut bytes);
    store.set_poisoned(true);
    {
        let resolved = blubber_core::raw::resolve_account_edit(
            store.bytes_mut(),
            *b"rewrite8",
            4,
            &TEXTS,
            &InlinePath::new(),
        )
        .unwrap();
        let plan = RewritePlan::check(&resolved.inspect_resolved()[12..22], &TEXTS, 2, 6).unwrap();
        let mut write = resolved.prepare_subtree(plan.encoded_len()).unwrap();
        let mut stream = plan.begin(write.destination()).unwrap();
        {
            stream.prepare_slot(3).unwrap();
            stream.destination()
        }
        .copy_from_slice(b"new");
        core::mem::forget(stream);
    }
    assert!(store.is_poisoned());
    assert_eq!(store.commit(22), Err(ErrorKind::Poisoned.into()));
}

#[test]
fn reserve_capacity_errors_and_sticky_encoder_damage() {
    for fault in 0..=7 {
        let mut bytes = original(4);
        let mut store = SliceStore::new(&mut bytes);
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut store, fault)));
        if fault == 0 {
            outcome.unwrap().unwrap();
            assert!(!store.is_poisoned());
            assert_eq!(&store.bytes()[8..12], &26u32.to_le_bytes());
            assert_eq!(
                &store.bytes()[16..],
                &[3, 0, 0, 0, b'n', b'e', b'w', b'n', b'e', b'w']
            );
        } else {
            if fault == 3 {
                assert!(outcome.is_err());
            } else {
                assert!(outcome.unwrap().is_err());
            }
            assert!(store.is_poisoned());
            assert_eq!(&store.bytes()[8..12], &22u32.to_le_bytes());
            assert_eq!(run(&mut store, 0), Err(ErrorKind::Poisoned.into()));
            assert_eq!(store.commit(22), Err(ErrorKind::Poisoned.into()));
            assert_eq!(run(&mut store, 0), Err(ErrorKind::Closed.into()));
        }
    }
    let mut bytes = original(0);
    let before = bytes.clone();
    let mut store = SliceStore::new(&mut bytes);
    assert_eq!(run(&mut store, 0), Err(ErrorKind::Capacity.into()));
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(feature = "alloc")]
#[test]
fn detached_readiness_and_vec_reacquisition_preserve_exact_bytes() {
    let mut bytes = original(0);
    bytes.shrink_to_fit();
    let mut store = blubber::VecStore::new(&mut bytes);
    run(&mut store, 0).unwrap();
    blubber_core::validate(&store.bytes()[12..26], &TEXTS, blubber_core::MAX_DEPTH).unwrap();
    assert_eq!(&store.bytes()[20..26], b"newnew");
    assert!(!store.is_poisoned());
    store.commit(26).unwrap();
}

#[cfg(feature = "alloc")]
#[test]
fn detached_encoder_damage_retains_poison_and_old_envelope_after_growth() {
    for fault in [2, 3, 7] {
        let mut bytes = original(0);
        let mut store = blubber::VecStore::new(&mut bytes);
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut store, fault)));
        if fault == 3 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(store.is_poisoned());
        assert_eq!(store.storage_len(), 26);
        assert_eq!(&store.bytes()[8..12], &22u32.to_le_bytes());
        assert_eq!(store.commit(22), Err(ErrorKind::Poisoned.into()));
    }
}

#[test]
fn direct_tail_publication_and_all_encoder_failures_follow_the_same_protocol() {
    for fault in 0..=7 {
        let mut bytes = original(4);
        let mut store = SliceStore::new(&mut bytes);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_selected(&mut store, fault, true)
        }));
        if fault == 0 {
            result.unwrap().unwrap();
            assert!(!store.is_poisoned());
            assert_eq!(&store.bytes()[8..12], &26u32.to_le_bytes());
            assert_eq!(&store.bytes()[20..26], b"newnew");
            blubber_core::validate(&store.bytes()[12..26], &TAIL_ROOT, blubber_core::MAX_DEPTH)
                .unwrap();
        } else {
            if fault == 3 {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(store.is_poisoned());
            assert_eq!(&store.bytes()[8..12], &22u32.to_le_bytes());
            assert_eq!(
                run_selected(&mut store, 0, true),
                Err(ErrorKind::Poisoned.into())
            );
            assert_eq!(store.commit(22), Err(ErrorKind::Poisoned.into()));
        }
    }
    let mut bytes = original(0);
    let before = bytes.clone();
    let mut store = SliceStore::new(&mut bytes);
    assert_eq!(
        run_selected(&mut store, 0, true),
        Err(ErrorKind::Capacity.into())
    );
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[cfg(feature = "alloc")]
#[test]
fn direct_tail_growth_falls_back_without_repeating_input_preflight() {
    let mut bytes = original(0);
    let mut store = blubber::VecStore::new(&mut bytes);
    let calls = core::cell::Cell::new(0);
    blubber::raw::rewrite_collection(
        &mut store,
        &TAIL_ROOT,
        &InlinePath::root_field(0),
        (*b"rewrite8", 4),
        |view| {
            calls.set(calls.get() + 1);
            Ok((RewritePlan::check(view.bytes(), &view.layout(), 2, 6)?, ()))
        },
        |(), stream| {
            for _ in 0..2 {
                stream.prepare_slot(3)?;
                stream.destination().copy_from_slice(b"new");
                stream.advance(3)?;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert!(!store.is_poisoned());
    assert_eq!(&store.bytes()[20..26], b"newnew");
    blubber_core::validate(&store.bytes()[12..26], &TAIL_ROOT, blubber_core::MAX_DEPTH).unwrap();
}

#[test]
fn direct_tail_preflight_and_empty_operations_preserve_the_entire_buffer() {
    let mut bytes = original(4);
    let before = bytes.clone();
    let mut store = SliceStore::new(&mut bytes);
    let result = blubber::raw::rewrite_collection(
        &mut store,
        &TAIL_ROOT,
        &InlinePath::root_field(0),
        (*b"rewrite8", 4),
        |view| Ok((RewritePlan::check(view.bytes(), &view.layout(), 1, 3)?, ())),
        |(), _| panic!("encoder ran after rejected count"),
    );
    assert_eq!(result, Err(ErrorKind::InvalidBoundary.into()));
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
    let mut empty = b"rewrite8".to_vec();
    empty.extend_from_slice(&16u32.to_le_bytes());
    empty.extend_from_slice(&0u32.to_le_bytes());
    empty.extend_from_slice(&[0xa5; 8]);
    let before = empty.clone();
    let mut store = SliceStore::new(&mut empty);
    blubber::raw::rewrite_collection(
        &mut store,
        &TAIL_ROOT,
        &InlinePath::root_field(0),
        (*b"rewrite8", 4),
        |view| Ok((RewritePlan::check(view.bytes(), &view.layout(), 0, 0)?, ())),
        |(), _| panic!("empty encoder ran"),
    )
    .unwrap();
    assert_eq!(store.bytes(), before);
    assert!(!store.is_poisoned());
}

#[test]
fn checked_local_admission_can_repair_corrupt_old_value_domains() {
    let mut bytes = original(4);
    bytes[20] = 0xff;
    assert!(blubber_core::validate(&bytes[12..22], &TAIL_ROOT, blubber_core::MAX_DEPTH).is_err());
    let mut expected = bytes.clone();
    expected[8..12].copy_from_slice(&26u32.to_le_bytes());
    expected[16..20].copy_from_slice(&3u32.to_le_bytes());
    expected[20..26].copy_from_slice(b"newnew");
    let mut store = SliceStore::new(&mut bytes);
    run_selected(&mut store, 0, true).unwrap();
    assert!(!store.is_poisoned());
    assert_eq!(store.bytes(), expected);
    blubber_core::validate(&store.bytes()[12..26], &TAIL_ROOT, blubber_core::MAX_DEPTH).unwrap();
}

#[cfg(feature = "derive")]
#[test]
fn direct_flat_map_tail_frames_fixed_prefix_keys_and_physical_storage() {
    let layout = Layout::Flat {
        checked: <CheckedTailMap as blubber::RootSchema>::__layout(),
    };
    let mut bytes = b"rewrite8".to_vec();
    bytes.extend_from_slice(&30u32.to_le_bytes());
    bytes.extend_from_slice(&[
        0x34, 0x12, 2, 0, 0, 0, 1, 1, 0, 0, 0, 3, 2, 0, 0, 0, b'a', b'b',
    ]);
    bytes.resize(34, 0xa5);
    blubber_core::validate(&bytes[12..30], &layout, blubber_core::MAX_DEPTH).unwrap();
    let mut expected = b"rewrite8".to_vec();
    expected.extend_from_slice(&34u32.to_le_bytes());
    expected.extend_from_slice(&[
        0x34, 0x12, 2, 0, 0, 0, 1, 3, 0, 0, 0, 3, 6, 0, 0, 0, b'n', b'e', b'w', b'n', b'e', b'w',
    ]);
    let mut store = SliceStore::new(&mut bytes);
    blubber::raw::rewrite_collection(
        &mut store,
        &layout,
        &InlinePath::root_field(1),
        (*b"rewrite8", 6),
        |view| {
            Ok((
                RewritePlan::check_admitted(view.bytes(), &view.layout(), 2, 6)?,
                (),
            ))
        },
        |(), stream| {
            for _ in 0..2 {
                stream.prepare_slot(3)?;
                stream.destination().copy_from_slice(b"new");
                stream.advance(3)?;
            }
            Ok(())
        },
    )
    .unwrap();
    assert!(!store.is_poisoned());
    assert_eq!(store.bytes(), expected);
    blubber_core::validate(&store.bytes()[12..34], &layout, blubber_core::MAX_DEPTH).unwrap();
}

#[path = "../../blubber-core/tests/support/uniform_fill.rs"]
mod uniform_fill;

#[cfg(feature = "derive")]
#[path = "support/variable_fill.rs"]
mod variable_fill;

#[cfg(feature = "derive")]
#[test]
fn uniform_fill_contract() {
    uniform_fill::uniform_fill_matches_individual_slots_including_partial_errors();
    variable_fill::variable_calls_errors_and_unwinds_keep_poison();
    uniform_fill::uniform_fill_unwind_matches_individual_prefix_bytes();
}
