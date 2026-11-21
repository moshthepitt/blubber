//! Creates, appends to and migrates a queue with rent paid by a signer.

#![cfg_attr(target_os = "solana", no_std)]
#![forbid(unsafe_code)]

use blubber::pinocchio::{EditOptions, Funding, Headroom, ReadStore};
use blubber::pinocchio_sdk;
use blubber::{ErrorKind, Schema, View};
use pinocchio_sdk::{
    account::MAX_PERMITTED_DATA_INCREASE,
    address::address_eq,
    error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    AccountView, Address, ProgramResult, Resize,
};
use pinocchio_system::instructions::Transfer;

#[cfg(target_os = "solana")]
pinocchio_sdk::program_entrypoint!(process_instruction, 4);
#[cfg(target_os = "solana")]
pinocchio_sdk::no_allocator!();
#[cfg(target_os = "solana")]
pinocchio_sdk::nostd_panic_handler!();

// docs:start schema
use blubber::List;

/// An append-only queue of numbers.
#[blubber::schema(account)]
pub struct Queue {
    /// Signer allowed to append.
    pub authority: Address,
    /// Values in insertion order.
    pub values: List<u64>,
}
// docs:end schema

// docs:start create
fn create(
    program_id: &Address,
    state: &mut AccountView,
    authority: &AccountView,
    payer: &AccountView,
    system_program: &AccountView,
) -> ProgramResult {
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Queue::create(
        state,
        payer,
        system_program,
        program_id,
        QueueInit {
            authority: *authority.address(),
            values: &[],
        },
    )?;
    Ok(())
}
// docs:end create

// docs:start push
fn push(
    program_id: &Address,
    state: &mut AccountView,
    authority: &AccountView,
    payer: &AccountView,
    system_program: &AccountView,
    value: u64,
) -> ProgramResult {
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let signer = *authority.address();
    // This program initializes the queue and is its only writer.
    Queue::with_edit_options(
        state,
        program_id,
        EditOptions {
            funding: Funding::TopUp {
                payer,
                system_program,
            },
            headroom: Headroom::UpTo(8), // One u64, with no spare bytes at opening.
            refund: None,
        },
        |queue| {
            if queue.try_authority()? != signer {
                return Err(ErrorKind::Unauthorized.into());
            }
            queue.values_mut().push(value)
        },
    )?;
    Ok(())
}
// docs:end push

// The first eight bytes of SHA-256("account:Queue") in the deployed format.
const BORSH_QUEUE: [u8; 8] = [0xcc, 0xa7, 0x06, 0xf7, 0x14, 0x21, 0x02, 0xbc];
const ENVELOPE_LEN: u32 = 12;
const LENGTH_WORD: u32 = ENVELOPE_LEN - BORSH_QUEUE.len() as u32;
const MAX_ACCOUNT_LEN: usize = 10 * 1024 * 1024;
const _: () = assert!(LENGTH_WORD as usize <= MAX_PERMITTED_DATA_INCREASE);

fn checked_migration_len(
    state: &AccountView,
    program_id: &Address,
    authority: &Address,
) -> blubber::Result<u32> {
    let old = ReadStore::new(state, program_id)?;
    let (identity, body) = old
        .bytes()
        .split_first_chunk::<8>()
        .ok_or(ErrorKind::Truncated)?;
    if *identity != BORSH_QUEUE {
        return Err(ErrorKind::Discriminator.into());
    }
    let (stored_authority, list) = body.split_first_chunk::<32>().ok_or(ErrorKind::Truncated)?;
    if *stored_authority != authority.to_bytes() {
        return Err(ErrorKind::Unauthorized.into());
    }
    let (count, items) = list.split_first_chunk::<4>().ok_or(ErrorKind::Truncated)?;
    let item_bytes = u32::from_le_bytes(*count)
        .checked_mul(8)
        .ok_or(ErrorKind::ArithmeticOverflow)?;
    if items.len() != item_bytes as usize {
        return Err(ProgramError::InvalidAccountData.into());
    }

    // Every u64 bit pattern is valid. The checked Borsh body can be the new payload.
    let new_len = old
        .bytes()
        .len()
        .checked_add(LENGTH_WORD as usize)
        .ok_or(ErrorKind::ArithmeticOverflow)?;
    if new_len > MAX_ACCOUNT_LEN {
        return Err(ErrorKind::Capacity.into());
    }
    Ok(new_len as u32)
}

fn migration_top_up(
    state: &AccountView,
    payer: &AccountView,
    system_program: &AccountView,
    new_len: u32,
) -> blubber::Result<u64> {
    let minimum = Rent::get()?.try_minimum_balance(new_len as usize)?;
    let top_up = minimum.saturating_sub(state.lamports());
    if top_up == 0 {
        return Ok(0);
    }
    if address_eq(state.address(), payer.address()) {
        return Err(ErrorKind::DuplicateAccount.into());
    }
    if !address_eq(system_program.address(), &pinocchio_system::ID)
        || !system_program.executable()
        || !payer.is_signer()
        || !payer.is_writable()
        || !payer.owned_by(&pinocchio_system::ID)
        || !payer.is_data_empty()
        || payer.executable()
    {
        return Err(ErrorKind::InvalidPayer.into());
    }
    payer.check_borrow_mut()?;
    if payer.lamports() < top_up {
        return Err(ErrorKind::RentShortfall.into());
    }
    Ok(top_up)
}

fn publish_queue(state: &mut AccountView, new_len: u32) -> blubber::Result<()> {
    let mut data = state.try_borrow_mut()?;
    let (identity, body) = data.split_at_mut(8);
    // The resize appended four zero bytes. Move the validated body right by four.
    body.rotate_right(LENGTH_WORD as usize);
    let (used_len, payload) = body.split_at_mut(LENGTH_WORD as usize);
    used_len.copy_from_slice(&new_len.to_le_bytes());
    View::new(payload, Queue::LAYOUT, ENVELOPE_LEN)?.validate()?;
    identity.copy_from_slice(&Queue::DISCRIMINATOR);
    Ok(())
}

// docs:start borsh-migrate
fn migrate(
    program_id: &Address,
    state: &mut AccountView,
    authority: &AccountView,
    payer: &AccountView,
    system_program: &AccountView,
) -> blubber::Result<()> {
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature.into());
    }
    if !state.is_writable() {
        return Err(ErrorKind::NotWritable.into());
    }
    let new_len = checked_migration_len(state, program_id, authority.address())?;
    let top_up = migration_top_up(state, payer, system_program, new_len)?;
    state.resize(new_len as usize)?;
    if top_up != 0 {
        Transfer {
            from: payer,
            to: state,
            lamports: top_up,
        }
        .invoke()?;
    }
    publish_queue(state, new_len)?;
    Queue::with_read_checked(state, program_id, |_| Ok(()))
}
// docs:end borsh-migrate

/// Takes the queue, authority, payer and System Program in that order.
///
/// Opcode 0 creates the queue and requires signatures from the queue address,
/// authority and payer. Opcode 1 appends one little-endian u64. Opcode 2
/// migrates a Borsh queue. Both require signatures from the authority and payer.
/// The queue and payer are writable.
pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    let [state, authority, payer, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    match data {
        [0] => create(program_id, state, authority, payer, system_program),
        [1, value @ ..] => {
            let bytes = value
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?;
            push(
                program_id,
                state,
                authority,
                payer,
                system_program,
                u64::from_le_bytes(bytes),
            )
        }
        [2] => migrate(program_id, state, authority, payer, system_program).map_err(Into::into),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
