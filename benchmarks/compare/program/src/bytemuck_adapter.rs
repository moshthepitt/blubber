//! Fixed capacity 1000; POD cast plus an explicit length bound.
use super::*;
pub fn run(state: &mut AccountView, w: u8, op: u8) -> Result<u64, ProgramError> {
    if w != 1 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let mut bytes = state.try_borrow_mut()?;
    let (head, body) = bytes.split_at_mut(8);
    let len = bytemuck::try_from_bytes_mut::<u64>(head).map_err(error)?;
    let v = bytemuck::try_cast_slice_mut::<u8, u64>(body).map_err(error)?;
    let n = *len as usize;
    if n > CAPACITY || v.len() != CAPACITY {
        return Err(ProgramError::InvalidAccountData);
    }
    let mut out = 0;
    match op {
        0 | 10 => {}
        1 => out = v[..n].iter().sum(),
        2 => out = v[..n].get(n / 2).copied().unwrap_or(0),
        3 => v[..n].fill(SET),
        4 => {
            if n == CAPACITY {
                return Err(ProgramError::AccountDataTooSmall);
            }
            v[n] = SET;
            *len += 1;
        }
        5 => {
            if n > 0 {
                v.copy_within(1..n, 0);
                *len -= 1;
            }
        }
        _ => unreachable!(),
    }
    Ok(out)
}
