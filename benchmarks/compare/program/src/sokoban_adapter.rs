//! Sokoban's red-black map at its normal POD load boundary (no graph scan).
use super::*;
use sokoban::{FromSlice, NodeAllocatorMap, RedBlackTree, ZeroCopy};
pub type Tree = RedBlackTree<Key, u64, CAPACITY>;
pub fn run(state: &mut AccountView, w: u8, op: u8, n: usize) -> Result<u64, ProgramError> {
    if w != 2 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let mut bytes = state.try_borrow_mut()?;
    let tree = Tree::load_mut_bytes(&mut bytes).ok_or(ProgramError::InvalidAccountData)?;
    let mut out = 0;
    match op {
        0 | 10 => {}
        1 => out = tree.get(&middle_key(n)).copied().unwrap_or(0),
        2 => {
            tree.insert(insert_key(n), SET)
                .ok_or(ProgramError::AccountDataTooSmall)?;
        }
        3 => {
            tree.remove(&middle_key(n));
        }
        4 => {
            for (_, v) in tree.iter_mut() {
                *v = SET;
            }
        }
        5 => out = tree.iter().map(|(_, v)| *v).sum(),
        _ => unreachable!(),
    }
    Ok(out)
}
pub fn encode(n: usize) -> Vec<u8> {
    let mut bytes = alloc::vec![0;core::mem::size_of::<Tree>()];
    let tree = Tree::new_from_slice(&mut bytes);
    for i in 0..n {
        tree.insert(key(i), i as u64 + 1).unwrap();
    }
    bytes
}
pub fn decode(bytes: &[u8]) -> BTreeMap<Key, u64> {
    Tree::load_bytes(bytes)
        .unwrap()
        .iter()
        .map(|(k, v)| (*k, *v))
        .collect()
}
