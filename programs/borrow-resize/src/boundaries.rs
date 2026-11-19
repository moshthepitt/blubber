//! Invocation boundaries through the same safe SDK account and store dispatch.

use super::*;
use pinocchio::instruction::{InstructionAccount, InstructionView};

pub(super) fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [80, action, a, b, c, d] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let amount = u32::from_le_bytes([*a, *b, *c, *d]) as usize;
    let [state, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    match action {
        0 => resize(id, state, amount)?,
        5..=7 => {
            let headroom = if *action == 5 {
                Headroom::Max
            } else {
                Headroom::UpTo(0)
            };
            let mut store = if *action == 7 {
                Store::new(state, id, Funding::Prepaid)?
            } else {
                Store::new_with_headroom(state, id, Funding::Prepaid, headroom)?
            };
            let mut root = Lease::load_mut(&mut store)?;
            root.value_mut().set(42)?;
            root.commit()?;
        }
        4 => {
            state.resize(amount)?;
            let mut bytes = state.try_borrow_mut()?;
            bytes[8..12].copy_from_slice(&(amount as u32).to_le_bytes());
            bytes[12] = 42;
        }
        1 | 2 => {
            // An inner invocation gets its own serialized length. Returning to
            // the caller must still obey the caller's original growth window.
            let original = state.data_len();
            resize(id, state, original + 1)?;
            let mut data = [80, 0, 0, 0, 0, 0];
            data[2..].copy_from_slice(&((original + amount) as u32).to_le_bytes());
            pinocchio::cpi::invoke(
                &InstructionView {
                    program_id: id,
                    accounts: &[InstructionAccount::writable(state.address())],
                    data: &data,
                },
                &[&*state],
            )?;
            assert_eq!(state.data_len(), original + amount);
            if *action == 2 {
                // A physical shrink cannot replenish the outer SDK allowance.
                resize(id, state, original - 1)?;
                let mut store =
                    Store::new_with_headroom(state, id, Funding::Prepaid, Headroom::Max)?;
                assert_eq!(
                    store.check_resize(original + INCREASE + 1),
                    Err(ErrorKind::GrowthLimit.into())
                );
                store.commit((original - 1) as u32)?;
            }
        }
        3 => {
            let original = state.data_len();
            resize(id, state, original + amount)?;
            return Err(ProgramError::Custom(0x4040));
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    Ok(())
}

fn resize(id: &Address, state: &mut AccountView, target: usize) -> ProgramResult {
    // Cross-invocation controls intentionally discover the full SDK window.
    let mut store = Store::new_with_headroom(state, id, Funding::Prepaid, Headroom::Max)?;
    store.resize(target)?;
    store.bytes_mut()[8..12].copy_from_slice(&(target as u32).to_le_bytes());
    store.bytes_mut()[12] = 42;
    store.commit(target as u32)?;
    Ok(())
}
