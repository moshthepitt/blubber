//! Same-invocation tour completion, CPI read and growth-window reopening.

use blubber::{
    pinocchio::{Funding, Headroom, ReadStore, Store},
    ErrorKind, ReadStorage, Resize,
};
use pinocchio::{
    cpi::invoke,
    error::ProgramError,
    instruction::{InstructionAccount, InstructionView},
    AccountView, Address, ProgramResult,
};

use crate::state::MyAccount;

pub(crate) fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    if ix == [61] {
        let [state] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let store = ReadStore::new(state, id)?;
        let acct = MyAccount::load(store.bytes())?;
        assert_eq!(acct.counter(), 1);
        assert_eq!(acct.note(), "grew after physical shrink");
        assert_eq!(acct.records().len(), 0);
        return Ok(());
    }
    let [60, mode] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let headroom = match mode {
        0 => Headroom::Max,
        1 => Headroom::UpTo(super::INCREASE),
        #[cfg(feature = "alloc")]
        2 => Headroom::OnDemand,
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    let [state, payer, system_program, _self_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let original = state.data_len();
    let ceiling = original + super::INCREASE;
    let actor = *payer.address();
    assert_eq!(
        original as u32,
        MyAccount::encoded_len_of(&crate::state::seed(actor))?
    );
    let mut store = Store::new_with_headroom(
        state,
        id,
        Funding::TopUp {
            payer,
            system_program,
        },
        headroom,
    )?;
    let mut acct = MyAccount::load_mut(&mut store)?;
    crate::exercise::exercise(&mut acct, actor)?;
    // Retain the whole opening window so the second session has no growth left.
    let used = acct.used_len();
    acct.reserve(ceiling as u32 - used)?;
    let receipt = acct.commit()?;
    assert_eq!(receipt.storage_len as usize, ceiling);
    assert_eq!(receipt.shrink_error, None);
    assert!(store.is_closed());
    drop(store);
    assert!(!state.is_borrowed());

    invoke(
        &InstructionView {
            program_id: id,
            accounts: &[InstructionAccount::readonly(state.address())],
            data: &[61],
        },
        &[&*state],
    )?;
    assert_eq!(state.data_len(), ceiling);

    let mut store = Store::new_with_headroom(state, id, Funding::Prepaid, headroom)?;
    assert_eq!(store.growth_limit(), ceiling);
    let mut acct = MyAccount::load_mut(&mut store)?;
    // CPI and a fresh lease must not reset this invocation's original window.
    #[cfg(feature = "alloc")]
    let expected = if headroom == Headroom::OnDemand {
        ErrorKind::RentShortfall
    } else {
        ErrorKind::GrowthLimit
    };
    #[cfg(not(feature = "alloc"))]
    let expected = ErrorKind::GrowthLimit;
    assert_eq!(
        acct.reserve(ceiling as u32 + 1 - used),
        Err(expected.into())
    );
    assert_eq!(acct.storage_len() as usize, ceiling);
    assert_eq!(acct.used_len(), used);
    acct.validate()?;
    assert!(acct.shrink_to_fit()?);
    let receipt = acct.commit()?;
    assert_eq!(receipt.storage_len, used);
    assert_eq!(receipt.shrink_error, None);
    drop(store);
    assert!(!state.is_borrowed());
    assert_eq!(state.data_len(), used as usize);
    Ok(())
}
