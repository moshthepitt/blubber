//! Inspect expected post-execution bank rent failures without disabling checks.
#![forbid(unsafe_code)]

use mollusk_svm::{
    result::{Check, InstructionResult},
    Mollusk,
};
use solana_account::Account;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

pub(super) fn rejected(
    mollusk: &mut Mollusk,
    instruction: &Instruction,
    accounts: &[(Pubkey, Account)],
    cpis: usize,
) -> InstructionResult {
    // Return the bank error so instruction-local assertions can be inspected.
    // Rent validation stays enabled; all result checks below remain mandatory.
    assert!(mollusk.config.rent_exempt_checks);
    let panic = mollusk.config.panic;
    mollusk.config.panic = false;
    let result = mollusk.process_instruction(instruction, accounts);
    mollusk.config.panic = panic;
    assert_eq!(result.raw_result, Ok(()), "instruction-local assertions");
    assert!(result.run_checks(
        &[
            Check::err(pinocchio::error::ProgramError::AccountNotRentExempt),
            Check::inner_instruction_count(cpis),
        ],
        &mollusk.config,
    ));
    // Result accounts expose instruction output, not committed bank state.
    result
}
