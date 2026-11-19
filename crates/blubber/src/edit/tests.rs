//! Observe funding preflight through safe host storage; absent from SBF builds.
#![forbid(unsafe_code)]

use super::*;
use crate::storage::private::{self, Write};
use crate::{CommitInfo, ReadStorage, Resize as _, SliceStore, Str, VecStore};
use core::cell::Cell;

struct Probed<'a, S> {
    inner: S,
    probe: &'a dyn Fn(usize) -> Result<()>,
}

impl<S: Resize> private::Sealed for Probed<'_, S> {}
impl<S: Resize> ReadStorage for Probed<'_, S> {
    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
    fn bytes(&self) -> &[u8] {
        self.inner.bytes()
    }
}
impl<S: Resize> Write for Probed<'_, S> {
    fn check_initialize_resize(&self, needed: usize) -> Result<()> {
        self.inner.check_initialize_resize(needed)
    }
    fn edit_access(&mut self) -> private::EditAccess<'_> {
        let mut access = self.inner.edit_access();
        access.preflight.probe = Some(self.probe);
        access
    }
    fn fixed_access(&mut self) -> Result<private::FixedAccess<'_>> {
        self.inner.fixed_access()
    }
}
impl<S: Resize> crate::Resize for Probed<'_, S> {
    fn storage_len(&self) -> usize {
        self.inner.storage_len()
    }
    fn growth_limit(&self) -> usize {
        self.inner.growth_limit()
    }
    fn check_resize(&self, len: usize) -> Result<()> {
        self.inner.check_resize(len)
    }
    fn is_poisoned(&self) -> bool {
        self.inner.is_poisoned()
    }
    fn __backend(&mut self, _: crate::storage::Writer) -> &mut dyn Resize {
        self
    }
}
impl<S: Resize> Resize for Probed<'_, S> {
    fn bytes_mut(&mut self) -> &mut [u8] {
        self.inner.bytes_mut()
    }
    fn resize(&mut self, len: usize) -> Result<()> {
        self.inner.resize(len)
    }
    fn set_poisoned(&mut self, value: bool) {
        self.inner.set_poisoned(value);
    }
    fn commit(&mut self, used: u32) -> Result<CommitInfo> {
        self.inner.commit(used)
    }
}

#[crate::schema(account)]
struct Text {
    text: Str,
}

#[crate::schema(account)]
struct BulkText {
    list: crate::List<Str>,
}

#[test]
fn bulk_funding_preflight_tracks_actual_byte_growth() {
    for (old, appended, required) in [
        (&["a"][..], &["bc"][..], 23usize),
        (&[][..], &[""][..], 16usize),
    ] {
        for permit in [false, true] {
            let mut bytes = [0; 64];
            let mut inner = SliceStore::new(&mut bytes[..required]);
            BulkText::initialize(&mut inner, BulkTextInit { list: old }).unwrap();
            let before = inner.bytes().to_vec();
            let calls = Cell::new(0);
            let probe = |needed| {
                calls.set(calls.get() + 1);
                assert_eq!(needed, required);
                if permit {
                    Ok(())
                } else {
                    Err(ErrorKind::RentShortfall.into())
                }
            };
            let resizes = Cell::new(0);
            let mut store = Window {
                inner: Probed {
                    inner,
                    probe: &probe,
                },
                hard: required,
                limit: required,
                resizes: &resizes,
            };
            let result = {
                let mut root = BulkText::load_trusted_mut(&mut store).unwrap();
                root.list_mut().extend_from_slice(appended)
            };
            let grows = required == 23;
            assert_eq!(calls.get(), usize::from(grows));
            assert_eq!(resizes.get(), 0);
            assert!(!store.is_poisoned());
            if grows && !permit {
                assert_eq!(result, Err(ErrorKind::RentShortfall.into()));
                assert_eq!(store.bytes(), before);
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(&store.bytes()[8..12], &(required as u32).to_le_bytes());
                let root = BulkText::load(store.bytes()).unwrap();
                assert_eq!(root.list().len(), if grows { 2 } else { 1 });
            }
        }
    }
}

#[test]
fn funding_preflight_byte_and_callback_growth_checks_exact_rent_before_writes() {
    // Tail byte loan, generic subtree, detached generic plan, callback subtree.
    for mode in 0..4 {
        for input in ["x", "xy", "new", "longer"] {
            for short in [false, true] {
                let mut bytes = alloc::vec![0; if mode == 2 { 15 } else { 64 }];
                let mut inner = VecStore::new(&mut bytes);
                Text::initialize(&mut inner, TextInit { text: "old" }).unwrap();
                let before = inner.bytes().to_vec();
                let required = 12 + input.len();
                let calls = Cell::new(0);
                // One lamport per byte plus the runtime's 128-byte overhead.
                let available = (128 + required - usize::from(short)) as u64;
                let probe = |needed| {
                    calls.set(calls.get() + 1);
                    assert_eq!(needed, required, "mode={mode}");
                    if available < (128 + needed) as u64 {
                        Err(ErrorKind::RentShortfall.into())
                    } else {
                        Ok(())
                    }
                };
                let mut store = Probed {
                    inner,
                    probe: &probe,
                };
                let result = {
                    let mut edit = AccountEdit::load::<Text>(&mut store, Text::DISCRIMINATOR)
                        .unwrap()
                        .field(0);
                    match mode {
                        0 => edit.replace_bytes(input.as_bytes()),
                        1 | 2 => edit.replace_bytes_generic(input.as_bytes()),
                        _ => edit.replace_with(
                            |_| Ok((input.len() as u32, ())),
                            |(), destination| destination.copy_from_slice(input.as_bytes()),
                        ),
                    }
                };
                let grows = input.len() > 3;
                assert_eq!(calls.get(), usize::from(grows), "mode={mode} input={input}");
                assert!(!store.is_poisoned());
                if grows && short {
                    assert_eq!(result, Err(ErrorKind::RentShortfall.into()));
                    assert_eq!(store.bytes(), before);
                } else {
                    result.unwrap();
                    assert_eq!(Text::load(store.bytes()).unwrap().text(), input);
                    Text::load(store.bytes()).unwrap().validate().unwrap();
                }
            }
        }
    }
}

#[test]
fn funding_preflight_selected_growth_checks_exact_rent_before_decode_or_encode() {
    fn check<const LIST_FRAME: bool, R: Default + core::fmt::Debug + PartialEq>(
        input: &str,
        short: bool,
        policy: u8,
        decode_fails: bool,
    ) {
        let mut bytes = [0; 64];
        Text::initialize(&mut SliceStore::new(&mut bytes), TextInit { text: "old" }).unwrap();
        let before = bytes;
        let mut path = InlinePath::new();
        path.push(Step::Field(0)).unwrap();
        let prepared = blubber_core::raw::resolve_account_edit(
            &mut bytes,
            Text::DISCRIMINATOR,
            Text::MIN_SIZE,
            &Layout::Flat {
                checked: Text::__layout(),
            },
            &path,
        )
        .unwrap()
        .prepare_replace(input.len() as u32, u32::MAX)
        .unwrap();
        let requested = 12 + input.len();
        let required = prepared.prepared_len() as usize;
        let (hard, limit) = match policy {
            0 => (required - 1, 64),
            1 => (64, required - 1),
            2 => (required, required),
            _ => (64, 64),
        };
        let quota = if required > hard {
            Some(ErrorKind::Capacity)
        } else if required > limit {
            Some(ErrorKind::GrowthLimit)
        } else {
            None
        };
        let calls = Cell::new(0);
        let available = 128 + requested - usize::from(short);
        let probe = |needed| {
            calls.set(calls.get() + 1);
            assert_eq!(needed, requested);
            if available < 128 + needed {
                Err(ErrorKind::RentShortfall.into())
            } else {
                Ok(())
            }
        };
        let decoded = Cell::new(false);
        let encoded = Cell::new(false);
        let mut poisoned = false;
        let mut stage = EditStage {
            input: None,
            removed: Err(ErrorKind::InvalidPlan.into()),
            encode: Some(|(), destination: &mut [u8]| {
                encoded.set(true);
                destination.copy_from_slice(input.as_bytes());
            }),
        };
        let result = stage_selected::<LIST_FRAME, _, _, _>(
            false,
            (prepared.into(), ()),
            &mut poisoned,
            (hard, limit, ErrorKind::GrowthLimit),
            &mut LoanStage {
                data: &mut stage,
                preflight: private::Preflight {
                    rent: None,
                    probe: Some(&probe),
                },
            },
            |_, _, _| {
                decoded.set(true);
                if decode_fails {
                    Err(ErrorKind::InvalidBool.into())
                } else {
                    Ok(R::default())
                }
            },
        );
        let grows = input.len() > 3;
        assert_eq!(calls.get(), usize::from(grows && quota.is_none()));
        assert!(!poisoned);
        if let Some(kind) = quota {
            assert_eq!(result.err(), Some(kind.into()));
            assert!(!decoded.get());
            assert!(!encoded.get());
            assert_eq!(bytes, before);
        } else if grows && short {
            assert_eq!(result.err(), Some(ErrorKind::RentShortfall.into()));
            assert!(!decoded.get());
            assert!(!encoded.get());
            assert_eq!(bytes, before);
        } else if decode_fails {
            assert_eq!(result.err(), Some(ErrorKind::InvalidBool.into()));
            assert!(decoded.get());
            assert!(!encoded.get());
            assert_eq!(bytes, before);
        } else {
            assert!(result.unwrap().is_none());
            assert!(decoded.get());
            assert!(encoded.get());
            assert_eq!(Text::load(&bytes).unwrap().text(), input);
            assert_eq!(stage.removed, Ok(R::default()));
        }
    }
    for input in ["x", "new", "longer"] {
        for short in [false, true] {
            for policy in 0..4 {
                for decode_fails in [false, true] {
                    check::<false, ()>(input, short, policy, decode_fails);
                    check::<true, ()>(input, short, policy, decode_fails);
                    check::<false, u32>(input, short, policy, decode_fails);
                    check::<true, u32>(input, short, policy, decode_fails);
                    check::<false, u64>(input, short, policy, decode_fails);
                    check::<true, u64>(input, short, policy, decode_fails);
                }
            }
        }
    }
}

#[crate::schema(account)]
struct PatchFunding {
    prefix: u16,
    values: crate::List<Str>,
    suffix: Str,
}

struct Window<'a, S> {
    inner: S,
    hard: usize,
    limit: usize,
    resizes: &'a Cell<usize>,
}
impl<S: Resize> private::Sealed for Window<'_, S> {}
impl<S: Resize> ReadStorage for Window<'_, S> {
    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
    fn bytes(&self) -> &[u8] {
        self.inner.bytes()
    }
}
impl<S: Resize> Write for Window<'_, S> {
    fn check_initialize_resize(&self, needed: usize) -> Result<()> {
        self.check_resize(needed)
    }
    fn fixed_access(&mut self) -> Result<private::FixedAccess<'_>> {
        self.inner.fixed_access()
    }
    fn edit_access(&mut self) -> private::EditAccess<'_> {
        let mut access = self.inner.edit_access();
        access.hard_limit = self.hard;
        access.limit = self.limit;
        access.limit_error = ErrorKind::GrowthLimit;
        access
    }
}
impl<S: Resize> crate::Resize for Window<'_, S> {
    fn storage_len(&self) -> usize {
        self.inner.storage_len()
    }
    fn growth_limit(&self) -> usize {
        self.limit
    }
    fn check_resize(&self, needed: usize) -> Result<()> {
        self.inner.check_resize(self.inner.storage_len())?;
        if needed > self.hard {
            return Err(ErrorKind::Capacity.into());
        }
        if needed > self.limit {
            return Err(ErrorKind::GrowthLimit.into());
        }
        self.inner.check_resize(needed)
    }
    fn is_poisoned(&self) -> bool {
        self.inner.is_poisoned()
    }
    fn __backend(&mut self, _: crate::storage::Writer) -> &mut dyn Resize {
        self
    }
}
impl<S: Resize> Resize for Window<'_, S> {
    fn bytes_mut(&mut self) -> &mut [u8] {
        self.inner.bytes_mut()
    }
    fn resize(&mut self, needed: usize) -> Result<()> {
        self.resizes.set(self.resizes.get() + 1);
        self.check_resize(needed)?;
        self.inner.resize(needed)
    }
    fn set_poisoned(&mut self, value: bool) {
        self.inner.set_poisoned(value);
    }
    fn commit(&mut self, used: u32) -> Result<CommitInfo> {
        self.inner.commit(used)
    }
}

fn patch_bytes() -> (alloc::vec::Vec<u8>, usize) {
    let mut bytes = alloc::vec![0; 128];
    PatchFunding::initialize(
        &mut VecStore::new(&mut bytes),
        PatchFundingInit {
            prefix: 73,
            values: &["old"],
            suffix: "suffix",
        },
    )
    .unwrap();
    let used = blubber_core::read_u32(&bytes, 8).unwrap() as usize;
    (bytes, used)
}

fn patch_route(root: &mut PatchFundingMut<'_>, route: usize) -> Result<()> {
    let mut list = root.values_mut();
    match route {
        0 => list.insert(0, "new"),
        1 => list.replace(0, "longer"),
        2 => list.insert_from_slice(0, &["a", "bb"]),
        3 => list.extend_from_slice(&["a", "bb"]),
        4 => list.pop().map(|_| ()),
        5 => list.replace(0, "old"),
        6 => list.truncate(1),
        7 => list.insert_from_slice(0, &[]),
        _ => list.extend_from_slice(&[]),
    }
}

#[test]
fn list_patch_limits_distinguish_exact_hard_and_window_boundaries() {
    for route in 0..4 {
        for detached in [false, true] {
            for policy in 0..3 {
                let (mut bytes, old_used) = patch_bytes();
                let required = old_used + [7, 3, 11, 11][route];
                bytes.resize(if detached { old_used } else { required }, 0);
                let before = bytes.clone();
                let resizes = Cell::new(0);
                let (hard, limit, error) = match policy {
                    0 => (required, required, None),
                    1 => (required - 1, required - 2, Some(ErrorKind::Capacity)),
                    _ => (required + 1, required - 1, Some(ErrorKind::GrowthLimit)),
                };
                let mut store = Window {
                    inner: VecStore::new(&mut bytes),
                    hard,
                    limit,
                    resizes: &resizes,
                };
                let result = {
                    let mut root = PatchFunding::load_trusted_mut(&mut store).unwrap();
                    patch_route(&mut root, route)
                };
                assert!(!store.is_poisoned());
                if let Some(error) = error {
                    assert_eq!(
                        result,
                        Err(error.into()),
                        "route={route} detached={detached} policy={policy}"
                    );
                    assert_eq!(store.bytes(), before);
                    assert_eq!(resizes.get(), 0);
                } else {
                    result.unwrap();
                    assert_eq!(resizes.get(), usize::from(detached));
                    assert_eq!(
                        blubber_core::read_u32(store.bytes(), 8).unwrap() as usize,
                        required
                    );
                    PatchFunding::load(store.bytes())
                        .unwrap()
                        .validate()
                        .unwrap();
                }
            }
        }
    }
}

#[test]
fn list_patch_funding_precedes_writes_and_skips_non_growth() {
    for route in 0..9 {
        for detached in [false, true] {
            for short in [false, true] {
                let (mut bytes, old_used) = patch_bytes();
                let growth = [7, 3, 11, 11, 0, 0, 0, 0, 0][route];
                let required = old_used + growth;
                bytes.resize(if detached { old_used } else { required }, 0);
                let before = bytes.clone();
                let calls = Cell::new(0);
                let resizes = Cell::new(0);
                let probe = |needed| {
                    calls.set(calls.get() + 1);
                    assert_eq!(needed, required);
                    if short {
                        Err(ErrorKind::RentShortfall.into())
                    } else {
                        Ok(())
                    }
                };
                let mut store = Window {
                    inner: Probed {
                        inner: VecStore::new(&mut bytes),
                        probe: &probe,
                    },
                    hard: required + 16,
                    limit: required + 16,
                    resizes: &resizes,
                };
                let result = {
                    let mut root = PatchFunding::load_trusted_mut(&mut store).unwrap();
                    patch_route(&mut root, route)
                };
                assert_eq!(
                    calls.get(),
                    usize::from(growth != 0),
                    "route={route} detached={detached}"
                );
                assert!(!store.is_poisoned());
                if growth != 0 && short {
                    assert_eq!(result, Err(ErrorKind::RentShortfall.into()));
                    assert_eq!(store.bytes(), before);
                    assert_eq!(resizes.get(), 0);
                } else {
                    result.unwrap();
                    assert_eq!(resizes.get(), usize::from(detached && growth != 0));
                    PatchFunding::load(store.bytes())
                        .unwrap()
                        .validate()
                        .unwrap();
                }
            }
        }
    }
}

#[test]
fn checked_list_faults_keep_the_strict_prewrite_frame() {
    for route in 0..4 {
        let mut bytes = alloc::vec![0; 128];
        let mut store = VecStore::new(&mut bytes);
        PatchFunding::initialize(
            &mut store,
            PatchFundingInit {
                prefix: 73,
                values: &["a", "bb", "ccc"],
                suffix: "suffix",
            },
        )
        .unwrap();
        let mut edit = AccountEdit::load::<PatchFunding>(&mut store, PatchFunding::DISCRIMINATOR)
            .unwrap()
            .field(1);
        let base = edit.view().unwrap().base() as usize;
        // Fault injection also retires the retained validation evidence.
        edit.store.bytes_mut()[base + 4..base + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        edit.ingress = None;
        let before = edit.store.bytes().to_vec();
        let result = if route == 3 {
            edit.try_bulk_insert::<Str>(Some(0), &["new"])
                .unwrap_or_else(|| {
                    edit.try_replace_with(
                        |view| {
                            let p =
                                crate::list_bulk::Insert::prepare::<Str>(view, Some(0), &["new"])?;
                            Ok((p.encoded_len(), p))
                        },
                        |p, bytes| p.encode::<Str>(&["new"], bytes),
                    )
                })
        } else {
            let request = match route {
                0 => ListRequest::Insert(0),
                1 => ListRequest::Replace(0),
                _ => ListRequest::Remove(0),
            };
            edit.dispatch_list_encoded(
                request,
                || Ok(u32::from(route != 2) * 3),
                |_| Ok(()),
                |bytes| {
                    if !bytes.is_empty() {
                        bytes.copy_from_slice(b"new");
                    }
                },
            )
        };
        assert!(result.is_err());
        assert_eq!(edit.store.bytes(), before);
        assert!(!edit.store.is_poisoned());
    }
}

#[crate::schema(account)]
struct MapPatchFunding {
    prefix: u16,
    values: crate::Map<u8, Str>,
    suffix: Str,
}

#[test]
fn map_patch_funding_and_limits_precede_writes() {
    for route in 0..4 {
        for detached in [false, true] {
            for policy in 0..4 {
                let mut bytes = alloc::vec![0; 128];
                MapPatchFunding::initialize(
                    &mut VecStore::new(&mut bytes),
                    MapPatchFundingInit {
                        prefix: 73,
                        values: &[(2, "old"), (4, "b")],
                        suffix: "suffix",
                    },
                )
                .unwrap();
                let old_used = blubber_core::read_u32(&bytes, 8).unwrap() as usize;
                let growth = [8, 3, 0, 0][route];
                let required = old_used + growth;
                bytes.resize(if detached { old_used } else { required }, 0);
                let before = bytes.clone();
                let calls = Cell::new(0);
                let resizes = Cell::new(0);
                let probe = |needed| {
                    calls.set(calls.get() + 1);
                    assert_eq!(needed, required);
                    if policy == 3 {
                        Err(ErrorKind::RentShortfall.into())
                    } else {
                        Ok(())
                    }
                };
                let mut store = Window {
                    inner: Probed {
                        inner: VecStore::new(&mut bytes),
                        probe: &probe,
                    },
                    hard: if policy == 1 { required - 1 } else { required },
                    limit: if policy == 2 { required - 1 } else { required },
                    resizes: &resizes,
                };
                let result = {
                    let mut root = MapPatchFunding::load_trusted_mut(&mut store).unwrap();
                    let mut map = root.values_mut();
                    match route {
                        0 => map.insert(3, "new").map(|_| ()),
                        1 => map.insert(2, "longer").map(|_| ()),
                        2 => map.remove(&2).map(|_| ()),
                        _ => map.clear(),
                    }
                };
                let expected = match policy {
                    1 => Some(ErrorKind::Capacity),
                    2 => Some(ErrorKind::GrowthLimit),
                    3 if growth != 0 => Some(ErrorKind::RentShortfall),
                    _ => None,
                };
                assert_eq!(
                    calls.get(),
                    usize::from(growth != 0 && policy != 1 && policy != 2)
                );
                assert!(!store.is_poisoned());
                if let Some(error) = expected {
                    assert_eq!(
                        result,
                        Err(error.into()),
                        "route={route} policy={policy} detached={detached}"
                    );
                    assert_eq!(store.bytes(), before);
                    assert_eq!(resizes.get(), 0);
                } else {
                    result.unwrap();
                    assert_eq!(resizes.get(), usize::from(detached && growth != 0));
                    MapPatchFunding::load(store.bytes())
                        .unwrap()
                        .validate()
                        .unwrap();
                }
            }
        }
    }
}

#[test]
fn checked_map_faults_keep_the_strict_prewrite_frame() {
    for route in 0..4 {
        let mut bytes = alloc::vec![0; 128];
        let mut store = VecStore::new(&mut bytes);
        MapPatchFunding::initialize(
            &mut store,
            MapPatchFundingInit {
                prefix: 73,
                values: &[(2, "a"), (4, "bb"), (6, "ccc")],
                suffix: "suffix",
            },
        )
        .unwrap();
        let mut edit =
            AccountEdit::load::<MapPatchFunding>(&mut store, MapPatchFunding::DISCRIMINATOR)
                .unwrap()
                .field(1);
        let base = edit.view().unwrap().base() as usize;
        // Fault injection retires the retained ingress evidence.
        edit.store.bytes_mut()[base + 10..base + 14].copy_from_slice(&u32::MAX.to_le_bytes());
        edit.ingress = None;
        let before = edit.store.bytes().to_vec();
        let operation = [
            blubber_core::raw::MapEdit::Insert,
            blubber_core::raw::MapEdit::Replace,
            blubber_core::raw::MapEdit::Remove,
            blubber_core::raw::MapEdit::Clear,
        ][route];
        let key: &[u8] = match route {
            0 => &[3],
            3 => &[],
            _ => &[2],
        };
        let result = edit.dispatch_map_encoded(
            operation,
            key,
            || Ok(if route < 2 { 3 } else { 0 }),
            |_| Ok(()),
            |destination| destination.fill(b'n'),
        );
        assert!(result.is_err());
        assert_eq!(edit.store.bytes(), before);
        assert!(!edit.store.is_poisoned());
    }
}

#[test]
fn lazy_ancestor_adapters_check_limits_before_funding_and_resize() {
    for old in ["old", ""] {
        for mode in 0..3 {
            for detached in [false, true] {
                for input in ["x", "xy", "new", "longer"] {
                    for policy in 0..3 {
                        if input.len() == old.len() && policy != 0 {
                            continue;
                        }
                        for short in [false, true] {
                            let mut bytes = alloc::vec![0; 128];
                            PatchFunding::initialize(
                                &mut VecStore::new(&mut bytes),
                                PatchFundingInit {
                                    prefix: 73,
                                    values: &[old],
                                    suffix: "suffix",
                                },
                            )
                            .unwrap();
                            let used = blubber_core::read_u32(&bytes, 8).unwrap() as usize;
                            bytes.resize(if detached { used } else { 64 }, 0);
                            let before = bytes.clone();
                            let new_used = used - old.len() + input.len();
                            let required = used.max(new_used);
                            let grows = input.len() > old.len();
                            let calls = Cell::new(0);
                            let resizes = Cell::new(0);
                            let probe = |needed| {
                                calls.set(calls.get() + 1);
                                assert_eq!(needed, new_used);
                                if short {
                                    Err(ErrorKind::RentShortfall.into())
                                } else {
                                    Ok(())
                                }
                            };
                            let (hard, limit, quota) = match policy {
                                0 => (required + 16, required + 16, None),
                                1 => (required - 1, required - 2, Some(ErrorKind::Capacity)),
                                _ => (required + 16, required - 1, Some(ErrorKind::GrowthLimit)),
                            };
                            let mut store = Window {
                                inner: Probed {
                                    inner: VecStore::new(&mut bytes),
                                    probe: &probe,
                                },
                                hard,
                                limit,
                                resizes: &resizes,
                            };
                            let result = {
                                let mut edit = AccountEdit::load_trusted::<PatchFunding>(
                                    &mut store,
                                    PatchFunding::DISCRIMINATOR,
                                )
                                .unwrap()
                                .field(1)
                                .index(0);
                                match mode {
                                    0 => edit.replace_bytes(input.as_bytes()),
                                    1 => edit.replace_bytes_generic(input.as_bytes()),
                                    _ => edit.replace_with(
                                        |_| Ok((input.len() as u32, ())),
                                        |(), destination| {
                                            destination.copy_from_slice(input.as_bytes())
                                        },
                                    ),
                                }
                            };
                            let expected = quota.or(if grows && short {
                                Some(ErrorKind::RentShortfall)
                            } else {
                                None
                            });
                            assert_eq!(
                                result.err(),
                                expected.map(Into::into),
                                "mode={mode} detached={detached} input={input} policy={policy} short={short}"
                            );
                            assert_eq!(
                                calls.get(),
                                usize::from(grows && quota.is_none()),
                                "mode={mode} detached={detached} input={input} policy={policy}"
                            );
                            assert_eq!(
                                resizes.get(),
                                usize::from(grows && detached && expected.is_none())
                            );
                            assert!(!store.is_poisoned());
                            if expected.is_some() {
                                assert_eq!(store.bytes(), before);
                            } else {
                                let root = PatchFunding::load(store.bytes()).unwrap();
                                assert_eq!(root.values().get(0), Some(input));
                                assert_eq!(root.prefix(), 73);
                                assert_eq!(root.suffix(), "suffix");
                                root.validate().unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn tail_count_fault_offsets_preserve_checked_policy() {
    for lazy in [false, true] {
        for offset in [0u32, 8] {
            let mut bytes = [0; 64];
            bytes[..8].copy_from_slice(b"countrow");
            bytes[8..12].copy_from_slice(&(16 + offset).to_le_bytes());
            let at = (12 + offset) as usize;
            bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            let before = bytes;
            let mut poisoned = false;
            let result = tail_walk(
                private::FixedAccess {
                    bytes: &mut bytes,
                    poisoned: &mut poisoned,
                },
                &lazy,
                *b"countrow",
                4 + offset,
                offset,
                8,
                <u64 as crate::RootSchema>::__layout(),
            );
            assert_eq!(
                result.err(),
                Some(Error::new(
                    ErrorKind::ArithmeticOverflow,
                    if lazy { 12 + offset } else { Error::NO_OFFSET },
                ))
            );
            assert_eq!(bytes, before);
            assert_eq!(poisoned, lazy);
        }
    }
}

#[test]
fn string_size_rejections_preserve_poison_before_writes() {
    for lazy in [false, true] {
        for input_len in [usize::MAX, u32::MAX as usize] {
            let mut bytes = [0; 64];
            Text::initialize(&mut SliceStore::new(&mut bytes), TextInit { text: "old" }).unwrap();
            let before = bytes;
            let layout = Layout::Flat {
                checked: <Text as RootSchema>::__layout(),
            };
            let path = InlinePath::root_field(0);
            let error = blubber_core::raw::prepare_string_write(
                &mut bytes,
                Text::DISCRIMINATOR,
                Text::MIN_SIZE,
                &layout,
                &path,
                input_len,
            )
            .err()
            .unwrap();
            assert_eq!(error, ErrorKind::ArithmeticOverflow.into());
            let mut poisoned = false;
            assert_eq!(
                input_extent_error(error, bytes.len(), lazy, &mut poisoned),
                error
            );
            assert!(!poisoned);
            assert_eq!(bytes, before);
        }
        for (error, physical) in [
            (ErrorKind::ArithmeticOverflow.into(), usize::MAX),
            (Error::new(ErrorKind::InvalidBoundary, 12), 64),
        ] {
            let mut poisoned = false;
            assert_eq!(
                input_extent_error(error, physical, lazy, &mut poisoned),
                error
            );
            assert_eq!(poisoned, lazy);
        }
    }
}

#[test]
fn map_stride_overflow_reports_the_count_word() {
    let layout = Layout::Map {
        key: blubber_core::KeyLayout {
            width: 1,
            domain: blubber_core::KeyDomain::Raw,
        },
        value: &Layout::Leaf(blubber_core::Leaf::Raw(u32::MAX)),
    };
    let view = View::new(&[1, 0, 0, 0], layout, 12).unwrap();
    assert_eq!(
        view.map_header().err(),
        Some(Error::new(ErrorKind::ArithmeticOverflow, 12))
    );
    let empty = View::new(&[0; 4], layout, 12).unwrap();
    assert_eq!(empty.map_header().unwrap().count, 0);
}

#[test]
fn direct_fixed_growth_overflow_keeps_poison_clear() {
    use blubber_core::{KeyDomain, KeyLayout, Leaf, VariableField};
    static LIST: Layout = Layout::List {
        element: &Layout::Leaf(Leaf::Raw(u32::MAX)),
    };
    static MAP: Layout = Layout::Map {
        key: KeyLayout {
            width: 1,
            domain: KeyDomain::Raw,
        },
        value: &Layout::Leaf(Leaf::Raw(u32::MAX)),
    };
    for map in [false, true] {
        static LIST_FIELDS: [VariableField; 1] = [VariableField {
            declaration: 0,
            layout: &LIST,
        }];
        static MAP_FIELDS: [VariableField; 1] = [VariableField {
            declaration: 0,
            layout: &MAP,
        }];
        let layout = Layout::Struct {
            fixed_bytes: 0,
            fixed: &[],
            variable: if map { &MAP_FIELDS } else { &LIST_FIELDS },
        };
        for lazy in [false, true] {
            for insert in [false, true] {
                if insert && !map {
                    continue;
                }
                let mut bytes = [0; 32];
                bytes[0] = 1;
                bytes[8..12].copy_from_slice(&16u32.to_le_bytes());
                let before = bytes;
                let path = InlinePath::root_field(0);
                let error = if insert {
                    blubber_core::raw::prepare_fixed_insert(
                        &mut bytes,
                        [1, 0, 0, 0, 0, 0, 0, 0],
                        4,
                        &layout,
                        &path,
                        u32::MAX,
                        &[1],
                    )
                    .err()
                    .unwrap()
                } else {
                    blubber_core::raw::prepare_tail_append(
                        &mut bytes,
                        [1, 0, 0, 0, 0, 0, 0, 0],
                        4,
                        &layout,
                        &path,
                        u32::MAX,
                        if map { Some(&[1]) } else { None },
                    )
                    .err()
                    .unwrap()
                };
                assert_eq!(error, ErrorKind::ArithmeticOverflow.into());
                let mut poisoned = false;
                assert_eq!(
                    input_extent_error(error, bytes.len(), lazy, &mut poisoned),
                    error
                );
                assert!(!poisoned);
                assert_eq!(bytes, before);
            }
        }
    }
}
