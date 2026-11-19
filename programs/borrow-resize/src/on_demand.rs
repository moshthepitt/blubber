//! Runtime checks for the alloc-gated re-lease path.

use super::*;

#[blubber::schema(account, discriminator = [68, 69, 77, 65, 78, 68, 48, 49])]
struct Demand {
    values: blubber::List<u64>,
}

#[inline(never)]
pub(super) fn run(id: &Address, accounts: &mut [AccountView], opcode: u8) -> ProgramResult {
    let (first, rest) = accounts.split_at_mut(1);
    let account = &mut first[0];
    let alias = rest.first_mut().ok_or(ProgramError::NotEnoughAccountKeys)?;
    let original = account.data_len();
    let mut store = Store::new_with_headroom(account, id, Funding::Prepaid, Headroom::OnDemand)?;
    assert_eq!(store.storage_len(), original);
    assert_eq!(store.growth_limit(), original);
    assert_eq!(alias.data_len(), original);
    assert!(matches!(ReadStore::new(alias, id), Err(e) if e == ErrorKind::BorrowConflict.into()));
    assert!(
        matches!(Store::new_with_headroom(alias, id, Funding::Prepaid, Headroom::OnDemand),
                    Err(e) if e == ErrorKind::BorrowConflict.into())
    );
    match opcode {
        34 => {
            store.resize(MAX)?;
            assert_eq!(store.storage_len(), MAX);
            assert_eq!(store.growth_limit(), MAX);
            assert_eq!(store.resize(MAX + 1), Err(ErrorKind::Capacity.into()));
            store.commit(original as u32)?;
        }
        20 => {
            store.resize(original)?;
            store.bytes_mut()[12] = 42;
            assert_eq!(store.storage_len(), original);
            assert_eq!(
                store.commit(original as u32)?.storage_len as usize,
                original
            );
        }
        21 | 26 | 28 => {
            store.resize(original + 8)?;
            assert_eq!(
                store.storage_len(),
                original + 8 + (original / 4).clamp(8, 2048)
            );
            assert!(store.bytes()[original..].iter().all(|b| *b == 0));
            store.bytes_mut()[12] = 42;
            if opcode == 28 {
                core::mem::forget(store);
                assert!(alias.is_borrowed_mut());
                assert_eq!(
                    alias.resize(original),
                    Err(ProgramError::AccountBorrowFailed)
                );
                return Ok(());
            }
            if opcode == 26 {
                drop(store);
                assert_eq!(alias.data_len(), original);
                assert!(!alias.is_borrowed());
                return Ok(());
            }
            store.commit((original + 8) as u32)?;
        }
        22 => {
            let mut events = 0;
            for i in 1..=50 {
                let before = store.storage_len();
                let needed = original + i * 8;
                store.resize(needed)?;
                events += usize::from(store.storage_len() != before);
                store.bytes_mut()[needed - 8..needed].copy_from_slice(&(i as u64).to_le_bytes());
            }
            assert_eq!((original, events, store.storage_len()), (64, 8, 478));
            for i in 1..=50 {
                let end = original + i * 8;
                assert_eq!(&store.bytes()[end - 8..end], &(i as u64).to_le_bytes());
            }
            store.commit((original + 400) as u32)?;
        }
        23 => {
            // Amortized target exceeds the invocation window; exact retry fits.
            store.resize(original + INCREASE)?;
            assert_eq!(store.storage_len(), original + INCREASE);
            assert_eq!(store.growth_limit(), original + INCREASE);
            assert_eq!(store.bytes()[12], 0x19);
            store.commit(original as u32)?;
        }
        24 | 25 => {
            let prefix: [u8; 64] = store.bytes()[..64].try_into().unwrap();
            let (needed, error) = if opcode == 24 {
                (original + INCREASE + 1, ErrorKind::GrowthLimit)
            } else {
                (MAX + 1, ErrorKind::Capacity)
            };
            assert_eq!(store.resize(needed), Err(error.into()));
            assert_eq!(store.storage_len(), original);
            assert_eq!(store.bytes(), prefix);
            assert!(alias.is_borrowed_mut());
            // Returned resize failure restores a usable, exclusive guard.
            store.bytes_mut()[12] = 42;
            store.resize(original + 8)?;
            store.commit(original as u32)?;
        }
        27 => {
            store.reserve(original as u32, 8)?;
            assert!(store.storage_len() >= original + 8);
            assert_eq!(
                store.commit(original as u32)?.storage_len as usize,
                original + 8
            );
            drop(store);
            let mut next =
                Store::new_with_headroom(alias, id, Funding::Prepaid, Headroom::OnDemand)?;
            next.reserve(original as u32, 32)?;
            assert!(next.shrink_to_fit(original as u32)?);
            next.commit(original as u32)?;
        }
        29 => typed_pushes(&mut store)?,
        30 => {
            store.commit((original - 37) as u32)?;
            drop(store);
            let mut next =
                Store::new_with_headroom(alias, id, Funding::Prepaid, Headroom::OnDemand)?;
            // Saved original remains available, including space D16 conservatively misses.
            next.resize(original + INCREASE)?;
            assert_eq!(
                next.resize(original + INCREASE + 1),
                Err(ErrorKind::GrowthLimit.into())
            );
            next.commit(original as u32)?;
        }
        31 => {
            store.resize(original + 8)?;
            store.set_poisoned(true);
            assert_eq!(
                store.commit(original as u32),
                Err(ErrorKind::Poisoned.into())
            );
            assert!(store.is_closed());
            assert!(!alias.is_borrowed());
            alias.resize(original)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

#[inline(never)]
fn typed_pushes(store: &mut Store<'_>) -> ProgramResult {
    store.bytes_mut().fill(0);
    let mut root = Demand::initialize(store, DemandInit { values: &[] })?;
    for i in 0..50 {
        push_one(&mut root, i)?;
    }
    for (i, value) in root.values().into_iter().enumerate() {
        assert_eq!(value, i as u64);
    }
    root.commit()?;
    Ok(())
}

#[inline(never)]
fn push_one(root: &mut DemandMut<'_>, value: u64) -> blubber::Result<()> {
    root.values_mut().push(value)
}
