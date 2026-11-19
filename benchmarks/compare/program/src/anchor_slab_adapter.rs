//! Anchor v2's native checked slab, exact growth and retained capacity on removal.
use super::*;
use anchor_slab::{accounts::Slab, AnchorAccount, Discriminator, Owner};
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
pub fn run(state: &mut AccountView, op: u8) -> Result<u64, ProgramError> {
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
                view.as_mut_slice().copy_within(1.., 0);
                view.truncate(len - 1);
            }
        }
        _ => unreachable!(),
    }
    Ok(0)
}
