//! Account guards and bounded headroom leases using safe SDK resize calls.

//!
//! Accounts must come from the Pinocchio runtime entrypoint. Complete a mutable
//! lease before CPI. Explicit commit trims storage and settles rent when needed.
//!
//! Generated account schemas expose `with_edit_options` for a scoped lease.
//! The default `Headroom::UpTo(0)` uses existing spare without speculative growth.
//! Request the positive encoded growth beyond opening spare for known edits
//! (including directory/ancestor bytes), or select [`Headroom::Max`].
//! The callback commits once on success; errors retain completed edits and Drop
//! attempts trimming without rent settlement. Propagate errors for rollback.
//!
//! ```
//! use blubber::{schema, List, Str};
//! use blubber::pinocchio::{EditOptions, Funding, Headroom};
//! use blubber::pinocchio_sdk::{AccountView, Address};
//! #[schema(account)]
//! struct Messages { items: List<Str>, note: Str }
//! fn append_checked(account: &mut AccountView, owner: &Address) -> blubber::Result<()> {
//!     // A nonempty string list needs 3 payload bytes plus a 4-byte end word.
//!     // Existing parent end words are patched; the trailing note moves.
//!     // With 4 bytes of opening spare, reserve 7 - 4 = 3 additional bytes.
//!     Messages::with_edit_checked_options(account, owner, EditOptions {
//!         funding: Funding::Prepaid, headroom: Headroom::UpTo(3), refund: None,
//!     }, |messages| messages.items_mut().push("new"))?;
//!     Ok(())
//! }
//! fn append_in_spare(account: &mut AccountView, owner: &Address) -> blubber::Result<()> {
//!     // The caller knows at least 7 spare bytes are already allocated.
//!     Messages::with_edit_checked_options(account, owner, EditOptions {
//!         funding: Funding::Prepaid, headroom: Headroom::UpTo(0), refund: None,
//!     }, |messages| messages.items_mut().push("new"))?;
//!     Ok(())
//! }
//! ```
//!
//! Rent uses the SDK's checked SIMD-0194 quote, including 128 bytes of account
//! overhead. An out-of-range length or rate returns
//! [`ProgramError::InvalidArgument`].

use crate::storage::Backend as Resize;
use crate::Resize as _;
use ::pinocchio::{
    account::{Ref, RefMut, MAX_PERMITTED_DATA_INCREASE},
    address::address_eq,
    sysvars::{rent::Rent, Sysvar},
};
use ::pinocchio::{error::ProgramError, Resize as SdkResize};
use ::pinocchio::{AccountView, Address};
use core::{cell::Cell, marker::PhantomData};

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

use crate::{storage::private, CommitInfo, ErrorKind, ReadStorage, Result};

pub(crate) mod creation;

/// Funding policy for completing an account session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Funding<'a> {
    /// The caller supplies the account's rent balance.
    #[default]
    Prepaid,
    /// Cover the final net rent deficit with at most one System transfer.
    TopUp {
        /// Writable signer, owned by System, with empty data.
        payer: &'a AccountView,
        /// The executable System Program account.
        system_program: &'a AccountView,
    },
}

/// When to acquire additional physical storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Headroom {
    /// Probe through current length plus 10,240, bounded by the 10 MiB cap.
    /// Opening can reserve bytes that the edit does not need. With an exhausted
    /// transaction growth budget, that reservation can return `InvalidRealloc`
    /// even for an equal-width edit. `UpTo(0)` avoids speculative reservation.
    /// SDK-local headroom does not certify transaction or caller CPI capacity.
    Max,
    /// Probe at most this many bytes beyond the current account length.
    /// May grant less; zero (the default) performs no speculative resize or allocation.
    UpTo(usize),
    /// Grow only when an edit needs room, using a working global allocator.
    ///
    /// Each growth attempt leaks an 8-byte handle slot on SBF, including failed
    /// requests. A 32 KiB heap holds at most 4,095 slots before other allocations.
    /// Bound total growth events across accounts and reopens. `no_allocator!`
    /// can abort on growth; compiler allocation elision is not a guarantee.
    #[cfg(feature = "alloc")]
    OnDemand,
}

impl Default for Headroom {
    fn default() -> Self {
        Self::UpTo(0)
    }
}

/// Funding, headroom and an optional authorized rent-refund recipient.
///
/// Default opening never grows speculatively: `Headroom::UpTo(0)` uses existing
/// spare. To request more physical room for known edits, sum
/// their positive growth, including directory and ancestor growth. Subtract
/// existing opening spare bytes when choosing the additional physical room. A lease may grant less than requested;
/// edits exceeding its capacity fail before writing. Keep [`Headroom::Max`]
/// for unknown sequences. This type requires neither `alloc` nor a runtime adapter.
///
/// ```
/// use blubber::pinocchio::{EditOptions, Funding, Headroom};
/// let options = EditOptions {
///     funding: Funding::Prepaid,
///     headroom: Headroom::UpTo(8 + 8), // Two u64 list pushes.
///     refund: None,
/// };
/// assert_eq!(options.headroom, Headroom::UpTo(16));
/// let default_options = EditOptions::from(Funding::Prepaid);
/// assert_eq!(default_options.headroom, Headroom::UpTo(0));
/// ```
pub struct EditOptions<'a> {
    /// Covers the final net rent deficit, with at most one transfer.
    pub funding: Funding<'a>,
    /// Opening lease policy. Exact realloc constraints select UpTo(0).
    pub headroom: Headroom,
    /// Receives excess rent after trim; authorize it in the application.
    /// It must be distinct from the state, payer and System Program accounts.
    pub refund: Option<&'a mut AccountView>,
}

impl<'a> From<Funding<'a>> for EditOptions<'a> {
    fn from(funding: Funding<'a>) -> Self {
        Self {
            funding,
            headroom: Headroom::default(),
            refund: None,
        }
    }
}

/// An authenticated account shared borrow. Opening never changes its length.
pub struct ReadStore<'a> {
    guard: Ref<'a, [u8]>,
}

impl<'a> ReadStore<'a> {
    /// Authenticates owner and non-executable state, then takes a shared guard.
    #[inline(always)]
    pub fn new(account: &'a AccountView, owner: &Address) -> Result<Self> {
        authenticate(account, owner)?;
        let guard = account
            .try_borrow()
            .map_err(|_| ErrorKind::BorrowConflict)?;
        Ok(Self { guard })
    }

    /// Opens a typed account under authenticated owner and shared exclusion.
    ///
    /// Checks schema identity and envelope; contents are checked when touched.
    /// The program must initialize or migrate this schema with checked contents,
    /// then use only approved writers. Loading does not certify global map order
    /// or disjoint directories. Use [`Self::load_checked`] for untrusted contents.
    /// With `verify-on-load`, also scans the complete root before returning.
    #[inline(always)]
    pub fn load<T: crate::AccountSchema>(&self) -> Result<T::Read<'_>> {
        T::__read_account(self.bytes(), false)
    }

    /// Verifies the complete account root under this authenticated shared guard.
    #[inline(always)]
    pub fn load_checked<T: crate::AccountSchema>(&self) -> Result<T::Read<'_>> {
        T::__read_account(self.bytes(), true)
    }

    /// Borrows the account bytes for at most this store's lifetime.
    #[inline(always)]
    pub fn bytes(&self) -> &[u8] {
        &self.guard
    }
}

impl private::Sealed for ReadStore<'_> {}

impl ReadStorage for ReadStore<'_> {
    fn is_closed(&self) -> bool {
        false
    }

    fn bytes(&self) -> &[u8] {
        self.bytes()
    }
}

/// An exclusive account lease using safe SDK resize and borrow operations.
///
/// Max and UpTo discover SDK-local session capacity at open. Failed probes
/// perform no data writes but consume CU. Shrink/reopen discovery conservatively
/// reaches current + 10,240; reopens and earlier CPI never reset the SDK allowance.
/// This capacity cannot predict the 20 MiB shared transaction growth budget,
/// ancestor CPI ceilings, or SIMD-0460 eager backing allocation faults. Runtime
/// faults are not returned SDK errors and cannot be caught by probing.
///
/// OnDemand (with `alloc`) keeps the opening guard and re-leases on growth before
/// any edit write. References always borrow this store, never a leaked slot.
/// Complete the session before CPI. Commit releases the guard before trimming;
/// Drop attempts the envelope's used length without settling rent. Forgetting
/// retains exclusion until invocation teardown. AccountView's valid runtime
/// memory contract remains SDK trust; leaked handles do not extend that memory.
///
/// Explicit commit closes the lease and trims before settling rent. If final
/// storage equals its opening length, the balance is unchanged and no refund is
/// attached, it skips the Rent sysvar and settlement. Shrinks settle against the
/// final length so legacy rent-paying accounts can become rent-exempt. This does not replace the
/// runtime's transaction rent checks. Otherwise Prepaid rejects a deficit and
/// TopUp funds it once. Growing edits and reservations preflight prospective
/// affordability before changing bytes or poison; equal-size and shrinking
/// edits perform no rent lookup. Only immutable rent parameters are cached;
/// each growth checks current balances. An attached refund receives only excess final rent. Propagate commit errors on-chain to roll
/// back edits and CPIs; catching an error retains completed work. Drop never
/// funds or refunds.
pub struct Store<'a> {
    guard: Option<RefMut<'a, [u8]>>,
    caller: PhantomData<&'a mut AccountView>,
    #[cfg(feature = "alloc")]
    headroom: Headroom,
    storage_len: usize,
    opening_len: usize,
    opening_lamports: u64,
    growth_limit: usize,
    growth_limited: bool,
    poisoned: bool,
    rent_check: RentCheck<'a>,
    refund: Option<&'a mut AccountView>,
    owner: Address,
}

// Direct loans carry this check too: discovered headroom is not prepaid rent.
// Reuse the session's rent sysvar and check live balances on every growth.
pub(crate) struct RentCheck<'a> {
    account: AccountView,
    funding: Funding<'a>,
    rent: Cell<Option<Rent>>,
    retained: Cell<usize>,
}

impl RentCheck<'_> {
    #[inline(always)]
    pub(crate) fn check(&self, required: usize) -> Result<()> {
        if required == 0 {
            return Ok(());
        }
        match self.check_growth(required) {
            Ok(()) => Ok(()),
            Err(ProgramError::AccountNotRentExempt) => Err(ErrorKind::RentShortfall.into()),
            Err(error) => Err(error.into()),
        }
    }

    #[inline(always)]
    fn check_growth(&self, required: usize) -> core::result::Result<(), ProgramError> {
        let minimum = self.minimum(required.max(self.retained.get()))?;
        let balance = self.account.lamports();
        if balance >= minimum {
            return Ok(());
        }
        if let Funding::TopUp { payer, .. } = self.funding {
            if payer.lamports() >= minimum - balance {
                return Ok(());
            }
        }
        Err(ProgramError::AccountNotRentExempt)
    }

    #[inline(always)]
    fn minimum(&self, len: usize) -> core::result::Result<u64, ProgramError> {
        let rent = match self.rent.get() {
            Some(rent) => rent,
            None => {
                let rent = Rent::get()?;
                self.rent.set(Some(rent));
                rent
            }
        };
        rent.try_minimum_balance(len)
    }
}

impl<'a> Store<'a> {
    /// Opens with [`Headroom::UpTo(0)`](Headroom::UpTo), using only the existing allocation.
    #[inline(always)]
    pub fn new(
        account: &'a mut AccountView,
        owner: &Address,
        funding: Funding<'a>,
    ) -> Result<Self> {
        Self::new_with_headroom(account, owner, funding, Headroom::default())
    }

    /// Opens a typed editor under authenticated owner and exclusive exclusion.
    ///
    /// Checks schema identity and envelope; contents are checked when touched.
    /// The program must initialize or migrate this schema with checked contents,
    /// then use only approved writers. Loading does not certify global map order
    /// or disjoint directories. Use [`Self::load_checked`] for untrusted contents.
    /// With `verify-on-load`, also scans the complete root before returning.
    /// Poison remains terminal until explicit checked recovery.
    #[inline(always)]
    pub fn load<T: crate::AccountSchema>(&mut self) -> Result<T::Mut<'_>> {
        crate::projection::Projection::<T>::load(self).map(T::project)
    }

    /// Verifies the complete root and returns an exclusive schema-bound editor.
    /// Valid poison is recovered only after a successful full scan.
    #[inline(always)]
    pub fn load_checked<T: crate::AccountSchema>(&mut self) -> Result<T::Mut<'_>> {
        crate::projection::Projection::<T>::load_checked(self).map(T::project)
    }

    /// Selects the mode before opening; UpTo may grant less than requested.
    /// UpTo(0) performs no speculative growth and requires no allocation.
    ///
    /// `growth_limit()` is the largest length accepted by SDK resize within the
    /// probe range for Max/UpTo, equal to their guard length. OnDemand reports
    /// currently leased capacity, which may increase after fallible growth.
    /// Checked length arithmetic fails before any resize. Other SDK resize
    /// failures propagate unchanged as program errors.
    #[inline(always)]
    pub fn new_with_headroom(
        account: &'a mut AccountView,
        owner: &Address,
        funding: Funding<'a>,
        headroom: Headroom,
    ) -> Result<Self> {
        authenticate(account, owner)?;
        if !account.is_writable() {
            return Err(ErrorKind::NotWritable.into());
        }
        account
            .check_borrow_mut()
            .map_err(|_| ErrorKind::BorrowConflict)?;
        check_funding(account, funding)?;
        let current = account.data_len();
        let opening_lamports = account.lamports();
        let (discovered, growth_limit, growth_limited) = match headroom {
            #[cfg(feature = "alloc")]
            Headroom::OnDemand => (current, current, true),
            _ => {
                let (target, cap) = probe_target(current, headroom)?;
                let discovered = session_limit(current, target, |len| account.resize(len))?;
                (
                    discovered,
                    discovered,
                    headroom == Headroom::Max || discovered < target || target == cap,
                )
            }
        };
        let mut handle = *account;
        let guard = match account.try_borrow_mut() {
            Ok(guard) => guard,
            Err(_) => {
                if handle.data_len() != current {
                    let _ = handle.resize(current);
                }
                return Err(ErrorKind::BorrowConflict.into());
            }
        };
        Ok(Self {
            guard: Some(guard),
            caller: PhantomData,
            #[cfg(feature = "alloc")]
            headroom,
            storage_len: discovered,
            opening_len: current,
            opening_lamports,
            growth_limit,
            growth_limited,
            poisoned: false,
            rent_check: RentCheck {
                account: handle,
                funding,
                rent: Cell::new(None),
                retained: Cell::new(0),
            },
            refund: None,
            owner: *owner,
        })
    }

    /// Sends excess final rent lamports to a program-authorized recipient.
    ///
    /// The caller must authorize this recipient before attaching it. It must be
    /// writable, non-executable, unborrowed and distinct from state and payer.
    /// Checks run now and again at commit; failure consumes and drops the lease.
    pub fn with_refund(mut self, recipient: &'a mut AccountView) -> Result<Self> {
        self.check_open()?;
        check_refund(&self.rent_check.account, self.rent_check.funding, recipient)?;
        self.refund = Some(recipient);
        Ok(self)
    }

    #[inline(always)]
    fn settle(&mut self) -> Result<(u64, u64)> {
        if matches!(self.rent_check.funding, Funding::Prepaid) && self.refund.is_none() {
            self.settle_prepaid()?;
            Ok((0, 0))
        } else {
            self.settle_with_parties()
        }
    }

    #[inline(always)]
    fn settle_prepaid(&self) -> Result<()> {
        authenticate(&self.rent_check.account, &self.owner)?;
        if self.rent_check.account.lamports() < self.rent_check.minimum(self.storage_len)? {
            return Err(ErrorKind::RentShortfall.into());
        }
        Ok(())
    }

    #[inline(never)]
    fn settle_with_parties(&mut self) -> Result<(u64, u64)> {
        // Admission checked the addresses, which stay fixed during the lease.
        // Recheck live account state and borrows before moving lamports.
        authenticate(&self.rent_check.account, &self.owner)?;
        if let Funding::TopUp {
            payer,
            system_program,
        } = self.rent_check.funding
        {
            if !system_program.executable() {
                return Err(ErrorKind::InvalidPayer.into());
            }
            check_payer_access(payer)?;
        }
        if let Some(recipient) = self.refund.as_deref() {
            check_refund_access(recipient)?;
        }
        let minimum = self.rent_check.minimum(self.storage_len)?;
        settle_minimum(
            &mut self.rent_check.account,
            match self.rent_check.funding {
                Funding::Prepaid => None,
                Funding::TopUp { payer, .. } => Some(payer),
            },
            self.refund.as_deref_mut(),
            minimum,
        )
    }

    #[cfg(feature = "alloc")]
    fn grow(&mut self, needed: usize) -> Result<()> {
        let target = growth_target(self.storage_len, needed);
        // Allocate while the old guard still excludes every duplicate handle.
        let slot: &'static mut AccountView = Box::leak(Box::new(self.rent_check.account));
        let result = re_lease(
            &mut self.guard,
            &mut self.poisoned,
            || resize_with_retry(needed, target, |len| self.rent_check.account.resize(len)),
            || slot.try_borrow_mut(),
        );
        self.storage_len = self.rent_check.account.data_len();
        self.growth_limit = self.storage_len;
        match result {
            Ok(()) => Ok(()),
            Err(ReLeaseError::Resize(ProgramError::InvalidRealloc)) => {
                Err(ErrorKind::GrowthLimit.into())
            }
            Err(ReLeaseError::Resize(error)) => Err(error.into()),
            Err(ReLeaseError::Reacquire(error)) => {
                // Terminal: no guard, so no typed access or poison recovery.
                debug_assert!(self.poisoned);
                Err(error.into())
            }
        }
    }

    fn check_open(&self) -> Result<()> {
        if self.guard.is_none() {
            Err(ErrorKind::Closed.into())
        } else if self.poisoned {
            Err(ErrorKind::Poisoned.into())
        } else {
            Ok(())
        }
    }

    #[inline(always)]
    fn check_capacity(&self, new_len: usize) -> Result<()> {
        self.check_open()?;
        if new_len > MAX_STORAGE {
            return Err(ErrorKind::Capacity.into());
        }
        #[cfg(feature = "alloc")]
        if self.headroom == Headroom::OnDemand {
            return Ok(());
        }
        if new_len > self.growth_limit {
            return Err(if self.growth_limited {
                ErrorKind::GrowthLimit
            } else {
                ErrorKind::Capacity
            }
            .into());
        }
        Ok(())
    }

    fn check_growth_funding(&self, required: usize) -> Result<()> {
        let used = self
            .guard
            .as_deref()
            .and_then(|bytes| bytes.get(8..12))
            .map(|word| u32::from_le_bytes(word.try_into().expect("used word")) as usize)
            .unwrap_or(0);
        if required > used {
            self.rent_check.check(required)?;
        }
        Ok(())
    }

    fn check_used(&self, used_len: u32) -> Result<usize> {
        let used = used_len as usize;
        if used > self.storage_len {
            Err(ErrorKind::UsedLength.into())
        } else {
            Ok(used)
        }
    }
}

impl private::Sealed for Store<'_> {}

impl ReadStorage for Store<'_> {
    fn is_closed(&self) -> bool {
        self.guard.is_none()
    }

    fn bytes(&self) -> &[u8] {
        self.guard.as_deref().expect("Blubber storage is closed")
    }
}

impl private::Write for Store<'_> {
    fn check_initialize_resize(&self, needed: usize) -> Result<()> {
        self.check_capacity(needed)?;
        self.rent_check.check(needed)
    }

    #[inline(always)]
    fn authenticated(&self) -> bool {
        true
    }

    fn exact_access(&mut self) -> private::FixedAccess<'_> {
        private::FixedAccess {
            bytes: self
                .guard
                .as_deref_mut()
                .expect("Blubber storage is closed"),
            poisoned: &mut self.poisoned,
        }
    }

    fn fixed_access(&mut self) -> Result<private::FixedAccess<'_>> {
        self.check_open()?;
        Ok(private::FixedAccess {
            bytes: self.guard.as_deref_mut().expect("checked open guard"),
            poisoned: &mut self.poisoned,
        })
    }
    fn edit_access(&mut self) -> private::EditAccess<'_> {
        let limit = self.growth_limit.min(MAX_STORAGE);
        #[cfg(feature = "alloc")]
        let limit = if self.headroom == Headroom::OnDemand {
            MAX_STORAGE
        } else {
            limit
        };
        private::EditAccess {
            preflight: private::Preflight {
                rent: Some(&self.rent_check),
                #[cfg(test)]
                probe: None,
            },
            bytes: self
                .guard
                .as_deref_mut()
                .expect("Blubber storage is closed"),
            poisoned: &mut self.poisoned,
            limit,
            hard_limit: MAX_STORAGE,
            limit_error: if self.growth_limited && limit < MAX_STORAGE {
                ErrorKind::GrowthLimit
            } else {
                ErrorKind::Capacity
            },
        }
    }
}

impl crate::Resize for Store<'_> {
    fn storage_len(&self) -> usize {
        self.storage_len
    }

    fn growth_limit(&self) -> usize {
        self.growth_limit
    }

    fn check_resize(&self, new_len: usize) -> Result<()> {
        self.check_capacity(new_len)?;
        self.check_growth_funding(new_len)
    }

    fn is_poisoned(&self) -> bool {
        self.poisoned
    }
    fn __backend(&mut self, _: crate::storage::Writer) -> &mut dyn crate::storage::Backend {
        self
    }
}

impl Resize for Store<'_> {
    fn bytes_mut(&mut self) -> &mut [u8] {
        self.guard
            .as_deref_mut()
            .expect("Blubber storage is closed")
    }

    fn resize(&mut self, new_len: usize) -> Result<()> {
        self.check_resize(new_len)?;
        #[cfg(feature = "alloc")]
        if self.headroom == Headroom::OnDemand && new_len > self.storage_len {
            self.grow(new_len)?;
        }
        // Logical shrink retains the guard and physical storage until commit.
        Ok(())
    }

    fn set_poisoned(&mut self, poisoned: bool) {
        assert!(!self.is_closed(), "Blubber storage is closed");
        self.poisoned = poisoned;
    }

    #[inline(always)]
    fn commit(&mut self, used_len: u32) -> Result<CommitInfo> {
        // Every first attempt closes, including invalid or poisoned completion.
        let guard = self.guard.take().ok_or(ErrorKind::Closed)?;
        drop(guard);
        if self.poisoned {
            return Err(ErrorKind::Poisoned.into());
        }
        let used = self.check_used(used_len)?;
        let final_len = used.max(self.rent_check.retained.get());
        let (storage_len, shrink_result) = trim(self.storage_len, final_len, |len| {
            let result = self.rent_check.account.resize(len);
            (self.rent_check.account.data_len(), result)
        });
        self.storage_len = storage_len;
        let (topped_up, refunded) = if settlement_unchanged(
            self.opening_len,
            self.opening_lamports,
            self.storage_len,
            self.rent_check.account.lamports(),
            self.refund.is_some(),
        ) {
            (0, 0)
        } else {
            self.settle()?
        };

        Ok(CommitInfo {
            used_len,
            storage_len: self.storage_len as u32,
            topped_up,
            refunded,
            shrink_error: shrink_result.err().map(Into::into),
        })
    }

    fn reserve(&mut self, used_len: u32, additional: usize) -> Result<()> {
        self.check_open()?;
        let required = self
            .check_used(used_len)?
            .checked_add(additional)
            .ok_or(ErrorKind::ArithmeticOverflow)?;
        blubber_core::checked_u32(required)?;
        self.resize(required)?;
        self.rent_check
            .retained
            .set(self.rent_check.retained.get().max(required));
        Ok(())
    }

    fn shrink_to_fit(&mut self, used_len: u32) -> Result<bool> {
        self.check_open()?;
        self.check_used(used_len)?;
        self.rent_check.retained.set(0);
        Ok(true)
    }
}

impl Drop for Store<'_> {
    fn drop(&mut self) {
        let used = self.guard.as_deref().and_then(|bytes| {
            let word: [u8; 4] = bytes.get(8..12)?.try_into().ok()?;
            let used = u32::from_le_bytes(word) as usize;
            (used >= 12 && used <= bytes.len()).then_some(used)
        });
        drop(self.guard.take());
        if let Some(used) = used.filter(|used| *used != self.storage_len) {
            let _ = self.rent_check.account.resize(used);
        }
    }
}

#[inline(always)]
fn authenticate(account: &AccountView, owner: &Address) -> Result<()> {
    if !account.owned_by(owner) {
        return Err(ErrorKind::WrongOwner.into());
    }
    if account.executable() {
        return Err(ErrorKind::Unauthorized.into());
    }
    if account.data_len() > MAX_STORAGE {
        return Err(ErrorKind::Capacity.into());
    }
    Ok(())
}

#[inline(always)]
fn settle_minimum(
    account: &mut AccountView,
    payer: Option<&AccountView>,
    refund: Option<&mut AccountView>,
    minimum: u64,
) -> Result<(u64, u64)> {
    let balance = account.lamports();
    if balance < minimum {
        let deficit = minimum - balance;
        let Some(payer) = payer else {
            return Err(ErrorKind::RentShortfall.into());
        };
        if payer.lamports() < deficit {
            return Err(ErrorKind::RentShortfall.into());
        }
        pinocchio_system::instructions::Transfer {
            from: payer,
            to: account,
            lamports: deficit,
        }
        .invoke()?;
        return Ok((deficit, 0));
    }
    if let Some(recipient) = refund.filter(|_| balance > minimum) {
        let refunded = balance - minimum;
        let (state_balance, recipient_balance) =
            refund_balances(balance, recipient.lamports(), refunded)?;
        account.set_lamports(state_balance);
        recipient.set_lamports(recipient_balance);
        return Ok((0, refunded));
    }
    Ok((0, 0))
}

#[inline(always)]
fn refund_balances(state: u64, recipient: u64, amount: u64) -> Result<(u64, u64)> {
    let state = state
        .checked_sub(amount)
        .ok_or(ErrorKind::ArithmeticOverflow)?;
    let recipient = recipient
        .checked_add(amount)
        .ok_or(ErrorKind::ArithmeticOverflow)?;
    Ok((state, recipient))
}

// Compare actual post-trim storage with the snapshot taken before headroom
// discovery. A failed trim or retained reserve can still require settlement.
fn settlement_unchanged(
    opening_len: usize,
    opening_lamports: u64,
    final_len: usize,
    final_lamports: u64,
    has_refund: bool,
) -> bool {
    final_len == opening_len && final_lamports == opening_lamports && !has_refund
}

// SDK failure and final physical length stay coupled, including failed trims.
fn trim(
    current: usize,
    target: usize,
    resize: impl FnOnce(usize) -> (usize, core::result::Result<(), ProgramError>),
) -> (usize, core::result::Result<(), ProgramError>) {
    if target == current {
        return (current, Ok(()));
    }
    resize(target)
}

fn probe_target(current: usize, headroom: Headroom) -> Result<(usize, usize)> {
    if current > MAX_STORAGE {
        return Err(ErrorKind::Capacity.into());
    }
    let cap = current
        .checked_add(MAX_PERMITTED_DATA_INCREASE)
        .ok_or(ErrorKind::ArithmeticOverflow)?
        .min(MAX_STORAGE);
    let target = match headroom {
        Headroom::Max => cap,
        #[cfg(feature = "alloc")]
        Headroom::OnDemand => current,
        Headroom::UpTo(n) => current
            .checked_add(n)
            .ok_or(ErrorKind::ArithmeticOverflow)?
            .min(cap),
    };
    Ok((target, cap))
}

// Private limit-provider boundary. A future safe SDK max_resize_len() can
// replace probing here without changing constructors or the user-facing modes.
fn session_limit(
    current: usize,
    target: usize,
    mut resize: impl FnMut(usize) -> core::result::Result<(), ProgramError>,
) -> Result<usize> {
    let mut physical = current;
    let result = discover(current, target, |len| {
        resize(len)?;
        physical = len;
        Ok(())
    });
    if result.is_err() && physical != current {
        // Some earlier probes may have grown successfully. Preserve the error
        // even if restoration fails; this is a best-effort physical rollback.
        let _ = resize(current);
    }
    result
}

#[cfg(feature = "alloc")]
fn growth_target(current: usize, needed: usize) -> usize {
    let slack = (needed - current).max((current / 4).min(2048));
    needed.saturating_add(slack).min(MAX_STORAGE)
}

#[cfg(feature = "alloc")]
fn resize_with_retry(
    needed: usize,
    target: usize,
    mut resize: impl FnMut(usize) -> core::result::Result<(), ProgramError>,
) -> core::result::Result<(), ProgramError> {
    match resize(target) {
        Err(ProgramError::InvalidRealloc) if target != needed => resize(needed),
        result => result,
    }
}

#[cfg(feature = "alloc")]
enum ReLeaseError {
    Resize(ProgramError),
    Reacquire(ProgramError),
}

// Only private SDK operations run in this gap: no user callback or CPI.
#[cfg(feature = "alloc")]
fn re_lease<G>(
    guard: &mut Option<G>,
    poisoned: &mut bool,
    resize: impl FnOnce() -> core::result::Result<(), ProgramError>,
    acquire: impl FnOnce() -> core::result::Result<G, ProgramError>,
) -> core::result::Result<(), ReLeaseError> {
    drop(guard.take());
    let resized = resize();
    match acquire() {
        Ok(new_guard) => *guard = Some(new_guard),
        Err(error) => {
            *poisoned = true;
            return Err(ReLeaseError::Reacquire(error));
        }
    }
    resized.map_err(ReLeaseError::Resize)
}

// Pinocchio 0.12.0/account-view 3.0.0: InvalidRealloc is returned before any
// mutation and accepted lengths form an interval. Other errors propagate. Keep
// the SBF canary across upgrades; runtime memory faults are not returned errors.
// Keep the accepted lower bound and rejected upper bound; every success grows.
fn discover(
    current: usize,
    target: usize,
    mut resize: impl FnMut(usize) -> core::result::Result<(), ProgramError>,
) -> Result<usize> {
    if target == current {
        return Ok(current);
    }
    match resize(target) {
        Ok(()) => return Ok(target),
        Err(ProgramError::InvalidRealloc) => (),
        Err(error) => return Err(error.into()),
    }
    let (mut low, mut high) = (current, target);
    while high - low > 1 {
        let mid = low + (high - low) / 2;
        match resize(mid) {
            Ok(()) => low = mid,
            Err(ProgramError::InvalidRealloc) => high = mid,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(low)
}

#[cfg(test)]
mod tests;

const MAX_STORAGE: usize = 10_485_760;

/// Quotes rent for a prospective account length under SIMD-0194.
#[inline(always)]
fn minimum_balance(len: usize) -> Result<u64> {
    Ok(Rent::get()?.try_minimum_balance(len)?)
}

#[inline(always)]
fn check_funding(state: &AccountView, funding: Funding<'_>) -> Result<()> {
    if let Funding::TopUp {
        payer,
        system_program,
    } = funding
    {
        if address_eq(state.address(), payer.address())
            || address_eq(state.address(), system_program.address())
            || address_eq(payer.address(), system_program.address())
        {
            return Err(ErrorKind::DuplicateAccount.into());
        }
        if !address_eq(system_program.address(), &pinocchio_system::ID)
            || !system_program.executable()
        {
            return Err(ErrorKind::InvalidPayer.into());
        }
        check_payer_access(payer)?;
    }
    Ok(())
}

fn check_payer_access(payer: &AccountView) -> Result<()> {
    if !payer.is_signer()
        || !payer.is_writable()
        || !payer.owned_by(&pinocchio_system::ID)
        || !payer.is_data_empty()
        || payer.executable()
    {
        return Err(ErrorKind::InvalidPayer.into());
    }
    payer
        .check_borrow_mut()
        .map_err(|_| ErrorKind::BorrowConflict)?;
    Ok(())
}

fn check_refund(state: &AccountView, funding: Funding<'_>, recipient: &AccountView) -> Result<()> {
    if address_eq(state.address(), recipient.address())
        || matches!(funding, Funding::TopUp { payer, .. } if address_eq(payer.address(), recipient.address()))
    {
        return Err(ErrorKind::DuplicateAccount.into());
    }
    check_refund_access(recipient)
}

fn check_refund_access(recipient: &AccountView) -> Result<()> {
    if !recipient.is_writable() {
        return Err(ErrorKind::NotWritable.into());
    }
    if recipient.executable() {
        return Err(ErrorKind::Unauthorized.into());
    }
    recipient
        .check_borrow_mut()
        .map_err(|_| ErrorKind::BorrowConflict)?;
    Ok(())
}
