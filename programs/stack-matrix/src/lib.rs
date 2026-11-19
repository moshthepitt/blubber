//! SBF stack acceptance across schema depth, fixed width and edit families.

#![cfg_attr(target_os = "solana", no_std)]
#![forbid(unsafe_code)]

// The overflow control deliberately omits the typed fixture call graph.
#[cfg(feature = "overflow")]
use blubber as _;

use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

#[cfg(target_os = "solana")]
pinocchio::program_entrypoint!(process_instruction, 1);
#[cfg(all(target_os = "solana", not(feature = "alloc")))]
pinocchio::no_allocator!();
#[cfg(all(target_os = "solana", feature = "alloc"))]
pinocchio::default_allocator!();
#[cfg(target_os = "solana")]
pinocchio::nostd_panic_handler!();

/// Runs one family against a nested schema, or the intentional overflow control.
pub fn process_instruction(id: &Address, accounts: &mut [AccountView], ix: &[u8]) -> ProgramResult {
    #[cfg(feature = "overflow")]
    {
        let _ = (id, accounts);
        let mut frame = [0u8; 8192];
        frame.fill(ix.first().copied().unwrap_or(1));
        core::hint::black_box(&mut frame);
        if frame[4096] == 255 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
    #[cfg(all(not(feature = "overflow"), feature = "constraint-prefix"))]
    {
        constraint_prefix::run(id, accounts, ix)
    }
    #[cfg(not(any(feature = "overflow", feature = "constraint-prefix")))]
    fixture::run(id, accounts, ix)
}

#[cfg(all(not(feature = "overflow"), feature = "constraint-prefix"))]
mod constraint_prefix;
#[cfg(not(any(feature = "overflow", feature = "constraint-prefix")))]
mod fixture;
