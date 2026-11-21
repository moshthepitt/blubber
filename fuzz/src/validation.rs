//! Complete validation plus bounded lazy access, including corrupt states.

use super::{account, bridge::Bridge, Packed, Record, ID};
use blubber::{Address, List, Map, MapKey, Schema, Str, View};

pub(crate) trait Walk: Schema {
    fn visit(value: Self::Read<'_>, budget: &mut usize);
}

macro_rules! leaves {
    ($($ty:ty),*) => { $(impl Walk for $ty {
        fn visit(_: Self::Read<'_>, _: &mut usize) {}
    })* };
}
leaves!(bool, u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, Address, Str, Packed);
impl<const N: usize> Walk for [u8; N] {
    fn visit(_: Self::Read<'_>, _: &mut usize) {}
}
impl<T: Walk> Walk for Option<T> {
    fn visit(value: Self::Read<'_>, budget: &mut usize) {
        if let Some(value) = value {
            T::visit(value, budget);
        }
    }
}
impl<T: Walk> Walk for List<T> {
    fn visit(value: Self::Read<'_>, budget: &mut usize) {
        let count = value.try_len().unwrap();
        for index in [0, count / 2, count.saturating_sub(1), count, usize::MAX] {
            if let Ok(Some(child)) = value.try_get(index) {
                T::visit(child, budget);
            }
        }
        let mut iter = value.try_iter();
        while *budget > 0 {
            *budget -= 1;
            match iter.next() {
                Some(Ok(child)) => T::visit(child, budget),
                Some(Err(_)) | None => {
                    assert!(iter.next().is_none());
                    assert!(iter.next().is_none());
                    break;
                }
            }
        }
    }
}
impl<K: MapKey + Default, T: Walk> Walk for Map<K, T> {
    fn visit(value: Self::Read<'_>, budget: &mut usize) {
        let _ = value.try_get(&K::default());
        let _ = value.try_contains_key(&K::default());
        let mut iter = value.try_iter();
        while *budget > 0 {
            *budget -= 1;
            match iter.next() {
                Some(Ok((key, child))) => {
                    let _ = value.try_get(&key);
                    T::visit(child, budget);
                }
                Some(Err(_)) | None => {
                    assert!(iter.next().is_none());
                    assert!(iter.next().is_none());
                    break;
                }
            }
        }
    }
}
impl Walk for Record {
    fn visit(value: Self::Read<'_>, budget: &mut usize) {
        let _ = value.try_name();
        let _ = value.try_flag();
        let _ = value.try_memo();
        if let Ok(tags) = value.try_tags() {
            List::<u16>::visit(tags, budget);
        }
    }
}

fn lazy<S: Walk>(view: View<'_>) {
    if let Ok(value) = view.read::<S>() {
        S::visit(value, &mut 256);
    }
}

pub(crate) fn check<S: Bridge + Walk + blubber::RootSchema>(bytes: &[u8], probe: u8) {
    let shape = S::schema();
    let expected = shape.decode(bytes);
    let view = View::new(bytes, S::LAYOUT, 0).unwrap();
    let actual = view.validate();
    assert_eq!(
        actual.is_ok(),
        expected.is_ok(),
        "validation differs: {shape:?}"
    );
    lazy::<S>(view);
    if let Ok(value) = expected {
        assert_eq!(S::value(S::decode_owned(view).unwrap()), value);
        assert_eq!(shape.encode(&value).unwrap(), bytes);
    }
    let base = match probe % 3 {
        0 => 12,
        1 => u32::MAX,
        _ => u32::MAX - bytes.len() as u32,
    };
    if let Ok(view) = View::new(bytes, S::LAYOUT, base) {
        let _ = view.validate();
        lazy::<S>(view);
    }
    for bytes in [bytes, &account(bytes)] {
        let expected = shape.decode_account(ID, bytes);
        let actual = View::load::<S>(bytes, ID);
        assert_eq!(
            actual.is_ok(),
            expected.is_ok(),
            "account validation differs: {shape:?}"
        );
        if let Ok(view) = View::load_trusted::<S>(bytes, ID) {
            lazy::<S>(view);
        }
        if let Ok(view) = actual {
            assert_eq!(
                S::value(S::decode_owned(view).unwrap()),
                expected.unwrap().value
            );
        }
    }
}
