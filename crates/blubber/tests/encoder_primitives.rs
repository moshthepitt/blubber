//! Primitive byte views, public codec bindings and exact surrounding frames.
#![forbid(unsafe_code)]

use blubber::{Address, Fixed, Schema, Str};
use blubber_core::encoding::{encode_frame, Encoder};
use blubber_core::{encoding_leaf as leaf, encoding_text as text};

fn array<const N: usize>() {
    let source: [u8; N] = core::array::from_fn(|i| (i % 251) as u8);
    let view = leaf::raw(&source);
    assert_eq!(
        core::mem::size_of_val(&view),
        core::mem::size_of::<&[u8; N]>()
    );
    assert_eq!(leaf::measure_twice(&view), N);
    let mut frame = [19u8; 300];
    encode_frame(&view, &mut frame, 3);
    assert_eq!(&frame[3..3 + N], &source);
    assert!(frame[..3].iter().all(|b| *b == 19));
    assert!(frame[3 + N..].iter().all(|b| *b == 19));
    let mut public = [0u8; N];
    <[u8; N] as Schema>::encode(&source, &mut public);
    assert_eq!(public, source);
    assert_eq!(leaf::compare_array(&source, &public), source.cmp(&public));
    for index in [0, N - 1] {
        let mut larger = source;
        larger[index] += 1;
        assert_eq!(leaf::compare_array(&source, &larger), source.cmp(&larger));
        assert_eq!(leaf::compare_array(&larger, &source), larger.cmp(&source));
    }
}

#[test]
fn positive_arrays_and_frames() {
    array::<1>();
    array::<2>();
    array::<3>();
    array::<8>();
    array::<16>();
    array::<32>();
    array::<65>();
    array::<257>();
}

#[test]
fn projected_numeric_widths_and_boolean_domain() {
    for value in [false, true] {
        let bytes = leaf::bool_bytes(value);
        let view = leaf::boolean(&bytes);
        let mut output = [19u8];
        view.encode(&mut output);
        assert_eq!(output, value.encode());
        assert_eq!(bool::decode(&output), Ok(value));
    }
    macro_rules! width {
        ($ty:ty, $value:expr, $constructor:ident) => {{
            let source: $ty = $value;
            let bytes = source.to_le_bytes();
            let view = leaf::$constructor(&bytes);
            let mut output = bytes;
            output.fill(19);
            view.encode(&mut output);
            assert_eq!(output, bytes);
            assert_eq!(leaf::measure_twice(&view), bytes.len());
            <$ty as Schema>::encode(&source, &mut output);
            assert_eq!(output, bytes);
        }};
    }
    width!(u8, 255, unsigned);
    width!(u16, u16::MAX, unsigned);
    width!(u32, u32::MAX, unsigned);
    width!(u64, u64::MAX, unsigned);
    width!(u128, u128::MAX, unsigned);
    width!(i8, i8::MIN, signed);
    width!(i16, i16::MIN, signed);
    width!(i32, i32::MIN, signed);
    width!(i64, i64::MIN, signed);
    width!(i128, i128::MIN, signed);
    let key = Address::new_from_array([37; 32]);
    assert_eq!(key.encode(), [37; 32]);
}

#[test]
fn utf8_scalar_boundaries_and_empty_frames() {
    for source in [
        "",
        "\0",
        "\x7f",
        "\u{80}",
        "\u{7ff}",
        "\u{800}",
        "\u{d7ff}",
        "\u{e000}",
        "\u{ffff}",
        "\u{10000}",
        "\u{10ffff}",
        "a\u{80}\u{800}\u{10000}",
    ] {
        string_frame(source);
    }
}

#[test]
fn rejected_lengths_preserve_destination() {
    for width in [2, 4] {
        let mut bytes = [19u8; 4];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <[u8; 3] as Schema>::encode(&[3, 5, 7], &mut bytes[..width]);
        }));
        assert!(result.is_err());
        assert_eq!(bytes, [19; 4]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Str::encode(&"abc", &mut bytes[..width]);
        }));
        assert!(result.is_err());
        assert_eq!(bytes, [19; 4]);
    }
}

fn string_frame(source: &str) {
    let view = text::text(source);
    assert_eq!(leaf::measure_twice(&view), source.len());
    assert_eq!(core::mem::size_of_val(&view), core::mem::size_of::<&str>());
    let mut frame = [19u8; 300];
    encode_frame(&view, &mut frame, 3);
    assert_eq!(&frame[3..3 + source.len()], source.as_bytes());
    assert!(frame[..3].iter().all(|b| *b == 19));
    assert!(frame[3 + source.len()..].iter().all(|b| *b == 19));
    let destination = &mut frame[3..3 + source.len()];
    destination.fill(29);
    Str::encode(&source, destination);
    assert_eq!(destination, source.as_bytes());
    assert_eq!(blubber_core::validate_utf8(destination), Ok(()));
}

#[test]
fn string_chunk_boundaries_and_frames() {
    for width in [15, 16, 17, 31, 32, 33, 65, 257] {
        let source: String = (0..width)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        string_frame(&source);
    }
    string_frame(&"a\u{80}\u{800}\u{10000}".repeat(4));
}

#[test]
fn map_regions_match_independent_fixed_and_indexed_oracles() {
    use blubber::Map;
    let values = [(-1i16, 0x1234u32), (0, 0x5678)];
    let mut frame = [19u8; 18];
    assert_eq!(Map::<i16, u32>::measure(&values.as_slice()), Ok(16));
    Map::<i16, u32>::encode(&values.as_slice(), &mut frame[1..17]);
    assert_eq!(frame[0], 19);
    assert_eq!(frame[17], 19);
    assert_eq!(
        &frame[1..17],
        &[2, 0, 0, 0, 0x7f, 0xff, 0x34, 0x12, 0, 0, 0x80, 0, 0x78, 0x56, 0, 0]
    );
    assert!(blubber_core::validate(
        &frame[1..17],
        Map::<i16, u32>::LAYOUT,
        blubber_core::MAX_DEPTH
    )
    .is_ok());
    let values = [(false, "a"), (true, "")];
    let mut frame = [19u8; 17];
    assert_eq!(Map::<bool, Str>::measure(&values.as_slice()), Ok(15));
    Map::<bool, Str>::encode(&values.as_slice(), &mut frame[1..16]);
    assert_eq!(frame[0], 19);
    assert_eq!(frame[16], 19);
    assert_eq!(
        &frame[1..16],
        &[2, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 0, b'a']
    );
    assert!(blubber_core::validate(
        &frame[1..16],
        Map::<bool, Str>::LAYOUT,
        blubber_core::MAX_DEPTH
    )
    .is_ok());
    for empty in [true, false] {
        let mut frame = [19u8; 6];
        if empty {
            Map::<bool, Str>::encode(&([].as_slice()), &mut frame[1..5]);
        } else {
            Map::<i16, u32>::encode(&([].as_slice()), &mut frame[1..5]);
        }
        assert_eq!(frame, [19, 0, 0, 0, 0, 19]);
    }
}

#[test]
fn map_preflight_and_encoding_retain_callback_order() {
    use blubber_core::encoding_map::{self as map, Entries};
    use blubber_core::encoding_product::Source;
    use blubber_core::{ErrorKind, Result};
    use core::cell::RefCell;
    struct Values<'a> {
        log: &'a RefCell<Vec<&'static str>>,
        fail: bool,
    }
    struct Value<'a> {
        log: &'a RefCell<Vec<&'static str>>,
        fail: bool,
    }
    impl Source for Value<'_> {
        fn is_fixed(&self) -> bool {
            true
        }
        fn measure(&self) -> Result<u32> {
            self.log.borrow_mut().push("measure");
            if self.fail {
                Err(ErrorKind::InvalidBool.into())
            } else {
                Ok(1)
            }
        }
        fn encode(&self, destination: &mut [u8]) {
            self.log.borrow_mut().push("encode");
            destination[0] = 3;
        }
    }
    impl<'a> Entries for Values<'a> {
        type Child = Value<'a>;
        fn count(&self) -> usize {
            2
        }
        fn value(&self, _: usize) -> Self::Child {
            Value {
                log: self.log,
                fail: self.fail,
            }
        }
        fn compare(&self, _: usize) -> core::cmp::Ordering {
            self.log.borrow_mut().push("compare");
            core::cmp::Ordering::Equal
        }
        fn write_key(&self, index: usize, destination: &mut [u8]) {
            self.log.borrow_mut().push("key");
            destination[0] = index as u8;
        }
    }
    let log = RefCell::new(Vec::new());
    let values = Values {
        log: &log,
        fail: true,
    };
    assert_eq!(
        map::measure(&values, 1, false, None),
        Err(ErrorKind::InvalidBool.into())
    );
    assert_eq!(*log.borrow(), ["measure"]);
    log.borrow_mut().clear();
    let values = Values {
        log: &log,
        fail: false,
    };
    assert_eq!(
        map::measure(&values, 1, false, Some(1)),
        Err(ErrorKind::DuplicateKey.into())
    );
    assert_eq!(*log.borrow(), ["measure", "compare"]);
    for (owned, indexed, expected) in [
        (false, false, &["measure"][..]),
        (true, false, &["key", "measure"][..]),
        (false, true, &["key", "measure"][..]),
    ] {
        log.borrow_mut().clear();
        let failing = Values {
            log: &log,
            fail: true,
        };
        let mut bytes = [19u8; 18];
        let size = if indexed { 16 } else { 8 };
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            map::encode(
                &failing,
                &mut bytes[1..1 + size],
                1,
                if indexed { None } else { Some(1) },
                owned,
            );
        }))
        .unwrap_err();
        let message = failure.downcast_ref::<String>().unwrap();
        assert!(message.starts_with(if owned {
            "previously measured owned map:"
        } else {
            "previously measured map input:"
        }));
        assert!(message.contains("InvalidBool"));
        assert_eq!(*log.borrow(), expected);
        assert_eq!(bytes[0], 19);
        assert!(bytes[1 + size..].iter().all(|b| *b == 19));
    }
    for (owned, indexed, expected) in [
        (
            false,
            false,
            ["measure", "key", "encode", "measure", "key", "encode"],
        ),
        (
            true,
            false,
            ["key", "measure", "encode", "key", "measure", "encode"],
        ),
        (
            false,
            true,
            ["key", "measure", "encode", "key", "measure", "encode"],
        ),
    ] {
        log.borrow_mut().clear();
        let mut bytes = [19u8; 18];
        let size = if indexed { 16 } else { 8 };
        map::encode(
            &values,
            &mut bytes[1..1 + size],
            1,
            if indexed { None } else { Some(1) },
            owned,
        );
        assert_eq!(*log.borrow(), expected);
        assert_eq!(bytes[0], 19);
        assert!(bytes[1 + size..].iter().all(|b| *b == 19));
    }
}
