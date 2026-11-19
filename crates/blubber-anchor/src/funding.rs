//! Namespaced constraints binding copied funding views for [`super::Blubber::edit`].
//!
//! Import this module under a local name, then attach both
//! `funding::payer = payer` and `funding::system_program = system_program` to a
//! mutable Blubber field. Anchor supplies full AccountViews through its
//! `AccountConstraint::Value` protocol. These hooks validate and copy handles;
//! they never transfer lamports. Store rechecks them before each managed edit.

use super::LoadPolicy;
use super::{sdk, Blubber};
use blubber::AccountSchema;
use sdk::{AccountConstraint, AccountView, Error, Owner};

/// Binds a writable System-owned signer with empty data as the TopUp payer.
pub struct PayerConstraint;

/// Binds the executable System Program; required together with [`PayerConstraint`].
pub struct SystemProgramConstraint;

impl<T: AccountSchema + Owner, P: LoadPolicy> AccountConstraint<Blubber<T, P>> for PayerConstraint {
    type Value = AccountView;

    fn check(account: &Blubber<T, P>, value: &AccountView) -> Result<(), Error> {
        if !matches!(account.session.lease, super::Guard::Exclusive(_)) {
            return Err(sdk::ErrorCode::ConstraintMut.into());
        }
        blubber::__private::pinocchio::check_payer(&account.account, value)?;
        account.payer.set(Some(*value));
        Ok(())
    }

    fn init(account: &mut Blubber<T, P>, value: &AccountView) -> Result<(), Error> {
        Self::check(account, value)
    }

    fn update(account: &mut Blubber<T, P>, value: &AccountView) -> Result<(), Error> {
        Self::check(account, value)
    }
}

impl<T: AccountSchema + Owner, P: LoadPolicy> AccountConstraint<Blubber<T, P>>
    for SystemProgramConstraint
{
    type Value = AccountView;

    fn check(account: &Blubber<T, P>, value: &AccountView) -> Result<(), Error> {
        if !matches!(account.session.lease, super::Guard::Exclusive(_)) {
            return Err(sdk::ErrorCode::ConstraintMut.into());
        }
        if account.account.address() == value.address() {
            return Err(blubber::Error::from(blubber::ErrorKind::DuplicateAccount).into());
        }
        if value.address() != &pinocchio_system::ID || !value.executable() {
            return Err(blubber::Error::from(blubber::ErrorKind::InvalidPayer).into());
        }
        account.system_program.set(Some(*value));
        Ok(())
    }

    fn init(account: &mut Blubber<T, P>, value: &AccountView) -> Result<(), Error> {
        Self::check(account, value)
    }

    fn update(account: &mut Blubber<T, P>, value: &AccountView) -> Result<(), Error> {
        Self::check(account, value)
    }
}
