//! Fresh-instruction compute fixtures with the ordinary no-heap account API.
#![cfg_attr(all(target_os = "solana", not(feature = "policy")), no_std)]
#![forbid(unsafe_code)]

/// Representative integer tables and their canonical digests.
pub mod digests;
/// Entry combinator observations and explicit-match controls.
#[cfg(any(feature = "entry-combinators", not(target_os = "solana")))]
pub mod entry_combinators;
/// D21 canonical identity vectors.
#[cfg(any(feature = "identity", not(target_os = "solana")))]
pub mod identity;
/// Raw storage lifecycle scenarios for scaling reports.
#[cfg(feature = "matrix")]
pub mod lease;
/// Scaling scenarios shared with the existing compute-unit bencher.
#[cfg(any(feature = "matrix", not(target_os = "solana")))]
pub mod matrix;
/// Alternate ordinary flat dispatch from the reviewer probe.
#[cfg(feature = "mixed-alternate")]
pub mod mixed_alternate;
/// Ordinary callers sharing iteration, callbacks, cursors and batch edits.
#[cfg(feature = "mixed-operations")]
pub mod mixed_operations;
/// D21 checked policy and verification fixtures.
#[cfg(any(
    feature = "policy",
    feature = "policy-pinocchio",
    not(target_os = "solana")
))]
pub mod policy;
/// Q8 byte-kernel measurements, separate from public typed API measurements.
#[cfg(feature = "rewrite-kernel")]
pub mod rewrite;
#[cfg(feature = "rewrite-api")]
pub mod rewrite_api;
/// Shared concrete account schemas.
pub mod schemas;
/// Isolated D20 traversal instructions and measurement inputs.
#[cfg(any(feature = "traversal", not(target_os = "solana")))]
pub mod traversal;

use blubber::pinocchio::{Funding, Headroom, Store};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};
use schemas::{AddressMap, Number, Numbers, PublicKeyMap, Text};

#[cfg(all(
    target_os = "solana",
    not(feature = "matrix"),
    not(feature = "traversal"),
    not(feature = "mixed-operations")
))]
pinocchio::program_entrypoint!(process_instruction, 1);
#[cfg(all(
    target_os = "solana",
    feature = "matrix",
    not(feature = "traversal"),
    not(feature = "mixed-operations")
))]
pinocchio::program_entrypoint!(process_instruction, 4);
#[cfg(all(
    target_os = "solana",
    feature = "mixed-operations",
    not(feature = "mixed-alternate")
))]
pinocchio::program_entrypoint!(mixed_operations::run, 1);
#[cfg(all(target_os = "solana", feature = "mixed-alternate"))]
pinocchio::program_entrypoint!(mixed_alternate::run, 1);
#[cfg(all(
    target_os = "solana",
    feature = "traversal",
    not(feature = "mixed-operations")
))]
pinocchio::program_entrypoint!(traversal::run, 1);
#[cfg(all(target_os = "solana", not(feature = "alloc")))]
pinocchio::no_allocator!();
#[cfg(all(target_os = "solana", feature = "alloc"))]
pinocchio::default_allocator!();
#[cfg(all(target_os = "solana", not(feature = "policy")))]
pinocchio::nostd_panic_handler!();

/// Runs one edit with Store/open/load/commit included in the measured instruction.
/// Opcodes: scalar 0, strings 1..=3, list 4, Pubkey map 5..=8, Address map 9..=12.
/// Validated-default scalar and string edits use 30 and 31.
pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    #[cfg(any(feature = "entry-combinators", not(target_os = "solana")))]
    if let Some((&90, data)) = instruction_data.split_first() {
        return entry_combinators::run(program_id, accounts, data);
    }
    #[cfg(any(
        feature = "policy",
        feature = "policy-pinocchio",
        not(target_os = "solana")
    ))]
    if let [33, op] = instruction_data {
        return policy::run(program_id, accounts, *op);
    }
    #[cfg(any(feature = "identity", not(target_os = "solana")))]
    if let [32, index] = instruction_data {
        let (_, discriminator, _, _, descriptor) =
            identity::vector(*index as usize).ok_or(ProgramError::InvalidInstructionData)?;
        let [account] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let mut output = account.try_borrow_mut()?;
        if output.len() != 8 + descriptor.len() {
            return Err(ProgramError::InvalidAccountData);
        }
        output[..8].copy_from_slice(&discriminator);
        output[8..].copy_from_slice(descriptor);
        return Ok(());
    }
    #[cfg(feature = "matrix")]
    if matches!(instruction_data.first(), Some(23..=29)) {
        return if instruction_data == [29] {
            matrix::scalar_kinds(program_id, accounts)
        } else if instruction_data.first() == Some(&28) {
            lease::run(program_id, accounts, instruction_data)
        } else {
            matrix::run(program_id, accounts, instruction_data)
        };
    }
    // Dynamic geometry keeps both directory passes in the measured SBF body.
    if instruction_data.first() == Some(&22) {
        let [state] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if instruction_data.len() != 25 {
            return Err(ProgramError::InvalidInstructionData);
        }
        let first = blubber_core::read_u32(instruction_data, 1)?;
        let stride = blubber_core::read_u32(instruction_data, 5)?;
        let count = blubber_core::read_u32(instruction_data, 9)?;
        let delta = i64::from_le_bytes(
            instruction_data[13..21]
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?,
        );
        let payload = blubber_core::read_u32(instruction_data, 21)?;
        let mut bytes = state.try_borrow_mut()?;
        blubber_core::raw::adjust_directory(&mut bytes, first, stride, count, delta, payload)?;
        return Ok(());
    }
    // Dynamic coordinates exercise the proved production movement on SBF.
    if let [21, a, b, c, d, e, f, g, h, i, j, k, l] = instruction_data {
        let [state] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let start = u32::from_le_bytes([*a, *b, *c, *d]);
        let end = u32::from_le_bytes([*e, *f, *g, *h]);
        let dest = u32::from_le_bytes([*i, *j, *k, *l]);
        let mut bytes = state.try_borrow_mut()?;
        blubber_core::raw::move_bytes(&mut bytes, blubber_core::Span { start, end }, dest)?;
        return Ok(());
    }
    if let [20, index, seed] = instruction_data {
        let [state] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let (_, table) =
            digests::table(usize::from(*index)).ok_or(ProgramError::InvalidInstructionData)?;
        let hash = digests::digest(table, *seed);
        state.try_borrow_mut()?.copy_from_slice(&hash.to_le_bytes());
        return Ok(());
    }
    let [opcode] = instruction_data else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    mark(10);
    mark(11);
    mark(0);
    // Exact-size inputs need explicit room for longer text, a list push or a map pair.
    let headroom = match *opcode {
        2 => Headroom::UpTo(text(2).len() - 8),
        4 => Headroom::UpTo(8),
        5..=12 => Headroom::UpTo(40),
        _ => Headroom::default(),
    };
    let mut store = Store::new_with_headroom(state, program_id, Funding::Prepaid, headroom)?;
    mark(1);
    match *opcode {
        0 => {
            let mut root = Number::load_trusted_mut(&mut store)?;
            mark(2);
            root.value_mut().set(123)?;
            mark(3);
            root.commit()?;
        }
        1..=3 => {
            let mut root = Text::load_trusted_mut(&mut store)?;
            mark(2);
            root.value_mut().set(text(*opcode))?;
            mark(3);
            root.commit()?;
        }
        4 => {
            let mut root = Numbers::load_trusted_mut(&mut store)?;
            mark(2);
            root.value_mut().push(100)?;
            mark(3);
            root.commit()?;
        }
        5..=8 => {
            let mut root = PublicKeyMap::load_trusted_mut(&mut store)?;
            mark(2);
            root.value_mut()
                .insert(blubber::Pubkey::from_bytes(key(*opcode - 5)), 123)?;
            mark(3);
            root.commit()?;
        }
        9..=12 => {
            let mut root = AddressMap::load_trusted_mut(&mut store)?;
            mark(2);
            root.value_mut()
                .insert(Address::new_from_array(key(*opcode - 9)), 123)?;
            mark(3);
            root.commit()?;
        }
        _ => validated(*opcode, &mut store)?,
    }
    mark(4);
    Ok(())
}

// Keep new validation fixtures off the existing instruction dispatch path.
#[inline(never)]
fn validated(opcode: u8, store: &mut Store<'_>) -> ProgramResult {
    match opcode {
        30 => {
            let mut root = Number::load_checked_mut(store)?;
            root.value_mut().set(123)?;
            root.commit()?;
        }
        31 => {
            let mut root = Text::load_checked_mut(store)?;
            root.value_mut().set(text(1))?;
            root.commit()?;
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    Ok(())
}

/// Replacement strings for same-length, growth and shrink cases.
pub fn text(opcode: u8) -> &'static str {
    match opcode {
        1 => "abcdefgh",
        2 => "0123456789abcdef0123456789abcdef",
        _ => "a",
    }
}

/// Append, middle, front and 31-byte common-prefix append keys.
pub fn key(case: u8) -> [u8; 32] {
    match case {
        0 => [255; 32],
        1 => [101; 32],
        2 => [0; 32],
        _ => {
            let mut key = [42; 32];
            key[31] = 255;
            key
        }
    }
}

#[inline(always)]
fn mark(_id: u64) {
    #[cfg(feature = "metrics")]
    {
        solana_program_log::log_64(0x4355, _id, 0, 0, 0);
        solana_program_log::log_compute_units();
    }
}
