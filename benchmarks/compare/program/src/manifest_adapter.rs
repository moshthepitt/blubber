//! Manifest's generic RB-tree and free list; allocation/header policy is caller code.
use super::*;
use core::cmp::Ordering;
use hypertree::{
    get_helper, get_mut_helper, validate_red_black_tree, FreeList, HyperTreeReadOperations,
    HyperTreeValueIteratorTrait, HyperTreeWriteOperations, RBNode, RedBlackTree,
    RedBlackTreeReadOnly, NIL,
};
use pinocchio::Resize;
pub const HEADER: usize = 32;
pub const SLOT: usize = 56;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Entry {
    key: Key,
    value: u64,
}
// Runtime account bases and the 32-byte header/56-byte slots preserve 8-byte alignment.
const _: () = assert!(
    core::mem::size_of::<RBNode<Entry>>() == SLOT && core::mem::align_of::<RBNode<Entry>>() <= 8
);
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}
impl core::fmt::Display for Entry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.value)
    }
}
#[derive(Clone, Copy)]
struct Header {
    root: u32,
    max: u32,
    free: u32,
    len: u32,
}
fn header(bytes: &[u8], checked: bool) -> Result<Header, ProgramError> {
    if bytes.len() < HEADER || !(bytes.len() - HEADER).is_multiple_of(SLOT) {
        return Err(ProgramError::InvalidAccountData);
    }
    let u = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    let h = Header {
        root: u(0),
        max: u(4),
        free: u(8),
        len: u(12),
    };
    let arena = &bytes[HEADER..];
    if h.len as usize > arena.len() / SLOT {
        return Err(ProgramError::InvalidAccountData);
    }
    for index in [h.root, h.max, h.free] {
        if index != NIL && (!(index as usize).is_multiple_of(SLOT) || index as usize >= arena.len())
        {
            return Err(ProgramError::InvalidAccountData);
        }
    }
    if checked {
        validate_red_black_tree::<Entry>(arena, h.root, h.max).map_err(error)?;
    }
    Ok(h)
}
fn save(bytes: &mut [u8], h: Header) {
    for (at, value) in [h.root, h.max, h.free, h.len].iter().enumerate() {
        bytes[at * 4..at * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
}
fn lookup(arena: &[u8], h: Header, key: Key) -> u32 {
    let tree = RedBlackTreeReadOnly::<Entry>::new(arena, h.root, h.max);
    tree.lookup_index(&Entry { key, value: 0 })
}
pub fn run(state: &mut AccountView, op: u8, n: usize, checked: bool) -> Result<u64, ProgramError> {
    if matches!(op, 0 | 1 | 5) {
        let bytes = state.try_borrow()?;
        let h = header(&bytes, checked)?;
        let arena = &bytes[HEADER..];
        return Ok(match op {
            1 => {
                let at = lookup(arena, h, middle_key(n));
                if at == NIL {
                    0
                } else {
                    get_helper::<RBNode<Entry>>(arena, at).get_value().value
                }
            }
            5 => {
                let tree = RedBlackTreeReadOnly::<Entry>::new(arena, h.root, h.max);
                tree.iter::<Entry>().map(|(_, v)| v.value).sum()
            }
            _ => 0,
        });
    }
    let mut h = {
        let bytes = state.try_borrow_mut()?;
        header(&bytes, checked)?
    };
    if op == 10 {
        return Ok(0);
    }
    let old_slots_bytes = state.data_len() - HEADER;
    let expanded = op == 2 && h.free == NIL;
    if expanded {
        state.resize(state.data_len() + SLOT)?;
    }
    let mut bytes = state.try_borrow_mut()?;
    let (prefix, arena) = bytes.split_at_mut(HEADER);
    match op {
        2 => {
            let mut free = FreeList::<[u8; 52]>::new(arena, h.free);
            if expanded {
                free.add(old_slots_bytes as u32);
            }
            let index = free.remove();
            h.free = free.get_head();
            let mut tree = RedBlackTree::<Entry>::new(arena, h.root, h.max);
            tree.insert(
                index,
                Entry {
                    key: insert_key(n),
                    value: SET,
                },
            );
            h.root = tree.get_root_index();
            h.max = tree.get_max_index();
            h.len += 1;
        }
        3 => {
            let index = lookup(arena, h, middle_key(n));
            if index != NIL {
                {
                    let mut tree = RedBlackTree::<Entry>::new(arena, h.root, h.max);
                    tree.remove_by_index(index);
                    h.root = tree.get_root_index();
                    h.max = tree.get_max_index();
                }
                let mut free = FreeList::<[u8; 52]>::new(arena, h.free);
                free.add(index);
                h.free = free.get_head();
                h.len -= 1;
            }
        }
        4 => {
            let mut index = h.max;
            while index != NIL {
                let next = {
                    let tree = RedBlackTreeReadOnly::<Entry>::new(arena, h.root, h.max);
                    tree.get_next_lower_index::<Entry>(index)
                };
                get_mut_helper::<RBNode<Entry>>(arena, index)
                    .get_mut_value()
                    .value = SET;
                index = next;
            }
        }
        _ => unreachable!(),
    }
    save(prefix, h);
    Ok(0)
}
pub fn encode(n: usize) -> Vec<u8> {
    let mut bytes = alloc::vec![0; HEADER+n*SLOT];
    let mut h = Header {
        root: NIL,
        max: NIL,
        free: NIL,
        len: n as u32,
    };
    let mut tree = RedBlackTree::<Entry>::new(&mut bytes[HEADER..], NIL, NIL);
    for i in 0..n {
        tree.insert(
            (i * SLOT) as u32,
            Entry {
                key: key(i),
                value: i as u64 + 1,
            },
        );
    }
    h.root = tree.get_root_index();
    h.max = tree.get_max_index();
    save(&mut bytes, h);
    bytes
}
pub fn decode(bytes: &[u8]) -> BTreeMap<Key, u64> {
    let h = header(bytes, true).unwrap();
    let tree = RedBlackTreeReadOnly::<Entry>::new(&bytes[HEADER..], h.root, h.max);
    let map: BTreeMap<_, _> = tree
        .iter::<Entry>()
        .map(|(_, v)| (v.key, v.value))
        .collect();
    assert_eq!(map.len(), h.len as usize);
    map
}
