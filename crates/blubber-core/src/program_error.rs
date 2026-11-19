//! Optional SDK conversions, outside the SDK-independent verified kernel.

use crate::Error;
use solana_program_error::ProgramError;

impl From<ProgramError> for Error {
    fn from(error: ProgramError) -> Self {
        Self::Program { code: error.into() }
    }
}

impl From<Error> for ProgramError {
    fn from(error: Error) -> Self {
        match error {
            Error::Fault { .. } => Self::Custom(error.custom_code().expect("fault has a code")),
            Error::Program { code } => Self::from(code),
        }
    }
}
