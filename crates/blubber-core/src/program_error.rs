//! Optional SDK conversions, outside the SDK-independent verified kernel.

use crate::{Error, ErrorKind};
use solana_program_error::ProgramError;

impl From<ProgramError> for Error {
    fn from(error: ProgramError) -> Self {
        Self::Program { code: error.into() }
    }
}

impl From<Error> for ProgramError {
    fn from(error: Error) -> Self {
        match error {
            Error::Fault {
                kind,
                offset: Error::NO_OFFSET,
            } => match kind {
                ErrorKind::WrongOwner => Self::InvalidAccountOwner,
                ErrorKind::BorrowConflict => Self::AccountBorrowFailed,
                ErrorKind::GrowthLimit => Self::InvalidRealloc,
                ErrorKind::RentShortfall => Self::AccountNotRentExempt,
                ErrorKind::AlreadyInitialized => Self::AccountAlreadyInitialized,
                _ => Self::Custom(0xB100 + kind as u32),
            },
            Error::Fault { kind, .. } => Self::Custom(0xB100 + kind as u32),
            Error::Program { code } => Self::from(code),
        }
    }
}
