//! Sokoban critbit: fixed 1000 leaves, with an injective workload-only u128 key.
use super::*;
use sokoban::{Critbit, FromSlice, NodeAllocatorMap, ZeroCopy};
pub type Tree = Critbit<u64, 2000, CAPACITY>;
fn project(key: Key) -> u128 {
    u128::from_be_bytes(key[..16].try_into().unwrap())
}
fn expand(key: u128) -> Key {
    let mut full = [42; 32];
    full[..16].copy_from_slice(&key.to_be_bytes());
    full
}
pub fn run(state: &mut AccountView, op: u8, n: usize) -> Result<u64, ProgramError> {
    let mut bytes = state.try_borrow_mut()?;
    let tree = Tree::load_mut_bytes(&mut bytes).ok_or(ProgramError::InvalidAccountData)?;
    let mut out = 0;
    match op {
        0 | 10 => {}
        1 => out = tree.get(&project(middle_key(n))).copied().unwrap_or(0),
        2 => {
            tree.insert(project(insert_key(n)), SET)
                .ok_or(ProgramError::AccountDataTooSmall)?;
        }
        3 => {
            tree.remove(&project(middle_key(n)));
        }
        4 => {
            for (_, value) in tree.iter_mut() {
                *value = SET;
            }
        }
        5 => out = tree.iter().map(|(_, value)| *value).sum(),
        _ => unreachable!(),
    }
    Ok(out)
}
pub fn encode(n: usize) -> Vec<u8> {
    let mut bytes = alloc::vec![0;core::mem::size_of::<Tree>()];
    let tree = Tree::new_from_slice(&mut bytes);
    for i in 0..n {
        tree.insert(project(key(i)), i as u64 + 1).unwrap();
    }
    bytes
}
pub fn decode(bytes: &[u8]) -> BTreeMap<Key, u64> {
    Tree::load_bytes(bytes)
        .unwrap()
        .iter()
        .map(|(key, value)| (expand(*key), *value))
        .collect()
}
