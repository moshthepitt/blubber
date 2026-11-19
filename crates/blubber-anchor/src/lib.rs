//! Native Anchor account fields backed by Blubber's typed storage.
//!
//! This unpublished adapter requires a working allocator and the exact SDK git
//! revision in its manifest. The crate README contains extracted, compiled raw
//! and native program recipes. [`Blubber`] provides scoped reads/edits, checked
//! reload after CPI, initialization, exact realloc and close. [`Checked`] selects
//! complete ingress validation. Default edit headroom is `UpTo(0)`.
//!
//! Each load/handoff retains a copied handle slot, never account bytes. Count
//! reopens, failed loads and all other allocations together; see [`HANDLE_SLOT_BYTES`].

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(test)]
extern crate std;

use alloc::boxed::Box;
use blubber::raw::Resize;
use blubber::Resize as _;
use core::cell::Cell;
use core::marker::PhantomData;
use core::ops::Deref;
use solana_account_view::{Ref, RefMut};

use blubber::__private::Projection;
use blubber::{AccountSchema, CommitInfo, View};

mod cpi;
pub mod funding;
mod lifecycle;

/// The exact Anchor SDK used by the native integration.
pub use anchor_lang as sdk;
use sdk::{AccountView, AnchorAccount, Error, Owner};

/// Shared scoped-edit options; see [`blubber::pinocchio::EditOptions`].
pub use blubber::pinocchio::EditOptions;

/// Bytes retained per native load, before allocator bookkeeping and alignment.
///
/// This is 8 on SBF. Count all loads, including failed validation and reopens,
/// and initialization/edit/realloc/CPI handoffs, including failures after preflight,
/// together with Pinocchio OnDemand slots and application allocations. A 32 KiB
/// bump heap has room for at most 4,095 eight-byte slots after its cursor, before
/// other allocations. Allocation failure can abort; Drop releases exclusion but
/// cannot reclaim a leaked slot. No reference with the slot's lifetime escapes.
pub const HANDLE_SLOT_BYTES: usize = core::mem::size_of::<AccountView>();

/// Authenticated lazy ingress, the default native account policy.
#[derive(Clone, Copy, Debug)]
pub struct Lazy;

/// Complete root verification before native account admission.
#[derive(Clone, Copy, Debug)]
pub struct Checked;

mod policy {
    pub trait Sealed {}
    impl Sealed for super::Lazy {}
    impl Sealed for super::Checked {}
}

/// Native account admission policy, sealed to [`Lazy`] and [`Checked`].
pub trait LoadPolicy: policy::Sealed {
    /// Whether admission validates the complete active root.
    const CHECKED: bool;
}

impl LoadPolicy for Lazy {
    const CHECKED: bool = false;
}
impl LoadPolicy for Checked {
    const CHECKED: bool = true;
}

const INLINE_PREFIX_BYTES: usize = 128;

#[cold]
fn identity_error<T: AccountSchema>(bytes: &[u8], error: blubber::Error) -> blubber::Error {
    if matches!(
        error,
        blubber::Error::Fault {
            kind: blubber::ErrorKind::Discriminator,
            ..
        }
    ) {
        let message =
            blubber::__private::pinocchio::identity_mismatch_message(T::DISCRIMINATOR, bytes);
        if let Ok(message) = core::str::from_utf8(&message) {
            sdk::solana_msg::msg!(message);
        }
    }
    error
}

#[inline(always)]
fn admission<T: AccountSchema, P: LoadPolicy>(bytes: &[u8]) -> blubber::Result<View<'_>> {
    if P::CHECKED {
        View::load::<T>(bytes, T::DISCRIMINATOR).map_err(|error| identity_error::<T>(bytes, error))
    } else {
        View::load_trusted::<T>(bytes, T::DISCRIMINATOR)
            .map_err(|error| identity_error::<T>(bytes, error))
    }
}

#[inline(always)]
fn initialized_view<T: AccountSchema, P: LoadPolicy>(bytes: &[u8]) -> blubber::Result<View<'_>> {
    let policy = blubber_core::encoding_publication::init_policy(
        blubber_core::encoding_product::Markers {
            encoder: T::__ENCODER_AUDITED,
            replay: T::__MEASURE_REUSABLE,
        },
        P::CHECKED,
        cfg!(feature = "verify-on-load"),
    );
    if policy.scan {
        View::load::<T>(bytes, T::DISCRIMINATOR).map_err(|error| identity_error::<T>(bytes, error))
    } else {
        View::load_trusted::<T>(bytes, T::DISCRIMINATOR)
            .map_err(|error| identity_error::<T>(bytes, error))
    }
}

// Large copied prefixes get their own SBF frame; small prefixes retain the
// inline scalar path. Copies remain checked and the public cap stays 1024B.
#[inline(always)]
fn copy_prefix<T: AccountSchema>(view: View<'_>) -> blubber::Result<T::Prefix> {
    if core::mem::size_of::<T::Prefix>() > INLINE_PREFIX_BYTES {
        copy_prefix_large::<T>(view)
    } else {
        T::read_prefix(view)
    }
}

#[inline(never)]
fn copy_prefix_large<T: AccountSchema>(view: View<'_>) -> blubber::Result<T::Prefix> {
    T::read_prefix(view)
}

enum Guard {
    Shared(Ref<'static, [u8]>),
    Exclusive(RefMut<'static, [u8]>),
    Released,
}

// Test the same session transitions with safe RefCell guards on hosts/Miri.
// AccountView itself can only be constructed by the runtime under D12.
trait Lease {
    fn bytes(&self) -> &[u8];
    fn bytes_mut(&mut self) -> Result<&mut [u8], Error>;
}

impl Lease for Guard {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Shared(data) => data,
            Self::Exclusive(data) => data,
            Self::Released => panic!("released Blubber account"),
        }
    }

    fn bytes_mut(&mut self) -> Result<&mut [u8], Error> {
        match self {
            Self::Exclusive(data) => Ok(data),
            Self::Shared(_) => Err(sdk::ErrorCode::ConstraintMut.into()),
            Self::Released => Err(blubber::Error::from(blubber::ErrorKind::Poisoned).into()),
        }
    }
}

struct Session<T: AccountSchema, L> {
    lease: L,
    // None is terminal, including unwinding through an edit callback. Keep the
    // guard even then: incomplete bytes must not gain an unchecked typed alias.
    prefix: Option<T::Prefix>,
    new: bool,
}

// Keep terminal sessions excluded even when a host catches callback unwinding.
// Store's temporary guard drops before this cleanup runs. Prefix publication
// stays disabled; a failed cleanup acquisition also leaves the session terminal.
struct Handoff<'a, T: AccountSchema, L: Lease, A: FnOnce() -> Result<L, Error>> {
    session: &'a mut Session<T, L>,
    acquire: Option<A>,
}

impl<T: AccountSchema, L: Lease, A: FnOnce() -> Result<L, Error>> Drop for Handoff<'_, T, L, A> {
    #[inline(always)]
    fn drop(&mut self) {
        if let Some(acquire) = self.acquire.take() {
            if let Ok(lease) = acquire() {
                self.session.lease = lease;
            }
        }
    }
}

impl<T: AccountSchema, L: Lease> Session<T, L> {
    #[inline(always)]
    fn load<P: LoadPolicy>(lease: L) -> Result<Self, Error> {
        let view = admission::<T, P>(lease.bytes())?;
        let prefix = copy_prefix::<T>(view)?;
        Ok(Self {
            lease,
            prefix: Some(prefix),
            new: false,
        })
    }

    // Publish New only after encoding, guard reacquisition and policy admission.
    fn initialize_with<P: LoadPolicy, R>(
        &mut self,
        released: L,
        encode: impl FnOnce(L) -> Result<R, Error>,
        acquire: impl FnOnce() -> Result<L, Error>,
    ) -> Result<R, Error> {
        blubber_core::encoding_publication::withdraw(&mut self.new, &mut self.prefix);
        let lease = core::mem::replace(&mut self.lease, released);
        let mut handoff = Handoff {
            session: self,
            acquire: Some(acquire),
        };
        let result = encode(lease);
        let acquire = handoff
            .acquire
            .take()
            .expect("pending initialization acquisition");
        let session = &mut *handoff.session;
        let acquired = match acquire() {
            Ok(guard) => {
                session.lease = guard;
                Ok(())
            }
            Err(error) => Err(error),
        };
        let receipt = blubber_core::encoding_publication::settle(result, acquired)?;
        let view = initialized_view::<T, P>(session.lease.bytes())?;
        blubber_core::encoding_publication::publish(&mut session.prefix, copy_prefix::<T>(view))?;
        Ok(receipt)
    }

    fn new(lease: L) -> Self {
        Self {
            lease,
            prefix: None,
            new: true,
        }
    }

    // All ordinary input errors precede the guard handoff and byte writes.
    fn check_initialize<'i>(
        &self,
        init: T::Init<'i>,
    ) -> Result<blubber::__private::MeasuredInit<'i, T>, Error> {
        if !self.new {
            self.check_ready()?;
            return Err(blubber::Error::from(blubber::ErrorKind::AlreadyInitialized).into());
        }
        let measured = blubber::__private::MeasuredInit::<T>::new(init)?;
        let used = measured.used()?;
        measured.check_minimum()?;
        if used as usize > self.lease.bytes().len() {
            return Err(blubber::Error::from(blubber::ErrorKind::Capacity).into());
        }
        Ok(measured)
    }

    #[inline(always)]
    fn check_ready(&self) -> Result<(), Error> {
        self.prefix
            .as_ref()
            .map(|_| ())
            .ok_or_else(|| blubber::Error::from(blubber::ErrorKind::Poisoned).into())
    }

    #[inline(always)]
    fn read<R>(&self, read: impl FnOnce(T::Read<'_>) -> blubber::Result<R>) -> Result<R, Error> {
        self.check_ready()?;
        // Authenticated writer history supplies the lazy validity premise. The
        // held guard rules out mutation by other copied handles between reads.
        Ok(read(T::__read_account(self.lease.bytes(), false)?)?)
    }

    // Called only after every close preflight. Invalidate while the exclusive
    // guard is live, then release it without ever publishing another prefix.
    fn invalidate(&mut self, released: L) -> Result<(), Error> {
        self.check_ready()?;
        let data = self.lease.bytes_mut()?;
        // A zero used length rejects every schema, including discriminator
        // overrides. Complementing the discriminator also rejects this type.
        for (closed, live) in data[..8].iter_mut().zip(T::DISCRIMINATOR) {
            *closed = !live;
        }
        data[8..12].fill(0);
        self.prefix = None;
        self.new = false;
        drop(core::mem::replace(&mut self.lease, released));
        Ok(())
    }

    // Allocate the acquisition slot before calling this. Only the prescribed
    // Store open/close operations run without a guard; callbacks run under Store.
    // The boolean carries destructive poison across Store's closed boundary.
    fn handoff<R>(
        &mut self,
        released: L,
        operation: impl FnOnce(L) -> (Result<R, Error>, bool),
        acquire: impl FnOnce() -> Result<L, Error>,
    ) -> Result<R, Error> {
        // Caller checked readiness and exclusive provenance before spending
        // the acquisition slot. No callback can intervene before this move.
        self.prefix = None;
        let lease = core::mem::replace(&mut self.lease, released);
        let mut handoff = Handoff {
            session: self,
            acquire: Some(acquire),
        };
        let (result, poisoned) = operation(lease);
        let acquire = handoff.acquire.take().expect("pending guard acquisition");
        let session = &mut *handoff.session;
        match acquire() {
            Ok(lease) => session.lease = lease,
            Err(error) => return result.and(Err(error)),
        }
        if poisoned {
            return result.and(Err(
                blubber::Error::from(blubber::ErrorKind::Poisoned).into()
            ));
        }
        match View::load_trusted::<T>(session.lease.bytes(), T::DISCRIMINATOR)
            .and_then(copy_prefix::<T>)
        {
            Ok(prefix) => session.prefix = Some(prefix),
            Err(error) => return result.and(Err(error.into())),
        }
        result
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sizing {
    Unset,
    Managed,
    Exact,
}

/// An authenticated native `#[derive(Accounts)]` field with lazy ingress.
///
/// Readonly loads hold shared SDK guards; mutable loads require writable
/// provenance and hold exclusive guards. Fixed constraint fields are detached
/// copies, accessible through Deref. Variable data stays inside [`Self::read`].
/// There is no DerefMut into bytes. Drop releases exclusion; forgetting a wrapper
/// retains it until invocation teardown. Only runtime-supplied views are valid.
///
/// Owner, identity, envelope and copied fixed domains are checked before
/// constraints run. Variable contents require initialization/migration through
/// audited writers and only schema-preserving edits since then. Touched reads
/// remain checked; distant corruption requires [`Self::verify`] or
/// `Blubber<T, Checked>`, which validates the complete root before admission.
/// `verify-on-load` adds complete ingress validation to ordinary loads.
pub struct Blubber<T: AccountSchema + Owner, P: LoadPolicy = Lazy> {
    account: AccountView,
    session: Session<T, Guard>,
    payer: Cell<Option<AccountView>>,
    system_program: Cell<Option<AccountView>>,
    sizing: Sizing,
    policy: PhantomData<P>,
}

impl<T: AccountSchema + Owner, P: LoadPolicy> Blubber<T, P> {
    /// Verifies the whole active root without recovering a terminal session.
    pub fn verify(&self) -> Result<(), Error> {
        self.session.check_ready()?;
        View::load::<T>(self.session.lease.bytes(), T::DISCRIMINATOR)?;
        Ok(())
    }

    #[inline(always)]
    fn load_inner(account: AccountView, mutable: bool) -> Result<Self, Error> {
        if core::mem::size_of::<T::Prefix>() > INLINE_PREFIX_BYTES {
            Self::load_large(account, mutable)
        } else {
            Self::load_inline(account, mutable)
        }
    }

    #[inline(never)]
    fn load_large(account: AccountView, mutable: bool) -> Result<Self, Error> {
        Self::load_inline(account, mutable)
    }

    #[inline(always)]
    fn load_inline(account: AccountView, mutable: bool) -> Result<Self, Error> {
        if !account.owned_by(&T::OWNER) || account.executable() {
            return Err(Error::IllegalOwner);
        }
        if mutable && !account.is_writable() {
            return Err(sdk::ErrorCode::ConstraintMut.into());
        }
        // Reject conflicts before spending heap. The final borrow is still
        // checked; SDK borrow state is shared by every duplicate handle.
        if mutable {
            account.check_borrow_mut()?;
        } else {
            account.check_borrow()?;
        }
        let slot = Box::leak(Box::new(account));
        let guard = if mutable {
            Guard::Exclusive(slot.try_borrow_mut()?)
        } else {
            Guard::Shared(slot.try_borrow()?)
        };
        Ok(Self {
            account,
            session: Session::load::<P>(guard)?,
            payer: Cell::new(None),
            system_program: Cell::new(None),
            sizing: Sizing::Unset,
            policy: PhantomData,
        })
    }

    /// Whether Anchor allocated this account and it still needs initialization.
    ///
    /// Reads, edits, constraint-prefix access and successful exit require a
    /// completed [`Self::initialize`]. A failed destructive initialization is
    /// terminal and returns false here.
    pub fn is_new(&self) -> bool {
        self.session.new
    }

    /// Encodes a borrowed initializer into a newly allocated account once.
    ///
    /// Uses a prepaid UpTo(0) lease, retains the exact opening allocation and
    /// returns its single commit receipt. Measurement/capacity errors preserve
    /// New state. Errors after handoff disable typed access; propagate them for
    /// instruction rollback. Existing accounts return `AlreadyInitialized`.
    /// The SDK guard is reacquired on every returned path before bytes are read.
    /// Lazy initialization skips the post-write scan only for a completely
    /// audited encoder graph. Checked wrappers, `verify-on-load` and unaudited
    /// codecs retain full validation; copied fixed domains are always checked.
    pub fn initialize(&mut self, init: T::Init<'_>) -> Result<CommitInfo, Error> {
        let measured = self.session.check_initialize(init)?;
        // Allocate before releasing exclusion. No application callback runs
        // during the handoff; the only user code is the ordinary schema codec.
        let slot = Box::leak(Box::new(self.account));
        let account = &mut self.account;
        self.session.initialize_with::<P, _>(
            Guard::Released,
            |lease| {
                let Guard::Exclusive(guard) = lease else {
                    return Err(sdk::ErrorCode::ConstraintMut.into());
                };
                let mut store = blubber::pinocchio::Store::__with_guard(
                    account,
                    &T::OWNER,
                    blubber::pinocchio::Funding::Prepaid,
                    blubber::pinocchio::Headroom::UpTo(0),
                    guard,
                )?;
                store.__retain_opening();
                let root = Projection::<T>::initialize_measured(&mut store, measured)?;
                Ok(root.commit()?)
            },
            || slot.try_borrow_mut().map(Guard::Exclusive),
        )
    }

    /// Initializes New accounts; validates readiness without changing existing
    /// state on Anchor's `init_if_needed` reuse branch.
    ///
    /// Returns `Some(receipt)` after creation, `None` on reuse. Reused input is
    /// ignored, including its measurement. Application authority constraints
    /// and Anchor's exact physical-space/rent checks still apply.
    /// Close retains an owned invalid envelope until runtime purge, so a later
    /// `init_if_needed` in the same transaction fails. After purge the PDA can
    /// be created again; bind creation authority in seeds or persistent state
    /// when the application requires authorization across account lifetimes.
    pub fn initialize_if_new(&mut self, init: T::Init<'_>) -> Result<Option<CommitInfo>, Error> {
        if self.is_new() {
            self.initialize(init).map(Some)
        } else {
            self.session.check_ready()?;
            let minimum = blubber::__private::pinocchio::minimum_balance(self.account.data_len())?;
            if self.account.lamports() < minimum {
                return Err(blubber::Error::from(blubber::ErrorKind::RentShortfall).into());
            }
            Ok(None)
        }
    }

    /// Reads under the held SDK guard. The result cannot borrow account bytes.
    #[inline(always)]
    pub fn read<R>(
        &self,
        read: impl FnOnce(T::Read<'_>) -> blubber::Result<R>,
    ) -> Result<R, Error> {
        self.session.read(read)
    }

    /// Edits and commits once, using constraint-bound funding when present.
    ///
    /// Bind both [`funding::PayerConstraint`] and [`funding::SystemProgramConstraint`]
    /// for one net TopUp. Default UpTo(0) uses the opening allocation in either
    /// case; use explicit options to request more room. Without bindings funding
    /// is prepaid. Incomplete bindings return InvalidPayer.
    /// Use [`Self::edit_with`] for explicit headroom, funding or refunds.
    /// After an exact realloc constraint, all edits use UpTo(0) and keep the
    /// exact allocation, including when the callback calls `shrink_to_fit`.
    /// Callback errors skip settlement; propagate them for runtime rollback.
    #[inline(always)]
    pub fn edit<R>(
        &mut self,
        edit: impl FnOnce(&mut T::Mut<'_>) -> blubber::Result<R>,
    ) -> Result<(R, CommitInfo), Error> {
        match (self.payer.get(), self.system_program.get()) {
            (None, None) => self.edit_with(
                EditOptions {
                    funding: blubber::pinocchio::Funding::Prepaid,
                    headroom: blubber::pinocchio::Headroom::UpTo(0),
                    refund: None,
                },
                edit,
            ),
            (Some(payer), Some(system_program)) => self.edit_with(
                blubber::pinocchio::Funding::TopUp {
                    payer: &payer,
                    system_program: &system_program,
                },
                edit,
            ),
            _ => Err(blubber::Error::from(blubber::ErrorKind::InvalidPayer).into()),
        }
    }

    /// Hands exclusion to the safe Pinocchio store, then reacquires it.
    ///
    /// A plain [`blubber::pinocchio::Funding`] selects UpTo(0) headroom. Pass
    /// [`EditOptions`] for UpTo/OnDemand or an authorized refund. The opening
    /// physical length is a reservation floor until the callback explicitly
    /// calls `shrink_to_fit`; `reserve` can raise it. Final trim is best-effort:
    /// rent and the receipt use actual physical length, and `shrink_error` is
    /// diagnostic. Settlement failures return errors without a second commit.
    ///
    /// An exact realloc constraint overrides headroom with UpTo(0) and keeps
    /// its allocation on commit and returned errors. `shrink_to_fit` then
    /// returns false; reservations above that allocation fail before writes.
    ///
    /// Each call retains one 8-byte SBF handle slot, plus OnDemand growth slots.
    /// Ordinary returned errors refresh the prefix after reacquisition; failed
    /// reacquisition, destructive poison and unwinding disable typed access.
    /// The original operation error takes precedence over cleanup errors.
    /// Propagate errors for instruction rollback; catching them can retain
    /// completed edits and Drop's best-effort trim without rent settlement.
    pub fn edit_with<'a, R>(
        &mut self,
        options: impl Into<EditOptions<'a>>,
        edit: impl FnOnce(&mut T::Mut<'_>) -> blubber::Result<R>,
    ) -> Result<(R, CommitInfo), Error> {
        self.session.check_ready()?;
        self.session.lease.bytes_mut()?;
        let mut options = options.into();
        let exact = self.sizing == Sizing::Exact;
        if exact {
            options.headroom = blubber::pinocchio::Headroom::UpTo(0);
        }
        blubber::__private::pinocchio::check_funding(&self.account, options.funding)?;
        if let Some(recipient) = options.refund.as_deref() {
            blubber::__private::pinocchio::check_refund(&self.account, options.funding, recipient)?;
        }
        let slot = Box::leak(Box::new(self.account));
        if !exact {
            self.sizing = Sizing::Managed;
        }
        let account = &mut self.account;
        self.session.handoff(
            Guard::Released,
            |lease| {
                let opened = match (options.headroom, lease) {
                    (
                        blubber::pinocchio::Headroom::UpTo(0)
                        | blubber::pinocchio::Headroom::OnDemand,
                        Guard::Exclusive(guard),
                    ) => blubber::pinocchio::Store::__with_guard(
                        account,
                        &T::OWNER,
                        options.funding,
                        options.headroom,
                        guard,
                    ),
                    (_, lease) => {
                        drop(lease);
                        blubber::pinocchio::Store::new_with_headroom(
                            account,
                            &T::OWNER,
                            options.funding,
                            options.headroom,
                        )
                    }
                };
                let mut store = match opened {
                    Ok(store) => store,
                    Err(error) => return (Err(error.into()), false),
                };
                if let Some(recipient) = options.refund {
                    store = match store.with_refund(recipient) {
                        Ok(store) => store,
                        Err(error) => return (Err(error.into()), false),
                    };
                }
                if exact {
                    store.__retain_exact();
                } else {
                    store.__retain_opening();
                }
                let result = managed_edit::<T, R>(&mut store, edit);
                (result.map_err(Error::from), store.is_poisoned())
            },
            || slot.try_borrow_mut().map(Guard::Exclusive),
        )
    }
}

// Shared with host/Miri controls; only the store supplies bytes and publication.
// This root is constructed here and only reborrowed into the callback. Its path
// stays empty, so root identity needs no second dispatch at completion. The
// checked used length and poison/closed failure behavior match Edit::commit.
#[inline(never)]
fn managed_edit<T: AccountSchema, R>(
    store: &mut impl Resize,
    edit: impl FnOnce(&mut T::Mut<'_>) -> blubber::Result<R>,
) -> blubber::Result<(R, CommitInfo)> {
    let (value, used) = {
        let mut root = Projection::<T>::load_trusted(store)?;
        let value = edit(&mut T::project(root.reborrow()))?;
        (value, root.used_len())
    };
    let used = match used {
        Ok(used) => used,
        Err(error) => {
            if !store.is_closed() {
                store.set_poisoned(true);
                let _ = store.commit(0);
            }
            return Err(error);
        }
    };
    Ok((value, store.commit(used)?))
}

impl<T: AccountSchema + Owner, P: LoadPolicy> Deref for Blubber<T, P> {
    type Target = T::Prefix;

    /// Returns copied constraint fields; panics after terminal invalidation.
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.session
            .prefix
            .as_ref()
            .expect("unusable Blubber account")
    }
}

impl<T: AccountSchema + Owner, P: LoadPolicy> AnchorAccount for Blubber<T, P> {
    type Data = T::Prefix;
    const MIN_DATA_LEN: usize = T::MIN_ACCOUNT_LEN as usize;
    // Genuine SDK guards must never enter Anchor's raw-marker relaxation path.
    const RELAX_READONLY_CPI_BORROW_FROM_MUT: bool = false;

    #[inline(always)]
    fn load(view: AccountView) -> Result<Self, Error> {
        Self::load_inner(view, false)
    }
    #[inline(always)]
    fn load_mut(view: AccountView) -> Result<Self, Error> {
        Self::load_inner(view, true)
    }
    fn account(&self) -> &AccountView {
        &self.account
    }
    fn exit(&mut self) -> Result<(), Error> {
        self.session.check_ready()
    }
}

// Format 1 is opaque to the stock account decoder; instruction metadata stays.
impl<T: AccountSchema + Owner, P: LoadPolicy> sdk::IdlAccountType for Blubber<T, P> {}

impl<T: AccountSchema + Owner, P: LoadPolicy> sdk::AccountInitialize for Blubber<T, P> {
    type Params<'a> = ();

    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        owner: &sdk::Address,
        _params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
        payer_signer_seeds: Option<&[&[u8]]>,
    ) -> Result<Self, Error> {
        if owner != &T::OWNER {
            return Err(Error::IllegalOwner);
        }
        if payer_signer_seeds.is_some() {
            return Err(Error::InvalidSeeds);
        }
        let space = blubber_core::checked_u32(space)?;
        if space < T::MIN_ACCOUNT_LEN {
            return Err(blubber::Error::from(blubber::ErrorKind::Capacity).into());
        }
        blubber::__private::pinocchio::preflight_accounts(account, payer, space, true)?;
        let seeds = if let Some(seeds) = signer_seeds {
            Some(blubber::__private::pinocchio::signer_seeds(
                account, owner, seeds,
            )?)
        } else if !account.is_signer() {
            return Err(Error::MissingRequiredSignature);
        } else {
            None
        };
        let deficit = blubber::__private::pinocchio::funding_amount(account, payer, space)?;
        // Spend the handle slot before the allocation CPI. Retain a genuine
        // exclusive guard through New state; no discriminator is stamped here.
        let slot = Box::leak(Box::new(*account));
        // Anchor's helper uses the SDK rate-only Rent view. Use its same safe
        // allocation instruction with the full-sysvar deficit from preflight.
        let create = pinocchio_system::instructions::CreateAccountAllowPrefund {
            to: account,
            space: u64::from(space),
            owner,
            funding: (deficit != 0).then_some(pinocchio_system::instructions::Funding {
                from: payer,
                lamports: deficit,
            }),
        };
        if let Some(seeds) = seeds.as_ref() {
            let signer = ::pinocchio::cpi::Signer::from(
                &seeds[..signer_seeds.expect("checked target seeds").len()],
            );
            create.invoke_signed(&[signer])?;
        } else {
            create.invoke()?;
        }
        Ok(Self {
            account: *account,
            session: Session::new(Guard::Exclusive(slot.try_borrow_mut()?)),
            payer: Cell::new(None),
            system_program: Cell::new(None),
            sizing: Sizing::Unset,
            policy: PhantomData,
        })
    }
}

#[cfg(all(test, feature = "derive"))]
mod tests;
