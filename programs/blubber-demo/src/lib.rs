//! Acceptance program for the Blubber account API.

#![cfg_attr(target_os = "solana", no_std)]
#![forbid(unsafe_code)]

/// The D13 account API acceptance exercise.
pub mod exercise;
/// Shared tour schema and borrowed initialization input.
pub mod state;

use blubber::pinocchio::{EditOptions, Funding, Headroom};
use blubber::pinocchio_sdk::{error::ProgramError, AccountView, Address, ProgramResult};
use state::{seed, MyAccount};

#[cfg(target_os = "solana")]
blubber::pinocchio_sdk::program_entrypoint!(process_instruction, 3);
#[cfg(target_os = "solana")]
blubber::pinocchio_sdk::no_allocator!();
#[cfg(target_os = "solana")]
blubber::pinocchio_sdk::nostd_panic_handler!();

/// Dispatches creation (0), read-only validation (1), or the edit tour (2).
///
/// Supply state, payer and System Program in that order. Creation requires
/// signatures from state and payer; editing requires the stored authority's
/// signature. The read-only instruction needs no signer or writable account.
pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [opcode] = instruction_data else {
        return Err(ProgramError::InvalidInstructionData);
    };
    let [state, payer, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if *opcode == 1 {
        return MyAccount::with_read_checked(state, program_id, |acct| {
            let _: &str = acct.note();
            let _: Option<u64> = acct.balances().get(&acct.authority());
            Ok(())
        })
        .map_err(Into::into);
    }
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let actor = *payer.address();
    match *opcode {
        0 => {
            let _receipt =
                MyAccount::create(state, payer, system_program, program_id, seed(actor))?;
        }
        2 => {
            let (_, _receipt) = MyAccount::with_edit_options(
                state,
                program_id,
                // The tour grows nested lists, maps and strings in one session.
                EditOptions {
                    funding: Funding::TopUp {
                        payer,
                        system_program,
                    },
                    headroom: Headroom::Max,
                    refund: None,
                },
                |acct| exercise::exercise(acct, actor),
            )?;
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    Ok(())
}
