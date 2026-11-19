//! Native lifecycle capabilities and lossless SDK error conversion.
#![forbid(unsafe_code)]
#![cfg(feature = "derive")]

use blubber::{Error, ErrorKind};
use blubber_anchor::{sdk, Blubber};

#[blubber::schema(account)]
pub struct State {
    pub counter: u64,
}
impl sdk::Owner for State {
    const OWNER: sdk::Address = sdk::Address::new_from_array([1; 32]);
}

#[test]
fn native_wrapper_exposes_exact_realloc_and_close_capabilities() {
    fn capabilities<T: sdk::AccountRealloc + sdk::AccountClose>() {}
    capabilities::<Blubber<State>>();
}

#[test]
fn faults_use_stable_custom_codes_and_offsets_remain_local() {
    for kind in [
        ErrorKind::Capacity,
        ErrorKind::UsedLength,
        ErrorKind::RentShortfall,
        ErrorKind::DuplicateAccount,
        ErrorKind::ArithmeticOverflow,
        ErrorKind::Poisoned,
    ] {
        for offset in [0, 8, 12, 1024, Error::NO_OFFSET] {
            let fault = Error::new(kind, offset);
            assert_eq!(
                sdk::Error::from(fault),
                sdk::Error::Custom(0xB100 + kind as u32)
            );
        }
    }
}

#[test]
fn native_sdk_errors_round_trip_without_an_adapter_error_layer() {
    for error in [
        sdk::Error::AccountBorrowFailed,
        sdk::Error::InvalidRealloc,
        sdk::Error::IllegalOwner,
        sdk::Error::MissingRequiredSignature,
        sdk::Error::InsufficientFunds,
        sdk::Error::UnsupportedSysvar,
        sdk::Error::Custom(0),
        sdk::Error::Custom(u32::MAX),
    ] {
        let fault = Error::from(error.clone());
        assert_eq!(sdk::Error::from(fault), error);
        assert_eq!(fault.custom_code(), None);
        assert_eq!(
            fault,
            Error::Program {
                code: u64::from(error)
            }
        );
    }
    for code in [0, 1, u32::MAX as u64, 1 << 32, 1 << 48, u64::MAX] {
        assert_eq!(
            sdk::Error::from(Error::Program { code }),
            sdk::Error::from(code)
        );
    }
}
