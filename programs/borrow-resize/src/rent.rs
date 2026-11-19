//! Rent settlement through real SDK guards, sysvars and System CPI.

use super::*;

#[inline(never)]
pub(super) fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [state, payer, system_program, recipient, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let mode = match ix.get(1) {
        Some(0) => Headroom::Max,
        Some(1) => Headroom::UpTo(INCREASE),
        #[cfg(feature = "alloc")]
        Some(2) => Headroom::OnDemand,
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    let funding = if ix.get(2) == Some(&1) {
        Funding::TopUp {
            payer,
            system_program,
        }
    } else {
        Funding::Prepaid
    };
    let action = *ix.get(4).ok_or(ProgramError::InvalidInstructionData)?;
    let expected = ProgramError::Custom(0xB100 + u32::from(*ix.get(13).unwrap_or(&0)));
    let original = state.data_len();
    let mut alias = *state;
    let mut payer_alias = *payer;
    let mut recipient_alias = *recipient;
    let payer_guard = (action == 8).then(|| payer_alias.try_borrow().unwrap());
    let recipient_guard = (action == 9).then(|| recipient_alias.try_borrow().unwrap());
    if matches!(action, 1 | 2) {
        let first_len = if action == 1 {
            original + 501
        } else {
            original - 37
        };
        let mut first = Store::new_with_headroom(state, id, funding, mode)?;
        first.resize(first_len)?;
        first.bytes_mut()[8..12].copy_from_slice(&(first_len as u32).to_le_bytes());
        let receipt = first.commit(first_len as u32)?;
        assert_eq!(receipt.storage_len as usize, first_len);
        assert_eq!(receipt.topped_up, 0);
        drop(first);
    }
    let opened = Store::new_with_headroom(state, id, funding, mode);
    if matches!(action, 6 | 8) {
        assert!(matches!(opened, Err(error) if ProgramError::from(error) == expected));
        assert_eq!(alias.data_len(), original, "invalid funding grew state");
        assert!(!alias.is_borrowed());
        return Ok(());
    }
    let store = opened?;
    #[cfg(feature = "alloc")]
    let on_demand = mode == Headroom::OnDemand;
    #[cfg(not(feature = "alloc"))]
    let on_demand = false;
    if matches!(action, 1 | 2) && !on_demand {
        let expected_limit = if action == 1 {
            original + INCREASE
        } else {
            original - 37 + INCREASE
        };
        assert_eq!(store.growth_limit(), expected_limit);
        assert_eq!(
            store.check_resize(expected_limit + 1),
            Err(ErrorKind::GrowthLimit.into())
        );
    }
    let attached = if ix.get(3) == Some(&1) {
        store.with_refund(recipient)
    } else {
        Ok(store)
    };
    if matches!(action, 7 | 9) {
        assert!(matches!(attached, Err(error) if ProgramError::from(error) == expected));
        assert!(!alias.is_borrowed());
        return Ok(());
    }
    drop(payer_guard);
    drop(recipient_guard);
    let mut store = attached?;
    let used = word(ix, 5)?;
    let retained = word(ix, 9)?;
    let resized = store.resize(used as usize);
    if action == 4 {
        assert_eq!(resized, Err(ErrorKind::RentShortfall.into()));
        assert!(!store.is_poisoned());
        assert_eq!(store.bytes()[12], 0x19);
        drop(store);
        assert_eq!(alias.data_len(), original);
        return Ok(());
    }
    resized?;
    if retained != 0 {
        store.reserve(used, retained as usize)?;
    }
    if action == 10 {
        assert!(store.shrink_to_fit(used)?);
    }
    store.bytes_mut()[12] = 42;
    store.bytes_mut()[8..12].copy_from_slice(&used.to_le_bytes());
    // Safe copied handles can change lamports while the data lease is held.
    // Preserve the instruction's total balance while testing both directions.
    if action == 12 {
        recipient_alias.set_lamports(recipient_alias.lamports().checked_sub(1).unwrap());
        alias.set_lamports(alias.lamports().checked_add(1).unwrap());
    } else if action == 13 {
        alias.set_lamports(alias.lamports().checked_sub(1).unwrap());
        payer_alias.set_lamports(payer_alias.lamports().checked_add(1).unwrap());
    }
    let starting_balance = alias.lamports();
    let recipient_balance = recipient_alias.lamports();
    if action == 3 {
        drop(store);
        assert_eq!(alias.lamports(), starting_balance);
        assert!(!alias.is_borrowed());
        return Ok(());
    }
    if action == 14 {
        store.set_poisoned(true);
    }
    let commit_used = if action == 15 {
        store.storage_len() as u32 + 1
    } else {
        used
    };
    let result = store.commit(commit_used);
    assert!(store.is_closed());
    assert!(!alias.is_borrowed());
    assert_eq!(store.commit(used), Err(ErrorKind::Closed.into()));
    if matches!(action, 4 | 11 | 14 | 15) {
        let error = match action {
            4 => ErrorKind::RentShortfall,
            11 => ErrorKind::ArithmeticOverflow,
            14 => ErrorKind::Poisoned,
            _ => ErrorKind::UsedLength,
        };
        assert_eq!(result, Err(error.into()));
        assert_eq!(alias.try_borrow()?[12], 42);
        assert_eq!(alias.lamports(), starting_balance);
        assert_eq!(recipient_alias.lamports(), recipient_balance);
        return Ok(());
    }
    let receipt = result?;
    let final_len = if action == 10 { used } else { used + retained };
    assert_eq!(receipt.used_len, used);
    assert_eq!(receipt.storage_len, final_len);
    assert_eq!(alias.data_len(), final_len as usize);
    assert_eq!(receipt.shrink_error, None);
    // Host fixtures compare both amounts to the runtime's full Rent sysvar.
    // Here verify that the receipt agrees with the actual balance changes.
    assert_eq!(
        alias.lamports() + receipt.refunded,
        starting_balance + receipt.topped_up
    );
    assert_eq!(
        recipient_alias.lamports(),
        recipient_balance + receipt.refunded
    );
    if action == 5 {
        return Err(ProgramError::Custom(99));
    }
    Ok(())
}

fn word(ix: &[u8], start: usize) -> core::result::Result<u32, ProgramError> {
    let bytes = ix
        .get(start..start + 4)
        .ok_or(ProgramError::InvalidInstructionData)?;
    Ok(u32::from_le_bytes(
        bytes
            .try_into()
            .map_err(|_| ProgramError::InvalidInstructionData)?,
    ))
}
