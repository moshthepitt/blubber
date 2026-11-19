//! Creation and borrowed initialization against runtime account memory.

blubber::__root_schema!(__ConcreteRoot0, Map<u16, Str>);

use super::*;
use blubber::{
    pinocchio::{allocate_pda, create_account, create_pda},
    raw::Edit,
    List, Map, Str,
};

#[blubber::schema(account, discriminator = [73, 78, 73, 84, 48, 48, 48, 49])]
struct Initial {
    value: u32,
    data: List<u8>,
}

#[inline(never)]
pub(super) fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [state, payer, system_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if ix.len() < 18 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let action = ix[1];
    let control = ix[3];
    let space = word(ix, 4);
    let bump = [ix[16]];
    let seed: &[u8] = if ix[17] == 1 { b"wrong" } else { b"init" };
    let long_seed = [0; 33];
    let seeds = [seed, &bump];
    let long_seeds = [&long_seed[..], &bump];
    let too_many = [&bump[..]; 17];
    let mut full_seeds = [&b"init"[..]; 16];
    full_seeds[15] = &bump;
    let seeds: &[&[u8]] = match ix[17] {
        2 => &long_seeds,
        3 => &too_many,
        4 => &full_seeds,
        _ => &seeds,
    };
    let state_alias = *state;
    let payer_alias = *payer;
    let _state_guard = (control == 2).then(|| state_alias.try_borrow().unwrap());
    let _payer_guard = (control == 3).then(|| payer_alias.try_borrow().unwrap());
    match action {
        0 => create_account(state, payer, system_program, id, space)?,
        1 => create_pda(state, payer, system_program, id, space, seeds)?,
        2 => allocate_pda(state, payer, system_program, id, space, seeds)?,
        3 => (),
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    // Creation must return with no guard retained; the next operation is a
    // fresh safe data lease. Keep the CPI-established original window intact.
    assert!(!state.is_borrowed());
    let original = state.data_len();
    let mode = match ix[2] {
        0 => Headroom::Max,
        1 => Headroom::UpTo(word(ix, 8) as usize),
        #[cfg(feature = "alloc")]
        2 => Headroom::OnDemand,
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    let mut store = Store::new_with_headroom(state, id, Funding::Prepaid, mode)?;
    let opened = store.storage_len();
    if control == 8 {
        // A creation CPI consumed all but this saved-original SDK window.
        assert_eq!(opened, INCREASE);
        assert_eq!(
            store.check_resize(INCREASE + 1),
            Err(ErrorKind::GrowthLimit.into())
        );
    }
    if control == 9 {
        // Independent of SDK-local capacity and selected headroom mode.
        assert_eq!(store.check_resize(MAX + 1), Err(ErrorKind::Capacity.into()));
        assert_eq!(store.resize(MAX + 1), Err(ErrorKind::Capacity.into()));
        assert_eq!(store.storage_len(), opened);
    }
    let payload = &ix[18..];
    let result = if control == 7 {
        let entries = [(2, "second"), (1, "first")];
        Edit::initialize::<__ConcreteRoot0>(&mut store, *b"INIT0001", &entries).map(|_| ())
    } else {
        Initial::initialize(
            &mut store,
            InitialInit {
                value: 42,
                data: payload,
            },
        )
        .map(|_| ())
    };
    let expected = word(ix, 12);
    if expected != 0 {
        let error = result.expect_err("initialization unexpectedly succeeded");
        assert_eq!(ProgramError::from(error), ProgramError::Custom(expected));
        assert_eq!(store.storage_len(), opened, "failed init changed length");
        assert!(!store.is_poisoned());
        // Host fixtures use a known nonzero spare pattern. Compare every byte
        // while the lease is still live, before transaction rollback or Drop.
        for (index, byte) in store.bytes().iter().enumerate() {
            let old = if action == 3 && index < original && (index >= 8 || control == 6) {
                0xa5
            } else {
                0
            };
            assert_eq!(*byte, old, "failed initialization wrote bytes");
        }
        if control == 4 {
            return Err(error.into());
        }
        return Ok(());
    }
    result?;
    let used = {
        let root = Initial::load_mut(&mut store)?;
        assert_eq!(root.value(), 42);
        assert_eq!(root.data().len(), payload.len());
        root.used_len() as usize
    };
    if control == 5 {
        assert!(
            matches!(Initial::initialize(&mut store, InitialInit { value: 7, data: b"replacement" }),
            Err(error) if error == blubber::Error::new(ErrorKind::AlreadyInitialized, 0))
        );
        assert_eq!(Initial::load(store.bytes())?.value(), 42);
        assert_eq!(store.bytes()[8..12], (used as u32).to_le_bytes());
    }
    assert!(store.bytes()[used..].iter().all(|byte| *byte == 0));
    let receipt = Initial::load_mut(&mut store)?.commit()?;
    assert_eq!(receipt.used_len as usize, 20 + payload.len());
    assert_eq!(receipt.storage_len, receipt.used_len);
    assert_eq!(receipt.topped_up, 0);
    assert_eq!(receipt.shrink_error, None);
    drop(store);
    if control == 10 {
        let mut reopened = Store::new_with_headroom(state, id, Funding::Prepaid, mode)?;
        assert_eq!(reopened.storage_len(), INCREASE);
        #[cfg(feature = "alloc")]
        let expected = if mode == Headroom::OnDemand {
            // Exact prepaid rent cannot cover another byte. OnDemand checks
            // affordability before attempting the SDK growth/re-lease.
            ErrorKind::RentShortfall
        } else {
            ErrorKind::GrowthLimit
        };
        #[cfg(not(feature = "alloc"))]
        let expected = ErrorKind::GrowthLimit;
        assert_eq!(reopened.resize(INCREASE + 1), Err(expected.into()));
        reopened.commit(receipt.used_len)?;
    }
    Ok(())
}

fn word(ix: &[u8], start: usize) -> u32 {
    u32::from_le_bytes(ix[start..start + 4].try_into().unwrap())
}
