//! CTFE-only schema tree assembly. Runtime descriptors contain only integer words.
use crate::{ErrorKind, Result, Schema, View};
use core::marker::PhantomData;

/// Type-level child sequence consumed by concrete const table construction.
#[doc(hidden)]
pub trait SchemaList {
    const ENCODER_AUDITED: bool = false;
    const MEASURE_REUSABLE: bool = false;
    const IS_NIL: bool;
    const COUNT: usize;
    const LEN: usize;
    const DEPTH: usize;
    type Head: Schema;
    type Tail: SchemaList;
}
/// Empty schema child sequence.
#[doc(hidden)]
pub enum Nil {}
/// Nonempty schema child sequence.
#[doc(hidden)]
pub struct Cons<H, T>(PhantomData<(H, T)>);
/// Uninhabited terminator for `Nil::Head`.
#[doc(hidden)]
pub enum Never {}
impl SchemaList for Nil {
    const ENCODER_AUDITED: bool = true;
    const MEASURE_REUSABLE: bool = true;
    const IS_NIL: bool = true;
    const COUNT: usize = 0;
    const LEN: usize = 0;
    const DEPTH: usize = 0;
    type Head = Never;
    type Tail = Nil;
}
impl<H: Schema, T: SchemaList> SchemaList for Cons<H, T> {
    const ENCODER_AUDITED: bool = H::__ENCODER_AUDITED && T::ENCODER_AUDITED;
    const MEASURE_REUSABLE: bool = H::__MEASURE_REUSABLE && T::MEASURE_REUSABLE;
    const IS_NIL: bool = false;
    const COUNT: usize = 1 + T::COUNT;
    const LEN: usize = H::__LEN + T::LEN;
    const DEPTH: usize = if H::__DEPTH > T::DEPTH {
        H::__DEPTH
    } else {
        T::DEPTH
    };
    type Head = H;
    type Tail = T;
}
impl Schema for Never {
    const KIND: crate::Kind = crate::Kind::Variable;
    const FIXED_SIZE: Option<u32> = None;
    const MIN_SIZE: u32 = 0;
    const LAYOUT: &'static blubber_core::Layout = &blubber_core::Layout::Str;
    const __LEN: usize = 0;
    const __HEADER: &'static [u32] = &[];
    type __Children = Nil;
    type Read<'a> = Self;
    type Mut<'a> = ();
    type Init<'a> = Self;
    type Removed = Self;
    fn read<'a>(_: View<'a>) -> Result<Self> {
        Err(ErrorKind::InvalidSchema.into())
    }
    fn read_removed(_: View<'_>) -> Result<Self> {
        Err(ErrorKind::InvalidSchema.into())
    }
    fn project(_: crate::__private::Projection<'_, Self>) {}
    fn measure(init: &Self) -> Result<u32> {
        match *init {}
    }
    fn encode(init: &Self, _: &mut [u8]) {
        match *init {}
    }
    #[cfg(feature = "alloc")]
    type Owned = Self;
    #[cfg(feature = "alloc")]
    fn measure_owned(value: &Self) -> Result<u32> {
        match *value {}
    }
    #[cfg(feature = "alloc")]
    fn encode_owned(value: &Self, _: &mut [u8]) {
        match *value {}
    }
    #[cfg(feature = "alloc")]
    fn decode_owned(_: View<'_>) -> Result<Self> {
        Err(ErrorKind::InvalidSchema.into())
    }
}

/// Concrete subtree table; no scratch array, allocation or child-count limit.
#[doc(hidden)]
pub const fn flatten<S: Schema, const N: usize>() -> [u32; N] {
    assert!(N == S::__LEN, "schema table length mismatch");
    assert!(N <= u32::MAX as usize, "schema word index exceeds u32");
    let mut words = [0; N];
    let end = emit::<S>(&mut words, 0);
    assert!(end == N, "schema subtree length mismatch");
    words
}

const fn emit<S: Schema>(out: &mut [u32], at: usize) -> usize {
    let fields = emit_children::<S::__Children>(out, at);
    let count = <S::__Children as SchemaList>::COUNT;
    let root = fields + count * 8;
    assert!(
        S::__HEADER.len() == 8 + count * 2,
        "schema header length mismatch"
    );
    assert!(at + S::__LEN == root + 8, "schema subtree length mismatch");
    let mut i = 0;
    while i < 8 {
        out[root + i] = S::__HEADER[i];
        i += 1;
    }
    out[root + 5] = if count == 0 { 0 } else { fields as u32 };
    out[root + 2] = 1;
    emit_fields::<S, S::__Children>(out, at, fields, root, 0, 0, 0);
    if out[root] == 9 && out[root + 4] == 0 {
        out[root + 6] = 1;
        let mut field = 0;
        while field < count {
            let child = out[fields + 8 * field + 3] as usize;
            if !(out[child] == 1
                || out[child] == 2
                || out[child] == 4
                || (out[child] == 9 && out[child + 6] == 1))
            {
                out[root + 6] = 0;
            }
            field += 1;
        }
    }
    root + 8
}
const fn emit_children<L: SchemaList>(out: &mut [u32], at: usize) -> usize {
    if L::IS_NIL {
        return at;
    }
    let end = emit::<L::Head>(out, at);
    emit_children::<L::Tail>(out, end)
}
#[allow(clippy::too_many_arguments)]
const fn emit_fields<S: Schema, L: SchemaList>(
    out: &mut [u32],
    at: usize,
    fields: usize,
    root: usize,
    ordinal: usize,
    fixed: usize,
    variable: usize,
) {
    if L::IS_NIL {
        return;
    }
    let child = at + <L::Head as Schema>::__LEN - 8;
    let is_struct = S::__HEADER[0] == 9;
    let is_fixed = is_struct && matches!(<L::Head as Schema>::KIND, crate::Kind::Fixed);
    let slot = if is_fixed {
        fixed
    } else {
        S::__HEADER[3] as usize + variable
    };
    let f = fields + 8 * slot;
    out[f] = 10;
    out[f + 1] = S::__HEADER[8 + 2 * ordinal];
    out[f + 2] = S::__HEADER[9 + 2 * ordinal];
    out[f + 3] = child as u32;
    out[fields + 8 * ordinal + 4] = slot as u32;
    if out[child + 2] >= out[root + 2] {
        out[root + 2] = out[child + 2] + 1;
    }
    emit_fields::<S, L::Tail>(
        out,
        at + <L::Head as Schema>::__LEN,
        fields,
        root,
        ordinal + 1,
        fixed + is_fixed as usize,
        variable + (!is_fixed) as usize,
    );
}

/// Certifies a promoted table without introducing bindings in the consumer.
#[doc(hidden)]
pub const fn schema_layout(words: &'static [u32]) -> blubber_core::CheckedLayout<'static> {
    match blubber_core::certify(words, words.len() - 8, blubber_core::MAX_DEPTH) {
        Some(checked) => checked,
        None => panic!("invalid schema table"),
    }
}

/// Materializes a schema table in a concrete test context and certifies it at CTFE.
#[doc(hidden)]
#[macro_export]
macro_rules! __layout_of {
    ($ty:ty) => {{
        const {
            $crate::__private::schema_layout(
                &const { $crate::__private::flatten::<$ty, { <$ty as $crate::Schema>::__LEN }>() },
            )
        }
    }};
}

/// Gives an internal test a concrete root marker for an otherwise nested schema.
#[doc(hidden)]
#[macro_export]
macro_rules! __root_schema {
    ($name:ident, $ty:ty) => {
        #[doc(hidden)]
        struct $name;
        impl $crate::Schema for $name {
            const WIRE_DESCRIPTOR: &'static $crate::identity::WireDescriptor =
                <$ty as $crate::Schema>::WIRE_DESCRIPTOR;
            const KIND: $crate::Kind = <$ty as $crate::Schema>::KIND;
            const FIXED_SIZE: ::core::option::Option<::core::primitive::u32> =
                <$ty as $crate::Schema>::FIXED_SIZE;
            const MIN_SIZE: ::core::primitive::u32 = <$ty as $crate::Schema>::MIN_SIZE;
            const LAYOUT: &'static $crate::__private::Layout = <$ty as $crate::Schema>::LAYOUT;
            const __LEN: ::core::primitive::usize = <$ty as $crate::Schema>::__LEN;
            const __HEADER: &'static [::core::primitive::u32] = <$ty as $crate::Schema>::__HEADER;
            type __Children = <$ty as $crate::Schema>::__Children;
            type Read<'a> = <$ty as $crate::Schema>::Read<'a>;
            type Mut<'a> = <$ty as $crate::Schema>::Mut<'a>;
            type Init<'a> = <$ty as $crate::Schema>::Init<'a>;
            type Removed = <$ty as $crate::Schema>::Removed;
            fn read<'a>(view: $crate::View<'a>) -> $crate::Result<Self::Read<'a>> {
                <$ty as $crate::Schema>::read(view)
            }
            fn read_removed(view: $crate::View<'_>) -> $crate::Result<Self::Removed> {
                <$ty as $crate::Schema>::read_removed(view)
            }
            fn project<'a>(edit: $crate::__private::Projection<'a, Self>) -> Self::Mut<'a> {
                $crate::raw::project::<$ty>($crate::raw::into_edit(edit))
            }
            fn measure(init: &Self::Init<'_>) -> $crate::Result<::core::primitive::u32> {
                <$ty as $crate::Schema>::measure(init)
            }
            fn encode(init: &Self::Init<'_>, out: &mut [::core::primitive::u8]) {
                <$ty as $crate::Schema>::encode(init, out);
            }
            $crate::__with_alloc! {
                type Owned = <$ty as $crate::Schema>::Owned;
                fn measure_owned(value: &Self::Owned) -> $crate::Result<::core::primitive::u32> {
                    <$ty as $crate::Schema>::measure_owned(value)
                }
                fn encode_owned(value: &Self::Owned, out: &mut [::core::primitive::u8]) {
                    <$ty as $crate::Schema>::encode_owned(value, out);
                }
                fn decode_owned(view: $crate::View<'_>) -> $crate::Result<Self::Owned> {
                    <$ty as $crate::Schema>::decode_owned(view)
                }
            }
        }
        impl $crate::RootSchema for $name {
            fn __layout() -> $crate::__private::CheckedLayout<'static> {
                $crate::__layout_of!($ty)
            }
        }
    };
}
