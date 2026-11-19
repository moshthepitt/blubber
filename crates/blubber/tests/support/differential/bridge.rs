//! Typed production dispatch and native conversions, outside the edit oracle.

#![forbid(unsafe_code)]

use blubber::{List, ListMut, Map, MapKey, Pubkey, Schema, Str};
use blubber_model::{
    ApiEdit, ApiReturn, Callbacks, Edit as ModelEdit, IntWidth, Key, KeySchema,
    PathSegment as Segment, Schema as ModelSchema, Value,
};

// Replay and differential fixtures compile different subsets of this bridge.
#[allow(dead_code)]
#[path = "api_bridge.rs"]
mod api_dispatch;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Observed {
    Unit,
    Old(Option<Value>),
    Present(bool),
}

pub(crate) trait Bridge: Schema {
    #[allow(dead_code)]
    fn value_from_read(_value: Self::Read<'_>) -> Value {
        panic!("missing read adapter")
    }
    #[allow(dead_code)]
    fn snapshot(_handle: &Self::Mut<'_>) -> blubber::Result<Value> {
        panic!("missing snapshot adapter")
    }

    #[allow(dead_code)]
    fn loan<'s>(_handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
        panic!("schema has no lending adapter")
    }
    #[allow(dead_code)]
    fn default_entry<'a, K: MapKey>(
        _entry: blubber::Entry<'a, K, Self>,
    ) -> blubber::Result<Self::Mut<'a>> {
        panic!("derived initializers have no Default contract")
    }
    #[allow(dead_code)]
    fn api(
        _handle: Self::Mut<'_>,
        _path: &[Segment],
        _edit: &ApiEdit,
        _calls: &mut Callbacks,
        _destination: &mut Option<Vec<u8>>,
    ) -> blubber::Result<ApiReturn> {
        panic!("API operation requires a container")
    }
    fn schema() -> ModelSchema;
    fn owned(value: &Value) -> Self::Owned;
    fn value(value: Self::Owned) -> Value;

    fn borrowed<'a>(value: &'a Value, arena: &'a bumpalo::Bump) -> Self::Init<'a>;
    fn removed(value: Option<Self::Removed>) -> Observed;
    fn apply(
        handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &ModelEdit,
    ) -> blubber::Result<Observed>;
}

trait Input: Bridge {
    fn input(value: &Self::Owned) -> Self::Init<'_>;
}

trait NativeKey: MapKey {
    fn schema() -> KeySchema;
    fn owned(value: &Key) -> Self;
    fn value(value: Self) -> Key;
}

macro_rules! integer {
    ($ty:ty, $variant:ident, $width:ident) => {
        impl Bridge for $ty {
            fn value_from_read(value: Self::Read<'_>) -> Value {
                <Self as Bridge>::value(value)
            }
            fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
                handle.try_get().map(<Self as Bridge>::value)
            }

            fn default_entry<'a, K0: MapKey>(
                entry: blubber::Entry<'a, K0, Self>,
            ) -> blubber::Result<Self::Mut<'a>> {
                entry.or_default()
            }

            fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
                handle.reborrow()
            }
            fn schema() -> ModelSchema {
                ModelSchema::$variant(IntWidth::$width)
            }
            fn owned(value: &Value) -> Self {
                let Value::$variant(value) = value else {
                    panic!("integer value")
                };
                (*value).try_into().unwrap()
            }
            fn value(value: Self) -> Value {
                Value::$variant(value.into())
            }

            fn borrowed<'a>(value: &'a Value, _arena: &'a bumpalo::Bump) -> Self::Init<'a> {
                <Self as Bridge>::owned(value)
            }
            fn removed(value: Option<Self::Removed>) -> Observed {
                Observed::Old(value.map(<Self as Bridge>::value))
            }
            fn apply(
                mut handle: Self::Mut<'_>,
                path: &[Segment],
                edit: &ModelEdit,
            ) -> blubber::Result<Observed> {
                assert!(path.is_empty());
                let ModelEdit::Set(value) = edit else {
                    panic!("scalar edit")
                };
                handle.set(<Self as Bridge>::owned(value))?;
                Ok(Observed::Unit)
            }
        }
        impl Input for $ty {
            fn input(value: &Self) -> Self {
                *value
            }
        }
        impl NativeKey for $ty {
            fn schema() -> KeySchema {
                KeySchema::$variant(IntWidth::$width)
            }
            fn owned(value: &Key) -> Self {
                let Key::$variant(value) = value else {
                    panic!("integer key")
                };
                (*value).try_into().unwrap()
            }
            fn value(value: Self) -> Key {
                Key::$variant(value.into())
            }
        }
    };
}

integer!(u8, Unsigned, W1);
integer!(u16, Unsigned, W2);
integer!(u32, Unsigned, W4);
integer!(u64, Unsigned, W8);
integer!(u128, Unsigned, W16);
integer!(i8, Signed, W1);
integer!(i16, Signed, W2);
integer!(i32, Signed, W4);
integer!(i64, Signed, W8);
integer!(i128, Signed, W16);

macro_rules! fixed {
    ($ty:ty, $schema:expr, $variant:ident, $to:expr, $from:expr) => {
        impl Bridge for $ty {
            fn value_from_read(value: Self::Read<'_>) -> Value {
                <Self as Bridge>::value(value)
            }
            fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
                handle.try_get().map(<Self as Bridge>::value)
            }

            fn default_entry<'a, K0: MapKey>(
                entry: blubber::Entry<'a, K0, Self>,
            ) -> blubber::Result<Self::Mut<'a>> {
                entry.or_default()
            }

            fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
                handle.reborrow()
            }
            fn schema() -> ModelSchema {
                $schema
            }
            fn owned(value: &Value) -> Self {
                let Value::$variant(value) = value else {
                    panic!("fixed value")
                };
                ($from)(value)
            }
            fn value(value: Self) -> Value {
                Value::$variant(($to)(value))
            }

            fn borrowed<'a>(value: &'a Value, _arena: &'a bumpalo::Bump) -> Self::Init<'a> {
                <Self as Bridge>::owned(value)
            }
            fn removed(value: Option<Self::Removed>) -> Observed {
                Observed::Old(value.map(<Self as Bridge>::value))
            }
            fn apply(
                mut handle: Self::Mut<'_>,
                path: &[Segment],
                edit: &ModelEdit,
            ) -> blubber::Result<Observed> {
                assert!(path.is_empty());
                let ModelEdit::Set(value) = edit else {
                    panic!("fixed edit")
                };
                handle.set(<Self as Bridge>::owned(value))?;
                Ok(Observed::Unit)
            }
        }
        impl Input for $ty {
            fn input(value: &Self) -> Self {
                *value
            }
        }
    };
}

fixed!(bool, ModelSchema::Bool, Bool, |v| v, |v: &bool| *v);
fixed!(
    Pubkey,
    ModelSchema::Pubkey,
    Pubkey,
    |v: Pubkey| v.to_bytes(),
    |v: &[u8; 32]| Pubkey::from_bytes(*v)
);

impl<const N: usize> Bridge for [u8; N] {
    fn value_from_read(value: Self::Read<'_>) -> Value {
        <Self as Bridge>::value(value)
    }
    fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
        handle.try_get().map(<Self as Bridge>::value)
    }

    fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
        handle.reborrow()
    }
    fn schema() -> ModelSchema {
        ModelSchema::Array(N.try_into().unwrap())
    }
    fn owned(value: &Value) -> Self {
        let Value::Array(value) = value else {
            panic!("array value")
        };
        value.as_slice().try_into().unwrap()
    }
    fn value(value: Self) -> Value {
        Value::Array(value.to_vec())
    }

    fn borrowed<'a>(value: &'a Value, _arena: &'a bumpalo::Bump) -> Self::Init<'a> {
        <Self as Bridge>::owned(value)
    }
    fn removed(value: Option<Self::Removed>) -> Observed {
        Observed::Old(value.map(<Self as Bridge>::value))
    }
    fn apply(
        mut handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &ModelEdit,
    ) -> blubber::Result<Observed> {
        assert!(path.is_empty());
        let ModelEdit::Set(value) = edit else {
            panic!("array edit")
        };
        handle.set(<Self as Bridge>::owned(value))?;
        Ok(Observed::Unit)
    }
}
impl<const N: usize> Input for [u8; N] {
    fn input(value: &Self) -> Self {
        *value
    }
}
impl<const N: usize> NativeKey for [u8; N] {
    fn schema() -> KeySchema {
        KeySchema::Array(N.try_into().unwrap())
    }
    fn owned(value: &Key) -> Self {
        let Key::Array(value) = value else {
            panic!("array key")
        };
        value.as_slice().try_into().unwrap()
    }
    fn value(value: Self) -> Key {
        Key::Array(value.to_vec())
    }
}
impl NativeKey for bool {
    fn schema() -> KeySchema {
        KeySchema::Bool
    }
    fn owned(value: &Key) -> Self {
        let Key::Bool(value) = value else {
            panic!("bool key")
        };
        *value
    }
    fn value(value: Self) -> Key {
        Key::Bool(value)
    }
}

impl NativeKey for Pubkey {
    fn schema() -> KeySchema {
        KeySchema::Pubkey
    }
    fn owned(value: &Key) -> Self {
        let Key::Pubkey(value) = value else {
            panic!("pubkey key")
        };
        Self::from_bytes(*value)
    }
    fn value(value: Self) -> Key {
        Key::Pubkey(value.to_bytes())
    }
}

impl Bridge for Str {
    fn value_from_read(value: Self::Read<'_>) -> Value {
        Value::Str(value.into())
    }
    fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
        handle.try_get().map(|v| Value::Str(v.into()))
    }

    fn default_entry<'a, K0: MapKey>(
        entry: blubber::Entry<'a, K0, Self>,
    ) -> blubber::Result<Self::Mut<'a>> {
        entry.or_default()
    }

    fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
        handle.reborrow()
    }
    fn schema() -> ModelSchema {
        ModelSchema::Str
    }
    fn owned(value: &Value) -> String {
        let Value::Str(value) = value else {
            panic!("string value")
        };
        value.clone()
    }
    fn value(value: String) -> Value {
        Value::Str(value)
    }

    fn borrowed<'a>(value: &'a Value, _arena: &'a bumpalo::Bump) -> Self::Init<'a> {
        let Value::Str(value) = value else {
            panic!("string input")
        };
        value
    }
    fn removed(value: Option<Self::Removed>) -> Observed {
        Observed::Present(value.is_some())
    }
    fn apply(
        mut handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &ModelEdit,
    ) -> blubber::Result<Observed> {
        assert!(path.is_empty());
        match edit {
            ModelEdit::Set(Value::Str(value)) => handle.set(value)?,
            ModelEdit::StringPush(value) => handle.push_str(value)?,
            ModelEdit::Clear => handle.clear()?,
            _ => panic!("string edit"),
        }
        Ok(Observed::Unit)
    }
}
impl Input for Str {
    fn input(value: &String) -> &str {
        value
    }
}

impl<T: Input> Bridge for Option<T> {
    fn value_from_read(value: Self::Read<'_>) -> Value {
        Value::Option(value.map(|v| Box::new(T::value_from_read(v))))
    }
    fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
        handle.try_get().map(<Self as Bridge>::value_from_read)
    }

    fn api(
        handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &ApiEdit,
        calls: &mut Callbacks,
        destination: &mut Option<Vec<u8>>,
    ) -> blubber::Result<ApiReturn> {
        api_dispatch::option::<T>(handle, path, edit, calls, destination)
    }

    fn default_entry<'a, K0: MapKey>(
        entry: blubber::Entry<'a, K0, Self>,
    ) -> blubber::Result<Self::Mut<'a>> {
        entry.or_default()
    }

    fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
        handle.reborrow()
    }
    fn schema() -> ModelSchema {
        ModelSchema::Option(Box::new(T::schema()))
    }
    fn owned(value: &Value) -> Self::Owned {
        let Value::Option(value) = value else {
            panic!("option value")
        };
        value.as_deref().map(T::owned)
    }
    fn value(value: Self::Owned) -> Value {
        Value::Option(value.map(|v| Box::new(T::value(v))))
    }

    fn borrowed<'a>(value: &'a Value, arena: &'a bumpalo::Bump) -> Self::Init<'a> {
        let Value::Option(value) = value else {
            panic!("option input")
        };
        value.as_deref().map(|v| T::borrowed(v, arena))
    }
    fn removed(value: Option<Self::Removed>) -> Observed {
        Observed::Present(value.is_some())
    }
    fn apply(
        mut handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &ModelEdit,
    ) -> blubber::Result<Observed> {
        if let Some((Segment::Some, rest)) = path.split_first() {
            return T::apply(handle.some_mut()?, rest, edit);
        }
        assert!(path.is_empty());
        match edit {
            ModelEdit::Clear => handle.set(None)?,
            ModelEdit::Set(value) => {
                let owned = Self::owned(value);
                handle.set(owned.as_ref().map(T::input))?;
            }
            _ => panic!("option edit"),
        }
        Ok(Observed::Unit)
    }
}
impl<T: Input> Input for Option<T> {
    fn input(value: &Self::Owned) -> Self::Init<'_> {
        value.as_ref().map(T::input)
    }
}

impl<T: Bridge> Bridge for List<T> {
    fn value_from_read(value: Self::Read<'_>) -> Value {
        Value::List(
            value
                .try_iter()
                .map(|v| T::value_from_read(v.unwrap()))
                .collect(),
        )
    }
    fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
        Ok(Value::List(
            handle
                .try_iter()?
                .map(|v| v.map(T::value_from_read))
                .collect::<blubber::Result<_>>()?,
        ))
    }

    fn api(
        handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &ApiEdit,
        calls: &mut Callbacks,
        destination: &mut Option<Vec<u8>>,
    ) -> blubber::Result<ApiReturn> {
        api_dispatch::list::<T>(handle, path, edit, calls, destination)
    }

    fn default_entry<'a, K0: MapKey>(
        entry: blubber::Entry<'a, K0, Self>,
    ) -> blubber::Result<Self::Mut<'a>> {
        entry.or_default()
    }

    fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
        handle.reborrow()
    }
    fn schema() -> ModelSchema {
        ModelSchema::List(Box::new(T::schema()))
    }
    fn owned(value: &Value) -> Self::Owned {
        let Value::List(values) = value else {
            panic!("list value")
        };
        values.iter().map(T::owned).collect()
    }
    fn value(value: Self::Owned) -> Value {
        Value::List(value.into_iter().map(T::value).collect())
    }

    fn borrowed<'a>(value: &'a Value, arena: &'a bumpalo::Bump) -> Self::Init<'a> {
        let Value::List(values) = value else {
            panic!("list input")
        };
        arena.alloc_slice_fill_with(values.len(), |i| T::borrowed(&values[i], arena))
    }
    fn removed(value: Option<Self::Removed>) -> Observed {
        Observed::Present(value.is_some())
    }
    fn apply(
        mut handle: ListMut<'_, T>,
        path: &[Segment],
        edit: &ModelEdit,
    ) -> blubber::Result<Observed> {
        if let Some((Segment::Index(index), rest)) = path.split_first() {
            if let ([], ModelEdit::Set(value)) = (rest, edit) {
                handle.replace_owned(*index, &T::owned(value))?;
                return Ok(Observed::Unit);
            }
            return T::apply(handle.at_mut(*index)?, rest, edit);
        }
        assert!(path.is_empty());
        match edit {
            ModelEdit::ListInsert { index, values } => {
                let arena = bumpalo::Bump::new();
                let inputs: Vec<_> = values.iter().map(|v| T::borrowed(v, &arena)).collect();
                if *index == handle.try_len()? {
                    handle.extend_from_slice(&inputs)?;
                } else {
                    handle.insert_from_slice(*index, &inputs)?;
                }
            }
            ModelEdit::ListRemove(index) => {
                return handle
                    .remove_owned(*index)
                    .map(|v| Observed::Old(Some(T::value(v))))
            }
            ModelEdit::ListPop => {
                return handle.pop_owned().map(|v| Observed::Old(v.map(T::value)))
            }
            ModelEdit::ListTruncate(count) => handle.truncate(*count)?,
            ModelEdit::Clear => handle.clear()?,
            _ => panic!("list edit"),
        }
        Ok(Observed::Unit)
    }
}

impl<K: NativeKey, T: Bridge> Bridge for Map<K, T> {
    fn value_from_read(value: Self::Read<'_>) -> Value {
        Value::Map(
            value
                .try_iter()
                .map(|v| {
                    let (k, v) = v.unwrap();
                    (K::value(k), T::value_from_read(v))
                })
                .collect(),
        )
    }
    fn snapshot(handle: &Self::Mut<'_>) -> blubber::Result<Value> {
        Ok(Value::Map(
            handle
                .try_iter()?
                .map(|v| v.map(|(k, v)| (K::value(k), T::value_from_read(v))))
                .collect::<blubber::Result<_>>()?,
        ))
    }

    fn api(
        handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &ApiEdit,
        calls: &mut Callbacks,
        destination: &mut Option<Vec<u8>>,
    ) -> blubber::Result<ApiReturn> {
        api_dispatch::map::<K, T>(handle, path, edit, calls, destination)
    }

    fn default_entry<'a, K0: MapKey>(
        entry: blubber::Entry<'a, K0, Self>,
    ) -> blubber::Result<Self::Mut<'a>> {
        entry.or_default()
    }

    fn loan<'s>(handle: &'s mut Self::Mut<'_>) -> Self::Mut<'s> {
        handle.reborrow()
    }
    fn schema() -> ModelSchema {
        ModelSchema::Map(K::schema(), Box::new(T::schema()))
    }
    fn owned(value: &Value) -> Self::Owned {
        let Value::Map(values) = value else {
            panic!("map value")
        };
        values
            .iter()
            .map(|(k, v)| (K::owned(k), T::owned(v)))
            .collect()
    }
    fn value(value: Self::Owned) -> Value {
        Value::Map(
            value
                .into_iter()
                .map(|(k, v)| (K::value(k), T::value(v)))
                .collect(),
        )
    }

    fn borrowed<'a>(value: &'a Value, arena: &'a bumpalo::Bump) -> Self::Init<'a> {
        let Value::Map(values) = value else {
            panic!("map input")
        };
        arena.alloc_slice_fill_iter(
            values
                .iter()
                .map(|(k, v)| (K::owned(k), T::borrowed(v, arena))),
        )
    }
    fn removed(value: Option<Self::Removed>) -> Observed {
        Observed::Present(value.is_some())
    }
    fn apply(
        mut handle: Self::Mut<'_>,
        path: &[Segment],
        edit: &ModelEdit,
    ) -> blubber::Result<Observed> {
        if let Some((Segment::Key(key), rest)) = path.split_first() {
            return T::apply(handle.at_mut(&K::owned(key))?, rest, edit);
        }
        assert!(path.is_empty());
        match edit {
            ModelEdit::MapInsert { key, value } => handle
                .insert_owned(K::owned(key), &T::owned(value))
                .map(T::removed),
            ModelEdit::MapRemove(key) => handle
                .remove_owned(&K::owned(key))
                .map(|v| Observed::Old(v.map(T::value))),
            ModelEdit::Clear => handle.clear().map(|()| Observed::Unit),
            _ => panic!("map edit"),
        }
    }
}
