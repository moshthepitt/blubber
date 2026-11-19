//! Full-width production arithmetic and bounded directory publication.

use blubber_core::{checked_add, checked_delta, checked_mul, checked_span, checked_u32, ErrorKind};

#[kani::proof]
fn arithmetic_add() {
    let a: usize = kani::any();
    let b: usize = kani::any();
    let sum = a as u128 + b as u128;
    let expected = if sum <= usize::MAX as u128 {
        Ok(sum as usize)
    } else {
        Err(ErrorKind::ArithmeticOverflow.into())
    };
    assert_eq!(checked_add(a, b), expected);
}

#[kani::proof]
fn arithmetic_mul() {
    let a: usize = kani::any();
    let b: usize = kani::any();
    let product = a as u128 * b as u128;
    let expected = if product <= usize::MAX as u128 {
        Ok(product as usize)
    } else {
        Err(ErrorKind::ArithmeticOverflow.into())
    };
    assert_eq!(checked_mul(a, b), expected);
}

#[kani::proof]
fn arithmetic_u32() {
    let value: usize = kani::any();
    assert_eq!(
        checked_u32(value).ok().map(u128::from),
        (value <= u32::MAX as usize).then_some(value as u128)
    );
    if let Err(error) = checked_u32(value) {
        assert_eq!(error, ErrorKind::ArithmeticOverflow.into());
    }
}

#[kani::proof]
fn arithmetic_delta() {
    let value: u32 = kani::any();
    let delta: i64 = kani::any();
    let sum = value as i128 + delta as i128;
    let expected = if (0..=u32::MAX as i128).contains(&sum) {
        Ok(sum as u32)
    } else {
        Err(ErrorKind::ArithmeticOverflow.into())
    };
    assert_eq!(checked_delta(value, delta), expected);
}

#[kani::proof]
fn arithmetic_span() {
    let start: u32 = kani::any();
    let len: u32 = kani::any();
    let limit: u32 = kani::any();
    let end = start as u64 + len as u64;
    let result = checked_span(start, len, limit);
    if end > u32::MAX as u64 {
        assert_eq!(
            result,
            Err(blubber_core::Error::new(
                ErrorKind::ArithmeticOverflow,
                start
            ))
        );
    } else if end > limit as u64 {
        assert_eq!(
            result,
            Err(blubber_core::Error::new(ErrorKind::InvalidBoundary, start))
        );
    } else {
        assert_eq!(
            result,
            Ok(blubber_core::Span {
                start,
                end: end as u32
            })
        );
    }
}

#[kani::proof]
#[kani::unwind(9)]
fn arithmetic_directory_delta() {
    let mut bytes: [u8; 16] = kani::any();
    let before = bytes;
    let first: u32 = kani::any();
    let stride: u32 = kani::any();
    let count: u32 = kani::any();
    let delta = i64::from(kani::any::<i8>());
    let payload: u32 = kani::any();
    let result =
        blubber_core::raw::adjust_directory(&mut bytes, first, stride, count, delta, payload);
    let mut expected = before;
    if result.is_ok() && delta != 0 && count != 0 {
        assert!(stride >= 4 && first <= 12 && count <= 4);
        assert!(u64::from(first) + u64::from(count - 1) * u64::from(stride) + 4 <= 16);
        for i in 0..count {
            let at = (u64::from(first) + u64::from(i) * u64::from(stride)) as usize;
            let old = u32::from_le_bytes(before[at..at + 4].try_into().unwrap());
            assert!(old <= payload);
            let new = old.checked_add_signed(delta as i32).unwrap();
            expected[at..at + 4].copy_from_slice(&new.to_le_bytes());
        }
    }
    // Exact word equality covers the complete arbitrary 16-byte allocation.
    let mut at = 0;
    while at < 16 {
        let actual: [u8; 8] = bytes[at..at + 8].try_into().unwrap();
        let reference: [u8; 8] = expected[at..at + 8].try_into().unwrap();
        assert_eq!(u64::from_le_bytes(actual), u64::from_le_bytes(reference));
        at += 8;
    }
}
