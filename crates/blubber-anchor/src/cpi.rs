//! Scoped checked handles and full revalidation across arbitrary callees.

use super::LoadPolicy;
use super::{copy_prefix, sdk, Blubber, Guard, Handoff, Lease, Session};
use alloc::boxed::Box;
use blubber::{AccountSchema, View};
use sdk::{CpiHandle, CpiHandleMut, Error, Owner};

impl<T: AccountSchema, L: Lease> Session<T, L> {
    // Unlike managed edits, a callee supplies no schema preservation contract.
    // Authenticate and validate every byte before publishing any copied field.
    pub(super) fn cpi<R>(
        &mut self,
        released: L,
        invoke: impl FnOnce() -> Result<R, Error>,
        acquire: impl FnOnce() -> Result<L, Error>,
        authenticate: impl FnOnce() -> Result<(), Error>,
    ) -> Result<R, Error> {
        self.check_ready()?;
        self.prefix = None;
        drop(core::mem::replace(&mut self.lease, released));
        let mut handoff = Handoff {
            session: self,
            acquire: Some(acquire),
        };
        let result = invoke();
        let acquire = handoff.acquire.take().expect("pending CPI acquisition");
        let session = &mut *handoff.session;
        match acquire() {
            Ok(lease) => session.lease = lease,
            Err(error) => return result.and(Err(error)),
        }
        let reload = authenticate().and_then(|()| {
            let view = View::load::<T>(session.lease.bytes(), T::DISCRIMINATOR)?;
            Ok(copy_prefix::<T>(view)?)
        });
        match reload {
            Ok(prefix) => session.prefix = Some(prefix),
            Err(error) => return result.and(Err(error)),
        }
        result
    }
}

impl<T: AccountSchema + Owner, P: LoadPolicy> Blubber<T, P> {
    /// Releases exclusive exclusion for a checked writable CPI, then reloads.
    ///
    /// Each preceding edit has already committed. The callback receives an
    /// ordinary Anchor checked handle; use generated CPI bindings or the SDK's
    /// checked invocation functions, with signer seeds when needed. Handles
    /// cannot escape or overlap typed access through this wrapper.
    ///
    /// Every returned path reacquires the original exclusive guard and fully
    /// validates owner, executable state, envelope and root before refreshing
    /// the prefix. Callback errors take precedence over reload errors. Failed
    /// acquisition/validation and host unwinding disable typed access terminally;
    /// propagate errors for instruction rollback. One handle slot is retained
    /// per call, including returned failures. CPI never resets the SDK's saved
    /// invocation length or growth window. Readonly loads reject this method.
    pub fn with_cpi<R>(
        &mut self,
        invoke: impl FnOnce(CpiHandleMut<'_>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        self.session.check_ready()?;
        self.session.lease.bytes_mut()?;
        self.cpi_inner(|account| invoke(CpiHandleMut::writable(account)))
    }

    /// Releases exclusion for a checked readonly CPI, then fully reloads.
    ///
    /// Works with shared and exclusive loads and restores that same mode.
    /// Takes exclusive wrapper access so prefix references and reads cannot
    /// span the callback. Uses the same error, terminal-state, heap and full
    /// reload contract as [`Self::with_cpi`]. A readonly handle grants no write
    /// privilege; another live shared wrapper can still exclude writable CPI.
    pub fn with_cpi_read<R>(
        &mut self,
        invoke: impl FnOnce(CpiHandle<'_>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        self.session.check_ready()?;
        self.cpi_inner(|account| invoke(CpiHandle::readonly(account)))
    }

    // A separate frame keeps caller-owned 1024-byte prefix copies from sharing
    // the reload's validation and publication temporaries on SBF.
    #[inline(never)]
    fn cpi_inner<R>(
        &mut self,
        invoke: impl FnOnce(&mut sdk::AccountView) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let exclusive = matches!(self.session.lease, Guard::Exclusive(_));
        // Allocate before releasing the original guard, including on unwind.
        let slot = Box::leak(Box::new(self.account));
        let mut account = self.account;
        let authenticate = self.account;
        self.session.cpi(
            Guard::Released,
            || invoke(&mut account),
            || {
                if exclusive {
                    slot.try_borrow_mut().map(Guard::Exclusive)
                } else {
                    slot.try_borrow().map(Guard::Shared)
                }
            },
            || {
                if !authenticate.owned_by(&T::OWNER) || authenticate.executable() {
                    return Err(Error::IllegalOwner);
                }
                Ok(())
            },
        )
    }
}
