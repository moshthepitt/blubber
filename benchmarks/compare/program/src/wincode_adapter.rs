//! Recommended bulk decode for owned collections; borrowed numeric slice reads.
use super::*;
use pinocchio::Resize;
#[derive(wincode::SchemaRead)]
#[repr(C)]
struct NumberEntry {
    key: Key,
    value: u64,
}
fn load<T>(state: &AccountView) -> Result<T, ProgramError>
where
    for<'a> T: wincode::SchemaRead<'a, wincode::config::DefaultConfig, Dst = T>,
{
    wincode::deserialize::<T>(&state.try_borrow()?).map_err(error)
}
fn save<T: wincode::SchemaWrite<wincode::config::DefaultConfig, Src = T>>(
    state: &mut AccountView,
    value: &T,
) -> ProgramResult {
    let len = wincode::serialized_size(value).map_err(error)? as usize;
    state.resize(len)?;
    wincode::serialize_into(&mut &mut state.try_borrow_mut()?[..], value).map_err(error)
}
pub fn run(state: &mut AccountView, w: u8, op: u8, n: usize) -> Result<u64, ProgramError> {
    if w == 2 && matches!(op, 0 | 1 | 5) {
        // BTreeMap's count + contiguous entries share the documented sequence
        // encoding. Borrowing does not validate sorted order (recorded separately).
        let bytes = state.try_borrow()?;
        let v = wincode::deserialize::<&[NumberEntry]>(&bytes).map_err(error)?;
        return Ok(match op {
            1 => v
                .binary_search_by_key(&middle_key(n), |e| e.key)
                .ok()
                .map(|i| v[i].value)
                .unwrap_or(0),
            5 => v.iter().map(|e| e.value).sum(),
            _ => 0,
        });
    }
    if w == 2 && matches!(op, 4 | 11) {
        let mut bytes = state.try_borrow_mut()?;
        let v = wincode::deserialize_mut::<&mut [NumberEntry]>(&mut bytes).map_err(error)?;
        if op == 4 {
            for e in v {
                e.value = SET;
            }
        }
        return Ok(0);
    }
    if w == 3 && matches!(op, 0 | 1) {
        let bytes = state.try_borrow()?;
        let v = wincode::deserialize::<Vec<&str>>(&bytes).map_err(error)?;
        return Ok(if op == 1 {
            v.iter().map(|s| text_sum(s)).sum()
        } else {
            0
        });
    }
    if w == 4 && matches!(op, 0 | 1) {
        let bytes = state.try_borrow()?;
        let v = wincode::deserialize::<BTreeMap<Key, &str>>(&bytes).map_err(error)?;
        return Ok(if op == 1 {
            v.get(&middle_key(n)).map(|s| text_sum(s)).unwrap_or(0)
        } else {
            0
        });
    }
    if w == 1 && matches!(op, 3 | 11) {
        let mut bytes = state.try_borrow_mut()?;
        let v = wincode::deserialize_mut::<&mut [u64]>(&mut bytes).map_err(error)?;
        if op == 3 {
            v.fill(SET);
        }
        return Ok(0);
    }
    if w == 1 && matches!(op, 0..=2) {
        let bytes = state.try_borrow()?;
        let v = wincode::deserialize::<&[u64]>(&bytes).map_err(error)?;
        return Ok(match op {
            1 => v.iter().sum(),
            2 => v.get(n / 2).copied().unwrap_or(0),
            _ => 0,
        });
    }
    macro_rules! go {
        ($ty:ty,$f:expr) => {{
            let mut v = load::<$ty>(state)?;
            let out = if op == 10 { 0 } else { $f(&mut v) };
            if !matches!(op, 0 | 10) && !matches!((w, op), (2, 1 | 5) | (3, 1) | (4, 1)) {
                save(state, &v)?;
            }
            Ok(out)
        }};
    }
    match w {
        1 => go!(Vec<u64>, |v| owned::list_op(v, op)),
        2 => go!(BTreeMap<Key,u64>,|v|owned::map_op(v,op,n)),
        3 => go!(Vec<String>, |v: &mut Vec<String>| owned::words_op(v, op)),
        4 => go!(BTreeMap<Key,String>,|v|owned::word_map_op(v,op,n)),
        _ => unreachable!(),
    }
}
