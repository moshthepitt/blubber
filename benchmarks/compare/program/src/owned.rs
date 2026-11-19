//! Whole-account owned decode/edit/encode, as in Anchor 1.x Account<T>.
use super::*;
use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::Resize;

pub fn list_op(v: &mut Vec<u64>, op: u8) -> u64 {
    match op {
        0 => 0,
        1 => v.iter().copied().sum(),
        2 => v.get(v.len() / 2).copied().unwrap_or(0),
        3 => {
            v.fill(SET);
            0
        }
        4 => {
            v.push(SET);
            0
        }
        5 => {
            if !v.is_empty() {
                v.remove(0);
            }
            0
        }
        _ => panic!("op"),
    }
}
pub fn map_op(v: &mut BTreeMap<Key, u64>, op: u8, n: usize) -> u64 {
    match op {
        0 => 0,
        1 => v.get(&middle_key(n)).copied().unwrap_or(0),
        2 => {
            v.insert(insert_key(n), SET);
            0
        }
        3 => {
            v.remove(&middle_key(n));
            0
        }
        4 => {
            for x in v.values_mut() {
                *x = SET;
            }
            0
        }
        5 => v.values().copied().sum(),
        _ => panic!("op"),
    }
}
pub fn words_op(v: &mut [String], op: u8) -> u64 {
    let mid = v.len() / 2;
    match op {
        0 => 0,
        1 => v.iter().map(|s| text_sum(s)).sum(),
        2 => {
            if let Some(s) = v.get_mut(mid) {
                *s = EQUAL.into();
            }
            0
        }
        3 => {
            if let Some(s) = v.get_mut(mid) {
                *s = GROWN.into();
            }
            0
        }
        4 => {
            for s in v {
                *s = GROWN.into();
            }
            0
        }
        5 => {
            for s in v {
                let next = grow(s);
                *s = core::str::from_utf8(&next).unwrap().into();
            }
            0
        }
        _ => panic!("op"),
    }
}
pub fn word_map_op(v: &mut BTreeMap<Key, String>, op: u8, n: usize) -> u64 {
    match op {
        0 => 0,
        1 => v.get(&middle_key(n)).map(|s| text_sum(s)).unwrap_or(0),
        2 => {
            v.insert(insert_key(n), EQUAL.into());
            0
        }
        3 => {
            for s in v.values_mut() {
                let next = grow(s);
                *s = core::str::from_utf8(&next).unwrap().into();
            }
            0
        }
        _ => panic!("op"),
    }
}
fn load<T: BorshDeserialize>(state: &AccountView) -> Result<T, ProgramError> {
    T::deserialize(&mut &state.try_borrow()?[..]).map_err(error)
}
fn save<T: BorshSerialize>(state: &mut AccountView, value: &T) -> ProgramResult {
    let len = borsh::object_length(value).map_err(error)?;
    state.resize(len)?;
    value
        .serialize(&mut &mut state.try_borrow_mut()?[..])
        .map_err(error)
}
pub fn borsh_run(state: &mut AccountView, w: u8, op: u8, n: usize) -> Result<u64, ProgramError> {
    macro_rules! go {
        ($ty:ty,$f:expr) => {{
            let mut v = load::<$ty>(state)?;
            let out = if op == 10 { 0 } else { $f(&mut v) };
            if !matches!(op, 0 | 10)
                && !matches!((w, op), (1, 1 | 2) | (2, 1 | 5) | (3, 1) | (4, 1))
            {
                save(state, &v)?;
            }
            Ok(out)
        }};
    }
    match w {
        1 => go!(Vec<u64>, |v| list_op(v, op)),
        2 => go!(BTreeMap<Key,u64>,|v|map_op(v,op,n)),
        3 => go!(Vec<String>, |v: &mut Vec<String>| words_op(v, op)),
        4 => go!(BTreeMap<Key,String>,|v|word_map_op(v,op,n)),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
