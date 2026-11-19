//! Wire fixtures and malformed-input checks for the built-in typed codecs.

#![forbid(unsafe_code)]

use blubber::{Error, ErrorKind, Fixed, Kind, List, Map, MapKey, Pubkey, Schema, Str};
use blubber_core::{check_layout, validate, KeyDomain, Layout, Leaf, MAX_DEPTH};
use core::fmt::Debug;

fn encoded<S: Schema>(init: &S::Init<'_>) -> Vec<u8> {
    let size = S::measure(init).unwrap() as usize;
    let mut first = vec![0xa5; size + 6];
    let mut second = vec![0x5a; size];
    S::encode(init, &mut first[3..3 + size]);
    S::encode(init, &mut second);
    assert_eq!(&first[..3], &[0xa5; 3]);
    assert_eq!(&first[3 + size..], &[0xa5; 3]);
    assert_eq!(&first[3..3 + size], second);
    validate(&second, S::LAYOUT, MAX_DEPTH).unwrap();
    second
}

fn roundtrips<T: Fixed + MapKey + Debug>(values: &[T]) {
    for &value in values {
        let bytes = value.encode();
        let key = value.encode_key();
        assert_eq!(bytes.as_ref().len(), T::SIZE as usize);
        assert_eq!(key.as_ref().len(), T::SIZE as usize);
        assert_eq!(T::decode(bytes.as_ref()), Ok(value));
        assert_eq!(T::decode_key(key.as_ref()), Ok(value));
        for &other in values {
            assert_eq!(
                value.cmp(&other),
                key.as_ref().cmp(other.encode_key().as_ref())
            );
        }
    }
}

fn exact_width<T: MapKey + Debug>() {
    let width = T::SIZE as usize;
    let bytes = vec![0; width + 2];
    for len in 0..=width + 2 {
        let expected = if len < width {
            Some(Error::new(ErrorKind::Truncated, len as u32))
        } else if len > width {
            Some(Error::new(ErrorKind::InvalidBoundary, width as u32))
        } else {
            None
        };
        for result in [T::decode(&bytes[..len]), T::decode_key(&bytes[..len])] {
            assert_eq!(result.err(), expected);
        }
    }
}

#[test]
fn all_integer_widths_roundtrip_and_preserve_order() {
    macro_rules! widths {
        ($($unsigned:ty => $signed:ty),* $(,)?) => {$(
            roundtrips::<$unsigned>(&[0, 1, <$unsigned>::MAX / 2,
                <$unsigned>::MAX / 2 + 1, <$unsigned>::MAX - 1, <$unsigned>::MAX]);
            roundtrips::<$signed>(&[<$signed>::MIN, <$signed>::MIN + 1,
                -1, 0, 1, <$signed>::MAX - 1, <$signed>::MAX]);
            exact_width::<$unsigned>();
            exact_width::<$signed>();
        )*};
    }
    widths!(u8 => i8, u16 => i16, u32 => i32, u64 => i64, u128 => i128);
}

#[test]
fn wire_examples_are_independent_of_host_endianness() {
    assert_eq!(256u16.encode(), [0, 1]);
    assert_eq!(256u16.encode_key(), [1, 0]);
    assert_eq!(u16::decode(&[0, 1]), Ok(256));
    assert_eq!(u16::decode_key(&[1, 0]), Ok(256));
    assert_eq!(0x0102_0304u32.encode(), [4, 3, 2, 1]);
    assert_eq!(0x0102_0304u32.encode_key(), [1, 2, 3, 4]);
    assert_eq!(0x0102_0304_0506_0708u64.encode(), [8, 7, 6, 5, 4, 3, 2, 1]);
    assert_eq!(
        0x0102_0304_0506_0708u64.encode_key(),
        [1, 2, 3, 4, 5, 6, 7, 8]
    );
    let wide = 0x0001_0203_0405_0607_0809_0a0b_0c0d_0e0fu128;
    assert_eq!(
        wide.encode(),
        [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
    );
    assert_eq!(
        wide.encode_key(),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
    );
    for (value, bytes) in [
        (i16::MIN, [0, 0]),
        (-1, [0x7f, 0xff]),
        (0, [0x80, 0]),
        (i16::MAX, [0xff, 0xff]),
    ] {
        assert_eq!(value.encode_key(), bytes);
        assert_eq!(i16::decode_key(&bytes), Ok(value));
    }
    assert_eq!((-2i32).encode(), [0xfe, 0xff, 0xff, 0xff]);
    assert_eq!((-2i32).encode_key(), [0x7f, 0xff, 0xff, 0xfe]);
    assert_eq!(
        (-2i64).encode_key(),
        [0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe]
    );
    assert_eq!(i128::MIN.encode_key(), [0; 16]);
    assert_eq!(i128::MAX.encode_key(), [0xff; 16]);
}

#[test]
fn one_byte_integer_domains_are_exhaustive() {
    let unsigned: Vec<u8> = (u8::MIN..=u8::MAX).collect();
    let signed: Vec<i8> = (i8::MIN..=i8::MAX).collect();
    roundtrips(&unsigned);
    roundtrips(&signed);
    for byte in unsigned {
        assert_eq!(i8::decode(&[byte]).unwrap().encode(), [byte]);
        assert_eq!(i8::decode_key(&[byte]).unwrap().encode_key(), [byte]);
    }
}

#[test]
fn bool_domain_is_exact_for_scalars_and_keys() {
    roundtrips(&[false, true]);
    exact_width::<bool>();
    for byte in 0..=255u8 {
        let expected = match byte {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::new(ErrorKind::InvalidBool, 0)),
        };
        assert_eq!(bool::decode(&[byte]), expected);
        assert_eq!(bool::decode_key(&[byte]), expected);
    }
    assert_eq!(
        bool::decode(&[2, 0]),
        Err(Error::new(ErrorKind::InvalidBoundary, 1))
    );
}

#[test]
fn raw_arrays_support_arbitrary_positive_widths() {
    fn width<const N: usize>() {
        roundtrips(&[[0; N], [1; N], [0xff; N]]);
        exact_width::<[u8; N]>();
        let mut first = [0; N];
        let mut second = [0; N];
        first[N - 1] = 0xff;
        second[0] = 1;
        roundtrips(&[first, second]);
        assert_eq!(first.encode_key(), first);
        assert_eq!(encoded::<[u8; N]>(&first), first);
    }
    width::<1>();
    width::<2>();
    width::<3>();
    width::<4>();
    width::<16>();
    width::<31>();
    width::<32>();
    width::<33>();
    width::<64>();
    width::<257>();
}

#[test]
fn pubkey_is_a_const_sdk_independent_raw_value() {
    const KEY: Pubkey = Pubkey::from_bytes([0x81; 32]);
    const BYTES: [u8; 32] = KEY.to_bytes();
    assert_eq!(BYTES, [0x81; 32]);
    assert_eq!(KEY.as_ref(), BYTES);
    assert_eq!(KEY.encode(), BYTES);
    assert_eq!(KEY.encode_key(), BYTES);
    assert_eq!(encoded::<Pubkey>(&KEY), BYTES);
    exact_width::<Pubkey>();
    roundtrips(&[Pubkey::default(), KEY, Pubkey::from_bytes([0xff; 32])]);
    let mut high_first = [0; 32];
    high_first[0] = 1;
    let mut high_last = [0; 32];
    high_last[31] = 0xff;
    roundtrips(&[
        Pubkey::from_bytes(high_first),
        Pubkey::from_bytes(high_last),
    ]);
}

#[test]
fn builtin_descriptors_match_fixed_codecs() {
    fn fixed<T: Fixed + Schema>() {
        assert_eq!(T::KIND, Kind::Fixed);
        assert_eq!(T::FIXED_SIZE, Some(T::SIZE));
        assert_eq!(T::MIN_SIZE, T::SIZE);
        check_layout(T::LAYOUT, MAX_DEPTH).unwrap();
    }
    fixed::<u8>();
    fixed::<u16>();
    fixed::<u32>();
    fixed::<u64>();
    fixed::<u128>();
    fixed::<i8>();
    fixed::<i16>();
    fixed::<i32>();
    fixed::<i64>();
    fixed::<i128>();
    fixed::<bool>();
    fixed::<Pubkey>();
    fixed::<[u8; 257]>();
    assert!(matches!(i128::LAYOUT, Layout::Leaf(Leaf::Signed(16))));
    assert!(matches!(u128::LAYOUT, Layout::Leaf(Leaf::Unsigned(16))));
    assert!(matches!(bool::KEY_LAYOUT.domain, KeyDomain::Bool));
    assert!(matches!(i8::KEY_LAYOUT.domain, KeyDomain::Signed));
    assert!(matches!(u8::KEY_LAYOUT.domain, KeyDomain::Unsigned));
    assert!(matches!(Pubkey::KEY_LAYOUT.domain, KeyDomain::Raw));
    assert_eq!(<[u8; 257]>::KEY_LAYOUT.width, 257);
    assert_eq!(encoded::<i32>(&-2), [0xfe, 0xff, 0xff, 0xff]);
    assert_eq!(encoded::<u128>(&u128::MAX), [0xff; 16]);
    assert_eq!(encoded::<bool>(&true), [1]);
}

#[test]
fn strings_have_no_header_and_bytes_keep_the_count_word() {
    for text in ["", "ascii", "\0", "é", "中🦀"] {
        assert_eq!(encoded::<Str>(&text), text.as_bytes());
    }
    assert_eq!(Str::KIND, Kind::Variable);
    assert_eq!(Str::FIXED_SIZE, None);
    assert_eq!(Str::MIN_SIZE, 0);
    assert_eq!(encoded::<List<u8>>(&&[][..]), [0, 0, 0, 0]);
    assert_eq!(
        encoded::<List<u8>>(&&[0, 0xff, 3][..]),
        [3, 0, 0, 0, 0, 0xff, 3]
    );
    assert_eq!(List::<u8>::MIN_SIZE, 4);
    assert!(matches!(
        List::<u8>::LAYOUT,
        Layout::List {
            element: Layout::Leaf(Leaf::Unsigned(1))
        }
    ));
}

#[test]
fn options_are_variable_and_reserve_only_the_active_payload() {
    assert_eq!(<Option<u64>>::KIND, Kind::Variable);
    assert_eq!(<Option<u64>>::FIXED_SIZE, None);
    assert_eq!(<Option<u64>>::MIN_SIZE, 1);
    assert_eq!(encoded::<Option<Str>>(&None), [0]);
    assert_eq!(encoded::<Option<Str>>(&Some("")), [1]);
    assert_eq!(encoded::<Option<Str>>(&Some("é")), [1, 0xc3, 0xa9]);
    assert_eq!(encoded::<Option<u16>>(&Some(256)), [1, 0, 1]);
    assert_eq!(
        encoded::<Option<Option<bool>>>(&Some(Some(true))),
        [1, 1, 1]
    );
    assert_eq!(encoded::<Option<Option<bool>>>(&Some(None)), [1, 0]);
    for bad in [&[][..], &[0, 0], &[2], &[1, 2], &[1, 0, 0]] {
        assert!(validate(bad, <Option<bool>>::LAYOUT, MAX_DEPTH).is_err());
    }
}

#[test]
fn borrowed_lists_use_elided_ends_and_fixed_strides() {
    assert_eq!(encoded::<List<u16>>(&&[7, 9][..]), [2, 0, 0, 0, 7, 0, 9, 0]);
    assert_eq!(encoded::<List<Str>>(&&[][..]), [0, 0, 0, 0]);
    assert_eq!(encoded::<List<Str>>(&&[""][..]), [1, 0, 0, 0]);
    assert_eq!(
        encoded::<List<Str>>(&&["", ""][..]),
        [2, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        encoded::<List<Str>>(&&["ab", "", "xyz"][..]),
        [3, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, b'a', b'b', b'x', b'y', b'z']
    );
    assert_eq!(
        encoded::<List<Option<u8>>>(&&[None, Some(9)][..]),
        [2, 0, 0, 0, 1, 0, 0, 0, 0, 1, 9]
    );
    let inner: &[u8] = &[1, 2];
    assert_eq!(
        encoded::<List<List<u8>>>(&&[inner, &[]][..]),
        [2, 0, 0, 0, 6, 0, 0, 0, 2, 0, 0, 0, 1, 2, 0, 0, 0, 0]
    );
}

#[test]
fn borrowed_maps_interleave_fixed_values_and_store_variable_terminal_ends() {
    assert_eq!(
        encoded::<Map<u16, Str>>(&&[(1, "x"), (256, "")][..]),
        [2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, b'x']
    );
    assert_eq!(
        encoded::<Map<i16, u16>>(&&[(-1, 256), (0, 7)][..]),
        [2, 0, 0, 0, 0x7f, 0xff, 0, 1, 0x80, 0, 7, 0]
    );
    assert_eq!(
        encoded::<Map<bool, Str>>(&&[(false, ""), (true, "")][..]),
        [2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0]
    );
    assert_eq!(encoded::<Map<u128, Str>>(&&[][..]), [0, 0, 0, 0]);
    let nested: &[(i8, &str)] = &[(-1, ""), (1, "x")];
    encoded::<Option<List<Map<i8, Str>>>>(&Some(&[nested][..]));
}

#[test]
fn map_measure_rejects_unsorted_and_duplicate_inputs() {
    for (entries, kind) in [
        (&[(1u16, "a"), (1, "b")][..], ErrorKind::DuplicateKey),
        (&[(256, "a"), (1, "b")][..], ErrorKind::UnsortedKeys),
    ] {
        let before = entries.to_vec();
        assert_eq!(
            <Map<u16, Str>>::measure(&entries),
            Err(Error::new(kind, Error::NO_OFFSET))
        );
        assert_eq!(before, entries);
    }
    assert_eq!(
        <Map<i8, bool>>::measure(&&[(0, true), (-1, false)][..]),
        Err(Error::new(ErrorKind::UnsortedKeys, Error::NO_OFFSET))
    );
}

/// A test-only, allocation-free description of any number of UTF-8 NUL bytes.
struct Zeros;
impl Schema for Zeros {
    const KIND: Kind = Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static Layout = &Layout::Str;
    const __LEN: usize = <blubber::Str as blubber::Schema>::__LEN;
    const __HEADER: &'static [u32] = <blubber::Str as blubber::Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = &'a [u8];
    type Mut<'a> = blubber::raw::Edit<'a>;

    fn project<'a>(edit: blubber::__private::Projection<'a, Self>) -> Self::Mut<'a> {
        blubber::raw::into_edit(edit)
    }
    type Init<'a> = u32;
    type Removed = ();
    #[cfg(feature = "alloc")]
    type Owned = u32;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &u32) -> blubber::Result<u32> {
        Self::measure(value)
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &u32, destination: &mut [u8]) {
        Self::encode(value, destination);
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(view: blubber::View<'_>) -> blubber::Result<u32> {
        view.validate()?;
        if view.bytes().iter().any(|byte| *byte != 0) {
            return Err(blubber::ErrorKind::InvalidBoundary.into());
        }
        blubber_core::checked_u32(view.bytes().len())
    }

    fn read<'a>(view: blubber::View<'a>) -> blubber::Result<Self::Read<'a>> {
        view.validate()?;
        Ok(view.bytes())
    }

    fn read_removed(view: blubber::View<'_>) -> blubber::Result<Self::Removed> {
        view.validate()
    }

    fn measure(init: &u32) -> blubber::Result<u32> {
        Ok(*init)
    }
    fn encode(init: &u32, destination: &mut [u8]) {
        assert_eq!(*init as usize, destination.len());
        destination.fill(0);
    }
}

#[test]
fn measure_checks_full_width_extents_without_allocating_them() {
    let overflow = Err(Error::new(ErrorKind::ArithmeticOverflow, Error::NO_OFFSET));
    assert_eq!(<Option<Zeros>>::measure(&Some(u32::MAX - 1)), Ok(u32::MAX));
    assert_eq!(<Option<Zeros>>::measure(&Some(u32::MAX)), overflow);
    assert_eq!(<List<Zeros>>::measure(&&[u32::MAX - 4][..]), Ok(u32::MAX));
    assert_eq!(<List<Zeros>>::measure(&&[u32::MAX - 3][..]), overflow);
    assert_eq!(
        <List<Zeros>>::measure(&&[u32::MAX - 8, 0][..]),
        Ok(u32::MAX)
    );
    assert_eq!(<List<Zeros>>::measure(&&[u32::MAX - 8, 1][..]), overflow);
    assert_eq!(
        <Map<u16, Zeros>>::measure(&&[(1, u32::MAX - 10)][..]),
        Ok(u32::MAX)
    );
    assert_eq!(
        <Map<u16, Zeros>>::measure(&&[(1, u32::MAX - 9)][..]),
        overflow
    );
    assert_eq!(
        <Map<u16, Zeros>>::measure(&&[(1, u32::MAX - 16), (2, 0)][..]),
        Ok(u32::MAX)
    );
    assert_eq!(
        <Map<u16, Zeros>>::measure(&&[(1, u32::MAX - 16), (2, 1)][..]),
        overflow
    );
}

#[test]
fn descriptor_depth_is_checked_even_for_absent_values() {
    type D1 = Option<u8>;
    type D2 = Option<D1>;
    type D4 = Option<Option<D2>>;
    type D8 = Option<Option<Option<Option<D4>>>>;
    type D16 = Option<Option<Option<Option<Option<Option<Option<Option<D8>>>>>>>>;
    assert!(<D4>::measure(&None).is_ok());
    assert_eq!(<D8>::measure(&None).is_ok(), MAX_DEPTH >= 9);
    assert_eq!(
        <D16>::measure(&None),
        Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET))
    );
    assert_eq!(
        Map::<u16, D16>::measure(&[].as_slice()),
        Err(Error::new(ErrorKind::InvalidSchema, Error::NO_OFFSET))
    );
}

#[test]
#[cfg(feature = "pinocchio")]
fn sdk_addresses_roundtrip_as_values_keys_and_compatibility_pubkeys() {
    use pinocchio::Address;
    let addresses = [
        Address::new_from_array([0; 32]),
        Address::new_from_array([7; 32]),
        Address::new_from_array([255; 32]),
    ];
    roundtrips(&addresses);
    for address in addresses {
        let key = Pubkey::from(address);
        assert_eq!(Address::from(key), address);
        assert_eq!(address.encode(), key.encode());
        assert_eq!(address.encode_key(), key.encode_key());
        assert_eq!(encoded::<Address>(&address), address.to_bytes());
    }
    assert_eq!(<Address as MapKey>::KEY_LAYOUT.width, 32);
    assert!(matches!(
        <Address as MapKey>::KEY_LAYOUT.domain,
        KeyDomain::Raw
    ));
    assert!(matches!(
        <Address as Schema>::LAYOUT,
        Layout::Leaf(Leaf::Raw(32))
    ));
    assert_eq!(
        Address::decode(&[0; 31]),
        Err(Error::new(ErrorKind::Truncated, 31))
    );
    assert_eq!(
        Address::decode_key(&[0; 33]),
        Err(Error::new(ErrorKind::InvalidBoundary, 32))
    );
}
