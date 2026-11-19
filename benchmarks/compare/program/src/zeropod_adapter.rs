//! Validated borrowed reads; root replacement for Vec edits this release lacks.
use super::*;
use pinocchio::Resize;
use zeropod::{Edit, ZeroPod};
pub fn run(state: &mut AccountView, w: u8, op: u8, n: usize) -> Result<u64, ProgramError> {
    let read = op == 0 || matches!((w, op), (1, 1 | 2) | (2, 1 | 5) | (3, 1) | (4, 1));
    if read {
        let bytes = state.try_borrow()?;
        let mut out = 0;
        match w {
            1 => {
                let v = zeropod::read::<Vec<u64>>(&bytes).map_err(error)?;
                if op == 1 {
                    out = v.iter().sum();
                } else if op == 2 {
                    out = v.get(n / 2).unwrap_or(0);
                }
            }
            2 => {
                let v = zeropod::read::<BTreeMap<Key, u64>>(&bytes).map_err(error)?;
                if op == 1 {
                    // Fixed-width entries support constant-time indexed access.
                    // Like the borrowed map loader, this assumes Borsh key order.
                    let target = middle_key(n);
                    let mut lo = 0;
                    let mut hi = v.len();
                    while lo < hi {
                        let mid = lo + (hi - lo) / 2;
                        let (key, value) = v.get(mid).unwrap();
                        match key.iter().cmp(target) {
                            core::cmp::Ordering::Less => lo = mid + 1,
                            core::cmp::Ordering::Greater => hi = mid,
                            core::cmp::Ordering::Equal => {
                                out = value;
                                break;
                            }
                        }
                    }
                } else if op == 5 {
                    out = v.iter().map(|(_, v)| v).sum();
                }
            }
            3 => {
                let v = zeropod::read::<Vec<String>>(&bytes).map_err(error)?;
                if op == 1 {
                    out = v.iter().map(text_sum).sum();
                }
            }
            4 => {
                let v = zeropod::read::<BTreeMap<Key, String>>(&bytes).map_err(error)?;
                if op == 1 {
                    out = v
                        .iter()
                        .find(|(k, _)| k.iter().eq(middle_key(n)))
                        .map(|(_, v)| text_sum(v))
                        .unwrap_or(0);
                }
            }
            _ => unreachable!(),
        }
        return Ok(out);
    }
    macro_rules! edit {
        ($ty:ty,$op:expr) => {{
            let mut value = {
                let bytes = state.try_borrow()?;
                let view = zeropod::read::<$ty>(&bytes).map_err(error)?;
                <$ty>::own(view)
            };
            let out = if op == 10 { 0 } else { $op(&mut value) };
            if op != 10 {
                let len = <$ty>::encoded_len(&value.input(), &[]).map_err(error)?;
                if len > state.data_len() {
                    state.resize(len)?;
                }
                {
                    let mut bytes = state.try_borrow_mut()?;
                    let mut edit = Edit::<$ty>::view(&mut bytes).map_err(error)?;
                    edit.set(value.input()).map_err(error)?;
                }
                state.resize(len)?;
            }
            Ok(out)
        }};
    }
    match w {
        1 => edit!(Vec<u64>, |v| owned::list_op(v, op)),
        2 => edit!(BTreeMap<Key,u64>,|v|owned::map_op(v,op,n)),
        3 => edit!(Vec<String>, |v: &mut Vec<String>| owned::words_op(v, op)),
        4 => edit!(BTreeMap<Key,String>,|v|owned::word_map_op(v,op,n)),
        _ => unreachable!(),
    }
}
