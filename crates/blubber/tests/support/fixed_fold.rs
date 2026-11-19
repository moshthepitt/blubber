//! Consuming fixed-map folds preserve stepping, codec effects and unwinding.
#![forbid(unsafe_code)]
// Repeated next calls are the independent oracle for the consuming fold.
#![allow(clippy::while_let_on_iterator)]

use blubber::{Error, ErrorKind, Fixed, Map, MapKey, Schema, View};
use std::{cell::RefCell, panic::AssertUnwindSafe};

std::thread_local! {
    static EVENTS: RefCell<Vec<(u8, u32)>> = const { RefCell::new(Vec::new()) };
}

fn event(kind: u8, value: u32) {
    EVENTS.with(|events| events.borrow_mut().push((kind, value)));
}

fn take_events() -> Vec<(u8, u32)> {
    EVENTS.with(|events| core::mem::take(&mut *events.borrow_mut()))
}

fn repeated_fold<I: Iterator, A>(mut iter: I, initial: A, mut f: impl FnMut(A, I::Item) -> A) -> A {
    let mut accumulator = initial;
    while let Some(item) = iter.next() {
        accumulator = f(accumulator, item);
    }
    accumulator
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Key(u8);

impl Fixed for Key {
    const SIZE: u32 = 1;
    type Encoded = [u8; 1];
    fn decode(_: &[u8]) -> blubber::Result<Self> {
        panic!("map fold must use the key codec")
    }
    fn encode(self) -> Self::Encoded {
        [self.0]
    }
}

impl MapKey for Key {
    const KEY_LAYOUT: blubber_core::KeyLayout = <u8 as MapKey>::KEY_LAYOUT;
    type KeyBytes = [u8; 1];
    fn encode_key(self) -> Self::KeyBytes {
        panic!("map fold must remain query-free")
    }
    fn decode_key(bytes: &[u8]) -> blubber::Result<Self> {
        let byte = u8::decode_key(bytes)?;
        event(b'k', u32::from(byte));
        match byte {
            252 => Err(Error::Program { code: 17 }),
            253 => Err(ErrorKind::InvalidTag.into()),
            254 => panic!("key decoder unwind"),
            255 => Err(Error::new(ErrorKind::InvalidTag, 0)),
            _ => Ok(Self(byte)),
        }
    }
}

struct Value;
impl Schema for Value {
    const KIND: blubber::Kind = blubber::Kind::Fixed;
    const FIXED_SIZE: Option<u32> = Some(1);
    const MIN_SIZE: u32 = 1;
    const LAYOUT: &'static blubber_core::Layout = <u8 as Schema>::LAYOUT;
    const __LEN: usize = <u8 as Schema>::__LEN;
    const __HEADER: &'static [u32] = <u8 as Schema>::__HEADER;
    type __Children = blubber::__private::Nil;
    type Read<'a> = &'a [u8];
    type Mut<'a> = blubber::raw::Edit<'a>;
    type Init<'a> = u8;
    type Removed = ();
    fn project(edit: blubber::__private::Projection<'_, Self>) -> Self::Mut<'_> {
        blubber::raw::into_edit(edit)
    }
    fn read(view: View<'_>) -> blubber::Result<Self::Read<'_>> {
        event(b'v', view.base());
        assert_ne!(view.bytes()[0], 254, "value decoder unwind");
        if view.bytes()[0] == 255 {
            return Err(Error::new(ErrorKind::InvalidTag, view.base()));
        }
        Ok(view.bytes())
    }
    fn read_removed(view: View<'_>) -> blubber::Result<()> {
        Self::read(view).map(|_| ())
    }
    fn measure(value: &u8) -> blubber::Result<u32> {
        <u8 as Schema>::measure(value)
    }
    fn encode(value: &u8, bytes: &mut [u8]) {
        <u8 as Schema>::encode(value, bytes);
    }
    blubber::__private::with_alloc! {
        type Owned = u8;
        fn measure_owned(value: &u8) -> blubber::Result<u32> { Self::measure(value) }
        fn encode_owned(value: &u8, bytes: &mut [u8]) { Self::encode(value, bytes); }
        fn decode_owned(view: View<'_>) -> blubber::Result<u8> { Ok(Self::read(view)?[0]) }
    }
}

fn map(bytes: &[u8], base: u32) -> blubber::MapRef<'_, Key, Value> {
    View::new(bytes, <Map<u8, u8>>::LAYOUT, base)
        .unwrap()
        .read::<Map<Key, Value>>()
        .unwrap()
}

#[test]
fn map_fold_matches_next_for_empty_singleton_partial_and_exhausted_states() {
    for count in 0..=4 {
        let mut bytes = (count as u32).to_le_bytes().to_vec();
        for i in 0..count {
            bytes.extend([i as u8, 9 + i as u8]);
        }
        for base in [0, 91, u32::MAX - bytes.len() as u32] {
            let map = map(&bytes, base);
            for consumed in 0..=count {
                let mut iter = map.iter();
                for _ in 0..consumed {
                    iter.next().unwrap();
                }
                take_events();
                let mut repeated = iter.clone();
                let mut expected = Vec::new();
                while let Some((key, value)) = repeated.next() {
                    event(b'f', u32::from(key.0));
                    expected.push((key, value));
                }
                assert!(repeated.next().is_none());
                assert!(repeated.next().is_none());
                assert_eq!(repeated.size_hint(), (0, Some(0)));
                let expected_events = take_events();
                let actual = iter.fold(Vec::new(), |mut out, (key, value)| {
                    event(b'f', u32::from(key.0));
                    out.push((key, value));
                    out
                });
                assert_eq!(actual, expected);
                assert_eq!(take_events(), expected_events);
                for (i, (_, value)) in actual.iter().enumerate() {
                    assert_eq!(value.as_ptr(), bytes[5 + 2 * (consumed + i)..].as_ptr());
                }
                assert_eq!(repeated.fold(19, |acc, _| acc + 1), 19);
            }
        }
    }
}

#[test]
fn map_fold_stops_at_the_same_codec_or_callback_unwind() {
    for slot in 0..3 {
        for key in [false, true] {
            for fault in [252, 253, 254, 255] {
                if !key && fault < 254 {
                    continue;
                }
                let mut bytes = [3, 0, 0, 0, 1, 7, 2, 8, 3, 9];
                bytes[4 + 2 * slot + usize::from(!key)] = fault;
                let mut expected_panic = None;
                for fold in [false, true] {
                    take_events();
                    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        let iter = map(&bytes, 71).iter();
                        if fold {
                            iter.fold((), |(), (key, _)| event(b'f', u32::from(key.0)));
                        } else {
                            let mut iter = iter;
                            while let Some((key, _)) = iter.next() {
                                event(b'f', u32::from(key.0));
                            }
                            unreachable!("fault must stop stepping");
                        }
                    }));
                    assert!(result.is_err());
                    let payload = result.unwrap_err();
                    let message = payload
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| payload.downcast_ref::<&str>().copied())
                        .unwrap();
                    if let Some(expected) = &expected_panic {
                        assert_eq!(message, expected);
                    }
                    expected_panic = Some(message.to_owned());
                    let actual = take_events();
                    let mut expected = Vec::new();
                    for i in 0..slot {
                        expected.extend([
                            (b'k', i as u32 + 1),
                            (b'v', 76 + 2 * i as u32),
                            (b'f', i as u32 + 1),
                        ]);
                    }
                    expected.push((
                        b'k',
                        if key {
                            u32::from(fault)
                        } else {
                            slot as u32 + 1
                        },
                    ));
                    if !key {
                        expected.push((b'v', 76 + 2 * slot as u32));
                    }
                    assert_eq!(actual, expected);
                }
            }
        }
        for fold in [false, true] {
            take_events();
            let bytes = [3, 0, 0, 0, 1, 7, 2, 8, 3, 9];
            let mut callbacks = 0;
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let callback = |(), (key, _): (Key, &[u8])| {
                    event(b'f', u32::from(key.0));
                    let at = callbacks;
                    callbacks += 1;
                    assert_ne!(at, slot, "callback unwind");
                };
                if fold {
                    map(&bytes, 71).iter().fold((), callback);
                } else {
                    let mut callback = callback;
                    let mut iter = map(&bytes, 71).iter();
                    while let Some(item) = iter.next() {
                        callback((), item);
                    }
                }
            }));
            assert!(result.is_err());
            assert_eq!(callbacks, slot + 1);
            let expected: Vec<_> = (0..=slot)
                .flat_map(|i| {
                    [
                        (b'k', i as u32 + 1),
                        (b'v', 76 + 2 * i as u32),
                        (b'f', i as u32 + 1),
                    ]
                })
                .collect();
            assert_eq!(take_events(), expected);
        }
    }
}

#[test]
fn map_fold_preserves_bool_faults_and_once_only_rebasing() {
    for key in 0..=2 {
        for value in 0..=2 {
            let bytes = [2, 0, 0, 0, 0, 1, key, value];
            for base in [0, 91, u32::MAX - bytes.len() as u32] {
                let map = View::new(&bytes, <Map<bool, bool>>::LAYOUT, base)
                    .unwrap()
                    .read::<Map<bool, bool>>()
                    .unwrap();
                let mut callbacks = Vec::new();
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    map.iter().fold((), |(), pair| callbacks.push(pair));
                }));
                assert_eq!(result.is_err(), key > 1 || value > 1);
                assert_eq!(callbacks[0], (false, true));
                assert_eq!(callbacks.len(), if result.is_err() { 1 } else { 2 });
                let mut iter = map.try_iter();
                assert_eq!(iter.next(), Some(Ok((false, true))));
                let expected = if key > 1 {
                    Err(Error::new(ErrorKind::InvalidBool, base + 6))
                } else if value > 1 {
                    Err(Error::new(ErrorKind::InvalidBool, base + 7))
                } else {
                    Ok((key != 0, value != 0))
                };
                assert_eq!(iter.next(), Some(expected));
                assert_eq!(iter.next(), None);
                assert_eq!(iter.clone().next(), None);
                assert_eq!(iter.next(), None);
            }
        }
    }
    for byte in [252, 253, 255] {
        let bytes = [1, 0, 0, 0, byte, 7];
        take_events();
        let mut iter = map(&bytes, 91).try_iter();
        let expected = match byte {
            252 => Error::Program { code: 17 },
            253 => ErrorKind::InvalidTag.into(),
            _ => Error::new(ErrorKind::InvalidTag, 95),
        };
        assert_eq!(iter.next(), Some(Err(expected)));
        assert_eq!(iter.next(), None);
        assert_eq!(take_events(), [(b'k', u32::from(byte))]);
    }
}

#[test]
fn map_fold_fallback_keeps_variable_and_foreign_descriptors() {
    let bytes = super::support::encoded::<Map<u16, blubber::Str>>(&&[(1, ""), (2, "é")][..]);
    let map = super::support::read::<Map<u16, blubber::Str>>(&bytes).unwrap();
    assert_eq!(
        map.iter().fold(Vec::new(), |mut out, pair| {
            out.push(pair);
            out
        }),
        [(1, ""), (2, "é")]
    );
    let bytes = super::support::encoded::<Map<u16, u32>>(&&[(1, 7)][..]);
    let map = View::new(&bytes, <Map<u16, u32>>::LAYOUT, 29)
        .unwrap()
        .read::<Map<u8, u32>>()
        .unwrap();
    assert!(std::panic::catch_unwind(|| map.iter().fold(0usize, |count, _| count + 1)).is_err());
    let bytes = [1, 0, 0, 0, 7, 9];
    take_events();
    let map = map_for_inherited_value(&bytes);
    assert_eq!(map.iter().fold(0, |_, (_, value)| value[0]), 9);
    assert_eq!(take_events(), [(b'v', 34)]);
}

fn map_for_inherited_value(bytes: &[u8]) -> blubber::MapRef<'_, u8, Value> {
    View::new(bytes, <Map<u8, i8>>::LAYOUT, 29)
        .unwrap()
        .read::<Map<u8, Value>>()
        .unwrap()
}

#[test]
fn map_fold_drops_the_accumulator_once_on_each_unwind_path() {
    struct Accumulator(u32);
    impl Drop for Accumulator {
        fn drop(&mut self) {
            event(b'd', self.0);
        }
    }
    struct CallbackGuard;
    impl CallbackGuard {
        fn observe(&self) {
            event(b'g', 0);
        }
    }
    impl Drop for CallbackGuard {
        fn drop(&mut self) {
            event(b'g', 1);
        }
    }
    for stop in 0..=3 {
        let mut expected = None;
        for fold in [false, true] {
            let mut bytes = [3, 0, 0, 0, 1, 7, 2, 8, 3, 9];
            if stop == 0 {
                bytes[6] = 254;
            }
            if stop == 1 {
                bytes[7] = 254;
            }
            take_events();
            let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let iter = map(&bytes, 71).iter();
                let guard = CallbackGuard;
                let callback = move |mut acc: Accumulator, (key, _): (Key, &[u8])| {
                    guard.observe();
                    acc.0 += 1;
                    event(b'f', u32::from(key.0));
                    assert!(stop != 2 || key.0 != 2, "callback unwind");
                    acc
                };
                let acc = Accumulator(0);
                let result = if fold {
                    iter.fold(acc, callback)
                } else {
                    repeated_fold(iter, acc, callback)
                };
                drop(result);
            }));
            assert_eq!(result.is_err(), stop != 3);
            let events = take_events();
            assert_eq!(events.iter().filter(|(kind, _)| *kind == b'd').count(), 1);
            if let Some(expected) = &expected {
                assert_eq!(&events, expected);
            }
            expected = Some(events);
        }
    }
}
