//! Light's documented runtime-capacity vector. Capacity remains 1000.
use super::*;
use light_zero_copy::vec::ZeroCopyVecU64;
pub fn run(state: &mut AccountView, w: u8, op: u8) -> Result<u64, ProgramError> {
    if w != 1 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let mut bytes = state.try_borrow_mut()?;
    let mut v = ZeroCopyVecU64::<u64>::from_bytes(&mut bytes).map_err(error)?;
    let out = match op {
        0 | 10 => 0,
        1 => v.as_slice().iter().sum(),
        2 => v.get(v.len() / 2).copied().unwrap_or(0),
        3 => {
            v.as_mut_slice().fill(SET);
            0
        }
        4 => {
            v.push(SET).map_err(|_| ProgramError::AccountDataTooSmall)?;
            0
        }
        5 => {
            let rest = v.as_slice().get(1..).unwrap_or(&[]).to_vec();
            v.clear();
            for value in rest {
                v.push(value).map_err(error)?;
            }
            0
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    Ok(out)
}
pub fn encode(n: usize) -> Vec<u8> {
    let mut bytes =
        alloc::vec![0;ZeroCopyVecU64::<u64>::required_size_for_capacity(CAPACITY as u64)];
    let mut v = ZeroCopyVecU64::<u64>::new(CAPACITY as u64, &mut bytes).unwrap();
    for x in numbers(n) {
        v.push(x).unwrap();
    }
    bytes
}
pub fn decode(bytes: &[u8]) -> Vec<u64> {
    let mut b = bytes.to_vec();
    ZeroCopyVecU64::<u64>::from_bytes(&mut b)
        .unwrap()
        .as_slice()
        .to_vec()
}
