//! Corrupted leaf bytes and exact lengths through production gateways.

use blubber::{Error, ErrorKind, Fixed};
use blubber_core::{partition_option, validate_utf8, Layout};

#[kani::proof]
#[kani::unwind(4)]
fn leaf_bool_and_option() {
    let bytes: [u8; 3] = kani::any();
    let len: u8 = kani::any();
    kani::assume(len <= 3);
    let region = &bytes[..len as usize];
    let decoded = bool::decode(region);
    assert_eq!(decoded.is_ok(), len == 1 && bytes[0] <= 1);
    if let Ok(value) = decoded {
        assert_eq!(value.encode(), [bytes[0]]);
    }
    if len == 0 {
        assert_eq!(decoded, Err(Error::new(ErrorKind::Truncated, 0)));
    } else if len > 1 {
        assert_eq!(decoded, Err(Error::new(ErrorKind::InvalidBoundary, 1)));
    } else if bytes[0] > 1 {
        assert_eq!(decoded, Err(Error::new(ErrorKind::InvalidBool, 0)));
    }
    let layout = Layout::Flat {
        checked: blubber::__layout_of!(Option<bool>),
    };
    let partition = partition_option(region, &layout);
    let valid_shape = (len == 1 && bytes[0] == 0) || (len == 2 && bytes[0] == 1);
    assert_eq!(partition.is_ok(), valid_shape);
    let expected = if len == 0 {
        Err(Error::new(ErrorKind::Truncated, 0))
    } else if bytes[0] > 1 {
        Err(Error::new(ErrorKind::InvalidTag, 0))
    } else if bytes[0] == 0 {
        if len == 1 {
            Ok(None)
        } else {
            Err(Error::new(ErrorKind::InvalidBoundary, 1))
        }
    } else if len < 2 {
        Err(Error::new(ErrorKind::Truncated, 1))
    } else if len > 2 {
        Err(Error::new(ErrorKind::InvalidBoundary, 2))
    } else {
        Ok(Some(blubber_core::Span { start: 1, end: 2 }))
    };
    assert_eq!(partition, expected);
    if len > 0 && bytes[0] > 1 {
        assert_eq!(partition, Err(Error::new(ErrorKind::InvalidTag, 0)));
    }
    kani::cover!(
        len == 2 && bytes[0] == 1 && bytes[1] == 1,
        "Some true accepted"
    );
}

macro_rules! integer {
    ($name:ident, $ty:ty, $width:literal) => {
        #[kani::proof]
        #[kani::unwind(19)]
        fn $name() {
            let value: $ty = kani::any();
            assert_eq!(Fixed::encode(value), value.to_le_bytes());
            assert_eq!(<$ty>::decode(value.encode().as_ref()), Ok(value));
            let bytes: [u8; 17] = kani::any();
            let len: u8 = kani::any();
            kani::assume(len <= 17);
            let result = <$ty>::decode(&bytes[..len as usize]);
            if len < $width {
                assert_eq!(result, Err(Error::new(ErrorKind::Truncated, len as u32)));
            } else if len > $width {
                assert_eq!(result, Err(Error::new(ErrorKind::InvalidBoundary, $width)));
            } else {
                let exact: [u8; $width] = bytes[..$width].try_into().unwrap();
                assert_eq!(result, Ok(<$ty>::from_le_bytes(exact)));
            }
        }
    };
}
integer!(leaf_u8, u8, 1);
integer!(leaf_u16, u16, 2);
integer!(leaf_u32, u32, 4);
integer!(leaf_u64, u64, 8);
integer!(leaf_u128, u128, 16);
integer!(leaf_i8, i8, 1);
integer!(leaf_i16, i16, 2);
integer!(leaf_i32, i32, 4);
integer!(leaf_i64, i64, 8);
integer!(leaf_i128, i128, 16);

macro_rules! utf8 {
    ($name:ident, $n:literal) => {
        #[kani::proof]
        #[kani::unwind(10)]
        fn $name() {
            let bytes: [u8; $n] = kani::any();
            let expected = core::str::from_utf8(&bytes)
                .map(|_| ())
                .map_err(|error| Error::new(ErrorKind::InvalidUtf8, error.valid_up_to() as u32));
            assert_eq!(validate_utf8(&bytes), expected);
        }
    };
}
utf8!(leaf_utf8_0, 0);
utf8!(leaf_utf8_1, 1);
utf8!(leaf_utf8_2, 2);
utf8!(leaf_utf8_3, 3);
utf8!(leaf_utf8_4, 4);
utf8!(leaf_utf8_5, 5);
utf8!(leaf_utf8_6, 6);
utf8!(leaf_utf8_7, 7);
utf8!(leaf_utf8_8, 8);

#[kani::proof]
#[kani::unwind(19)]
fn leaf_fixed_region_decoders() {
    let bytes: [u8; 16] = kani::any();
    let len: u8 = kani::any();
    kani::assume(len <= 16);
    let input = &bytes[..len as usize];
    macro_rules! same { ($($t:ty),*) => { $(assert_eq!(<$t>::__decode_region(input), <$t>::decode(input));)* }; }
    same!(u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, bool);
    assert_eq!(<[u8; 8]>::__decode_region(input), <[u8; 8]>::decode(input));
}

#[path = "../../../crates/blubber/tests/support/region.rs"]
mod region;

fn chunk_walk<S: blubber::Schema, const N: usize>()
where
    for<'a> S: blubber::Schema<Read<'a> = &'a [u8]>,
{
    let payload: [u8; 4] = kani::any();
    let n = N as u8;
    let mut bytes = [0; 8];
    bytes[0] = n;
    bytes[4..].copy_from_slice(&payload);
    let view = blubber::View::new(
        &bytes[..4 + n as usize],
        <blubber::List<S> as blubber::Schema>::LAYOUT,
        100,
    )
    .unwrap();
    let list = view.read::<blubber::List<S>>().unwrap();
    let mut iter = list.try_iter();
    for i in 0..n as usize {
        let expected = list.try_get(i).map(Option::unwrap);
        assert_eq!(iter.clone().next(), Some(expected));
        assert_eq!(iter.next(), Some(expected));
        if expected.is_err() {
            break;
        }
    }
    assert!(iter.next().is_none());
    assert!(iter.clone().next().is_none());
    assert_eq!(iter.size_hint(), (0, Some(0)));
    let mut final_fault = true;
    for i in 0..N {
        final_fault &= payload[i] == if i + 1 == N { 255 } else { 0 };
    }
    kani::cover!(
        final_fault,
        "empty exhaustion or custom domain fault at final chunk"
    );
}

#[kani::proof]
#[kani::unwind(10)]
fn leaf_chunk_custom_domain_n0() {
    chunk_walk::<region::Domain, 0>();
}

#[kani::proof]
#[kani::unwind(10)]
fn leaf_chunk_custom_domain_n1() {
    chunk_walk::<region::Domain, 1>();
}

#[kani::proof]
#[kani::unwind(10)]
fn leaf_chunk_custom_domain_n2() {
    chunk_walk::<region::Domain, 2>();
}

#[kani::proof]
#[kani::unwind(10)]
fn leaf_chunk_custom_domain_n3() {
    chunk_walk::<region::Domain, 3>();
}

#[kani::proof]
#[kani::unwind(10)]
fn leaf_chunk_custom_domain_n4() {
    chunk_walk::<region::Domain, 4>();
}

#[kani::proof]
#[kani::unwind(10)]
fn leaf_chunk_bool() {
    use blubber::{List, Schema, View};
    let payload: [u8; 4] = kani::any();
    let n: u8 = kani::any();
    kani::assume(n <= 4);
    let mut bytes = [0; 8];
    bytes[0] = n;
    bytes[4..].copy_from_slice(&payload);
    let list = View::new(&bytes[..4 + n as usize], <List<bool>>::LAYOUT, 100)
        .unwrap()
        .read::<List<bool>>()
        .unwrap();
    let mut iter = list.try_iter();
    assert_eq!(
        blubber_core::raw::fixed_chunk_end(100, 4 + n as usize),
        104 + n as u32
    );
    for i in 0..n as usize {
        assert_eq!(
            blubber_core::raw::fixed_chunk_base(104 + n as u32, n as usize - i),
            104 + i as u32
        );
        let expected = if payload[i] <= 1 {
            Ok(payload[i] == 1)
        } else {
            Err(Error::new(ErrorKind::InvalidBool, 104 + i as u32))
        };
        assert_eq!(iter.next(), Some(expected));
        if expected.is_err() {
            break;
        }
    }
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert_eq!(iter.size_hint(), (0, Some(0)));
    kani::cover!(
        n == 4 && payload == [0, 1, 0, 1],
        "all bool chunks accepted"
    );
    kani::cover!(
        n == 4 && payload == [0, 1, 2, 1],
        "bool fault rebased once and fused"
    );
}

#[kani::proof]
#[kani::unwind(10)]
fn leaf_chunk_admission() {
    let bytes: [u8; 8] = kani::any();
    let len: u8 = kani::any();
    let width: u8 = kani::any();
    kani::assume(len <= 8 && width <= 2);
    let result = blubber_core::raw::fixed_read_payload(&bytes[..len as usize], width as u32);
    let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let end = 4 + count as u64 * width as u64;
    let expected = if width == 0 {
        Err(ErrorKind::InvalidSchema.into())
    } else if len < 4 {
        Err(Error::new(ErrorKind::Truncated, 0))
    } else if end > u32::MAX as u64 {
        Err(ErrorKind::ArithmeticOverflow.into())
    } else if end > len as u64 {
        Err(Error::new(ErrorKind::Truncated, 4))
    } else if end < len as u64 {
        Err(Error::new(ErrorKind::InvalidBoundary, end as u32))
    } else {
        Ok(&bytes[4..len as usize])
    };
    assert_eq!(result, expected);
    kani::cover!(
        width == 1 && count == 4 && len == 8,
        "nonempty exact payload admitted"
    );
}

#[derive(blubber::Fixed, Clone, Copy, Debug, PartialEq, Eq)]
struct ChunkRecord {
    flag: bool,
    number: u16,
}

#[kani::proof]
#[kani::unwind(5)]
fn leaf_chunk_decoder_selection() {
    use blubber::{Schema, View};
    let bytes: [u8; 3] = kani::any();
    let len: u8 = kani::any();
    let base: u16 = kani::any();
    kani::assume(len <= 3);
    let input = &bytes[..len as usize];
    let foreign = Layout::Leaf(blubber_core::Leaf::Raw(1));
    let view = View::new(input, foreign, base as u32).unwrap();
    macro_rules! same { ($($ty:ty),*) => { $(
        assert!(!<$ty>::__READ_REGION_NEEDS_LAYOUT);
        assert_eq!(<$ty>::read_region(input, *<$ty>::LAYOUT, base as u32), <$ty>::read(view));
    )* }; }
    same!(bool, u8, u16, i16, [u8; 1], ChunkRecord);
    kani::cover!(
        len == 3 && bytes[0] == 1,
        "native derived region accepted through foreign metadata"
    );
}
