//! Exact pre-handler sizing and successful-exit close through safe SDK APIs.

use super::LoadPolicy;
use super::{sdk, Blubber, Guard, Lease, Sizing};
use ::pinocchio::Resize as _;
use alloc::boxed::Box;
use blubber::{__private::pinocchio::MAX_STORAGE, AccountSchema, ErrorKind, View};
use sdk::{AccountClose, AccountRealloc, AccountView, Error, Owner};

// Check all geometry before spending a handle slot or releasing exclusion.
pub(super) fn check_space(new_space: usize, used: usize) -> Result<(), Error> {
    if new_space > MAX_STORAGE {
        return Err(blubber::Error::from(ErrorKind::Capacity).into());
    }
    if new_space < used {
        return Err(blubber::Error::from(ErrorKind::UsedLength).into());
    }
    Ok(())
}

pub(super) fn close_balance(balance: u64, recipient: u64) -> Result<u64, Error> {
    recipient
        .checked_add(balance)
        .ok_or_else(|| blubber::Error::from(ErrorKind::ArithmeticOverflow).into())
}

impl<T: AccountSchema + Owner, P: LoadPolicy> Blubber<T, P> {
    fn check_lifecycle(&mut self) -> Result<(), Error> {
        self.session.check_ready()?;
        self.session.lease.bytes_mut()?;
        if !self.account.owned_by(&T::OWNER) || self.account.executable() {
            return Err(Error::IllegalOwner);
        }
        if !self.account.is_writable() {
            return Err(sdk::ErrorCode::ConstraintMut.into());
        }
        Ok(())
    }
}

impl<T: AccountSchema + Owner, P: LoadPolicy> AccountRealloc for Blubber<T, P> {
    /// Selects exact sizing and settles final rent once before the handler.
    ///
    /// The writable System-owned signer pays a net deficit or receives all
    /// excess above final rent. No separate Anchor refund path runs. Safe SDK
    /// resize preserves the invocation's original growth window and zeroes new
    /// bytes, including when `zero` is false. Subsequent edits preserve this
    /// exact allocation. Realloc after a managed edit returns InvalidRealloc.
    fn realloc_account(
        &mut self,
        new_space: usize,
        payer: AccountView,
        _zero: bool,
    ) -> Result<(), Error> {
        self.check_lifecycle()?;
        if self.sizing == Sizing::Managed {
            return Err(Error::InvalidRealloc);
        }
        let view = View::load_trusted::<T>(self.session.lease.bytes(), T::DISCRIMINATOR)?;
        check_space(new_space, view.bytes().len() + 12)?;
        blubber::__private::pinocchio::check_payer(&self.account, &payer)?;
        // Preflight the complete rent profile and both balances before resize.
        let minimum = blubber::__private::pinocchio::minimum_balance(new_space)?;
        let balance = self.account.lamports();
        let deficit = minimum.saturating_sub(balance);
        let excess = balance.saturating_sub(minimum);
        payer
            .lamports()
            .checked_sub(deficit)
            .ok_or_else(|| blubber::Error::from(ErrorKind::RentShortfall))?
            .checked_add(excess)
            .ok_or_else(|| blubber::Error::from(ErrorKind::ArithmeticOverflow))?;
        let slot = Box::leak(Box::new(self.account));
        self.sizing = Sizing::Exact;
        let account = &mut self.account;
        self.session.handoff(
            Guard::Released,
            |lease| {
                drop(lease);
                let result = (|| {
                    if account.data_len() != new_space {
                        account.resize(new_space)?;
                    }
                    let mut recipient = payer;
                    blubber::__private::pinocchio::settle_account(
                        account,
                        Some(&payer),
                        Some(&mut recipient),
                        new_space,
                    )?;
                    Ok(())
                })();
                (result, false)
            },
            || slot.try_borrow_mut().map(Guard::Exclusive),
        )
    }
}

impl<T: AccountSchema + Owner, P: LoadPolicy> AccountClose for Blubber<T, P> {
    /// Closes a ready mutable account and transfers its entire balance once.
    ///
    /// Applications authorize the distinct writable recipient through their
    /// Accounts constraints. All checks precede discriminator invalidation.
    /// Closed wrappers reject every typed operation and further close attempts.
    /// The zero-lamport account retains its owner and an invalid twelve-byte
    /// envelope until runtime purge. This rejects later reads and
    /// `init_if_needed` recreation in the same transaction, including after a
    /// rent top-up or raw regrowth. Funding can keep the invalid account alive
    /// across transactions. After purge, creation is possible again; authorize
    /// that branch through seeds or persistent application state.
    fn close(&mut self, mut destination: AccountView) -> Result<(), Error> {
        self.check_lifecycle()?;
        blubber::__private::pinocchio::check_refund(
            &self.account,
            blubber::pinocchio::Funding::Prepaid,
            &destination,
        )?;
        let balance = close_balance(self.account.lamports(), destination.lamports())?;
        self.session.invalidate(Guard::Released)?;
        // Safe SDK shrink checks exclusion again. Keep ownership and the
        // invalid envelope so Anchor selects its existing-account branch.
        // No callbacks or CPI occur during this handoff.
        self.account.resize(12)?;
        destination.set_lamports(balance);
        self.account.set_lamports(0);
        Ok(())
    }
}
