//! checked and authenticated SDK ingress with explicit feature controls.
use blubber::{
    pinocchio::{Funding, Headroom, ReadStore, Store},
    ErrorKind, List, Map, Resize, Str,
};
use pinocchio::{AccountView, Address, ProgramResult};

#[blubber::schema(account, namespace = "d21:policy")]
/// Account with independent prefix, UTF-8, map and list validation work.
pub struct PolicyState {
    /// Copied fixed constraint domain.
    pub flag: bool,
    /// Independent UTF-8 leaf.
    pub text: Str,
    /// Canonical map order and value domains.
    pub entries: Map<u8, bool>,
    /// Scan-cost input.
    pub flags: List<bool>,
}

#[cfg(feature = "policy")]
impl blubber_anchor::sdk::Owner for PolicyState {
    const OWNER: Address = Address::new_from_array([7; 32]);
}

/// Runs a scoped policy control with the genuine runtime account guard.
pub fn run(owner: &Address, accounts: &mut [AccountView], op: u8) -> ProgramResult {
    let [account] = accounts else {
        return Err(pinocchio::error::ProgramError::NotEnoughAccountKeys);
    };
    let mut alias = *account;
    match op {
        0 => {
            let store = ReadStore::new(account, owner)?;
            let _root = store.load_checked::<PolicyState>()?;
        }
        1 => {
            let mut store = Store::new(account, owner, Funding::Prepaid)?;
            let root = store.load_checked::<PolicyState>()?;
            root.commit()?;
        }
        // Rejection must precede callbacks, which deliberately do no verification.
        2 => PolicyState::with_read_checked(account, owner, |_| Ok(()))?,
        3 => {
            PolicyState::with_edit_checked(account, owner, Funding::Prepaid, |_| Ok(()))?;
        }
        4 => PolicyState::with_read(account, owner, |root| {
            let _ = root.try_flag()?;
            Ok(())
        })?,
        5 => PolicyState::with_read(account, owner, |root| root.verify())?,
        9 => {
            let store = ReadStore::new(account, owner)?;
            let _root = store.load::<PolicyState>()?;
            assert!(alias.try_borrow_mut().is_err());
            let other = ReadStore::new(&alias, owner)?;
            let _other_root = other.load::<PolicyState>()?;
        }
        10 | 11 => {
            let mut store = Store::new(account, owner, Funding::Prepaid)?;
            assert!(alias.try_borrow().is_err());
            assert!(alias.try_borrow_mut().is_err());
            let root = if op == 10 {
                store.load::<PolicyState>()?
            } else {
                // The public trait-object route must retain SDK provenance.
                let backend: &mut dyn Resize = &mut store;
                PolicyState::load_mut(backend)?
            };
            root.commit()?;
            assert!(matches!(store.load::<PolicyState>(), Err(e) if e == ErrorKind::Closed.into()));
        }
        12 => PolicyState::with_read(account, owner, |_| {
            assert!(alias.try_borrow_mut().is_err());
            Ok(())
        })?,
        13 => {
            PolicyState::with_edit(account, owner, Funding::Prepaid, |_| {
                assert!(alias.try_borrow().is_err());
                Ok(())
            })?;
        }
        14 => PolicyState::with_read(account, owner, |root| root.try_flag().map(|_| ()))?,
        15 => PolicyState::with_read(account, owner, |root| root.try_text().map(|_| ()))?,
        16 => PolicyState::with_read(account, owner, |root| root.verify())?,
        17 => {
            // A local successful edit must not certify or repair distant damage.
            PolicyState::with_edit(account, owner, Funding::Prepaid, |root| {
                root.flag_mut().set(true)
            })?;
        }
        18 => {
            let mut store = Store::new(account, owner, Funding::Prepaid)?;
            blubber::raw::backend(&mut store).set_poisoned(true);
            assert!(
                matches!(store.load::<PolicyState>(), Err(e) if e == ErrorKind::Poisoned.into())
            );
            assert!(
                matches!(PolicyState::load_mut(&mut store), Err(e) if e == ErrorKind::Poisoned.into())
            );
            let root = store.load_checked::<PolicyState>()?;
            root.verify()?;
            root.commit()?;
            assert!(matches!(store.load::<PolicyState>(), Err(e) if e == ErrorKind::Closed.into()));
        }
        19 => {
            let store = ReadStore::new(account, owner)?;
            store.load::<PolicyState>()?.verify()?;
            core::mem::forget(store);
            assert!(
                matches!(Store::new(&mut alias, owner, Funding::Prepaid), Err(e) if e == ErrorKind::BorrowConflict.into())
            );
        }
        20 => {
            let mut store =
                Store::new_with_headroom(account, owner, Funding::Prepaid, Headroom::UpTo(0))?;
            store.load::<PolicyState>()?.verify()?;
            core::mem::forget(store);
            assert!(
                matches!(ReadStore::new(&alias, owner), Err(e) if e == ErrorKind::BorrowConflict.into())
            );
        }
        #[cfg(feature = "policy")]
        6 => {
            use blubber_anchor::{sdk::AnchorAccount, Blubber};
            let root = Blubber::<PolicyState, blubber_anchor::Checked>::load(*account)?;
            if account.check_borrow_mut().is_ok() {
                return Err(pinocchio::error::ProgramError::InvalidAccountData);
            }
            if !root.flag {
                return Err(pinocchio::error::ProgramError::InvalidAccountData);
            }
        }
        #[cfg(feature = "policy")]
        7 => {
            use blubber_anchor::{sdk::AnchorAccount, Blubber};
            let mut root = Blubber::<PolicyState, blubber_anchor::Checked>::load_mut(*account)?;
            if Blubber::<PolicyState, blubber_anchor::Checked>::load(*account).is_ok() {
                return Err(pinocchio::error::ProgramError::InvalidAccountData);
            }
            root.edit(|_| Ok(()))?;
        }
        #[cfg(feature = "policy")]
        8 => {
            use blubber_anchor::{sdk::AnchorAccount, Blubber};
            let root = Blubber::<PolicyState>::load(*account)?;
            root.verify()?;
            root.read(|reader| reader.verify())?;
        }
        _ => return Err(pinocchio::error::ProgramError::InvalidInstructionData),
    }
    if !matches!(op, 19 | 20) {
        assert!(!account.is_borrowed());
    }
    Ok(())
}
