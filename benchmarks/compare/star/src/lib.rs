//! Star Frame's native AccountInfo and documented Shared/ExclusiveWrapper APIs.
#![deny(unsafe_code)]
extern crate alloc;
use compare_program::{grow, insert_key, middle_key, text_sum, Key, CAPACITY, EQUAL, GROWN, SET};
use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
fn error<E>(_: E) -> ProgramError {
    ProgramError::InvalidAccountData
}
#[cfg(target_os = "solana")]
pinocchio::program_entrypoint!(run, 1);
#[cfg(target_os = "solana")]
pinocchio_new::default_allocator!();
#[cfg(target_os = "solana")]
pinocchio_new::default_panic_handler!();
use star_frame::unsize::UnsizedType;
use star_frame::{
    data_types::PackedValue,
    unsize::{
        impls::{
            List, ListExclusiveImpl, Map, MapExclusiveImpl, UnsizedList, UnsizedListExclusiveImpl,
            UnsizedMap, UnsizedMapExclusiveImpl, UnsizedString, UnsizedStringExclusiveImpl,
        },
        wrapper::{ExclusiveWrapperTop, SharedWrapper},
    },
};
pub type NumList = List<PackedValue<u64>>;
pub type NumMap = Map<Key, PackedValue<u64>>;
pub type TextList = UnsizedList<UnsizedString>;
pub type TextMap = UnsizedMap<Key, UnsizedString>;

fn run(id: &Pubkey, accounts: &[AccountInfo], ix: &[u8]) -> ProgramResult {
    let [2, w, op, lo, hi] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let n = u16::from_le_bytes([*lo, *hi]) as usize;
    if n > CAPACITY || state.owner() != id || !state.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }
    let checksum = operate(state, *w, *op, n)?;
    #[cfg(all(feature = "heap", target_os = "solana"))]
    {
        let sentinel = alloc::vec::Vec::<u8>::with_capacity(1);
        let heap =
            sentinel.as_ptr() as usize - pinocchio_new::entrypoint::HEAP_START_ADDRESS as usize - 8;
        let mut out = [0; 16];
        out[..8].copy_from_slice(&checksum.to_le_bytes());
        out[8..].copy_from_slice(&(heap as u64).to_le_bytes());
        pinocchio_new::cpi::set_return_data(&out);
    }
    #[cfg(not(all(feature = "heap", target_os = "solana")))]
    pinocchio_new::cpi::set_return_data(&checksum.to_le_bytes());
    Ok(())
}
fn operate(
    state: &pinocchio::account_info::AccountInfo,
    w: u8,
    op: u8,
    n: usize,
) -> Result<u64, ProgramError> {
    let read = op == 0 || matches!((w, op), (1, 1 | 2) | (2, 1 | 5) | (3, 1) | (4, 1));
    let mut out = 0;
    {
        if read {
            match w {
                1 => {
                    let v = SharedWrapper::new::<NumList>(state).map_err(error)?;
                    if op == 1 {
                        out = v.iter().map(|x| x.0).sum();
                    } else if op == 2 {
                        out = v.get(n / 2).map(|x| x.0).unwrap_or(0);
                    }
                }
                2 => {
                    let v = SharedWrapper::new::<NumMap>(state).map_err(error)?;
                    if op == 1 {
                        out = v.get(&middle_key(n)).map(|x| x.0).unwrap_or(0);
                    } else if op == 5 {
                        out = v.iter().map(|(_, x)| x.0).sum();
                    }
                }
                3 => {
                    let v = SharedWrapper::new::<TextList>(state).map_err(error)?;
                    if op == 1 {
                        for s in v.iter() {
                            out += text_sum(s.map_err(error)?.as_str().map_err(error)?);
                        }
                    }
                }
                4 => {
                    let v = SharedWrapper::new::<TextMap>(state).map_err(error)?;
                    if op == 1 {
                        if let Some(s) = v.get(&middle_key(n)).map_err(error)? {
                            out = text_sum(s.as_str().map_err(error)?);
                        }
                    }
                }
                _ => unreachable!(),
            }
        } else {
            match w {
                1 => {
                    let mut v = ExclusiveWrapperTop::<NumList, _>::new(state).map_err(error)?;
                    match op {
                        10 => {}
                        3 => v.as_mut_slice().fill(SET.into()),
                        4 => v.push(SET.into()).map_err(error)?,
                        5 => {
                            if n > 0 {
                                v.remove(0).map_err(error)?;
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                2 => {
                    let mut v = ExclusiveWrapperTop::<NumMap, _>::new(state).map_err(error)?;
                    match op {
                        10 => {}
                        2 => {
                            v.insert(insert_key(n), SET.into()).map_err(error)?;
                        }
                        3 => {
                            v.remove(&middle_key(n)).map_err(error)?;
                        }
                        4 => {
                            for (_, x) in v.iter_mut() {
                                x.0 = SET;
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                3 => {
                    let mut v = ExclusiveWrapperTop::<TextList, _>::new(state).map_err(error)?;
                    match op {
                        10 => {}
                        2 | 3 => {
                            if n > 0 {
                                v.get_exclusive(n / 2)
                                    .map_err(error)?
                                    .unwrap()
                                    .set_from_owned(if op == 2 { EQUAL } else { GROWN }.to_string())
                                    .map_err(error)?;
                            }
                        }
                        4 => v
                            .set_from_owned((0..n).map(|_| GROWN.to_string()).collect())
                            .map_err(error)?,
                        5 => {
                            let mut values = TextList::owned_from_ptr(&v).map_err(error)?;
                            for s in &mut values {
                                let next = grow(s);
                                *s = core::str::from_utf8(&next).unwrap().into();
                            }
                            v.set_from_owned(values).map_err(error)?;
                        }
                        _ => unreachable!(),
                    }
                }
                4 => {
                    let mut v = ExclusiveWrapperTop::<TextMap, _>::new(state).map_err(error)?;
                    match op {
                        10 => {}
                        2 => {
                            v.insert(insert_key(n), star_frame::unsize::init::DefaultInit)
                                .map_err(error)?;
                            v.get_exclusive(&insert_key(n))
                                .map_err(error)?
                                .unwrap()
                                .set(EQUAL)
                                .map_err(error)?;
                        }
                        3 => {
                            let mut values = TextMap::owned_from_ptr(&v).map_err(error)?;
                            for s in values.values_mut() {
                                let next = grow(s);
                                *s = core::str::from_utf8(&next).unwrap().into();
                            }
                            v.set_from_owned(values).map_err(error)?;
                        }
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!(),
            }
        }
    }
    Ok(out)
}
