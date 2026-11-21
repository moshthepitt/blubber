//! Stable faults and lossless SDK error conversions on their canonical domain.

use blubber::{Error, ErrorKind};
use solana_program_error::ProgramError;

const KINDS: [ErrorKind; 30] = [
    ErrorKind::Truncated,
    ErrorKind::Discriminator,
    ErrorKind::UsedLength,
    ErrorKind::ArithmeticOverflow,
    ErrorKind::InvalidBoundary,
    ErrorKind::InvalidBool,
    ErrorKind::InvalidUtf8,
    ErrorKind::InvalidTag,
    ErrorKind::UnsortedKeys,
    ErrorKind::DuplicateKey,
    ErrorKind::IndexOutOfBounds,
    ErrorKind::MissingKey,
    ErrorKind::MissingValue,
    ErrorKind::Utf8Boundary,
    ErrorKind::Capacity,
    ErrorKind::Allocation,
    ErrorKind::DepthExceeded,
    ErrorKind::WrongOwner,
    ErrorKind::NotWritable,
    ErrorKind::BorrowConflict,
    ErrorKind::GrowthLimit,
    ErrorKind::RentShortfall,
    ErrorKind::Unauthorized,
    ErrorKind::InvalidPayer,
    ErrorKind::DuplicateAccount,
    ErrorKind::AlreadyInitialized,
    ErrorKind::Closed,
    ErrorKind::Poisoned,
    ErrorKind::InvalidPlan,
    ErrorKind::InvalidSchema,
];

#[kani::proof]
#[kani::unwind(2)]
fn error_codes_faults() {
    let a: usize = kani::any();
    let b: usize = kani::any();
    kani::assume(a < KINDS.len() && b < KINDS.len());
    let offset: u32 = kani::any();
    let error = Error::new(KINDS[a], offset);
    assert_eq!(KINDS[a] as usize, a + 1);
    assert_eq!(error.custom_code(), Some(0xB101 + a as u32));
    assert_eq!(
        error.custom_code() == Error::from(KINDS[b]).custom_code(),
        a == b
    );
    let expected = match (offset, a + 1) {
        (Error::NO_OFFSET, 18) => ProgramError::InvalidAccountOwner,
        (Error::NO_OFFSET, 20) => ProgramError::AccountBorrowFailed,
        (Error::NO_OFFSET, 21) => ProgramError::InvalidRealloc,
        (Error::NO_OFFSET, 22) => ProgramError::AccountNotRentExempt,
        (Error::NO_OFFSET, 26) => ProgramError::AccountAlreadyInitialized,
        _ => ProgramError::Custom(0xB101 + a as u32),
    };
    assert_eq!(ProgramError::from(error), expected);
    assert_eq!(
        Error::from(KINDS[a]),
        Error::new(KINDS[a], Error::NO_OFFSET)
    );
    kani::cover!(a == 29, "last stable fault included");
}

#[kani::proof]
fn error_codes_program() {
    let custom: u32 = kani::any();
    let sdk = ProgramError::Custom(custom);
    let error = Error::from(sdk.clone());
    assert_eq!(
        error,
        Error::Program {
            code: if custom == 0 {
                1u64 << 32
            } else {
                custom as u64
            }
        }
    );
    assert_eq!(ProgramError::from(error), sdk);
    // All native variants are selected by the full SDK canonical u64 range.
    let native: u64 = kani::any();
    kani::assume((2..=26).contains(&native));
    let code = native << 32;
    let sdk = ProgramError::from(code);
    assert_eq!(Error::from(sdk.clone()), Error::Program { code });
    assert_eq!(ProgramError::from(Error::from(sdk.clone())), sdk);
    assert_eq!(u64::from(ProgramError::from(code)), code);
    // An arbitrary stored u64 stays intact in Blubber; SDK decoding has its
    // own fallback for noncanonical codes, so arbitrary SDK bijection is false.
    let code: u64 = kani::any();
    let base: u32 = kani::any();
    assert_eq!(Error::Program { code }.custom_code(), None);
    assert_eq!(
        Error::Program { code }.rebase(base),
        Error::Program { code }
    );
}

#[kani::proof]
fn error_codes_rebase() {
    let offset: u32 = kani::any();
    let base: u32 = kani::any();
    let error = Error::new(ErrorKind::InvalidBoundary, offset);
    let sum = offset as u64 + base as u64;
    let expected = if offset == Error::NO_OFFSET {
        error
    } else if sum >= u32::MAX as u64 {
        ErrorKind::ArithmeticOverflow.into()
    } else {
        Error::new(ErrorKind::InvalidBoundary, sum as u32)
    };
    assert_eq!(error.rebase(base), expected);
}
