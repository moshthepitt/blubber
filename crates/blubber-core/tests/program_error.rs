//! Optional production SDK conversions, including native and custom zero codes.

#![forbid(unsafe_code)]
#![cfg(feature = "program-error")]

use blubber_core::{Error, ErrorKind};
use solana_program_error::ProgramError;

#[test]
fn custom_and_native_sdk_errors_roundtrip() {
    for custom in [0, 1, 0xB101, 0xB11E, u32::MAX] {
        let sdk = ProgramError::Custom(custom);
        let error = Error::from(sdk.clone());
        assert_eq!(error.custom_code(), None);
        assert_eq!(ProgramError::from(error), sdk);
        assert_eq!(error, Error::Program { code: sdk.into() });
    }
    for native in 2u64..=26 {
        let code = native << 32;
        let sdk = ProgramError::from(code);
        assert_eq!(Error::from(sdk.clone()), Error::Program { code });
        assert_eq!(ProgramError::from(Error::from(sdk.clone())), sdk);
    }
}

#[test]
fn faults_use_reserved_custom_codes() {
    for (kind, code) in [
        (ErrorKind::Truncated, 0xB101),
        (ErrorKind::ArithmeticOverflow, 0xB104),
        (ErrorKind::InvalidSchema, 0xB11E),
    ] {
        for offset in [0, 12, u32::MAX] {
            assert_eq!(
                ProgramError::from(Error::new(kind, offset)),
                ProgramError::Custom(code)
            );
        }
    }
}

#[test]
fn noncanonical_stored_codes_follow_the_sdk_fallback() {
    for code in [0, u64::MAX, (27 << 32) | 7] {
        let error = Error::Program { code };
        assert_eq!(error.rebase(123), error);
        assert_eq!(ProgramError::from(error), ProgramError::from(code));
    }
}
