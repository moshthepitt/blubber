//! Isolated competitor fixtures. No dependency points from blubber into here.
#![cfg_attr(all(target_os = "solana", not(feature = "baselines")), no_std)]
#![deny(unsafe_code)]
extern crate alloc;
use alloc::{collections::BTreeMap, string::String, vec::Vec};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};
#[cfg(feature = "anchor-slab")]
#[allow(unsafe_code)] // Upstream bytemuck derive expansions implement unsafe marker traits.
pub mod anchor_slab_adapter;
pub mod blubber_adapter;
pub mod blubber_checked_adapter;
#[cfg(feature = "baselines")]
pub mod bytemuck_adapter;
#[cfg(feature = "baselines")]
pub mod critbit_adapter;
#[cfg(feature = "light")]
pub mod light_adapter;
#[cfg(feature = "manifest")]
#[allow(unsafe_code)] // Upstream bytemuck derive expansions implement unsafe marker traits.
pub mod manifest_adapter;
pub mod owned;
#[cfg(feature = "pinapod")]
#[allow(unsafe_code)] // Upstream PinaPod derive expansions implement its unsafe traits.
pub mod pinapod_adapter;
#[cfg(feature = "baselines")]
pub mod sokoban_adapter;
#[cfg(feature = "wincode")]
#[allow(unsafe_code)] // Upstream SchemaRead derive implements its unsafe traits.
pub mod wincode_adapter;
#[cfg(feature = "zeropod")]
pub mod zeropod_adapter;
#[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
pinocchio::program_entrypoint!(run, 1);
#[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
pinocchio::default_allocator!();
#[cfg(all(
    target_os = "solana",
    not(feature = "baselines"),
    not(feature = "no-entrypoint")
))]
pinocchio::nostd_panic_handler!();
#[cfg(all(
    target_os = "solana",
    feature = "baselines",
    not(feature = "no-entrypoint")
))]
pinocchio::default_panic_handler!();

pub const CAPACITY: usize = 1000;
pub const SET: u64 = 123;
pub const EQUAL: &str = "sameSIZE";
pub const GROWN: &str = "replacement grows";
pub type Key = [u8; 32];

pub fn key(i: usize) -> Key {
    let mut key = [42; 32];
    key[..4].copy_from_slice(&(2 * (i + 1) as u32).to_be_bytes());
    key
}
pub fn middle_key(n: usize) -> Key {
    key(n / 2)
}
pub fn insert_key(n: usize) -> Key {
    let mut k = key(n / 2);
    let x = u32::from_be_bytes(k[..4].try_into().unwrap()) - 1;
    k[..4].copy_from_slice(&x.to_be_bytes());
    k
}
pub fn word(i: usize) -> String {
    alloc::format!("{i:08x}")
}
pub fn grow(old: &str) -> [u8; 17] {
    let mut next = [b'!'; 17];
    next[..8].copy_from_slice(old.as_bytes());
    next[8..16].copy_from_slice(old.as_bytes());
    next
}
pub fn text_sum(s: &str) -> u64 {
    s.bytes().map(u64::from).sum()
}
pub fn numbers(n: usize) -> Vec<u64> {
    (0..n).map(|i| i as u64 + 1).collect()
}
pub fn number_map(n: usize) -> BTreeMap<Key, u64> {
    (0..n).map(|i| (key(i), i as u64 + 1)).collect()
}
pub fn words(n: usize) -> Vec<String> {
    (0..n).map(word).collect()
}
pub fn word_map(n: usize) -> BTreeMap<Key, String> {
    (0..n).map(|i| (key(i), word(i))).collect()
}
pub fn error<E>(_: E) -> ProgramError {
    ProgramError::InvalidAccountData
}
pub fn ops(w: u8) -> &'static [u8] {
    match w {
        1 | 2 => &[1, 2, 3, 4, 5],
        3 => &[1, 2, 3, 4, 5],
        4 => &[1, 2, 3],
        _ => &[],
    }
}
pub fn op_name(w: u8, op: u8) -> &'static str {
    match (w, op) {
        (_, 0) => "idle",
        (1, 1) | (3, 1) => "read_all",
        (1, 2) => "get_middle",
        (1, 3) | (2, 4) => "set_all",
        (1, 6) => "set_all_fill",
        (1, 4) => "push",
        (1, 5) => "remove_front",
        (2, 1) | (4, 1) => "lookup",
        (2, 2) | (4, 2) => "insert_middle",
        (2, 3) => "remove",
        (2, 5) => "iterate_sum",
        (3, 2) => "set_equal",
        (3, 3) => "grow_one",
        (3, 4) => "grow_all",
        (3, 5) | (4, 3) => "grow_all_old",
        _ => "unknown",
    }
}
pub fn run(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    let [lib, w, op, lo, hi] = ix else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let n = u16::from_le_bytes([*lo, *hi]) as usize;
    if n > CAPACITY || state.owner() != id || !state.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }
    let checksum = match lib {
        0 => owned::borsh_run(state, *w, *op, n)?,
        1 => blubber_adapter::run::<false>(id, state, *w, *op, n)?,
        13 => blubber_adapter::run::<true>(id, state, *w, *op, n)?,
        14 => blubber_checked_adapter::run::<false>(id, state, *w, *op, n)?,
        #[cfg(feature = "baselines")]
        3 => bytemuck_adapter::run(state, *w, *op)?,
        #[cfg(feature = "baselines")]
        4 => sokoban_adapter::run(state, *w, *op, n)?,
        #[cfg(feature = "zeropod")]
        5 => zeropod_adapter::run(state, *w, *op, n)?,
        #[cfg(feature = "wincode")]
        6 => wincode_adapter::run(state, *w, *op, n)?,
        #[cfg(feature = "pinapod")]
        7 => pinapod_adapter::run(state, *w, *op, n)?,
        #[cfg(feature = "light")]
        8 => light_adapter::run(state, *w, *op)?,
        #[cfg(feature = "anchor-slab")]
        9 => anchor_slab_adapter::run(state, *op)?,
        #[cfg(feature = "baselines")]
        10 => critbit_adapter::run(state, *op, n)?,
        #[cfg(feature = "manifest")]
        11 | 12 => manifest_adapter::run(state, *op, n, *lib == 12)?,
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    #[cfg(all(feature = "heap", target_os = "solana"))]
    {
        // A one-byte allocation reports the forward bump allocator's high water
        // through a safe pointer-to-integer conversion. Separate diagnostic ELF.
        let sentinel = Vec::<u8>::with_capacity(1);
        let heap =
            sentinel.as_ptr() as usize - pinocchio::entrypoint::HEAP_START_ADDRESS as usize - 8;
        let mut out = [0; 16];
        out[..8].copy_from_slice(&checksum.to_le_bytes());
        out[8..].copy_from_slice(&(heap as u64).to_le_bytes());
        pinocchio::cpi::set_return_data(&out);
    }
    #[cfg(not(all(feature = "heap", target_os = "solana")))]
    pinocchio::cpi::set_return_data(&checksum.to_le_bytes());
    Ok(())
}
