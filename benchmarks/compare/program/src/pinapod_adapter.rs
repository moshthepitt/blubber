//! Compact PinaPod tails and the documented validated patch lifecycle.
use super::*;
use pinapod::pod::PodU64;
use pinapod::PinaPod;
use pinocchio::Resize;
#[derive(PinaPod)]
#[pinapod(compact)]
pub struct PinaNumbers {
    pub values: pinapod::Vec<u64, 1001>,
}
#[derive(PinaPod)]
#[pinapod(compact)]
pub struct PinaWords {
    pub values: pinapod::Vec<pinapod::String<17>, 1001>,
}
pub fn run(state: &mut AccountView, w: u8, op: u8, _n: usize) -> Result<u64, ProgramError> {
    match w {
        1 => {
            let mut values: Vec<PodU64>;
            {
                let bytes = state.try_borrow()?;
                let view = PinaNumbers::read_prefix(&bytes).map_err(error)?;
                let v = view.values();
                if matches!(op, 0..=2) {
                    return Ok(match op {
                        1 => v.iter().map(|x| x.get()).sum(),
                        2 => v.get(v.len() / 2).map(|x| x.get()).unwrap_or(0),
                        _ => 0,
                    });
                }
                if op == 10 {
                    return Ok(0);
                }
                values = if op == 3 {
                    alloc::vec![PodU64::from(SET); v.len()]
                } else {
                    v.to_vec()
                };
            }
            match op {
                3 => {}
                4 => values.push(SET.into()),
                5 => {
                    if !values.is_empty() {
                        values.remove(0);
                    }
                }
                _ => unreachable!(),
            }
            let patch = PinaNumbersPatch::new().replace_values(&values);
            let needed = {
                let bytes = state.try_borrow()?;
                PinaNumbers::updated_len(&bytes, &patch).map_err(error)?
            };
            if needed > state.data_len() {
                state.resize(needed)?;
            }
            let used = {
                let mut bytes = state.try_borrow_mut()?;
                PinaNumbers::update(&mut bytes, &patch).map_err(error)?
            };
            state.resize(used)?;
            Ok(0)
        }
        3 => {
            let mut values: Vec<pinapod::String<17>>;
            {
                let bytes = state.try_borrow()?;
                let view = PinaWords::read_prefix(&bytes).map_err(error)?;
                let v = view.values();
                if op == 0 {
                    return Ok(0);
                }
                if op == 1 {
                    return Ok(v.iter().map(|s| text_sum(s.as_str())).sum());
                }
                if op == 10 {
                    return Ok(0);
                }
                values = if op == 4 {
                    alloc::vec![pinapod::String::<17>::try_from(GROWN).map_err(error)?; v.len()]
                } else {
                    v.to_vec()
                };
            }
            let mid = values.len() / 2;
            match op {
                2 | 3 => {
                    if let Some(s) = values.get_mut(mid) {
                        s.try_set(if op == 2 { EQUAL } else { GROWN })
                            .map_err(error)?;
                    }
                }
                4 => {}
                5 => {
                    for s in &mut values {
                        let next = grow(s.as_str());
                        s.try_set(core::str::from_utf8(&next).unwrap())
                            .map_err(error)?;
                    }
                }
                _ => unreachable!(),
            }
            let patch = PinaWordsPatch::new().replace_values(&values);
            let needed = {
                let bytes = state.try_borrow()?;
                PinaWords::updated_len(&bytes, &patch).map_err(error)?
            };
            if needed > state.data_len() {
                state.resize(needed)?;
            }
            let used = {
                let mut bytes = state.try_borrow_mut()?;
                PinaWords::update(&mut bytes, &patch).map_err(error)?
            };
            state.resize(used)?;
            Ok(0)
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
pub fn encode(w: u8, n: usize) -> Vec<u8> {
    match w {
        1 => {
            let v: Vec<PodU64> = numbers(n).into_iter().map(Into::into).collect();
            let patch = PinaNumbersPatch::new().replace_values(&v);
            let mut bytes = alloc::vec![0;PinaNumbers::MAX_SIZE];
            let used = PinaNumbers::initialize(&mut bytes, &patch).unwrap();
            bytes.truncate(used);
            bytes
        }
        3 => {
            let v: Vec<_> = words(n)
                .iter()
                .map(|s| pinapod::String::<17>::try_from(s.as_str()).unwrap())
                .collect();
            let patch = PinaWordsPatch::new().replace_values(&v);
            let mut bytes = alloc::vec![0;PinaWords::MAX_SIZE];
            let used = PinaWords::initialize(&mut bytes, &patch).unwrap();
            bytes.truncate(used);
            bytes
        }
        _ => unreachable!(),
    }
}
pub fn decode_numbers(bytes: &[u8]) -> Vec<u64> {
    PinaNumbers::read_prefix(bytes)
        .unwrap()
        .values()
        .iter()
        .map(|x| x.get())
        .collect()
}
pub fn decode_words(bytes: &[u8]) -> Vec<String> {
    PinaWords::read_prefix(bytes)
        .unwrap()
        .values()
        .iter()
        .map(|s| s.as_str().into())
        .collect()
}
