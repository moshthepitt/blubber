//! Symbolic native values and corrupted bytes through real Fixed derive output.

#[path = "../../../crates/blubber/tests/support/fixed.rs"]
mod shapes;

use blubber::{Error, ErrorKind, Fixed, Schema};
use shapes::{Limits, Padded};

#[kani::proof]
#[kani::unwind(5)]
fn fixed_struct_limits_roundtrip() {
    let value = Limits {
        enabled: kani::any(),
        limit: kani::any(),
    };
    let wire = value.encode();
    assert_eq!(Limits::SIZE, 3);
    assert_eq!(core::mem::size_of::<Limits>(), 4);
    assert_eq!(
        wire,
        [
            u8::from(value.enabled),
            value.limit as u8,
            (value.limit >> 8) as u8
        ]
    );
    assert_eq!(Limits::decode(&wire), Ok(value));
    assert_eq!(Limits::measure(&value), Ok(3));
    let mut destination: [u8; 3] = kani::any();
    <Limits as Schema>::encode(&value, &mut destination);
    assert_eq!(destination, wire);
}

#[kani::proof]
#[kani::unwind(5)]
fn fixed_struct_limits_corrupted() {
    let bytes: [u8; 3] = kani::any();
    let decoded = Limits::decode(&bytes);
    assert_eq!(decoded.is_ok(), bytes[0] <= 1);
    match decoded {
        Ok(value) => {
            assert_eq!(value.enabled, bytes[0] == 1);
            assert_eq!(value.limit, u16::from_le_bytes([bytes[1], bytes[2]]));
            assert_eq!(value.encode(), bytes);
        }
        Err(error) => assert_eq!(error, Error::new(ErrorKind::InvalidBool, 0)),
    }
}

#[kani::proof]
#[kani::unwind(17)]
fn fixed_struct_padded_roundtrip() {
    let value = Padded {
        flag: kani::any(),
        total: kani::any(),
        limits: Limits {
            enabled: kani::any(),
            limit: kani::any(),
        },
        raw: kani::any(),
    };
    let wire = value.encode();
    assert_eq!(Padded::SIZE, 15);
    assert_eq!(core::mem::size_of::<Padded>(), 32);
    assert_eq!(core::mem::offset_of!(Padded, total), 8);
    assert_eq!(wire[0], u8::from(value.flag));
    assert_eq!(&wire[1..9], &value.total.to_le_bytes());
    assert_eq!(&wire[9..12], &value.limits.encode());
    assert_eq!(&wire[12..], &value.raw);
    assert_eq!(Padded::decode(&wire), Ok(value));
    assert_eq!(Padded::measure(&value), Ok(15));
    let mut destination: [u8; 15] = kani::any();
    <Padded as Schema>::encode(&value, &mut destination);
    assert_eq!(destination, wire);
}

#[kani::proof]
#[kani::unwind(17)]
fn fixed_struct_padded_corrupted() {
    let bytes: [u8; 15] = kani::any();
    let decoded = Padded::decode(&bytes);
    assert_eq!(decoded.is_ok(), bytes[0] <= 1 && bytes[9] <= 1);
    match decoded {
        Ok(value) => assert_eq!(value.encode(), bytes),
        Err(error) => assert_eq!(
            error,
            Error::new(ErrorKind::InvalidBool, if bytes[0] > 1 { 0 } else { 9 })
        ),
    }
}

#[kani::proof]
#[kani::unwind(18)]
fn fixed_struct_exact_lengths() {
    let bytes: [u8; 17] = kani::any();
    let length: u8 = kani::any();
    kani::assume(length <= 17);
    let region = &bytes[..length as usize];
    let result = Padded::decode(region);
    if length < 15 {
        assert_eq!(result, Err(Error::new(ErrorKind::Truncated, length as u32)));
    } else if length > 15 {
        assert_eq!(result, Err(Error::new(ErrorKind::InvalidBoundary, 15)));
    } else {
        assert_eq!(result.is_ok(), bytes[0] <= 1 && bytes[9] <= 1);
    }
    let result = Limits::decode(region);
    if length < 3 {
        assert_eq!(result, Err(Error::new(ErrorKind::Truncated, length as u32)));
    } else if length > 3 {
        assert_eq!(result, Err(Error::new(ErrorKind::InvalidBoundary, 3)));
    } else {
        assert_eq!(result.is_ok(), bytes[0] <= 1);
    }
}
