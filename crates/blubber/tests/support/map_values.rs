//! Values-only iteration checks slots without decoding key bytes.
#![forbid(unsafe_code)]

use blubber::{Error, ErrorKind, Fixed, Map, MapKey, MapRef, Schema, View};
use core::iter::FusedIterator;

std::thread_local! {
    static VALUES: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn map<K: MapKey, V: Schema>(bytes: &[u8], base: u32) -> MapRef<'_, K, V> {
    View::new(bytes, <Map<K, V>>::LAYOUT, base)
        .unwrap()
        .read::<Map<K, V>>()
        .unwrap()
}

#[test]
fn values_ignore_all_bool_key_bytes_and_continue_after_each_value_error() {
    for (key, value) in (0..=255)
        .map(|key| (key, 1))
        .chain((0..=255).map(|value| (255, value)))
    {
        let bytes = [3, 0, 0, 0, key, 1, key, value, key, 0];
        let map = map::<bool, bool>(&bytes, 37);
        let mut values = map.try_values();
        assert_eq!(values.size_hint(), (3, Some(3)));
        assert_eq!(values.next(), Some(Ok(true)));
        let expected = if value <= 1 {
            Ok(value != 0)
        } else {
            Err(Error::new(ErrorKind::InvalidBool, 44))
        };
        assert_eq!(values.next(), Some(expected));
        assert_eq!(values.len(), 1);
        assert_eq!(values.next(), Some(Ok(false)));
        assert_eq!(values.next(), None);
        assert_eq!(values.clone().next(), None);
        assert_eq!(values.next(), None);
        if key > 1 {
            let mut ordinary = map.try_iter();
            assert_eq!(
                ordinary.next(),
                Some(Err(Error::new(ErrorKind::InvalidBool, 41)))
            );
            assert_eq!(ordinary.next(), None);
        }
    }
}

#[test]
fn values_empty_singleton_partial_clones_and_full_width_bases() {
    fn fused<T: FusedIterator + ExactSizeIterator + Clone>(_: &T) {}
    let empty = [0; 4];
    let mut iter = map::<u8, u64>(&empty, u32::MAX - 4).try_values();
    fused(&iter);
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    let one = [1, 0, 0, 0, 255, 2];
    let mut iter = map::<bool, bool>(&one, u32::MAX - 6).try_values();
    let mut cloned = iter.clone();
    assert_eq!(
        iter.next(),
        Some(Err(Error::new(ErrorKind::InvalidBool, u32::MAX - 1)))
    );
    assert_eq!(
        cloned.next(),
        Some(Err(Error::new(ErrorKind::InvalidBool, u32::MAX - 1)))
    );
    assert_eq!(iter.next(), None);
    assert_eq!(cloned.next(), None);
    let bytes = [3, 0, 0, 0, 255, 10, 254, 20, 253, 30];
    let mut iter = map::<u8, u8>(&bytes, 19).try_values();
    assert_eq!(iter.next(), Some(Ok(10)));
    assert_eq!(iter.clone().collect::<Vec<_>>(), [Ok(20), Ok(30)]);
    assert_eq!(iter.collect::<Vec<_>>(), [Ok(20), Ok(30)]);
}

#[test]
fn values_check_actual_widths_for_empty_and_nonempty_tree_and_flat_layouts() {
    for bytes in [&[0u8, 0, 0, 0][..], &[1, 0, 0, 0, 8, 9, 10][..]] {
        let view = View::new(bytes, <Map<u16, u8>>::LAYOUT, 17).unwrap();
        let mut iter = blubber::MapRef::<u8, u8>::from_view(view)
            .unwrap()
            .try_values();
        assert_eq!(iter.size_hint(), (1, Some(1)));
        assert_eq!(iter.next(), Some(Err(ErrorKind::InvalidSchema.into())));
        assert_eq!(iter.size_hint(), (0, Some(0)));
        assert_eq!(iter.next(), None);
        let flat = View::new(
            bytes,
            blubber_core::Layout::Flat {
                checked: blubber::__layout_of!(Map<u16, u8>),
            },
            17,
        )
        .unwrap();
        let mut iter = blubber::MapRef::<u8, u8>::from_view(flat)
            .unwrap()
            .try_values();
        assert_eq!(iter.next(), Some(Err(ErrorKind::InvalidSchema.into())));
        assert_eq!(iter.next(), None);
    }
    let bytes = [1, 0, 0, 0, 7, 9, 10];
    let view = View::new(&bytes, <Map<u8, u16>>::LAYOUT, 17).unwrap();
    let mut iter = blubber::MapRef::<u8, u8>::from_view(view)
        .unwrap()
        .try_values();
    assert_eq!(iter.next(), Some(Err(ErrorKind::InvalidSchema.into())));
    assert_eq!(iter.next(), None);
    for layout in [
        *<Map<bool, bool>>::LAYOUT,
        blubber_core::Layout::Flat {
            checked: blubber::__layout_of!(Map<bool, bool>),
        },
    ] {
        let bytes = [2, 0, 0, 0, 255, 2, 254, 1];
        let view = View::new(&bytes, layout, 17).unwrap();
        let mut iter = MapRef::<bool, bool>::from_view(view).unwrap().try_values();
        assert_eq!(
            iter.next(),
            Some(Err(Error::new(ErrorKind::InvalidBool, 22)))
        );
        assert_eq!(iter.next(), Some(Ok(true)));
        assert_eq!(iter.next(), None);
    }
    for bytes in [&[0u8, 0, 0, 0][..], &[1, 0, 0, 0, 9, 9, 10][..]] {
        let view = View::new(
            bytes,
            blubber_core::Layout::Flat {
                checked: blubber::__layout_of!(Map<u8, u16>),
            },
            17,
        )
        .unwrap();
        let mut iter = MapRef::<u8, u8>::from_view(view).unwrap().try_values();
        assert_eq!(iter.next(), Some(Err(ErrorKind::InvalidSchema.into())));
        assert_eq!(iter.next(), None);
    }
    for layout in [
        *<Map<u8, blubber::Str>>::LAYOUT,
        blubber_core::Layout::Flat {
            checked: blubber::__layout_of!(Map<u8, blubber::Str>),
        },
    ] {
        for bytes in [&[0u8, 0, 0, 0][..], &[1, 0, 0, 0, 9, 1, 0, 0, 0, b'a'][..]] {
            let view = View::new(bytes, layout, 17).unwrap();
            let mut iter = MapRef::<u8, u8>::from_view(view).unwrap().try_values();
            assert_eq!(iter.next(), Some(Err(ErrorKind::InvalidSchema.into())));
            assert_eq!(iter.next(), None);
        }
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct UnreadKey;
impl Fixed for UnreadKey {
    const SIZE: u32 = 1;
    type Encoded = [u8; 1];
    fn decode(_: &[u8]) -> blubber::Result<Self> {
        panic!("key decode called")
    }
    fn encode(self) -> Self::Encoded {
        panic!("key encode called")
    }
}
impl MapKey for UnreadKey {
    const KEY_LAYOUT: blubber_core::KeyLayout = <u8 as MapKey>::KEY_LAYOUT;
    type KeyBytes = [u8; 1];
    fn encode_key(self) -> Self::KeyBytes {
        panic!("key encode called")
    }
    fn decode_key(_: &[u8]) -> blubber::Result<Self> {
        panic!("key decode called")
    }
}

#[test]
fn values_never_call_key_codecs_and_preserve_immutable_frames() {
    let bytes = [2, 0, 0, 0, 255, 7, 254, 9];
    let before = bytes;
    let view = View::new(&bytes, <Map<u8, u8>>::LAYOUT, 23).unwrap();
    let map = MapRef::<UnreadKey, u8>::from_view(view).unwrap();
    assert_eq!(map.try_values().collect::<Vec<_>>(), [Ok(7), Ok(9)]);
    assert_eq!(bytes, before);
    let ordinary = std::panic::catch_unwind(|| map.try_iter().next());
    assert!(ordinary.is_err());
}

#[cfg(feature = "derive")]
#[test]
fn values_fixed_records_keep_nested_field_offsets() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, blubber::Fixed)]
    struct Record {
        number: u16,
        enabled: bool,
    }
    let bytes = [2, 0, 0, 0, 255, 0x34, 0x12, 2, 254, 0x78, 0x56, 1];
    let mut iter = map::<u8, Record>(&bytes, 29).try_values();
    assert_eq!(
        iter.next(),
        Some(Err(Error::new(ErrorKind::InvalidBool, 36)))
    );
    assert_eq!(
        iter.next(),
        Some(Ok(Record {
            number: 0x5678,
            enabled: true
        }))
    );
    assert_eq!(iter.next(), None);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EffectValue(u8);

impl Fixed for EffectValue {
    const SIZE: u32 = 1;
    type Encoded = [u8; 1];
    fn decode(bytes: &[u8]) -> blubber::Result<Self> {
        let value = u8::decode(bytes)?;
        VALUES.with(|calls| calls.borrow_mut().push(value));
        match value {
            252 => Err(ErrorKind::InvalidTag.into()),
            253 => Err(Error::Program { code: 17 }),
            254 => panic!("value decoder unwind"),
            255 => Err(Error::new(ErrorKind::InvalidBool, 0)),
            _ => Ok(Self(value)),
        }
    }
    fn encode(self) -> Self::Encoded {
        [self.0]
    }
}

impl Schema for EffectValue {
    const KIND: blubber::Kind = blubber::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <u8 as Schema>::LAYOUT;
    const __LEN: usize = <u8 as Schema>::__LEN;
    const __HEADER: &'static [u32] = <u8 as Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = Self;
    type Mut<'a> = blubber::raw::Edit<'a>;
    type Init<'a> = Self;
    type Removed = Self;
    fn project(edit: blubber::__private::Projection<'_, Self>) -> Self::Mut<'_> {
        blubber::raw::into_edit(edit)
    }
    fn read(view: View<'_>) -> blubber::Result<Self> {
        view.read_fixed::<Self>()
    }
    fn read_removed(view: View<'_>) -> blubber::Result<Self> {
        Self::read(view)
    }
    fn measure(_: &Self) -> blubber::Result<u32> {
        Ok(1)
    }
    fn encode(value: &Self, bytes: &mut [u8]) {
        bytes.copy_from_slice(&[value.0]);
    }
    blubber::__private::with_alloc! {
        type Owned = Self;
        fn measure_owned(value: &Self) -> blubber::Result<u32> { Self::measure(value) }
        fn encode_owned(value: &Self, bytes: &mut [u8]) { <Self as Schema>::encode(value, bytes); }
        fn decode_owned(view: View<'_>) -> blubber::Result<Self> { Self::read(view) }
    }
}

#[test]
fn values_decode_once_preserve_error_kinds_and_consume_before_unwind() {
    VALUES.with(|calls| calls.borrow_mut().clear());
    let bytes = [5, 0, 0, 0, 9, 252, 8, 253, 7, 254, 6, 255, 5, 7];
    let mut iter = map::<UnreadKey, EffectValue>(&bytes, 53).try_values();
    assert_eq!(iter.next(), Some(Err(ErrorKind::InvalidTag.into())));
    assert_eq!(iter.next(), Some(Err(Error::Program { code: 17 })));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| iter.next()));
    assert!(result.is_err());
    assert_eq!(iter.len(), 2);
    assert_eq!(
        iter.next(),
        Some(Err(Error::new(ErrorKind::InvalidBool, 64)))
    );
    assert_eq!(iter.next(), Some(Ok(EffectValue(7))));
    assert_eq!(iter.next(), None);
    VALUES.with(|calls| assert_eq!(*calls.borrow(), [252, 253, 254, 255, 7]));
}
