//! Anchor v2's native checked slab, exact growth and retained capacity on removal.
#![forbid(unsafe_code)]

extern crate alloc;
use anchor_slab::{accounts::Slab, AnchorAccount, Discriminator, Owner};
use compare_program::{CAPACITY, SET};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

#[cfg(target_os = "solana")]
pinocchio::program_entrypoint!(run, 1);
#[cfg(target_os = "solana")]
pinocchio::default_allocator!();
#[cfg(target_os = "solana")]
pinocchio::default_panic_handler!();

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Header {}
impl Owner for Header {
    const OWNER: Address = Address::new_from_array([7; 32]);
}
impl Discriminator for Header {
    const DISCRIMINATOR: &'static [u8] = b"CMPSLAB!";
}
type Numbers = Slab<Header, u64>;

pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [9, 1, op, lo, hi] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let n = u16::from_le_bytes([*lo, *hi]) as usize;
    if n > CAPACITY || !state.owned_by(id) || !state.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }
    let checksum = operate(state, *op)?;
    #[cfg(all(feature = "heap", target_os = "solana"))]
    {
        let sentinel = alloc::vec::Vec::<u8>::with_capacity(1);
        let heap =
            sentinel.as_ptr() as usize - pinocchio::entrypoint::HEAP_START_ADDRESS as usize - 8;
        let mut out = [0; 16];
        out[..8].copy_from_slice(&checksum.to_le_bytes());
        out[8..].copy_from_slice(&(heap as u64).to_le_bytes());
        pinocchio::cpi::set_return_data(&out);
    }
    #[cfg(not(all(feature = "heap", target_os = "solana")))]
    pinocchio::cpi::set_return_data(&checksum.to_le_bytes());
    Ok(())
}

fn operate(state: &AccountView, op: u8) -> Result<u64, ProgramError> {
    if matches!(op, 0..=2) {
        let view = Numbers::load(*state)?;
        return Ok(match op {
            1 => view.as_slice().iter().sum(),
            2 => view.as_slice().get(view.len() / 2).copied().unwrap_or(0),
            _ => 0,
        });
    }
    let mut view = Numbers::load_mut(*state)?;
    match op {
        10 => {}
        3 => view.as_mut_slice().fill(SET),
        4 => {
            // The fixture prepaid rent. Native resize does not transfer lamports.
            view.resize_to_capacity((view.len() + 1) as u32)?;
            view.try_push(SET)?;
        }
        5 => {
            let len = view.len();
            if len > 0 {
                let entries = view.as_mut_slice();
                if let Some(tail) = entries.last().copied() {
                    entries.rotate_left(1);
                    // Truncation leaves the vacated slot in the account.
                    if let Some(slot) = entries.last_mut() {
                        *slot = tail;
                    }
                }
                view.truncate(len - 1);
            }
        }
        _ => unreachable!(),
    }
    Ok(0)
}
