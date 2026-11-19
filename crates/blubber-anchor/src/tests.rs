use super::*;
use alloc::{vec, vec::Vec};
use blubber::{List, SliceStore, Str};
use core::cell::{Ref, RefCell, RefMut};

extern crate std;

#[blubber::schema(account)]
pub struct State {
    pub counter: u64,
    pub flag: bool,
    pub text: Str,
    pub values: List<Str>,
}

enum HostGuard<'a> {
    Shared(Ref<'a, Vec<u8>>),
    Exclusive(RefMut<'a, Vec<u8>>),
    Released,
}
impl Lease for HostGuard<'_> {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Shared(g) => g,
            Self::Exclusive(g) => g,
            Self::Released => panic!("released host guard"),
        }
    }
    fn bytes_mut(&mut self) -> Result<&mut [u8], Error> {
        match self {
            Self::Exclusive(g) => Ok(g),
            _ => Err(sdk::ErrorCode::ConstraintMut.into()),
        }
    }
}
fn bytes() -> Vec<u8> {
    let mut bytes = vec![0; 256];
    State::initialize(
        &mut SliceStore::new(&mut bytes),
        StateInit {
            counter: 7,
            flag: true,
            text: "hello",
            values: &["a", "bc"],
        },
    )
    .unwrap()
    .commit()
    .unwrap();
    bytes
}
struct HostSession<'a> {
    inner: Session<State, HostGuard<'a>>,
    cell: &'a RefCell<Vec<u8>>,
}
impl<'a> Deref for HostSession<'a> {
    type Target = Session<State, HostGuard<'a>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl core::ops::DerefMut for HostSession<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl HostSession<'_> {
    fn edit<R>(
        &mut self,
        edit: impl FnOnce(&mut StateMut<'_>) -> blubber::Result<R>,
    ) -> Result<(R, CommitInfo), Error> {
        self.inner.check_ready()?;
        self.inner.lease.bytes_mut()?;
        let cell = self.cell;
        self.inner.handoff(
            HostGuard::Released,
            |lease| {
                drop(lease);
                let mut bytes = cell.borrow_mut();
                let mut store = SliceStore::new(&mut bytes);
                let result = managed_edit::<State, R>(&mut store, edit);
                (result.map_err(Error::from), store.is_poisoned())
            },
            || {
                cell.try_borrow_mut()
                    .map(HostGuard::Exclusive)
                    .map_err(|_| Error::AccountBorrowFailed)
            },
        )
    }
}
fn load(cell: &RefCell<Vec<u8>>, mutable: bool) -> Result<HostSession<'_>, Error> {
    let guard = if mutable {
        HostGuard::Exclusive(
            cell.try_borrow_mut()
                .map_err(|_| Error::AccountBorrowFailed)?,
        )
    } else {
        HostGuard::Shared(cell.try_borrow().map_err(|_| Error::AccountBorrowFailed)?)
    };
    Ok(HostSession {
        inner: Session::load::<super::Checked>(guard)?,
        cell,
    })
}

#[test]
fn duplicate_guard_permutations_and_drop() {
    for first in [false, true] {
        for second in [false, true] {
            let cell = RefCell::new(bytes());
            let one = load(&cell, first).unwrap();
            let two = load(&cell, second);
            assert_eq!(two.is_ok(), !first && !second);
            drop(two);
            drop(one);
            assert!(load(&cell, true).is_ok());
        }
    }
}

#[test]
fn forgotten_guards_keep_exclusion() {
    for mutable in [false, true] {
        let cell = RefCell::new(bytes());
        core::mem::forget(load(&cell, mutable).unwrap());
        assert!(load(&cell, true).is_err());
        assert_eq!(load(&cell, false).is_ok(), !mutable);
    }
}

#[test]
fn full_ingress_rejects_distant_corruption_and_releases_guard() {
    let good = bytes();
    let used = u32::from_le_bytes(good[8..12].try_into().unwrap()) as usize;
    for (offset, value) in [(0, 0), (9, 1), (20, 2), (used - 1, 255)] {
        let cell = RefCell::new(good.clone());
        cell.borrow_mut()[offset] = value;
        assert!(load(&cell, false).is_err(), "offset {offset}");
        assert!(cell.try_borrow_mut().is_ok());
    }
}

#[test]
fn reads_and_copied_prefix_refresh_after_edits_and_errors() {
    let cell = RefCell::new(bytes());
    let mut session = load(&cell, true).unwrap();
    assert_eq!(session.read(|s| Ok(s.text().len())).unwrap(), 5);
    let old = session.prefix.unwrap();
    let (value, receipt) = session
        .edit(|s| {
            s.counter_mut().set(11)?;
            Ok(29)
        })
        .unwrap();
    assert_eq!(value, 29);
    assert_eq!(receipt.storage_len, 256);
    assert_eq!(old.counter, 7);
    assert_eq!(session.prefix.unwrap().counter, 11);
    let error = session
        .edit::<()>(|s| {
            s.counter_mut().set(13)?;
            Err(blubber::ErrorKind::Capacity.into())
        })
        .unwrap_err();
    assert_eq!(
        error,
        blubber::Error::from(blubber::ErrorKind::Capacity).into()
    );
    assert_eq!(session.prefix.unwrap().counter, 13);
    assert_eq!(session.read(|s| Ok(s.counter())).unwrap(), 13);
}

#[test]
fn readonly_edit_rejection_preserves_readiness() {
    let cell = RefCell::new(bytes());
    let mut session = load(&cell, false).unwrap();
    assert_eq!(
        session.edit(|_| Ok(())).unwrap_err(),
        sdk::ErrorCode::ConstraintMut.into()
    );
    assert!(session.check_ready().is_ok());
}

#[test]
fn unwind_is_terminal_and_reacquires_exclusion() {
    let cell = RefCell::new(bytes());
    let mut session = load(&cell, true).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = session.edit::<()>(|s| {
            s.counter_mut().set(99)?;
            panic!("callback interrupted")
        });
    }));
    assert!(result.is_err());
    assert!(session.prefix.is_none());
    assert!(session.read(|_| Ok(())).is_err());
    assert!(session.edit(|_| Ok(())).is_err());
    assert!(session.check_ready().is_err());
    // Store unwinds first; handoff cleanup reacquires exclusion, without
    // publishing a prefix or making the terminal typed session usable.
    assert!(cell.try_borrow().is_err());
    drop(session);
    assert_eq!(
        load(&cell, false)
            .unwrap()
            .read(|s| Ok(s.counter()))
            .unwrap(),
        99
    );
}

#[test]
fn capacity_failure_keeps_bytes_and_prefix() {
    let cell = RefCell::new(bytes());
    let before = cell.borrow().clone();
    let mut session = load(&cell, true).unwrap();
    assert!(session
        .edit(|s| s.text_mut().set(&"x".repeat(300)))
        .is_err());
    assert_eq!(session.lease.bytes(), before);
    assert!(session.prefix.is_some());
}

#[test]
fn explicit_trim_uses_store_receipt_and_reopens_spare() {
    let cell = RefCell::new(bytes());
    let mut session = load(&cell, true).unwrap();
    let (_, receipt) = session.edit(|s| s.shrink_to_fit()).unwrap();
    assert_eq!(receipt.storage_len, receipt.used_len);
    session.edit(|s| s.text_mut().set("longer text")).unwrap();
    assert_eq!(session.read(|s| Ok(s.text().len())).unwrap(), 11);
}

#[test]
fn new_state_excludes_reads_edits_and_exit_before_initialization() {
    let cell = RefCell::new(vec![0; 256]);
    let mut session = HostSession {
        inner: Session::<State, _>::new(HostGuard::Exclusive(cell.borrow_mut())),
        cell: &cell,
    };
    assert!(session.new);
    assert!(session.check_ready().is_err());
    assert!(session.read(|_| Ok(())).is_err());
    assert!(session.edit(|_| Ok(())).is_err());
    assert!(cell.try_borrow().is_err());
    assert!(session.new);
    assert!(session.lease.bytes().iter().all(|b| *b == 0));
}

#[test]
fn new_measurement_errors_are_atomic_and_ready_reinitialization_is_rejected() {
    let cell = RefCell::new(vec![0; 32]);
    let new = Session::<State, _>::new(HostGuard::Exclusive(cell.borrow_mut()));
    let long = "x".repeat(100);
    let init = StateInit {
        counter: 7,
        flag: true,
        text: &long,
        values: &[],
    };
    assert_eq!(
        new.check_initialize(init).err(),
        Some(blubber::Error::from(blubber::ErrorKind::Capacity).into())
    );
    assert!(new.new && new.prefix.is_none());
    assert!(new.lease.bytes().iter().all(|b| *b == 0));
    drop(new);
    let cell = RefCell::new(bytes());
    let ready = load(&cell, true).unwrap();
    assert_eq!(
        ready
            .check_initialize(StateInit {
                counter: 7,
                flag: true,
                text: &long,
                values: &[]
            })
            .err(),
        Some(blubber::Error::from(blubber::ErrorKind::AlreadyInitialized).into())
    );
    assert_eq!(ready.prefix.unwrap().counter, 7);
}

#[test]
fn handoff_reacquires_after_operation_errors_and_preserves_precedence() {
    for operation_error in [false, true] {
        for acquire_error in [false, true] {
            let cell = RefCell::new(bytes());
            let mut session = load(&cell, true).unwrap();
            let result = session.inner.handoff(
                HostGuard::Released,
                |lease| {
                    drop(lease);
                    assert!(cell.try_borrow_mut().is_ok());
                    (
                        if operation_error {
                            Err(Error::Custom(71))
                        } else {
                            Ok(9)
                        },
                        false,
                    )
                },
                || {
                    if acquire_error {
                        Err(Error::Custom(72))
                    } else {
                        Ok(HostGuard::Exclusive(cell.borrow_mut()))
                    }
                },
            );
            assert_eq!(
                result,
                if operation_error {
                    Err(Error::Custom(71))
                } else if acquire_error {
                    Err(Error::Custom(72))
                } else {
                    Ok(9)
                }
            );
            assert_eq!(session.check_ready().is_ok(), !acquire_error);
            assert_eq!(cell.try_borrow_mut().is_err(), !acquire_error);
        }
    }
}

#[test]
fn destructive_handoff_keeps_reacquired_exclusion_and_terminal_poison() {
    let cell = RefCell::new(bytes());
    let mut session = load(&cell, true).unwrap();
    let error = session.inner.handoff(
        HostGuard::Released,
        |lease| {
            drop(lease);
            (Err::<(), _>(Error::Custom(73)), true)
        },
        || Ok(HostGuard::Exclusive(cell.borrow_mut())),
    );
    assert_eq!(error, Err(Error::Custom(73)));
    assert!(session.check_ready().is_err());
    assert!(cell.try_borrow_mut().is_err());
    assert!(session.read(|_| Ok(())).is_err());
}

#[test]
fn managed_edit_publishes_once_and_checks_closed_completion() {
    let mut bytes = bytes();
    let mut store = SliceStore::new(&mut bytes);
    let (returned, receipt) = managed_edit::<State, _>(&mut store, |s| {
        assert_eq!(s.storage_len(), 256);
        s.counter_mut().set(31)?;
        s.values_mut().push("borrowed")?;
        Ok(19)
    })
    .unwrap();
    assert_eq!(returned, 19);
    assert_eq!(receipt.storage_len, 256);
    assert!(blubber::ReadStorage::is_closed(&store));
    assert_eq!(
        store.commit(receipt.used_len).unwrap_err(),
        blubber::ErrorKind::Closed.into()
    );
    let read = State::load(&bytes).unwrap();
    assert_eq!(read.counter(), 31);
    assert_eq!(read.values().get(2).unwrap(), "borrowed");
}

#[blubber::schema(account)]
pub struct WideSessionState {
    pub prefix: [u8; 1024],
    pub text: Str,
}

#[test]
fn widest_prefix_load_and_handoff_refresh_preserve_all_bytes() {
    let mut prefix = [0; 1024];
    for (index, byte) in prefix.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    let mut bytes = vec![0; 2048];
    WideSessionState::initialize(
        &mut SliceStore::new(&mut bytes),
        WideSessionStateInit {
            prefix,
            text: "wide",
        },
    )
    .unwrap()
    .commit()
    .unwrap();
    let cell = RefCell::new(bytes);
    let mut session = Session::<WideSessionState, _>::load::<super::Checked>(HostGuard::Exclusive(
        cell.borrow_mut(),
    ))
    .unwrap();
    let before = session.prefix.unwrap();
    assert_eq!(before.prefix, prefix);
    session.check_ready().unwrap();
    session.lease.bytes_mut().unwrap();
    let (_, receipt) = session
        .handoff(
            HostGuard::Released,
            |lease| {
                drop(lease);
                let mut bytes = cell.borrow_mut();
                let mut store = SliceStore::new(&mut bytes);
                let result = managed_edit::<WideSessionState, _>(&mut store, |s| {
                    s.text_mut().set("refreshed")
                });
                (result.map_err(Error::from), store.is_poisoned())
            },
            || Ok(HostGuard::Exclusive(cell.borrow_mut())),
        )
        .unwrap();
    assert_eq!(receipt.storage_len, 2048);
    assert_eq!(session.prefix.unwrap().prefix, before.prefix);
    assert_eq!(session.read(|s| Ok(s.text().len())).unwrap(), 9);
    assert!(cell.try_borrow_mut().is_err());
}

#[test]
fn close_invalidation_releases_exclusion_and_denies_all_typed_access() {
    let cell = RefCell::new(bytes());
    let mut session = load(&cell, true).unwrap();
    session.inner.invalidate(HostGuard::Released).unwrap();
    assert!(session.check_ready().is_err());
    assert!(session.read(|s| Ok(s.counter())).is_err());
    assert!(session.edit(|s| s.counter_mut().set(99)).is_err());
    assert!(session.inner.invalidate(HostGuard::Released).is_err());
    assert!(!session.new);
    let data = cell.borrow_mut();
    assert_eq!(&data[..8], &State::DISCRIMINATOR.map(|byte| !byte));
    assert_eq!(&data[8..12], &[0; 4]);
    assert!(View::load::<State>(&data, State::DISCRIMINATOR).is_err());
}

#[test]
fn close_rejects_shared_and_new_sessions_without_invalidating_bytes() {
    let cell = RefCell::new(bytes());
    let before = cell.borrow().clone();
    let mut shared = load(&cell, false).unwrap();
    assert!(shared.inner.invalidate(HostGuard::Released).is_err());
    assert!(shared.check_ready().is_ok());
    drop(shared);
    let mut new = Session::<State, _>::new(HostGuard::Exclusive(cell.borrow_mut()));
    assert!(new.invalidate(HostGuard::Released).is_err());
    assert!(new.new);
    drop(new);
    assert_eq!(*cell.borrow(), before);
}

#[test]
fn lifecycle_preflights_preserve_caps_used_bytes_and_lamport_sums() {
    use super::lifecycle::{check_space, close_balance};
    for used in [12, 48, 256, 10_485_760] {
        assert!(check_space(used, used).is_ok());
        assert!(check_space(used - 1, used).is_err());
        assert!(check_space(10_485_760, used).is_ok());
        assert!(check_space(10_485_761, used).is_err());
    }
    assert_eq!(close_balance(10, 20), Ok(30));
    assert_eq!(close_balance(u64::MAX, 0), Ok(u64::MAX));
    assert!(close_balance(1, u64::MAX).is_err());
}

#[test]
fn cpi_releases_restores_both_guard_modes_and_refreshes_prefix() {
    for mutable in [false, true] {
        let cell = RefCell::new(bytes());
        let mut session = load(&cell, mutable).unwrap();
        let old = session.prefix.unwrap();
        let result = session.inner.cpi(
            HostGuard::Released,
            || {
                let mut bytes = cell.borrow_mut();
                let mut store = SliceStore::new(&mut bytes);
                managed_edit::<State, _>(&mut store, |s| s.counter_mut().set(29))?;
                Ok(17)
            },
            || {
                Ok(if mutable {
                    HostGuard::Exclusive(cell.borrow_mut())
                } else {
                    HostGuard::Shared(cell.borrow())
                })
            },
            || Ok(()),
        );
        assert_eq!(result, Ok(17));
        assert_eq!(old.counter, 7);
        assert_eq!(session.prefix.unwrap().counter, 29);
        assert_eq!(session.read(|s| Ok(s.counter())).unwrap(), 29);
        assert!(cell.try_borrow_mut().is_err());
        assert_eq!(cell.try_borrow().is_err(), mutable);
    }
}

#[test]
fn cpi_full_reload_rejects_envelope_scalar_directory_and_utf8_damage() {
    let good = bytes();
    let used = u32::from_le_bytes(good[8..12].try_into().unwrap()) as usize;
    for (offset, value) in [(0, 0), (9, 1), (20, 2), (21, 255), (used - 1, 255)] {
        for operation_error in [false, true] {
            let cell = RefCell::new(good.clone());
            let mut session = load(&cell, true).unwrap();
            let error = session
                .inner
                .cpi(
                    HostGuard::Released,
                    || {
                        cell.borrow_mut()[offset] = value;
                        if operation_error {
                            Err(Error::Custom(91))
                        } else {
                            Ok(())
                        }
                    },
                    || Ok(HostGuard::Exclusive(cell.borrow_mut())),
                    || Ok(()),
                )
                .unwrap_err();
            if operation_error {
                assert_eq!(error, Error::Custom(91));
            }
            assert!(session.prefix.is_none());
            assert!(session.read(|_| Ok(())).is_err());
            assert!(session.edit(|_| Ok(())).is_err());
            assert!(cell.try_borrow().is_err());
        }
    }
}

#[test]
fn cpi_authentication_failure_denies_prefix_even_with_valid_bytes() {
    for operation_error in [false, true] {
        let cell = RefCell::new(bytes());
        let mut session = load(&cell, true).unwrap();
        let result = session.inner.cpi(
            HostGuard::Released,
            || {
                if operation_error {
                    Err::<(), _>(Error::Custom(91))
                } else {
                    Ok(())
                }
            },
            || Ok(HostGuard::Exclusive(cell.borrow_mut())),
            || Err(Error::IllegalOwner),
        );
        assert_eq!(
            result,
            Err(if operation_error {
                Error::Custom(91)
            } else {
                Error::IllegalOwner
            })
        );
        assert!(session.prefix.is_none());
        assert!(cell.try_borrow().is_err());
    }
}

#[test]
fn cpi_returned_errors_reacquire_validate_and_keep_original_error() {
    let cell = RefCell::new(bytes());
    let mut session = load(&cell, true).unwrap();
    assert_eq!(
        session.inner.cpi(
            HostGuard::Released,
            || Err::<(), _>(Error::Custom(91)),
            || Ok(HostGuard::Exclusive(cell.borrow_mut())),
            || Ok(()),
        ),
        Err(Error::Custom(91))
    );
    assert_eq!(session.prefix.unwrap().counter, 7);
    assert!(session.check_ready().is_ok());
    assert!(cell.try_borrow().is_err());
}

#[test]
fn cpi_reacquisition_failures_are_terminal_with_original_error_precedence() {
    for operation_error in [false, true] {
        let cell = RefCell::new(bytes());
        let mut session = load(&cell, true).unwrap();
        let result = session.inner.cpi(
            HostGuard::Released,
            || {
                if operation_error {
                    Err::<(), _>(Error::Custom(91))
                } else {
                    Ok(())
                }
            },
            || Err(Error::AccountBorrowFailed),
            || panic!("authentication must follow acquisition"),
        );
        assert_eq!(
            result,
            Err(if operation_error {
                Error::Custom(91)
            } else {
                Error::AccountBorrowFailed
            })
        );
        assert!(session.prefix.is_none());
        assert!(session.read(|_| Ok(())).is_err());
        assert!(session
            .inner
            .cpi(
                HostGuard::Released,
                || Ok(()),
                || panic!("terminal"),
                || Ok(())
            )
            .is_err());
    }
}

#[test]
fn cpi_unwind_reacquires_without_publishing_prefix() {
    let cell = RefCell::new(bytes());
    let mut session = load(&cell, true).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), Error> = session.inner.cpi(
            HostGuard::Released,
            || panic!("callee interrupted"),
            || Ok(HostGuard::Exclusive(cell.borrow_mut())),
            || Ok(()),
        );
    }));
    assert!(result.is_err());
    assert!(session.prefix.is_none());
    assert!(cell.try_borrow().is_err());
}

#[test]
fn lazy_admission_checks_prefix_and_touched_paths_without_global_evidence() {
    let mut data = bytes();
    let used = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    data[used - 1] = 255;
    let cell = RefCell::new(data);
    let lazy = Session::<State, _>::load::<super::Lazy>(HostGuard::Shared(cell.borrow()));
    if cfg!(feature = "verify-on-load") {
        assert!(lazy.is_err());
    } else {
        let lazy = lazy.unwrap();
        assert_eq!(lazy.prefix.unwrap().counter, 7);
        assert_eq!(lazy.read(|s| Ok(s.counter())).unwrap(), 7);
        assert!(lazy.read(|s| s.values().try_get(1).map(|_| ())).is_err());
        assert!(View::load::<State>(lazy.lease.bytes(), State::DISCRIMINATOR).is_err());
        assert!(cell.try_borrow_mut().is_err());
    }
    assert!(Session::<State, _>::load::<super::Checked>(HostGuard::Shared(cell.borrow())).is_err());
    assert!(cell.try_borrow_mut().is_ok());
    cell.borrow_mut()[20] = 2;
    assert!(Session::<State, _>::load::<super::Lazy>(HostGuard::Shared(cell.borrow())).is_err());
    assert!(cell.try_borrow_mut().is_ok());
}

/// Deliberately violates the encoder contract to test conservative admission.
#[derive(Clone, Copy)]
pub struct Unreviewed;
std::thread_local! {
    static UNREVIEWED_MEASUREMENTS: Cell<usize> = const { Cell::new(0) };
}
impl blubber::Fixed for Unreviewed {
    const SIZE: u32 = 1;
    type Encoded = [u8; 1];
    fn decode(bytes: &[u8]) -> blubber::Result<Self> {
        <bool as blubber::Fixed>::decode(bytes)?;
        Ok(Self)
    }
    fn encode(self) -> [u8; 1] {
        [2]
    }
}
#[derive(Clone, Copy, blubber::Fixed)]
/// Fixed derives must retain the fallback for handwritten field codecs.
pub struct UnreviewedFixed {
    /// Deliberately unaudited fixed encoder.
    child: Unreviewed,
}
#[derive(Clone, Copy, blubber::Fixed)]
/// Fully audited native fixed composition.
pub struct ReviewedFixed {
    /// Checked bool domain.
    flag: bool,
    /// Native alignment is excluded from packed wire bytes.
    value: u64,
}
impl blubber::Schema for Unreviewed {
    const KIND: blubber::Kind = blubber::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <bool as blubber::Schema>::LAYOUT;
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        <bool as blubber::Schema>::WIRE_DESCRIPTOR;
    const __LEN: usize = <bool as blubber::Schema>::__LEN;
    const __HEADER: &'static [u32] = <bool as blubber::Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = bool;
    type Mut<'a> = blubber::raw::Edit<'a>;
    type Init<'a> = u8;
    type Removed = bool;
    fn project<'a>(projection: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::into_edit(projection)
    }
    fn read(view: View<'_>) -> blubber::Result<bool> {
        view.read_fixed::<bool>()
    }
    fn read_removed(view: View<'_>) -> blubber::Result<bool> {
        Self::read(view)
    }
    fn __read_fixed(view: View<'_>) -> blubber::Result<u8> {
        Ok(u8::from(Self::read(view)?))
    }
    fn measure(init: &u8) -> blubber::Result<u32> {
        UNREVIEWED_MEASUREMENTS.with(|calls| calls.set(calls.get() + 1));
        // A deliberately invalid measurement tests pre-handoff rejection.
        Ok(if *init == 254 { 0 } else { 1 })
    }
    fn encode(init: &u8, destination: &mut [u8]) {
        destination[0] = *init;
        assert_ne!(*init, 255, "corrupted encoder panic");
    }
    type Owned = u8;
    fn measure_owned(init: &u8) -> blubber::Result<u32> {
        Self::measure(init)
    }
    fn encode_owned(init: &u8, destination: &mut [u8]) {
        Self::encode(init, destination)
    }
    fn decode_owned(view: View<'_>) -> blubber::Result<u8> {
        Ok(u8::from(Self::read(view)?))
    }
}

#[blubber::schema(account)]
/// A handwritten codec several levels below a generated root.
pub struct UnreviewedRoot {
    /// Ensures transitive audit propagation through struct/list/option.
    values: List<Option<Unreviewed>>,
}

#[test]
fn initialization_policy_propagates_the_complete_encoder_graph() {
    use blubber::Schema;
    const {
        assert!(State::__ENCODER_AUDITED);
    }
    const {
        assert!(!UnreviewedRoot::__ENCODER_AUDITED);
        assert!(!<UnreviewedFixed as blubber::Schema>::__ENCODER_AUDITED);
        assert!(!<UnreviewedFixed as blubber::Fixed>::__ENCODER_AUDITED);
        assert!(<ReviewedFixed as blubber::Schema>::__ENCODER_AUDITED);
        assert!(<ReviewedFixed as blubber::Fixed>::__ENCODER_AUDITED);
    }
    for value in [1, 2, 255] {
        let cell = RefCell::new(vec![0; 128]);
        let mut session =
            Session::<UnreviewedRoot, _>::new(HostGuard::Exclusive(cell.borrow_mut()));
        let init = UnreviewedRootInit {
            values: &[Some(value)],
        };
        let measured = session.check_initialize(init).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            session.initialize_with::<super::Lazy, _>(
                HostGuard::Released,
                |mut lease| {
                    let mut store = SliceStore::new(lease.bytes_mut()?);
                    Ok(
                        Projection::<UnreviewedRoot>::initialize_measured(&mut store, measured)?
                            .commit()?,
                    )
                },
                || Ok(HostGuard::Exclusive(cell.borrow_mut())),
            )
        }));
        assert!(!session.new);
        assert!(cell.try_borrow_mut().is_err());
        if value == 255 {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().is_ok(), value == 1);
        }
        assert_eq!(session.prefix.is_some(), value == 1);
        if value != 1 {
            assert!(session.check_ready().is_err());
            assert!(session
                .check_initialize(UnreviewedRootInit { values: &[] })
                .is_err());
            assert!(
                initialized_view::<UnreviewedRoot, super::Lazy>(session.lease.bytes()).is_err()
            );
            assert!(
                initialized_view::<UnreviewedRoot, super::Checked>(session.lease.bytes()).is_err()
            );
        }
        drop(session);
        assert!(cell.try_borrow_mut().is_ok());
    }
}

#[test]
fn prepared_initialization_preserves_handwritten_measurement_calls() {
    use blubber::Schema;
    const {
        assert!(!UnreviewedRoot::__MEASURE_REUSABLE);
        // Fixed derive's measurement is a constant; it never invokes fields.
        assert!(<UnreviewedFixed as Schema>::__MEASURE_REUSABLE);
    }
    UNREVIEWED_MEASUREMENTS.with(|calls| calls.set(0));
    let mut bytes = vec![0xA5; 128];
    bytes[..8].fill(0);
    let before = bytes.clone();
    let cell = RefCell::new(bytes);
    let mut session = Session::<UnreviewedRoot, _>::new(HostGuard::Exclusive(cell.borrow_mut()));
    let measured = session
        .check_initialize(UnreviewedRootInit { values: &[Some(1)] })
        .unwrap();
    assert_eq!(session.lease.bytes(), before);
    UNREVIEWED_MEASUREMENTS.with(|calls| assert_eq!(calls.get(), 1));
    session
        .initialize_with::<super::Lazy, _>(
            HostGuard::Released,
            |mut lease| {
                let mut store = SliceStore::new(lease.bytes_mut()?);
                Ok(
                    Projection::<UnreviewedRoot>::initialize_measured(&mut store, measured)?
                        .commit()?,
                )
            },
            || Ok(HostGuard::Exclusive(cell.borrow_mut())),
        )
        .unwrap();
    // Preflight, root initialization, struct child sizing, list child sizing.
    UNREVIEWED_MEASUREMENTS.with(|calls| assert_eq!(calls.get(), 4));
    let view =
        View::load::<UnreviewedRoot>(session.lease.bytes(), UnreviewedRoot::DISCRIMINATOR).unwrap();
    assert!(session.lease.bytes()[12 + view.bytes().len()..]
        .iter()
        .all(|byte| *byte == 0));
}

#[blubber::schema(account)]
/// An unaudited fixed child can deliberately understate its minimum width.
pub struct MinimumRoot {
    value: Unreviewed,
}

#[test]
fn invalid_minimum_measurement_preserves_new_state_before_handoff() {
    let cell = RefCell::new(vec![0; 128]);
    let session = Session::<MinimumRoot, _>::new(HostGuard::Exclusive(cell.borrow_mut()));
    assert_eq!(
        session
            .check_initialize(MinimumRootInit { value: 254 })
            .err(),
        Some(blubber::Error::from(blubber::ErrorKind::InvalidBoundary).into())
    );
    assert!(session.new && session.prefix.is_none());
    assert!(session.lease.bytes().iter().all(|byte| *byte == 0));
    assert!(cell.try_borrow_mut().is_err());
}

#[test]
fn pure_prepared_initialization_matches_independent_bytes_at_and_above_minimum() {
    use blubber::Schema;
    use blubber_model::{IntWidth::W8, Schema as Model, Value};
    const {
        assert!(State::__MEASURE_REUSABLE);
    }
    let model = Model::Struct(vec![
        Model::Unsigned(W8),
        Model::Bool,
        Model::Str,
        Model::List(alloc::boxed::Box::new(Model::Str)),
    ]);
    for (text, values) in [("", &[][..]), ("λabc", &["", "bc"][..])] {
        let expected = model
            .encode_account(
                State::DISCRIMINATOR,
                &Value::Struct(vec![
                    Value::Unsigned(7),
                    Value::Bool(true),
                    Value::Str(text.into()),
                    Value::List(
                        values
                            .iter()
                            .map(|value| Value::Str((*value).into()))
                            .collect(),
                    ),
                ]),
                256,
            )
            .unwrap();
        let mut bytes = vec![0xA5; 256];
        bytes[..8].fill(0);
        let before = bytes.clone();
        let cell = RefCell::new(bytes);
        let mut session = Session::<State, _>::new(HostGuard::Exclusive(cell.borrow_mut()));
        let measured = session
            .check_initialize(StateInit {
                counter: 7,
                flag: true,
                text,
                values,
            })
            .unwrap();
        assert!(session.new && session.prefix.is_none());
        assert_eq!(session.lease.bytes(), before);
        let receipt = session
            .initialize_with::<super::Lazy, _>(
                HostGuard::Released,
                |mut lease| {
                    let mut store = SliceStore::new(lease.bytes_mut()?);
                    Ok(Projection::<State>::initialize_measured(&mut store, measured)?.commit()?)
                },
                || Ok(HostGuard::Exclusive(cell.borrow_mut())),
            )
            .unwrap();
        assert_eq!(session.lease.bytes(), expected);
        assert_eq!(
            receipt.used_len,
            u32::from_le_bytes(expected[8..12].try_into().unwrap())
        );
        assert_eq!(receipt.storage_len, 256);
        assert!(!session.new);
        assert_eq!(session.prefix.as_ref().unwrap().counter, 7);
        assert!(session.prefix.as_ref().unwrap().flag);
        session
            .read(|view| {
                assert_eq!(view.text(), text);
                assert_eq!(view.values().len(), values.len());
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn init_handoff_keeps_first_fault_and_withdraws_prefix_on_lost_acquisition() {
    for operation_error in [false, true] {
        for acquire_error in [false, true] {
            let cell = RefCell::new(vec![0; 128]);
            let mut session = Session::<State, _>::new(HostGuard::Exclusive(cell.borrow_mut()));
            let measured = session
                .check_initialize(StateInit {
                    counter: 7,
                    flag: true,
                    text: "λ",
                    values: &["", "bc"],
                })
                .unwrap();
            let result = session.initialize_with::<super::Lazy, _>(
                HostGuard::Released,
                |mut lease| {
                    if operation_error {
                        return Err(Error::Custom(71));
                    }
                    let mut store = SliceStore::new(lease.bytes_mut()?);
                    Projection::<State>::initialize_measured(&mut store, measured)?.commit()?;
                    Ok(())
                },
                || {
                    if acquire_error {
                        Err(Error::Custom(72))
                    } else {
                        Ok(HostGuard::Exclusive(cell.borrow_mut()))
                    }
                },
            );
            let expected = if operation_error {
                Err(Error::Custom(71))
            } else if acquire_error {
                Err(Error::Custom(72))
            } else {
                Ok(())
            };
            assert_eq!(result, expected);
            assert!(!session.new);
            assert_eq!(session.prefix.is_some(), result.is_ok());
            assert_eq!(session.check_ready().is_ok(), result.is_ok());
            assert_eq!(cell.try_borrow_mut().is_err(), !acquire_error);
            if !operation_error && acquire_error {
                let bytes = cell.borrow();
                let view = View::load::<State>(&bytes, State::DISCRIMINATOR).unwrap();
                assert_eq!(view.bytes()[0], 7);
            }
        }
    }
}

/// Handwritten key with no canonical-encoder audit opt-in.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct UnreviewedKey(u8);
impl blubber::Fixed for UnreviewedKey {
    const SIZE: u32 = 1;
    type Encoded = [u8; 1];
    fn decode(bytes: &[u8]) -> blubber::Result<Self> {
        Ok(Self(<u8 as blubber::Fixed>::decode(bytes)?))
    }
    fn encode(self) -> [u8; 1] {
        [self.0]
    }
}
impl blubber::MapKey for UnreviewedKey {
    const KEY_LAYOUT: blubber_core::KeyLayout = <u8 as blubber::MapKey>::KEY_LAYOUT;
    type KeyBytes = [u8; 1];
    fn encode_key(self) -> [u8; 1] {
        [self.0]
    }
    fn decode_key(bytes: &[u8]) -> blubber::Result<Self> {
        <Self as blubber::Fixed>::decode(bytes)
    }
}

#[blubber::schema]
/// Generic derived codecs must propagate their instantiated child audits.
pub struct InitGeneric<T> {
    value: T,
}

#[blubber::schema]
/// Nested derived codec used by native initialization admission.
pub struct InitChild {
    /// Bool validity and UTF-8 must survive exact nested encoding.
    flag: bool,
    /// Borrowed nested string input.
    text: Str,
}
#[blubber::schema(account)]
/// Exercises every variable encoder composition in one account.
pub struct InitGraph {
    /// Nested options, lists and indexed-value maps.
    entries: blubber::Map<i16, List<Option<InitChild>>>,
}

#[test]
fn audited_nested_initialization_matches_exact_bytes_and_spare_frames() {
    use blubber::Schema;
    const {
        assert!(InitGraph::__ENCODER_AUDITED);
        assert!(InitGeneric::<InitChild>::__ENCODER_AUDITED);
        assert!(!InitGeneric::<Unreviewed>::__ENCODER_AUDITED);
        assert!(!blubber::Map::<UnreviewedKey, u8>::__ENCODER_AUDITED);
        assert!(InitGraph::__MEASURE_REUSABLE);
        assert!(InitGeneric::<InitChild>::__MEASURE_REUSABLE);
        assert!(!InitGeneric::<Unreviewed>::__MEASURE_REUSABLE);
        assert!(!blubber::Map::<UnreviewedKey, u8>::__MEASURE_REUSABLE);
    }
    let text = alloc::string::String::from("λabc");
    let children = [
        Some(InitChildInit {
            flag: true,
            text: &text,
        }),
        None,
    ];
    let entries = [(-7i16, &children[..])];
    let mut bytes = vec![0; 256];
    InitGraph::initialize(
        &mut SliceStore::new(&mut bytes),
        InitGraphInit { entries: &entries },
    )
    .unwrap()
    .commit()
    .unwrap();
    use blubber_model::{IntWidth::W2, Key, KeySchema, Schema as Model, Value};
    let model = Model::Struct(alloc::vec![Model::Map(
        KeySchema::Signed(W2),
        alloc::boxed::Box::new(Model::List(alloc::boxed::Box::new(Model::Option(
            alloc::boxed::Box::new(Model::Struct(alloc::vec![Model::Bool, Model::Str]))
        ))))
    )]);
    let value = Value::Struct(alloc::vec![Value::Map(alloc::collections::BTreeMap::from(
        [(
            Key::Signed(-7),
            Value::List(alloc::vec![
                Value::Option(Some(alloc::boxed::Box::new(Value::Struct(alloc::vec![
                    Value::Bool(true),
                    Value::Str(text.clone())
                ])))),
                Value::Option(None)
            ])
        )]
    ))]);
    assert_eq!(
        bytes,
        model
            .encode_account(InitGraph::DISCRIMINATOR, &value, 256)
            .unwrap()
    );
    let checked = View::load::<InitGraph>(&bytes, InitGraph::DISCRIMINATOR).unwrap();
    let used = checked.bytes().len() + 12;
    let exact = bytes[..used].to_vec();
    bytes[used..].fill(0xA5);
    for view in [
        initialized_view::<InitGraph, super::Lazy>(&bytes).unwrap(),
        initialized_view::<InitGraph, super::Checked>(&bytes).unwrap(),
    ] {
        assert_eq!(view.bytes(), &exact[12..]);
        view.validate().unwrap();
    }
    assert!(bytes[used..].iter().all(|b| *b == 0xA5));
    assert_eq!(
        InitGraph::load(&bytes)
            .unwrap()
            .entries()
            .get(&-7)
            .unwrap()
            .get(0)
            .unwrap()
            .unwrap()
            .text(),
        text
    );
}

/// Deliberately broken true-marker contract exercises route selection and bounds.
#[derive(Clone, Copy)]
pub struct ReplayProbe;
impl blubber::Schema for ReplayProbe {
    const __ENCODER_AUDITED: bool = true;
    const __MEASURE_REUSABLE: bool = true;
    const KIND: blubber::Kind = blubber::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <bool as blubber::Schema>::LAYOUT;
    const WIRE_DESCRIPTOR: &'static blubber::identity::WireDescriptor =
        <bool as blubber::Schema>::WIRE_DESCRIPTOR;
    const __LEN: usize = <bool as blubber::Schema>::__LEN;
    const __HEADER: &'static [u32] = <bool as blubber::Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = bool;
    type Mut<'a> = blubber::raw::Edit<'a>;
    type Init<'a> = u8;
    type Removed = bool;
    fn project<'a>(projection: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::into_edit(projection)
    }
    fn read(view: View<'_>) -> blubber::Result<bool> {
        view.read_fixed::<bool>()
    }
    fn read_removed(view: View<'_>) -> blubber::Result<bool> {
        Self::read(view)
    }
    fn measure(_: &u8) -> blubber::Result<u32> {
        UNREVIEWED_MEASUREMENTS.with(|calls| calls.set(calls.get() + 1));
        Ok(1)
    }
    fn encode(init: &u8, destination: &mut [u8]) {
        destination[0] = *init;
    }
    type Owned = u8;
    fn measure_owned(init: &u8) -> blubber::Result<u32> {
        Self::measure(init)
    }
    fn encode_owned(init: &u8, destination: &mut [u8]) {
        Self::encode(init, destination);
    }
    fn decode_owned(view: View<'_>) -> blubber::Result<u8> {
        Ok(u8::from(Self::read(view)?))
    }
}

#[blubber::schema(account)]
/// Complete marked graph exercises native, fixed-child and tail replay.
pub struct ReplayGraph {
    list: List<ReplayProbe>,
    map: blubber::Map<u8, ReplayProbe>,
    tail: List<ReplayProbe>,
}

#[test]
fn marked_graph_selects_proved_route_with_checked_and_feature_scans() {
    marked_initialization::<super::Lazy>();
    marked_initialization::<super::Checked>();
}

fn marked_initialization<P: super::LoadPolicy>() {
    use blubber::Schema;
    const {
        assert!(ReplayGraph::__ENCODER_AUDITED && ReplayGraph::__MEASURE_REUSABLE);
    }
    for value in [1, 2] {
        UNREVIEWED_MEASUREMENTS.with(|calls| calls.set(0));
        let cell = RefCell::new(vec![0; 128]);
        let mut session = Session::<ReplayGraph, _>::new(HostGuard::Exclusive(cell.borrow_mut()));
        let values = [value, 0];
        let measured = session
            .check_initialize(ReplayGraphInit {
                list: &values,
                map: &[(1, 0), (2, 1)],
                tail: &[0, 1],
            })
            .unwrap();
        let result = session.initialize_with::<P, _>(
            HostGuard::Released,
            |mut lease| {
                let mut store = SliceStore::new(lease.bytes_mut()?);
                Ok(
                    Projection::<ReplayGraph>::initialize_measured(&mut store, measured)?
                        .commit()?,
                )
            },
            || Ok(HostGuard::Exclusive(cell.borrow_mut())),
        );
        let accepted = value == 1 || !(P::CHECKED || cfg!(feature = "verify-on-load"));
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(session.prefix.is_some(), accepted);
        // This codec violates its true-marker promise. The lazy route trusts
        // that promise; checked routes still reject its invalid bool bytes.
        // Exact typed reads retain their domain checks in every policy.
        if accepted && value == 2 {
            let view = View::load_trusted::<ReplayGraph>(
                session.lease.bytes(),
                ReplayGraph::DISCRIMINATOR,
            )
            .unwrap();
            assert!(ReplayGraph::read(view).unwrap().list().try_get(0).is_err());
        }
        // The native replay is removed; child sizing still runs as needed.
        UNREVIEWED_MEASUREMENTS.with(|calls| assert_eq!(calls.get(), 12));
    }
}

#[test]
fn anchor_options_use_the_shared_type_without_conversion() {
    let options: EditOptions<'_> = super::EditOptions::from(blubber::pinocchio::Funding::Prepaid);
    assert_eq!(options.headroom, blubber::pinocchio::Headroom::UpTo(0));
    assert_eq!(options.funding, blubber::pinocchio::Funding::Prepaid);
    assert!(options.refund.is_none());
}
