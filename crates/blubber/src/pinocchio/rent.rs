//! Read the complete serialized sysvar, including the legacy threshold.

use ::pinocchio::{
    error::ProgramError,
    sysvars::{get_sysvar, rent::RENT_ID},
};

use crate::Result;

use super::MAX_STORAGE;

const OVERHEAD: u64 = 128;
const ONE: u64 = 0x3ff0_0000_0000_0000;
const TWO: u64 = 0x4000_0000_0000_0000;
const MAX_RENT_BYTES: u64 = OVERHEAD + MAX_STORAGE as u64;

#[inline(always)]
/// Internal checked account operation for runtime adapters.
#[doc(hidden)]
pub fn minimum_balance(len: usize) -> Result<u64> {
    from_bytes(parameters()?, len)
}

pub(super) fn parameters() -> Result<[u8; 17]> {
    let mut bytes = [0; 17];
    get_sysvar(&mut bytes, &RENT_ID, 0)?;
    Ok(bytes)
}

pub(super) fn from_bytes(bytes: [u8; 17], len: usize) -> Result<u64> {
    minimum_with_rate(rate_from_bytes(bytes)?, len)
}

pub(super) fn rate_from_bytes(bytes: [u8; 17]) -> Result<u64> {
    let rate = u64::from_le_bytes(bytes[..8].try_into().expect("rent rate"));
    let threshold = u64::from_le_bytes(bytes[8..16].try_into().expect("rent threshold"));
    let (multiplier, max_rate) = match threshold {
        ONE => (1u64, u64::MAX / MAX_RENT_BYTES),
        TWO => (2u64, u64::MAX / MAX_RENT_BYTES / 2),
        _ => return Err(ProgramError::UnsupportedSysvar.into()),
    };
    if rate > max_rate {
        return Err(ProgramError::InvalidArgument.into());
    }
    Ok(rate.wrapping_mul(multiplier))
}

#[inline(always)]
pub(super) fn minimum_with_rate(rate: u64, len: usize) -> Result<u64> {
    if len > MAX_STORAGE {
        return Err(ProgramError::InvalidArgument.into());
    }
    // rate_from_bytes bounds the coefficient for every supported storage size.
    Ok((OVERHEAD + len as u64).wrapping_mul(rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)]
    fn supported_parameters_match_the_runtime_cohort() {
        for (threshold, max_rate) in [(1f64, 1_759_197_129_867), (2f64, 879_598_564_933)] {
            for rate in [0, 19, 3480, 6960, max_rate] {
                for burn in [0, 50, 100, 255] {
                    let runtime = solana_rent::Rent {
                        lamports_per_byte: rate,
                        exemption_threshold: threshold.to_le_bytes(),
                        burn_percent: burn,
                    };
                    let mut bytes = [0; 17];
                    bytes[..8].copy_from_slice(&rate.to_le_bytes());
                    bytes[8..16].copy_from_slice(&threshold.to_le_bytes());
                    bytes[16] = burn;
                    for len in [0, 1, 12, 1024, 10_240, MAX_STORAGE] {
                        assert_eq!(from_bytes(bytes, len), Ok(runtime.minimum_balance(len)));
                    }
                    assert_eq!(
                        from_bytes(bytes, MAX_STORAGE + 1),
                        Err(ProgramError::InvalidArgument.into())
                    );
                    bytes[..8].copy_from_slice(&(max_rate + 1).to_le_bytes());
                    assert_eq!(
                        from_bytes(bytes, 0),
                        Err(ProgramError::InvalidArgument.into())
                    );
                }
            }
        }
    }

    #[test]
    fn custom_and_nonfinite_thresholds_are_rejected() {
        for threshold in [
            0f64,
            -1.0,
            0.5,
            1.5,
            2.5,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            let mut bytes = [0; 17];
            bytes[..8].copy_from_slice(&3480u64.to_le_bytes());
            bytes[8..16].copy_from_slice(&threshold.to_le_bytes());
            assert_eq!(
                from_bytes(bytes, 0),
                Err(ProgramError::UnsupportedSysvar.into())
            );
        }
    }
}
