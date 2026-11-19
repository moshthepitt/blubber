//! Raw lifecycle measurements through the production safe account adapter.

use crate::mark;
use blubber::raw::Resize as _;
use blubber::{
    pinocchio::{Funding, Headroom, ReadStore, Store},
    ErrorKind, Resize,
};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

const _: () = assert!(core::mem::size_of::<AccountView>() == 8);

/// Runs a raw lease scenario; arguments are mode, action, TopUp, refund, reserve.
#[inline(never)]
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [28, mode, action, top_up, refund, retained] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state, payer, system_program, recipient] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let original = state.data_len();
    let initial = original;
    let mode = match mode {
        0 => Headroom::Max,
        1 => Headroom::UpTo(0),
        2 => Headroom::UpTo(32),
        #[cfg(feature = "alloc")]
        3 => Headroom::OnDemand,
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    #[cfg(feature = "alloc")]
    let on_demand = mode == Headroom::OnDemand;
    #[cfg(not(feature = "alloc"))]
    let on_demand = false;
    let funding = if *top_up == 1 {
        Funding::TopUp {
            payer,
            system_program,
        }
    } else {
        Funding::Prepaid
    };
    // A previous completed session changes the SDK-local remaining window.
    if matches!(action, 3 | 4) {
        let next = if *action == 3 {
            original + 501
        } else {
            original - 37
        };
        // The preceding session must grow to test the next lease's remaining window.
        let mut first = Store::new_with_headroom(state, id, funding, Headroom::Max)?;
        first.resize(next)?;
        first.bytes_mut()[8..12].copy_from_slice(&(next as u32).to_le_bytes());
        first.commit(next as u32)?;
    }
    let opening = state.data_len();
    mark(10);
    mark(11);
    mark(0);
    if *action == 6 {
        let read = ReadStore::new(state, id)?;
        mark(1);
        mark(2);
        core::hint::black_box(read.bytes());
        mark(3);
        drop(read);
        mark(4);
        telemetry(initial, opening, opening, opening, 0);
        return Ok(());
    }
    let store = Store::new_with_headroom(state, id, funding, mode)?;
    let opened = store.storage_len();
    mark(1);
    mark(2);
    let mut store = if *refund == 1 {
        store.with_refund(recipient)?
    } else {
        store
    };
    let mut used = opening;
    let mut growths = 0;
    let mut zeroed = opened - opening;
    let mut resize = |store: &mut Store<'_>, target| -> blubber::Result<()> {
        let old = store.storage_len();
        store.resize(target)?;
        let new = store.storage_len();
        if new > old {
            growths += 1;
            zeroed += new - old;
        }
        Ok(())
    };
    match *action {
        0 | 3 | 4 | 5 => (),
        1 => {
            used += 8;
            resize(&mut store, used)?;
        }
        2 => {
            for _ in 0..50 {
                used += 8;
                resize(&mut store, used)?;
            }
        }
        7 => {
            used = 64;
            resize(&mut store, used)?;
        }
        8 => {
            used = initial + 10_240;
            resize(&mut store, used)?;
        }
        9 => {
            assert_eq!(
                store.resize(initial + 10_241),
                Err(ErrorKind::GrowthLimit.into())
            );
            assert_eq!(store.storage_len(), opened);
            growths += u64::from(on_demand);
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    if *retained != 0 {
        let old = store.storage_len();
        store.reserve(used as u32, usize::from(*retained))?;
        let new = store.storage_len();
        if new > old {
            growths += 1;
            zeroed += new - old;
        }
    }
    store.bytes_mut()[12] = 42;
    store.bytes_mut()[8..12].copy_from_slice(&(used as u32).to_le_bytes());
    let peak = store.storage_len();
    mark(3);
    let receipt = store.commit(used as u32)?;
    assert_eq!(receipt.storage_len as usize, used + usize::from(*retained));
    drop(store);
    if *action == 5 {
        let mut again = Store::new_with_headroom(state, id, funding, mode)?;
        zeroed += again.storage_len() - used;
        again.commit(used as u32)?;
    }
    mark(4);
    telemetry(
        initial,
        opening,
        opened,
        peak,
        (growths << 32) | zeroed as u64,
    );
    Ok(())
}

fn telemetry(_initial: usize, _opening: usize, _opened: usize, _peak: usize, _counts: u64) {
    #[cfg(feature = "metrics")]
    {
        solana_program_log::log_64(
            0x4c53,
            _initial as u64,
            _opening as u64,
            _opened as u64,
            _peak as u64,
        );
        solana_program_log::log_64(0x4c54, _counts, 0, 0, 0);
    }
}
